//! Wisp's external-capture speech engine. Hardware callbacks only frame/queue
//! PCM; one dedicated worker owns AEC and neural state, before Opus encoding.
use anyhow::{Context, bail};
use df::tract::{DfParams, DfTract, RuntimeParams};
use livekit::webrtc::native::apm::AudioProcessingModule;
use ndarray::{ArrayView2, ArrayViewMut2};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{Notify, mpsc, oneshot};
use tracing::{info, warn};
use wisp_protocol::{AudioPreset, AudioState};

pub(crate) const AUDIO_SAMPLE_RATE: u32 = 48_000;
pub(crate) const AUDIO_FRAME_SAMPLES: usize = 480;
pub(crate) const AUDIO_FRAME_BUDGET_US: u32 = 10_000;
const MAX_CAPTURE_FRAMES: usize = 6;
const MAX_FRAME_AGE: Duration = Duration::from_millis(60);
pub(crate) type PcmFrame = [i16; AUDIO_FRAME_SAMPLES];

pub(crate) struct CapturedFrame {
    pub samples: PcmFrame,
    pub captured: Instant,
    pub sequence: u64,
    pub epoch: u64,
}

#[derive(Default)]
struct CaptureState {
    frames: VecDeque<CapturedFrame>,
    partial: Vec<i16>,
    partial_started: Option<Instant>,
    next_sequence: u64,
    epoch: u64,
    next_source: u64,
    active_source: u64,
}

#[derive(Default)]
pub(crate) struct CaptureQueue {
    state: Mutex<CaptureState>,
    ready: Notify,
}

impl CaptureQueue {
    pub fn new_source(&self) -> u64 {
        let mut state = self.state.lock().expect("capture queue poisoned");
        state.next_source += 1;
        state.next_source
    }

    pub fn activate(&self, source: u64) {
        let mut state = self.state.lock().expect("capture queue poisoned");
        state.active_source = source;
        Self::clear(&mut state);
    }

    fn clear(state: &mut CaptureState) {
        state.frames.clear();
        state.partial.clear();
        state.partial_started = None;
        state.epoch = state.epoch.wrapping_add(1);
    }

    pub fn reset(&self) {
        Self::clear(&mut self.state.lock().expect("capture queue poisoned"));
    }

    pub fn is_current(&self, frame: &CapturedFrame) -> bool {
        frame.captured.elapsed() <= MAX_FRAME_AGE
            && frame.epoch == self.state.lock().expect("capture queue poisoned").epoch
    }

    pub fn queued_ms(&self) -> u16 {
        let state = self.state.lock().expect("capture queue poisoned");
        u16::try_from(state.frames.len() * 10 + state.partial.len() / 48).unwrap_or(u16::MAX)
    }

    pub fn push(&self, source: u64, samples: &[i16]) {
        self.push_at(source, samples, Instant::now());
    }

    fn push_at(&self, source: u64, mut samples: &[i16], now: Instant) {
        let mut state = self.state.lock().expect("capture queue poisoned");
        if source != state.active_source {
            return;
        }
        if state
            .partial_started
            .is_some_and(|started| now.duration_since(started) > MAX_FRAME_AGE)
        {
            Self::clear(&mut state);
        }
        while !samples.is_empty() {
            let count = (AUDIO_FRAME_SAMPLES - state.partial.len()).min(samples.len());
            state.partial_started.get_or_insert(now);
            state.partial.extend_from_slice(&samples[..count]);
            samples = &samples[count..];
            if state.partial.len() == AUDIO_FRAME_SAMPLES {
                let frame = CapturedFrame {
                    samples: state
                        .partial
                        .as_slice()
                        .try_into()
                        .expect("complete PCM frame"),
                    captured: state.partial_started.take().expect("frame start time"),
                    sequence: state.next_sequence,
                    epoch: state.epoch,
                };
                state.next_sequence = state.next_sequence.wrapping_add(1);
                state.partial.clear();
                if state.frames.len() == MAX_CAPTURE_FRAMES {
                    state.frames.pop_front();
                }
                state.frames.push_back(frame);
            }
        }
        // The partial device chunk shares the same 60 ms budget as complete
        // frames; it must not silently add another 9 ms to a full queue.
        if state.frames.len() == MAX_CAPTURE_FRAMES && !state.partial.is_empty() {
            state.frames.pop_front();
        }
        drop(state);
        self.ready.notify_one();
    }

