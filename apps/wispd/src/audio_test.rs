//! Explicit, local-only microphone recording and A/B playback. PCM never leaves memory.
use crate::audio::{
    AUDIO_FRAME_SAMPLES, AUDIO_SAMPLE_RATE, CaptureQueue, DenoiserService, preset_code,
};
use anyhow::{Context, bail};
use gstreamer::{self as gst, prelude::*};
use serde::Serialize;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{sync::Mutex as AsyncMutex, task::JoinHandle};
use wisp_protocol::AudioPreset;

const MAX_FRAMES: usize = 800;

#[derive(Clone, Default, Serialize)]
pub(crate) struct TestStatus {
    pub phase: &'static str,
    pub duration_ms: u32,
    pub input_level: u8,
    pub preset: AudioPreset,
    pub playback: Option<&'static str>,
    pub error: Option<String>,
}

struct Sample {
    status: TestStatus,
    original: Vec<i16>,
    processed: Vec<i16>,
}
impl Default for Sample {
    fn default() -> Self {
        Self {
            status: TestStatus {
                phase: "idle",
                ..TestStatus::default()
            },
            original: Vec::new(),
            processed: Vec::new(),
        }
    }
}

#[derive(Default)]
pub(crate) struct AudioTest {
    sample: Arc<Mutex<Sample>>,
    task: AsyncMutex<Option<JoinHandle<()>>>,
    denoiser: AsyncMutex<Option<Arc<DenoiserService>>>,
}

impl Drop for AudioTest {
    fn drop(&mut self) {
        if let Some(task) = self.task.get_mut().take() {
            task.abort();
        }
    }
}

/// Cancelling a task must close its hardware pipeline, including during an await.
struct Pipeline(gst::Pipeline);
impl Drop for Pipeline {
    fn drop(&mut self) {
        let _ = self.0.set_state(gst::State::Null);
    }
}

impl AudioTest {
    pub fn status(&self) -> TestStatus {
        self.sample
            .lock()
            .expect("audio test lock poisoned")
            .status
            .clone()
    }

    pub async fn stop(&self, discard: bool) {
        if let Some(task) = self.task.lock().await.take() {
            task.abort();
            let _ = task.await;
        }
        if let Some(denoiser) = self.denoiser.lock().await.take() {
            let _ = denoiser.stop_session().await;
        }
        let mut sample = self.sample.lock().expect("audio test lock poisoned");
        if discard {
            *sample = Sample::default();
        } else {
            sample.status.phase = if sample.original.is_empty() {
                "idle"
            } else {
                "ready"
            };
            sample.status.playback = None;
            sample.status.input_level = 0;
        }
    }

    pub async fn record(
        &self,
        microphone: &str,
        preset: AudioPreset,
        denoiser: Arc<DenoiserService>,
    ) -> anyhow::Result<()> {
        self.stop(true).await;
        let frames = Arc::new(CaptureQueue::default());
        let (pipeline, source) =
            crate::media::create_microphone_capture_pipeline(microphone, frames.clone())?;
        let pipeline = Pipeline(pipeline);
        frames.activate(source);
        denoiser.start_session().await?;
        *self.denoiser.lock().await = Some(denoiser.clone());
        if let Err(error) = pipeline.0.set_state(gst::State::Playing) {
            self.stop(true).await;
            return Err(error).context("start microphone test");
        }
        {
            let mut sample = self.sample.lock().expect("audio test lock poisoned");
            sample.status.phase = "recording";
            sample.status.preset = preset;
        }
        let sample = self.sample.clone();
        *self.task.lock().await = Some(tokio::spawn(async move {
            let result = tokio::time::timeout(
                Duration::from_secs(12),
                record_frames(&sample, frames, preset, &denoiser),
            )
            .await;
            drop(pipeline); // Close the microphone before allowing any playback.
            let _ = denoiser.stop_session().await;
            let mut sample = sample.lock().expect("audio test lock poisoned");
            let error = match result {
                Ok(Ok(())) => None,
                Ok(Err(e)) => Some(e.to_string()),
                Err(_) => Some("Microphone test timed out. Check the selected device.".into()),
            };
            if error.is_some() {
                *sample = Sample::default();
            }
            sample.status.phase = if error.is_some() { "idle" } else { "ready" };
            sample.status.error = error;
            sample.status.input_level = 0;
        }));
        Ok(())
    }

    pub async fn play(&self, speaker: &str, original: bool) -> anyhow::Result<()> {
        self.stop(false).await;
        let samples = {
            let sample = self.sample.lock().expect("audio test lock poisoned");
            if sample.original.is_empty() {
                bail!("Record a microphone sample first");
            }
            if original {
                sample.original.clone()
            } else {
                sample.processed.clone()
            }
        };
        self.play_samples(speaker, samples, original).await
    }

    pub(crate) async fn play_clip(&self, speaker: &str, samples: Vec<i16>) -> anyhow::Result<()> {
        self.play_samples(speaker, samples, true).await
    }