    pub async fn recv(&self) -> CapturedFrame {
        loop {
            // Register before inspecting so a producer cannot strand a frame.
            let notified = self.ready.notified();
            {
                let mut state = self.state.lock().expect("capture queue poisoned");
                while let Some(frame) = state.frames.pop_front() {
                    if frame.captured.elapsed() <= MAX_FRAME_AGE {
                        return frame;
                    }
                }
            }
            notified.await;
        }
    }
}

pub(crate) const fn preset_code(preset: AudioPreset) -> u8 {
    match preset {
        AudioPreset::Clear => 0,
        AudioPreset::Natural => 1,
        AudioPreset::Studio => 2,
    }
}

fn preset_from_code(code: u8) -> AudioPreset {
    match code {
        1 => AudioPreset::Natural,
        2 => AudioPreset::Studio,
        _ => AudioPreset::Clear,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum DenoiserBackend {
    DeepFilterNet,
    WebRtc,
}

impl DenoiserBackend {
    pub fn from_atomic(value: u8) -> Self {
        if value == Self::WebRtc as u8 {
            Self::WebRtc
        } else {
            Self::DeepFilterNet
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::DeepFilterNet => "deepfilternet",
            Self::WebRtc => "webrtc",
        }
    }
    const fn latency_ms(self) -> u16 {
        match self {
            Self::DeepFilterNet => 30,
            Self::WebRtc => 0,
        }
    }
}

pub(crate) fn apply_denoiser_state(state: &mut AudioState, backend: DenoiserBackend) {
    state.denoiser_active = state.preset == AudioPreset::Clear;
    state.denoiser = state.denoiser_active.then(|| backend.name().to_owned());
    state.processing_latency_ms = if state.denoiser_active {
        backend.latency_ms()
    } else {
        0
    };
}

/// Attenuation only: fast peak protection and a 100 ms recovery, never lifting
/// room noise or deciding whether quiet speech deserves to pass.
struct PeakLimiter {
    gain: f32,
}
impl Default for PeakLimiter {
    fn default() -> Self {
        Self { gain: 1.0 }
    }
}
impl PeakLimiter {
    #[allow(clippy::cast_possible_truncation)]
    fn process(&mut self, input: &[f32; AUDIO_FRAME_SAMPLES]) -> PcmFrame {
        let mut output = [0; AUDIO_FRAME_SAMPLES];
        // Frame lookahead catches sudden peaks without clipping their attack.
        let peak = input
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .map(f32::abs)
            .fold(0.0, f32::max);
        let target = if peak > 0.95 { 0.95 / peak } else { 1.0 };
        self.gain = target.min(self.gain + (1.0 - self.gain) * 0.095_163);
        for (out, &sample) in output.iter_mut().zip(input) {
            *out = if sample.is_finite() {
                (sample * self.gain * 32768.0)
                    .round()
                    .clamp(-32768.0, 32767.0) as i16
            } else {
                0
            };
        }
        output
    }
}

enum NeuralDenoiser {
    DeepFilterNet(Box<DfTract>),
    WebRtc,
}
impl NeuralDenoiser {
    fn from_template(template: Option<&DfTract>, fallback: bool) -> Self {
        if !fallback && let Some(model) = template {
            return Self::DeepFilterNet(Box::new(model.clone()));
        }
        Self::WebRtc
    }
    fn backend(&self) -> DenoiserBackend {
        match self {
            Self::DeepFilterNet(_) => DenoiserBackend::DeepFilterNet,
            Self::WebRtc => DenoiserBackend::WebRtc,
        }
    }
    fn process(&mut self, input: &PcmFrame) -> anyhow::Result<[f32; AUDIO_FRAME_SAMPLES]> {
        let mut output = [0.0; AUDIO_FRAME_SAMPLES];
        match self {
            Self::DeepFilterNet(model) => {
                let input = input.map(|v| f32::from(v) / 32768.0);
                model
                    .process(
                        ArrayView2::from_shape((1, AUDIO_FRAME_SAMPLES), &input)?,
                        ArrayViewMut2::from_shape((1, AUDIO_FRAME_SAMPLES), &mut output)?,
                    )
                    .context("process DeepFilterNet frame")?;
            }
            Self::WebRtc => bail!("WebRTC suppression belongs to the explicit APM stage"),
        }
        if output.iter().any(|v| !v.is_finite()) {
            bail!("non-finite neural output");
        }
        Ok(output)
    }
}

fn create_deepfilter_model() -> anyhow::Result<DfTract> {
    // Preserve the trained full-band model and its defaults. Layering a gate or
    // a second suppressor after it damages consonants and low-volume speech.
    let model = DfTract::new(DfParams::default(), &RuntimeParams::default())
        .context("load embedded DeepFilterNet model")?;
    if model.sr != AUDIO_SAMPLE_RATE as usize || model.hop_size != AUDIO_FRAME_SAMPLES {
        bail!("unsupported DeepFilterNet frame format");
    }
    let delay = model.fft_size - model.hop_size + model.lookahead * model.hop_size;
    if delay != 1440 {
        bail!("unexpected DeepFilterNet lookahead");
    }
    Ok(model)
}

struct SpeechProcessor {
    template: Option<DfTract>,
    neural: NeuralDenoiser,
    apm: Option<AudioProcessingModule>,
    preset: Option<AudioPreset>,
    limiter: PeakLimiter,
    fallback: bool,
    slow_frames: u8,
    warmup_frames: u8,
}

impl SpeechProcessor {
    fn new() -> Self {
        let template = match create_deepfilter_model() {
            Ok(model) => Some(model),
            Err(error) => {
                warn!(%error, "DeepFilterNet unavailable; using WebRTC noise suppression");
                None
            }
        };
        let neural = NeuralDenoiser::from_template(template.as_ref(), false);
        info!(backend = neural.backend().name(), "speech processor ready");
        Self {
            template,
            neural,
            apm: None,
            preset: None,
            limiter: PeakLimiter::default(),
            fallback: false,
            slow_frames: 0,
            warmup_frames: 0,
        }
    }

    fn reset(&mut self, preset: AudioPreset) {
        self.neural = NeuralDenoiser::from_template(self.template.as_ref(), self.fallback);
        self.apm = if preset == AudioPreset::Studio {
            None
        } else {
            let mut apm =
                AudioProcessingModule::new(true, false, true, preset == AudioPreset::Natural);
            apm.use_playout_reference(true);
            // AEC3 estimates the acoustic/device delay from the render history.
            // No neural lookahead has been applied at this point.
            let _ = apm.set_stream_delay_ms(0);
            Some(apm)
        };
        self.preset = Some(preset);
        self.limiter = PeakLimiter::default();
    }

    fn process(
        &mut self,
        mut input: PcmFrame,
        preset: AudioPreset,
        reset: bool,
    ) -> anyhow::Result<ProcessedFrame> {
        if reset || self.preset != Some(preset) {
            self.reset(preset);
        }
        if preset == AudioPreset::Studio {
            return Ok(ProcessedFrame {
                samples: input,
                reference_frames: 0,
            });
        }
        let apm = self.apm.as_mut().expect("speech APM initialized");
        if preset == AudioPreset::Clear && self.neural.backend() == DenoiserBackend::WebRtc {
            apm.enable_high_noise_suppression();
        }
        apm.process_stream(&mut input, 48_000, 1)
            .context("process microphone echo cancellation")?;
        let reference_frames = apm.playout_reference_frames();
        let processed = if preset == AudioPreset::Clear
            && self.neural.backend() == DenoiserBackend::DeepFilterNet
        {
            let started = Instant::now();
            let output = match self.neural.process(&input) {
                Ok(output) => output,
                Err(error) => {
                    warn!(%error, "neural processing failed; switching to WebRTC noise suppression");
                    self.fallback = true;
                    self.neural = NeuralDenoiser::from_template(None, true);
                    // Do not leak a raw frame when the neural stage fails. The
                    // next frame uses the same AEC history with WebRTC NS on.
                    [0.0; AUDIO_FRAME_SAMPLES]
                }
            };
            // Sustained overload must not turn a conversation into a growing
            // recording backlog. Keep the lighter backend until the next join.
            self.observe_cost(started.elapsed());
            output
        } else {
            input.map(|v| f32::from(v) / 32768.0)
        };
        Ok(ProcessedFrame {
            samples: self.limiter.process(&processed),
            reference_frames,
        })
    }

    fn observe_cost(&mut self, elapsed: Duration) {
        if self.warmup_frames < 20 {
            self.warmup_frames += 1;
            return;
        }
        self.slow_frames = if elapsed > Duration::from_millis(10) {
            self.slow_frames.saturating_add(1)
        } else {
            0
        };
        if self.slow_frames >= 8 && self.neural.backend() == DenoiserBackend::DeepFilterNet {
            warn!(
                "DeepFilterNet exceeded the realtime budget; switching to WebRTC noise suppression for this session"
            );
            self.fallback = true;
            self.neural = NeuralDenoiser::from_template(None, true);
            self.slow_frames = 0;
        }
    }
}

pub(crate) struct ProcessedFrame {
    pub samples: PcmFrame,
    pub reference_frames: u64,
}
enum Request {
    Start(oneshot::Sender<()>),
    Process {
        input: Box<PcmFrame>,
        preset: u8,
        reset: bool,
        response: oneshot::Sender<anyhow::Result<ProcessedFrame>>,
    },
}
pub(crate) struct DenoiserService {
    requests: Option<mpsc::Sender<Request>>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl DenoiserService {
    pub fn spawn(backend: Arc<AtomicU8>) -> anyhow::Result<Self> {
        let (requests, mut rx) = mpsc::channel::<Request>(2);
        let worker = std::thread::Builder::new()
            .name("wisp-speech".into())
            .spawn(move || {
                let mut processor = SpeechProcessor::new();
                backend.store(processor.neural.backend() as u8, Ordering::Release);
                while let Some(request) = rx.blocking_recv() {
                    match request {
                        Request::Start(response) => {
                            processor.fallback = false;
                            processor.preset = None;
                            processor.apm = None;
                            processor.neural =
                                NeuralDenoiser::from_template(processor.template.as_ref(), false);
                            processor.warmup_frames = 0;
                            processor.slow_frames = 0;
                            backend.store(processor.neural.backend() as u8, Ordering::Release);
                            let _ = response.send(());
                        }
                        Request::Process {
                            input,
                            preset,
                            reset,
                            response,
                        } => {
                            if response.is_closed() {
                                continue;
                            }
                            let result = processor.process(*input, preset_from_code(preset), reset);
                            backend.store(processor.neural.backend() as u8, Ordering::Release);
                            let _ = response.send(result);
                        }
                    }
                }
            })
            .context("start speech worker")?;
        Ok(Self {
            requests: Some(requests),
            worker: Some(worker),
        })
    }
    pub async fn start_session(&self) -> anyhow::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.requests
            .as_ref()
            .context("speech worker unavailable")?
            .send(Request::Start(tx))
            .await?;
        rx.await.context("speech worker stopped")
    }
    pub async fn process(
        &self,
        input: &PcmFrame,
        preset: u8,
        reset: bool,
    ) -> anyhow::Result<ProcessedFrame> {
        let (response, rx) = oneshot::channel();
        self.requests
            .as_ref()
            .context("speech worker unavailable")?
            .send(Request::Process {
                input: Box::new(*input),
                preset,
                reset,
                response,
            })
            .await?;
        rx.await.context("speech worker stopped")?
    }
}
impl Drop for DenoiserService {
    fn drop(&mut self) {
        self.requests.take();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            warn!("speech worker panicked");
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
mod tests {
    use super::*;

    fn queue() -> (CaptureQueue, u64) {
        let queue = CaptureQueue::default();
        let source = queue.new_source();
        queue.activate(source);
        (queue, source)
    }

    #[tokio::test]
    async fn arbitrary_device_chunks_preserve_sample_order() {
        let (queue, source) = queue();
        let samples: Vec<i16> = (0..1440).map(|v| v as i16).collect();
        for chunk in samples.chunks(137) {
            queue.push(source, chunk);
        }
        let mut received = Vec::new();
        for _ in 0..3 {
            received.extend_from_slice(&queue.recv().await.samples);
        }
        assert_eq!(received, samples);
        assert_eq!(queue.queued_ms(), 0);
    }

    #[tokio::test]
    async fn overload_keeps_only_recent_sixty_milliseconds() {
        let (queue, source) = queue();
        for value in 0..100 {
            queue.push(source, &[value; AUDIO_FRAME_SAMPLES]);
        }
        assert_eq!(queue.queued_ms(), 60);
        for value in 94..100 {
            let frame = queue.recv().await;
            assert_eq!(frame.samples, [value; AUDIO_FRAME_SAMPLES]);
            assert_eq!(frame.sequence, value as u64);
        }
    }

    #[tokio::test]
    async fn partial_device_chunks_share_the_capture_budget() {
        let (queue, source) = queue();
        queue.push(source, &[1; AUDIO_FRAME_SAMPLES * MAX_CAPTURE_FRAMES + 479]);
        assert_eq!(queue.queued_ms(), 59);
        assert_eq!(queue.recv().await.sequence, 1);
    }

    #[tokio::test]
    async fn expired_capture_and_inflight_audio_cannot_escape_a_reset() {
        let (queue, source) = queue();
        queue.push_at(
            source,
            &[1; AUDIO_FRAME_SAMPLES],
            Instant::now().checked_sub(Duration::from_secs(1)).unwrap(),
        );
        queue.push(source, &[2; AUDIO_FRAME_SAMPLES]);
        let frame = queue.recv().await;
        assert_eq!(frame.samples, [2; AUDIO_FRAME_SAMPLES]);
        assert!(queue.is_current(&frame));
        queue.reset();
        assert!(!queue.is_current(&frame));
    }

    #[tokio::test]
    async fn device_switch_rejects_old_callbacks_and_partial_frames() {
        let (queue, source) = queue();
        queue.push(source, &[1; 100]);
        let replacement = queue.new_source();
        // Preparing a device must not interrupt the working microphone.
        queue.push(source, &[1; 380]);
        assert_eq!(queue.recv().await.samples, [1; AUDIO_FRAME_SAMPLES]);
        queue.activate(replacement);
        queue.push(source, &[9; AUDIO_FRAME_SAMPLES]);
        queue.push(replacement, &[2; AUDIO_FRAME_SAMPLES]);
        assert_eq!(queue.recv().await.samples, [2; AUDIO_FRAME_SAMPLES]);
        assert_eq!(queue.queued_ms(), 0);
    }

    #[test]
    fn peak_protection_preserves_quiet_speech_and_rejects_nonfinite_output() {
        let mut limiter = PeakLimiter::default();
        let quiet = [0.000_5; AUDIO_FRAME_SAMPLES];
        assert_eq!(limiter.process(&quiet), [16; AUDIO_FRAME_SAMPLES]);
        let mut loud = [1.5; AUDIO_FRAME_SAMPLES];
        loud[0] = f32::NAN;
        loud[1] = f32::INFINITY;
        let output = limiter.process(&loud);
        assert_eq!(&output[..2], &[0, 0]);
        assert!(output.iter().all(|v| v.unsigned_abs() <= 31130));
        for _ in 0..100 {
            limiter.process(&[0.0; AUDIO_FRAME_SAMPLES]);
        }
        assert_eq!(limiter.process(&quiet), [16; AUDIO_FRAME_SAMPLES]);
    }

    #[test]
    fn unprocessed_mode_is_bit_exact_and_never_attaches_echo_reference() {
        let mut processor = SpeechProcessor::new();
        let input = std::array::from_fn(|i| i16::try_from(i).unwrap().wrapping_mul(137));
        assert_eq!(
            processor
                .process(input, AudioPreset::Studio, false)
                .unwrap()
                .samples,
            input
        );
        assert!(processor.apm.is_none());
    }

    #[test]
    fn neural_reset_does_not_replay_previous_speech() {
        let model = create_deepfilter_model().unwrap();
        let mut processor = NeuralDenoiser::from_template(Some(&model), false);
        for _ in 0..10 {
            processor.process(&[12_000; AUDIO_FRAME_SAMPLES]).unwrap();
        }
        // Cloning a pristine template resets recurrent, FFT, and lookahead state.
        processor = NeuralDenoiser::from_template(Some(&model), false);
        for _ in 0..8 {
            let output = processor.process(&[0; AUDIO_FRAME_SAMPLES]).unwrap();
            assert!(output.iter().all(|v| v.abs() < 1.0 / 32768.0));
        }
    }

    #[test]
    fn sustained_overload_switches_once_even_across_capture_discontinuities() {
        let mut processor = SpeechProcessor::new();
        for _ in 0..28 {
            processor.reset(AudioPreset::Clear);
            processor.observe_cost(Duration::from_millis(12));
        }
        assert_eq!(processor.neural.backend(), DenoiserBackend::WebRtc);
        processor.reset(AudioPreset::Clear);
        assert_eq!(processor.neural.backend(), DenoiserBackend::WebRtc);
    }

    fn noise(seed: &mut u32) -> i16 {
        *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        ((*seed >> 16) as i16) / 4
    }

    #[test]
    fn neural_suppression_reduces_stationary_noise() {
        let model = create_deepfilter_model().unwrap();
        for mut processor in [NeuralDenoiser::from_template(Some(&model), false)] {
            let mut seed = 42;
            let mut before = 0.0_f64;
            let mut after = 0.0_f64;
            for frame in 0..200 {
                let input = std::array::from_fn(|_| noise(&mut seed));
                let output = processor.process(&input).unwrap();
                if frame > 100 {
                    before += input
                        .iter()
                        .map(|v| (f64::from(*v) / 32768.0).powi(2))
                        .sum::<f64>();
                    after += output.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>();
                }
            }
            let reduction = 10.0 * (before / after.max(1e-15)).log10();
            eprintln!(
                "{} noise reduction: {reduction:.1} dB",
                processor.backend().name()
            );
            assert!(reduction > 12.0, "stationary noise reduction: {reduction}");
        }
    }

    #[test]
    fn lightweight_fallback_suppresses_noise_without_neural_processing() {
        let mut processor = SpeechProcessor::new();
        processor.fallback = true;
        processor.reset(AudioPreset::Clear);
        let mut seed = 42;
        let mut before = 0.0_f64;
        let mut after = 0.0_f64;
        for frame in 0..200 {
            let input = std::array::from_fn(|_| noise(&mut seed));
            let output = processor
                .process(input, AudioPreset::Clear, false)
                .unwrap()
                .samples;
            if frame > 100 {
                before += input.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>();
                after += output.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>();
            }
        }
        let reduction = 10.0 * (before / after.max(1e-15)).log10();
        eprintln!("lightweight noise reduction: {reduction:.1} dB");
        assert!(reduction > 12.0, "lightweight noise reduction: {reduction}");
    }

    #[test]
    fn explicit_echo_cancellation_reduces_delayed_speaker_signal() {
        let mut apm = AudioProcessingModule::new(true, false, true, false);
        apm.set_stream_delay_ms(40).unwrap();
        let mut delay = VecDeque::from(vec![[0; AUDIO_FRAME_SAMPLES]; 4]);
        let mut seed = 123;
        let mut before = 0.0_f64;
        let mut after = 0.0_f64;
        for frame in 0..600 {
            let mut render = std::array::from_fn::<_, AUDIO_FRAME_SAMPLES, _>(|_| noise(&mut seed));
            delay.push_back(render);
            let mut microphone = delay.pop_front().unwrap().map(|v| v / 2);
            apm.process_reverse_stream(&mut render, 48_000, 1).unwrap();
            if frame > 400 {
                before += microphone
                    .iter()
                    .map(|v| f64::from(*v).powi(2))
                    .sum::<f64>();
            }
            apm.process_stream(&mut microphone, 48_000, 1).unwrap();
            if frame > 400 {
                after += microphone
                    .iter()
                    .map(|v| f64::from(*v).powi(2))
                    .sum::<f64>();
            }
        }
        let reduction = 10.0 * (before / after.max(1e-15)).log10();
        eprintln!("echo reduction: {reduction:.1} dB");
        assert!(reduction > 15.0, "echo reduction: {reduction}");
    }
}