    async fn play_samples(
        &self,
        speaker: &str,
        samples: Vec<i16>,
        original: bool,
    ) -> anyhow::Result<()> {
        self.stop(false).await;
        let (pipeline, source) = playback_pipeline(speaker)?;
        let pipeline = Pipeline(pipeline);
        // PipeWire cannot render a multi-second buffer as one quantum: it may
        // complete playback while dropping the oversized buffer. Queue the same
        // 10 ms PCM frames used by capture, each with its own timestamp.
        for (index, frame) in samples.chunks(AUDIO_FRAME_SAMPLES).enumerate() {
            let offset = u64::try_from(index * AUDIO_FRAME_SAMPLES)?;
            let duration = u64::try_from(frame.len())?;
            let bytes: Vec<u8> = frame.iter().flat_map(|v| v.to_le_bytes()).collect();
            let mut buffer = gst::Buffer::from_mut_slice(bytes);
            {
                let buffer = buffer.get_mut().context("prepare test playback")?;
                buffer.set_pts(gst::ClockTime::from_nseconds(
                    offset * 1_000_000_000 / u64::from(AUDIO_SAMPLE_RATE),
                ));
                buffer.set_duration(gst::ClockTime::from_nseconds(
                    duration * 1_000_000_000 / u64::from(AUDIO_SAMPLE_RATE),
                ));
            }
            source.push_buffer(buffer).context("queue test playback")?;
        }
        source.end_of_stream().context("finish test playback")?;
        pipeline
            .0
            .set_state(gst::State::Playing)
            .context("start test playback")?;
        let bus = pipeline.0.bus().context("test playback bus")?;
        {
            let mut sample = self.sample.lock().expect("audio test lock poisoned");
            sample.status.phase = "playing";
            sample.status.playback = Some(if original { "original" } else { "processed" });
            sample.status.error = None;
        }
        let sample = self.sample.clone();
        *self.task.lock().await = Some(tokio::spawn(async move {
            let started = Instant::now();
            let error = loop {
                if let Some(message) =
                    bus.pop_filtered(&[gst::MessageType::Eos, gst::MessageType::Error])
                {
                    break match message.view() {
                        gst::MessageView::Error(e) => {
                            Some(format!("Speaker test failed: {}", e.error()))
                        }
                        _ => None,
                    };
                }
                if started.elapsed() > Duration::from_secs(12) {
                    break Some("Speaker test timed out. Check the selected device.".into());
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            };
            drop(pipeline);
            let mut sample = sample.lock().expect("audio test lock poisoned");
            sample.status.phase = "ready";
            sample.status.playback = None;
            sample.status.error = error;
        }));
        Ok(())
    }
}

async fn record_frames(
    sample: &Mutex<Sample>,
    frames: Arc<CaptureQueue>,
    preset: AudioPreset,
    denoiser: &DenoiserService,
) -> anyhow::Result<()> {
    let mut previous = None;
    for _ in 0..MAX_FRAMES {
        let frame = tokio::time::timeout(Duration::from_secs(3), frames.recv())
            .await
            .context("No microphone audio received")?;
        let reset = previous.is_none_or(|seq| frame.sequence != seq + 1);
        previous = Some(frame.sequence);
        let processed = denoiser
            .process(&frame.samples, preset_code(preset), reset)
            .await?;
        let peak = frame
            .samples
            .iter()
            .map(|v| u32::from(v.unsigned_abs()))
            .max()
            .unwrap_or(0);
        let mut sample = sample.lock().expect("audio test lock poisoned");
        sample.original.extend_from_slice(&frame.samples);
        sample.processed.extend_from_slice(&processed.samples);
        sample.status.duration_ms =
            u32::try_from(sample.original.len() / AUDIO_FRAME_SAMPLES * 10).unwrap_or(8000);
        sample.status.input_level = u8::try_from(peak * 100 / 32768).unwrap_or(100);
    }
    Ok(())
}

fn playback_pipeline(speaker: &str) -> anyhow::Result<(gst::Pipeline, gstreamer_app::AppSrc)> {
    gst::init()?;
    let name = speaker.strip_prefix("default: ").unwrap_or(speaker);
    let sink = crate::media::discovered_capture_devices()?
        .into_iter()
        .find(|device| device.has_classes("Audio/Sink") && device.display_name() == name)
        .with_context(|| format!("Speaker is no longer available: {speaker}"))?
        .create_element(Some("wisp-test-speaker"))?;
    let caps = gst::Caps::builder("audio/x-raw")
        .field("format", "S16LE")
        .field("layout", "interleaved")
        .field("rate", 48_000_i32)
        .field("channels", 1_i32)
        .build();
    let source = gstreamer_app::AppSrc::builder()
        .caps(&caps)
        .format(gst::Format::Time)
        .build();
    let convert = gst::ElementFactory::make("audioconvert").build()?;
    let resample = gst::ElementFactory::make("audioresample").build()?;
    let pipeline = gst::Pipeline::default();
    pipeline.add_many([source.upcast_ref(), &convert, &resample, &sink])?;
    gst::Element::link_many([source.upcast_ref(), &convert, &resample, &sink])?;
    Ok((pipeline, source))
}
