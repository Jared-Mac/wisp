use anyhow::{Context, bail};
use ashpd::desktop::{
    CreateSessionOptions, PersistMode, Session,
    screencast::{
        CursorMode, OpenPipeWireRemoteOptions, Screencast, SelectSourcesOptions, SourceType,
        StartCastOptions,
    },
};
use df::tract::{DfParams, DfTract, RuntimeParams};
use futures_util::StreamExt;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use livekit::track::VideoQuality;
use livekit::{
    E2eeOptions,
    e2ee::{
        EncryptionType,
        key_provider::{KeyProvider, KeyProviderOptions},
    },
    options::{TrackPublishOptions, VideoCodec, VideoEncoderBackend, VideoEncoding},
    prelude::{
        AudioProcessingOptions, DataPacket, LocalAudioTrack, LocalTrack, LocalVideoTrack,
        Participant, PlatformAudio, PlayoutDeviceInfo, RecordingDeviceInfo, RemoteAudioTrack,
        RemoteTrack, RemoteTrackPublication, RemoteVideoTrack, Room, RoomEvent, RoomOptions,
        RtcAudioSource, TrackKind, TrackSid, TrackSource,
    },
    rtc_engine::lk_runtime::LkRuntime,
    webrtc::{
        audio_frame::AudioFrame,
        audio_source::{AudioSourceOptions, native::NativeAudioSource},
        audio_stream::native::NativeAudioStream,
        peer_connection_factory::native::PeerConnectionFactoryExt,
        prelude::VideoFormatType,
        video_frame::{I420Buffer, VideoFrame, VideoRotation, native::VideoFrameBufferExt},
        video_source::{RtcVideoSource, VideoResolution, native::NativeVideoSource},
        video_stream::native::NativeVideoStream,
    },
};
use ndarray::{ArrayView2, ArrayViewMut2};
use nnnoiseless::DenoiseState;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fs,
    os::fd::{AsRawFd, OwnedFd},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    time::Instant,
};
use tokio::{
    sync::{Mutex as AsyncMutex, mpsc},
    task::{JoinHandle, JoinSet},
};
use tracing::{debug, info, warn};
use wisp_protocol::{
    AudioDevice, AudioPreset, AudioState, CameraState, DEFAULT_DEEPFILTER_STRENGTH, HangoutId,
    LiveKitTokenResponse, RemoteVideoState, RemoteVideoTarget, ScreenShareState,
    VideoCodecPreference, VideoDevice, VideoQualityPreset, VideoSettings, VideoSource,
};

use crate::surface::{RgbaFrame, SurfaceController};

const AUDIO_SAMPLE_RATE: u32 = 48_000;
const AUDIO_FRAME_SAMPLES: usize = 480;
const DEEPFILTER_LATENCY_MS: u16 = 30;
const RNNOISE_LATENCY_MS: u16 = 10;
const AUDIO_FRAME_BUDGET_US: u32 = 10_000;
const VIDEO_WATCH_TOPIC: &str = "wisp.video-watch.v1";
const PREVIEW_MAX_WIDTH: u32 = 480;
const PREVIEW_MAX_HEIGHT: u32 = 270;

#[derive(Debug, Serialize, Deserialize)]
struct VideoWatchSignal {
    participant: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    participant_identity: Option<String>,
    source: VideoSource,
    watching: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum DenoiserBackend {
    DeepFilterNet,
    Rnnoise,
}

impl DenoiserBackend {
    fn from_atomic(value: u8) -> Self {
        if value == Self::Rnnoise as u8 {
            Self::Rnnoise
        } else {
            Self::DeepFilterNet
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::DeepFilterNet => "deepfilternet",
            Self::Rnnoise => "rnnoise",
        }
    }

    const fn latency_ms(self) -> u16 {
        match self {
            Self::DeepFilterNet => DEEPFILTER_LATENCY_MS,
            Self::Rnnoise => RNNOISE_LATENCY_MS,
        }
    }
}

enum NeuralDenoiser {
    DeepFilterNet(Box<DfTract>),
    Rnnoise {
        state: Box<DenoiseState<'static>>,
        first_frame: bool,
    },
}

impl NeuralDenoiser {
    fn deepfilternet(model: DfTract) -> Self {
        Self::DeepFilterNet(Box::new(model))
    }

    fn rnnoise() -> Self {
        Self::Rnnoise {
            state: DenoiseState::new(),
            first_frame: true,
        }
    }

    const fn backend(&self) -> DenoiserBackend {
        match self {
            Self::DeepFilterNet(_) => DenoiserBackend::DeepFilterNet,
            Self::Rnnoise { .. } => DenoiserBackend::Rnnoise,
        }
    }

    fn process_frame(&mut self, input: &[i16]) -> anyhow::Result<Vec<i16>> {
        match self {
            Self::DeepFilterNet(model) => deepfilter_frame(model, input),
            Self::Rnnoise { state, first_frame } => Ok(rnnoise_frame(state, input, first_frame)),
        }
    }

    fn set_strength(&mut self, strength: u8) {
        if let Self::DeepFilterNet(model) = self {
            // DeepFilterNet defines 0 dB as bypass and 100 dB as unlimited
            // attenuation, which maps directly to Wisp's 0–100 strength scale.
            model.set_atten_lim(f32::from(strength));
        }
    }
}

enum DenoiserRequest {
    StartSession {
        response: tokio::sync::oneshot::Sender<DenoiserBackend>,
    },
    Process {
        input: Vec<i16>,
        response: tokio::sync::oneshot::Sender<Vec<i16>>,
    },
}

struct DenoiserService {
    requests: Option<mpsc::Sender<DenoiserRequest>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl DenoiserService {
    fn spawn(backend_state: Arc<AtomicU8>, strength: Arc<AtomicU8>) -> anyhow::Result<Self> {
        let (requests, mut request_rx) = mpsc::channel::<DenoiserRequest>(2);
        let worker = std::thread::Builder::new()
            .name("wisp-denoiser".into())
            .spawn(move || {
                let mut denoiser = preferred_neural_denoiser();
                let mut session_processed_audio = false;
                let mut applied_strength = strength.load(Ordering::Acquire);
                denoiser.set_strength(applied_strength);
                backend_state.store(denoiser.backend() as u8, Ordering::Release);

                while let Some(request) = request_rx.blocking_recv() {
                    match request {
                        DenoiserRequest::StartSession { response } => {
                            if session_processed_audio {
                                denoiser = preferred_neural_denoiser();
                                applied_strength = strength.load(Ordering::Acquire);
                                denoiser.set_strength(applied_strength);
                                backend_state
                                    .store(denoiser.backend() as u8, Ordering::Release);
                            }
                            session_processed_audio = false;
                            let _ = response.send(denoiser.backend());
                        }
                        DenoiserRequest::Process { input, response } => {
                            let requested_strength = strength.load(Ordering::Acquire);
                            if requested_strength != applied_strength {
                                denoiser.set_strength(requested_strength);
                                applied_strength = requested_strength;
                            }
                            session_processed_audio = true;
                            let output = match denoiser.process_frame(&input) {
                                Ok(output) => output,
                                Err(error) => {
                                    warn!(%error, "DeepFilterNet processing failed; falling back to RNNoise");
                                    denoiser = NeuralDenoiser::rnnoise();
                                    backend_state.store(
                                        DenoiserBackend::Rnnoise as u8,
                                        Ordering::Release,
                                    );
                                    denoiser.process_frame(&input).unwrap_or_else(
                                        |fallback_error| {
                                            warn!(%fallback_error, "RNNoise fallback failed; publishing raw audio");
                                            input
                                        },
                                    )
                                }
                            };
                            if response.send(output).is_err() {
                                debug!("discarding denoised frame after microphone pipeline stopped");
                            }
                        }
                    }
                }
            })
            .context("start neural denoiser worker")?;
        Ok(Self {
            requests: Some(requests),
            worker: Some(worker),
        })
    }

    async fn start_session(&self) -> anyhow::Result<DenoiserBackend> {
        let (response, result) = tokio::sync::oneshot::channel();
        self.requests
            .as_ref()
            .context("neural denoiser worker is unavailable")?
            .send(DenoiserRequest::StartSession { response })
            .await
            .context("start neural denoiser session")?;
        result.await.context("neural denoiser worker stopped")
    }

    async fn process(&self, input: Vec<i16>) -> anyhow::Result<Vec<i16>> {
        let (response, result) = tokio::sync::oneshot::channel();
        self.requests
            .as_ref()
            .context("neural denoiser worker is unavailable")?
            .send(DenoiserRequest::Process { input, response })
            .await
            .context("queue neural denoiser frame")?;
        result.await.context("neural denoiser worker stopped")
    }
}

impl Drop for DenoiserService {
    fn drop(&mut self) {
        self.requests.take();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            warn!("neural denoiser worker panicked");
        }
    }
}

#[derive(Debug)]
pub(crate) enum MediaEvent {
    Reconnecting {
        generation: u64,
    },
    Reconnected {
        generation: u64,
    },
    Disconnected {
        generation: u64,
        reason: String,
    },
    AudioSubscribed {
        generation: u64,
        participant: String,
    },
    AudioUnsubscribed {
        generation: u64,
        participant: String,
    },
    RemoteMuteChanged {
        generation: u64,
        participant: String,
        muted: bool,
    },
    AudioFrames {
        generation: u64,
        participant: String,
        total: u64,
        level: u8,
    },
    InputLevel {
        generation: u64,
        level: u8,
    },
    AudioTelemetry {
        generation: u64,
        level: u8,
        processing_time_us: u32,
        processing_deadline_misses: u64,
        capture_queue_ms: u16,
    },
    ActiveSpeakers {
        generation: u64,
        speakers: Vec<String>,
    },
    VideoSubscribed {
        generation: u64,
        target: RemoteVideoTarget,
    },
    VideoAvailable {
        generation: u64,
        target: RemoteVideoTarget,
        mime_type: String,
        simulcasted: bool,
    },
    VideoUnavailable {
        generation: u64,
        target: RemoteVideoTarget,
    },
    VideoUnsubscribed {
        generation: u64,
        target: RemoteVideoTarget,
    },
    VideoFrames {
        generation: u64,
        target: RemoteVideoTarget,
        total: u64,
        track_total: u64,
        width: u32,
        height: u32,
    },
    ScreenShareFrames {
        generation: u64,
        total: u64,
    },
    ScreenShareStopped {
        generation: u64,
        error: Option<String>,
    },
    CameraFrames {
        generation: u64,
        total: u64,
    },
    CameraStopped {
        generation: u64,
        error: Option<String>,
    },
    VideoViewerChanged {
        generation: u64,
        source: VideoSource,
        participant: String,
        watching: bool,
    },
    VideoViewerDisconnected {
        generation: u64,
        participant: String,
    },
    SurfaceOpened {
        target: RemoteVideoTarget,
    },
    SurfaceClosed {
        target: RemoteVideoTarget,
    },
    SurfaceVisibilityChanged {
        target: RemoteVideoTarget,
        visible: bool,
    },
    SurfaceResized {
        target: RemoteVideoTarget,
        width: u32,
        height: u32,
    },
    SurfaceRendered {
        target: RemoteVideoTarget,
        total: u64,
    },
    SurfaceError {
        target: Option<RemoteVideoTarget>,
        message: String,
    },
}

pub(crate) struct ConnectedMedia {
    pub microphone: String,
    pub speaker: String,
    pub audio: AudioState,
    pub remote_audio_participants: Vec<String>,
    pub remote_muted_participants: Vec<String>,
    pub remote_videos: Vec<RemoteVideoState>,
    pub e2ee_enabled: bool,
}

pub(crate) struct AudioInventory {
    pub state: AudioState,
    pub microphone: Option<String>,
    pub speaker: Option<String>,
    pub error: Option<String>,
}

pub(crate) struct ScreenShareInfo {
    pub state: ScreenShareState,
}

pub(crate) struct CameraInfo {
    pub state: CameraState,
}

#[derive(Debug)]
struct AudioPreferences {
    preferred_input_id: Option<String>,
    preferred_output_id: Option<String>,
    selected_input_id: Option<String>,
    selected_output_id: Option<String>,
    preset: AudioPreset,
    deepfilter_strength: u8,
}

impl Default for AudioPreferences {
    fn default() -> Self {
        Self {
            preferred_input_id: None,
            preferred_output_id: None,
            selected_input_id: None,
            selected_output_id: None,
            preset: AudioPreset::default(),
            deepfilter_strength: DEFAULT_DEEPFILTER_STRENGTH,
        }
    }
}

#[derive(Debug, Default)]
struct VideoPreferences {
    selected_camera_id: Option<String>,
    quality: VideoQualityPreset,
    codec: VideoCodecPreference,
}

struct MediaSession {
    hangout_id: HangoutId,
    room: Arc<Room>,
    microphone: LocalAudioTrack,
    microphone_source: NativeAudioSource,
    microphone_capture: gst::Pipeline,
    microphone_frames: mpsc::UnboundedSender<Vec<i16>>,
    microphone_task: JoinHandle<()>,
    screen_share: Option<ScreenShareSession>,
    camera: Option<CameraSession>,
    _platform_audio: PlatformAudio,
    remote_audio: Arc<Mutex<HashMap<String, (String, RemoteAudioTrack)>>>,
    remote_video: Arc<Mutex<HashMap<RemoteVideoTarget, RemoteTrackPublication>>>,
    desired_video: Arc<Mutex<HashSet<RemoteVideoTarget>>>,
    event_task: JoinHandle<()>,
}

struct ScreenShareSession {
    publication_sid: TrackSid,
    pipeline: gst::Pipeline,
    portal_session: Session<Screencast>,
    _pipewire_remote: OwnedFd,
    monitor_running: Arc<AtomicBool>,
    monitor_task: JoinHandle<()>,
}

struct CameraSession {
    publication_sid: TrackSid,
    pipeline: gst::Pipeline,
    monitor_running: Arc<AtomicBool>,
    monitor_task: JoinHandle<()>,
}

pub(crate) struct MediaManager {
    pub(crate) video_bridge: crate::video_bridge::VideoBridge,
    operation: AsyncMutex<()>,
    session: AsyncMutex<Option<MediaSession>>,
    generation: AtomicU64,
    connected: Arc<AtomicBool>,
    deafened: Arc<AtomicBool>,
    received_frames: Arc<AtomicU64>,
    received_video_frames: Arc<AtomicU64>,
    input_level: Arc<AtomicU8>,
    capture_queue_samples: Arc<AtomicU64>,
    platform_audio: Mutex<Option<PlatformAudio>>,
    audio_preferences: Mutex<AudioPreferences>,
    participant_volumes: Arc<Mutex<HashMap<String, f64>>>,
    video_preferences: Mutex<VideoPreferences>,
    neural_denoiser_enabled: Arc<AtomicBool>,
    deepfilter_strength: Arc<AtomicU8>,
    denoiser_backend: Arc<AtomicU8>,
    denoiser: Arc<DenoiserService>,
    surface: Option<SurfaceController>,
    event_tx: mpsc::UnboundedSender<MediaEvent>,
    e2ee_key: Mutex<Option<Vec<u8>>>,
    local_name: String,
}

impl MediaManager {
    pub(crate) fn encryption_configured(&self) -> bool {
        self.e2ee_key
            .lock()
            .expect("media encryption key lock poisoned")
            .as_ref()
            .is_some_and(|key| key.len() >= 16)
    }

    pub(crate) fn set_encryption_key(&self, key: Option<String>) {
        *self
            .e2ee_key
            .lock()
            .expect("media encryption key lock poisoned") = key.map(String::into_bytes);
    }

    pub(crate) fn new(
        surface_enabled: bool,
        e2ee_key: Option<String>,
        local_name: String,
    ) -> (Self, mpsc::UnboundedReceiver<MediaEvent>) {
        remove_local_preview(VideoSource::Camera);
        remove_local_preview(VideoSource::ScreenShare);
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let denoiser_backend = Arc::new(AtomicU8::new(DenoiserBackend::DeepFilterNet as u8));
        let deepfilter_strength = Arc::new(AtomicU8::new(DEFAULT_DEEPFILTER_STRENGTH));
        let denoiser = Arc::new(
            DenoiserService::spawn(denoiser_backend.clone(), deepfilter_strength.clone())
                .expect("start neural denoiser service"),
        );
        let surface = if surface_enabled {
            match SurfaceController::spawn(event_tx.clone()) {
                Ok(surface) => Some(surface),
                Err(error) => {
                    let _ = event_tx.send(MediaEvent::SurfaceError {
                        target: None,
                        message: error.to_string(),
                    });
                    None
                }
            }
        } else {
            None
        };
        (
            Self {
                video_bridge: crate::video_bridge::VideoBridge::default(),
                operation: AsyncMutex::new(()),
                session: AsyncMutex::new(None),
                generation: AtomicU64::new(0),
                connected: Arc::new(AtomicBool::new(false)),
                deafened: Arc::new(AtomicBool::new(false)),
                received_frames: Arc::new(AtomicU64::new(0)),
                received_video_frames: Arc::new(AtomicU64::new(0)),
                input_level: Arc::new(AtomicU8::new(0)),
                capture_queue_samples: Arc::new(AtomicU64::new(0)),
                platform_audio: Mutex::new(None),
                audio_preferences: Mutex::new(AudioPreferences::default()),
                participant_volumes: Arc::new(Mutex::new(HashMap::new())),
                video_preferences: Mutex::new(VideoPreferences::default()),
                neural_denoiser_enabled: Arc::new(AtomicBool::new(true)),
                deepfilter_strength,
                denoiser_backend,
                denoiser,
                surface,
                event_tx,
                e2ee_key: Mutex::new(e2ee_key.map(String::into_bytes)),
                local_name,
            },
            event_rx,
        )
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    fn platform_audio(&self) -> anyhow::Result<PlatformAudio> {
        let mut audio = self
            .platform_audio
            .lock()
            .expect("platform audio lock poisoned");
        if audio.is_none() {
            let platform = PlatformAudio::new().context("initialize microphone and speaker")?;
            // Microphone samples are supplied through GStreamer's PipeWire source. Leaving
            // WebRTC's separate PulseAudio recorder enabled creates an unused capture thread
            // and can race the audio transport teardown during a LiveKit reconnect.
            LkRuntime::instance()
                .pc_factory()
                .set_adm_recording_enabled(false);
            *audio = Some(platform);
        }
        Ok(audio
            .as_ref()
            .expect("platform audio was initialized")
            .clone())
    }

    pub(crate) async fn is_active(&self) -> bool {
        self.session.lock().await.is_some()
    }

    fn denoiser_backend(&self) -> DenoiserBackend {
        DenoiserBackend::from_atomic(self.denoiser_backend.load(Ordering::Acquire))
    }

    pub(crate) async fn refresh_audio_devices(&self) -> AudioInventory {
        let _operation = self.operation.lock().await;
        let active = self.is_active().await;
        match self.platform_audio() {
            Ok(audio) => self.reconcile_audio_devices(&audio, active, false),
            Err(error) => self.unavailable_audio_inventory(error.to_string()),
        }
    }

    pub(crate) async fn select_input_device(&self, id: &str) -> anyhow::Result<AudioInventory> {
        let _operation = self.operation.lock().await;
        let audio = self.platform_audio()?;
        let device = audio
            .recording_devices()
            .find(|device| recording_device_id(device) == id)
            .with_context(|| format!("microphone is no longer available: {id}"))?;
        let active = self.is_active().await;
        if active {
            let mut session = self.session.lock().await;
            if let Some(session) = session.as_mut() {
                let replacement = create_microphone_capture_pipeline(
                    &device.name,
                    session.microphone_frames.clone(),
                    self.capture_queue_samples.clone(),
                )?;
                replacement
                    .set_state(gst::State::Playing)
                    .context("start the replacement microphone capture pipeline")?;
                let previous = std::mem::replace(&mut session.microphone_capture, replacement);
                let _ = previous.set_state(gst::State::Null);
            }
        }
        {
            let mut preferences = self
                .audio_preferences
                .lock()
                .expect("audio preferences lock poisoned");
            preferences.preferred_input_id = Some(id.to_owned());
            preferences.selected_input_id = Some(id.to_owned());
        }
        Ok(self.reconcile_audio_devices(&audio, active, false))
    }

    pub(crate) async fn select_output_device(&self, id: &str) -> anyhow::Result<AudioInventory> {
        let _operation = self.operation.lock().await;
        let audio = self.platform_audio()?;
        let device = audio
            .playout_devices()
            .find(|device| playout_device_id(device) == id)
            .with_context(|| format!("speaker is no longer available: {id}"))?;
        let active = self.is_active().await;
        select_playout_device(&audio, &device, active)
            .with_context(|| format!("select speaker {}", device.name))?;
        {
            let mut preferences = self
                .audio_preferences
                .lock()
                .expect("audio preferences lock poisoned");
            preferences.preferred_output_id = Some(id.to_owned());
            preferences.selected_output_id = Some(id.to_owned());
        }
        Ok(self.reconcile_audio_devices(&audio, active, false))
    }

    pub(crate) async fn set_audio_preset(
        &self,
        preset: AudioPreset,
    ) -> anyhow::Result<AudioInventory> {
        let _operation = self.operation.lock().await;
        let audio = self.platform_audio()?;
        audio
            .configure_audio_processing(processing_options(preset))
            .with_context(|| format!("apply {preset} audio preset"))?;
        if let Some(session) = self.session.lock().await.as_ref() {
            session
                .microphone_source
                .set_audio_options(source_processing_options(preset));
        }
        self.audio_preferences
            .lock()
            .expect("audio preferences lock poisoned")
            .preset = preset;
        self.neural_denoiser_enabled
            .store(preset == AudioPreset::Clear, Ordering::Release);
        Ok(self.reconcile_audio_devices(&audio, self.is_active().await, false))
    }

    pub(crate) async fn set_deepfilter_strength(
        &self,
        strength: u8,
    ) -> anyhow::Result<AudioInventory> {
        if strength > 100 {
            bail!("DeepFilterNet strength must be between 0 and 100");
        }
        let _operation = self.operation.lock().await;
        self.deepfilter_strength.store(strength, Ordering::Release);
        self.audio_preferences
            .lock()
            .expect("audio preferences lock poisoned")
            .deepfilter_strength = strength;
        let audio = self.platform_audio()?;
        Ok(self.reconcile_audio_devices(&audio, self.is_active().await, false))
    }

    pub(crate) fn video_settings(&self) -> VideoSettings {
        let preferences = self
            .video_preferences
            .lock()
            .expect("video preferences lock poisoned");
        detected_video_settings(preferences.quality, preferences.codec)
    }

    pub(crate) async fn refresh_camera_devices(&self) -> anyhow::Result<CameraState> {
        let _operation = self.operation.lock().await;
        let devices = enumerate_camera_devices()?;
        let mut preferences = self
            .video_preferences
            .lock()
            .expect("video preferences lock poisoned");
        let selected_device_id = preferences
            .selected_camera_id
            .as_ref()
            .filter(|selected| devices.iter().any(|(device, _)| &device.id == *selected))
            .cloned()
            .or_else(|| devices.first().map(|(device, _)| device.id.clone()));
        preferences
            .selected_camera_id
            .clone_from(&selected_device_id);
        Ok(CameraState {
            devices: devices.into_iter().map(|(device, _)| device).collect(),
            selected_device_id,
            ..CameraState::default()
        })
    }

    pub(crate) async fn select_camera_device(&self, id: &str) -> anyhow::Result<CameraState> {
        let _operation = self.operation.lock().await;
        let camera_active = self
            .session
            .lock()
            .await
            .as_ref()
            .is_some_and(|session| session.camera.is_some());
        if camera_active {
            bail!("stop the camera before changing devices");
        }
        let devices = enumerate_camera_devices()?;
        if !devices.iter().any(|(device, _)| device.id == id) {
            bail!("camera is no longer available: {id}");
        }
        self.video_preferences
            .lock()
            .expect("video preferences lock poisoned")
            .selected_camera_id = Some(id.to_owned());
        Ok(CameraState {
            devices: devices.into_iter().map(|(device, _)| device).collect(),
            selected_device_id: Some(id.to_owned()),
            ..CameraState::default()
        })
    }

    pub(crate) async fn set_video_quality(
        &self,
        quality: VideoQualityPreset,
    ) -> anyhow::Result<VideoSettings> {
        let _operation = self.operation.lock().await;
        if self.local_video_active().await {
            bail!("stop screen sharing and the camera before changing video quality");
        }
        let mut preferences = self
            .video_preferences
            .lock()
            .expect("video preferences lock poisoned");
        preferences.quality = quality;
        Ok(detected_video_settings(
            preferences.quality,
            preferences.codec,
        ))
    }

    pub(crate) async fn set_video_codec(
        &self,
        codec: VideoCodecPreference,
    ) -> anyhow::Result<VideoSettings> {
        let _operation = self.operation.lock().await;
        if self.local_video_active().await {
            bail!("stop screen sharing and the camera before changing video codec");
        }
        let mut preferences = self
            .video_preferences
            .lock()
            .expect("video preferences lock poisoned");
        preferences.codec = codec;
        Ok(detected_video_settings(
            preferences.quality,
            preferences.codec,
        ))
    }

    async fn local_video_active(&self) -> bool {
        self.session
            .lock()
            .await
            .as_ref()
            .is_some_and(|session| session.screen_share.is_some() || session.camera.is_some())
    }

    fn unavailable_audio_inventory(&self, error: String) -> AudioInventory {
        let preferences = self
            .audio_preferences
            .lock()
            .expect("audio preferences lock poisoned");
        let mut state = AudioState {
            preset: preferences.preset,
            deepfilter_strength: preferences.deepfilter_strength,
            ..AudioState::default()
        };
        apply_denoiser_state(&mut state, self.denoiser_backend());
        AudioInventory {
            state,
            microphone: None,
            speaker: None,
            error: Some(error),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn reconcile_audio_devices(
        &self,
        audio: &PlatformAudio,
        active: bool,
        force_selection: bool,
    ) -> AudioInventory {
        let recording_devices = audio.recording_devices().collect::<Vec<_>>();
        let playout_devices = audio.playout_devices().collect::<Vec<_>>();
        let input_devices = recording_devices
            .iter()
            .map(protocol_recording_device)
            .collect::<Vec<_>>();
        let output_devices = playout_devices
            .iter()
            .map(protocol_playout_device)
            .collect::<Vec<_>>();

        let mut preferences = self
            .audio_preferences
            .lock()
            .expect("audio preferences lock poisoned");
        let next_input =
            preferred_or_first(&input_devices, preferences.preferred_input_id.as_deref())
                .map(|device| device.id.clone());
        let next_output =
            preferred_or_first(&output_devices, preferences.preferred_output_id.as_deref())
                .map(|device| device.id.clone());
        let input_changed = preferences.selected_input_id != next_input;
        let output_changed = preferences.selected_output_id != next_output;
        let mut errors = Vec::new();

        if let Some(id) = &next_input
            && (force_selection || input_changed)
        {
            preferences.selected_input_id = Some(id.clone());
        }
        if let Some(id) = &next_output
            && (force_selection || output_changed)
            && let Some(device) = playout_devices
                .iter()
                .find(|device| playout_device_id(device) == *id)
        {
            let result = select_playout_device(audio, device, active);
            match result {
                Ok(()) => preferences.selected_output_id.clone_from(&next_output),
                Err(error) => {
                    errors.push(format!("select speaker {}: {error}", device.name));
                    preferences.selected_output_id = preferences
                        .selected_output_id
                        .take()
                        .filter(|id| output_devices.iter().any(|device| &device.id == id));
                }
            }
        }

        if next_input.is_none() {
            preferences.selected_input_id = None;
        } else if !input_changed && !force_selection {
            preferences.selected_input_id.clone_from(&next_input);
        }
        if next_output.is_none() {
            preferences.selected_output_id = None;
        } else if !output_changed && !force_selection {
            preferences.selected_output_id.clone_from(&next_output);
        }
        if preferences.preferred_input_id.is_none() {
            let selected = preferences.selected_input_id.clone();
            preferences.preferred_input_id = selected;
        }
        if preferences.preferred_output_id.is_none() {
            let selected = preferences.selected_output_id.clone();
            preferences.preferred_output_id = selected;
        }
        if input_devices.is_empty() {
            errors.push("no microphone is available".into());
        }
        if output_devices.is_empty() {
            errors.push("no speaker is available".into());
        }

        let selected_input_id = preferences.selected_input_id.clone();
        let selected_output_id = preferences.selected_output_id.clone();
        let microphone = selected_device_name(&input_devices, selected_input_id.as_deref());
        let speaker = selected_device_name(&output_devices, selected_output_id.as_deref());
        let mut state = AudioState {
            input_devices,
            output_devices,
            selected_input_id,
            selected_output_id,
            preset: preferences.preset,
            input_level: 0,
            denoiser_active: false,
            denoiser: None,
            processing_latency_ms: 0,
            deepfilter_strength: preferences.deepfilter_strength,
            processing_time_us: 0,
            processing_deadline_misses: 0,
            capture_queue_ms: 0,
        };
        apply_denoiser_state(&mut state, self.denoiser_backend());
        AudioInventory {
            state,
            microphone,
            speaker,
            error: (!errors.is_empty()).then(|| errors.join("; ")),
        }
    }

    pub(crate) async fn is_connected_to(&self, hangout_id: HangoutId) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        self.session
            .lock()
            .await
            .as_ref()
            .is_some_and(|session| session.hangout_id == hangout_id)
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) async fn connect(
        &self,
        hangout_id: HangoutId,
        credentials: LiveKitTokenResponse,
        muted: bool,
        deafened: bool,
    ) -> anyhow::Result<ConnectedMedia> {
        let _operation = self.operation.lock().await;
        if self.is_connected_to(hangout_id).await {
            bail!("already connected to this room");
        }
        self.disconnect_session().await;

        let platform_audio = self.platform_audio()?;
        self.denoiser.start_session().await?;
        let inventory = self.reconcile_audio_devices(&platform_audio, false, false);
        if let Some(error) = &inventory.error {
            bail!(error.clone());
        }
        let microphone = inventory
            .microphone
            .clone()
            .context("no microphone is available")?;
        let speaker = inventory
            .speaker
            .clone()
            .context("no speaker is available")?;
        platform_audio
            .configure_audio_processing(processing_options(inventory.state.preset))
            .context("configure audio processing")?;

        let mut room_options = RoomOptions::default();
        room_options.auto_subscribe = true;
        room_options.dynacast = true;
        let e2ee_key = self
            .e2ee_key
            .lock()
            .expect("media encryption key lock poisoned")
            .clone();
        if let Some(key) = &e2ee_key {
            if key.len() < 16 {
                bail!("WISP_E2EE_KEY must contain at least 16 bytes");
            }
            room_options.encryption = Some(E2eeOptions {
                encryption_type: EncryptionType::Gcm,
                key_provider: KeyProvider::with_shared_key(
                    KeyProviderOptions::default(),
                    key.clone(),
                ),
            });
        }
        let (room, events) = Room::connect(&credentials.url, &credentials.token, room_options)
            .await
            .with_context(|| format!("connect to LiveKit room {}", credentials.room))?;
        if e2ee_key.is_some() {
            room.e2ee_manager().set_enabled(true);
        }
        let room = Arc::new(room);
        let microphone_source = NativeAudioSource::new(
            source_processing_options(inventory.state.preset),
            AUDIO_SAMPLE_RATE,
            1,
            0,
        );
        let microphone_track = LocalAudioTrack::create_audio_track(
            "microphone",
            RtcAudioSource::Native(microphone_source.clone()),
        );
        if muted {
            microphone_track.mute();
        }
        self.capture_queue_samples.store(0, Ordering::Release);
        let (microphone_frames, captured_frames) = mpsc::unbounded_channel();
        let microphone_capture = create_microphone_capture_pipeline(
            &microphone,
            microphone_frames.clone(),
            self.capture_queue_samples.clone(),
        )?;
        if let Err(error) = room
            .local_participant()
            .publish_track(
                LocalTrack::Audio(microphone_track.clone()),
                TrackPublishOptions {
                    source: TrackSource::Microphone,
                    ..Default::default()
                },
            )
            .await
        {
            let _ = room.close().await;
            return Err(error).context("publish microphone to LiveKit");
        }
        if let Err(error) = microphone_capture.set_state(gst::State::Playing) {
            let _ = room.close().await;
            return Err(error).context("start microphone capture");
        }

        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let microphone_task = tokio::spawn(run_microphone_pipeline(
            captured_frames,
            microphone_source.clone(),
            microphone_track.clone(),
            self.neural_denoiser_enabled.clone(),
            self.denoiser.clone(),
            self.input_level.clone(),
            self.capture_queue_samples.clone(),
            self.event_tx.clone(),
            generation,
        ));

        self.received_frames.store(0, Ordering::Release);
        self.received_video_frames.store(0, Ordering::Release);
        self.input_level.store(0, Ordering::Release);
        self.connected.store(true, Ordering::Release);
        self.deafened.store(deafened, Ordering::Release);
        let remote_audio = Arc::new(Mutex::new(HashMap::new()));
        let remote_video = Arc::new(Mutex::new(HashMap::new()));
        let desired_video = Arc::new(Mutex::new(HashSet::new()));
        let event_context = RoomEventContext {
            video_bridge: self.video_bridge.clone(),
            generation,
            event_tx: self.event_tx.clone(),
            room: room.clone(),
            local_name: self.local_name.clone(),
            local_identity: room.local_participant().identity().to_string(),
            remote_audio: remote_audio.clone(),
            participant_volumes: self.participant_volumes.clone(),
            remote_video: remote_video.clone(),
            desired_video: desired_video.clone(),
            received_frames: self.received_frames.clone(),
            received_video_frames: self.received_video_frames.clone(),
            connected: self.connected.clone(),
            deafened: self.deafened.clone(),
            surface: self.surface.clone(),
        };
        let mut remote_audio_participants = Vec::new();
        let mut remote_muted_participants = Vec::new();
        let mut initial_remote_videos = Vec::new();
        for participant in room.remote_participants().into_values() {
            let name = participant.name();
            for publication in participant.track_publications().into_values() {
                match publication.kind() {
                    TrackKind::Audio => {
                        remote_audio_participants.push(name.clone());
                        if publication.is_muted() {
                            remote_muted_participants.push(name.clone());
                        }
                    }
                    TrackKind::Video => {
                        if let Some(source) = protocol_video_source(publication.source()) {
                            initial_remote_videos.push(RemoteVideoState {
                                target: RemoteVideoTarget {
                                    participant: name.clone(),
                                    source,
                                },
                                mime_type: publication.mime_type(),
                                simulcasted: publication.simulcasted(),
                                subscribed: false,
                                surface_open: false,
                                surface_visible: false,
                                requested_quality: None,
                                received_frames: 0,
                                rendered_frames: 0,
                                width: None,
                                height: None,
                                error: None,
                            });
                        }
                    }
                }
                event_context.register_publication(name.clone(), &publication);
            }
        }
        remote_audio_participants.sort();
        remote_audio_participants.dedup();
        remote_muted_participants.sort();
        remote_muted_participants.dedup();
        initial_remote_videos.sort_by(|left, right| {
            left.target
                .participant
                .cmp(&right.target.participant)
                .then_with(|| {
                    left.target
                        .source
                        .to_string()
                        .cmp(&right.target.source.to_string())
                })
        });
        let event_task = tokio::spawn(run_room_events(events, event_context));
        *self.session.lock().await = Some(MediaSession {
            hangout_id,
            room,
            microphone: microphone_track,
            microphone_source,
            microphone_capture,
            microphone_frames,
            microphone_task,
            screen_share: None,
            camera: None,
            _platform_audio: platform_audio,
            remote_audio,
            remote_video,
            desired_video,
            event_task,
        });

        info!(
            room = %credentials.room,
            microphone = %microphone,
            speaker = %speaker,
            "LiveKit media connected"
        );
        Ok(ConnectedMedia {
            microphone,
            speaker,
            audio: inventory.state,
            remote_audio_participants,
            remote_muted_participants,
            remote_videos: initial_remote_videos,
            e2ee_enabled: e2ee_key.is_some(),
        })
    }

    pub(crate) async fn disconnect(&self) {
        let _operation = self.operation.lock().await;
        self.disconnect_session().await;
    }

    async fn disconnect_session(&self) {
        self.video_bridge.clear();
        self.connected.store(false, Ordering::Release);
        self.input_level.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        let Some(session) = self.session.lock().await.take() else {
            return;
        };
        if let Some(surface) = &self.surface {
            let targets = session
                .remote_video
                .lock()
                .expect("remote video lock poisoned")
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            for target in targets {
                let _ = surface.close(target);
            }
        }
        let _ = session.microphone_capture.set_state(gst::State::Null);
        session.microphone_task.abort();
        let _ = session.microphone_task.await;
        self.capture_queue_samples.store(0, Ordering::Release);
        if let Some(screen_share) = session.screen_share {
            stop_screen_share_session(&session.room, screen_share).await;
        }
        if let Some(camera) = session.camera {
            stop_camera_session(&session.room, camera).await;
        }
        if let Err(error) = session.room.close().await {
            warn!(%error, "LiveKit room did not close cleanly");
        }
        session.event_task.abort();
        let _ = session.event_task.await;
        info!("LiveKit media disconnected");
    }

    pub(crate) async fn set_muted(&self, muted: bool) {
        if let Some(session) = self.session.lock().await.as_ref() {
            if muted {
                session.microphone.mute();
                self.input_level.store(0, Ordering::Release);
                let _ = self.event_tx.send(MediaEvent::InputLevel {
                    generation: self.generation(),
                    level: 0,
                });
            } else {
                session.microphone.unmute();
            }
        }
    }

    pub(crate) async fn set_deafened(&self, deafened: bool) {
        self.deafened.store(deafened, Ordering::Release);
        let remote_audio = self
            .session
            .lock()
            .await
            .as_ref()
            .map(|session| session.remote_audio.clone());
        if let Some(remote_audio) = remote_audio {
            for (_, track) in remote_audio
                .lock()
                .expect("remote audio lock poisoned")
                .values()
            {
                if deafened {
                    track.disable();
                } else {
                    track.enable();
                }
            }
        }
    }

    pub(crate) async fn set_participant_volumes(
        &self,
        volumes: HashMap<String, f64>,
    ) -> anyhow::Result<()> {
        if volumes
            .values()
            .any(|v| !v.is_finite() || !(0.0..=2.0).contains(v))
        {
            bail!("participant volumes must be between 0 and 200 percent");
        }
        self.participant_volumes
            .lock()
            .expect("volume lock poisoned")
            .clone_from(&volumes);
        let session = self.session.lock().await;
        if let Some(session) = session.as_ref() {
            for (name, track) in session
                .remote_audio
                .lock()
                .expect("remote audio lock poisoned")
                .values()
            {
                track
                    .rtc_track()
                    .set_playout_volume(*volumes.get(name).unwrap_or(&1.0));
            }
        }
        Ok(())
    }

    pub(crate) async fn start_screen_share(&self) -> anyhow::Result<ScreenShareInfo> {
        let _operation = self.operation.lock().await;
        let (room, generation) = {
            let session = self.session.lock().await;
            let session = session.as_ref().context("join a room before sharing")?;
            if session.screen_share.is_some() {
                bail!("screen sharing is already active");
            }
            (session.room.clone(), self.generation())
        };

        let (quality, codec) = {
            let preferences = self
                .video_preferences
                .lock()
                .expect("video preferences lock poisoned");
            (preferences.quality, preferences.codec)
        };
        let (screen_share, state) =
            create_screen_share(&room, generation, quality, codec, self.event_tx.clone()).await?;
        let mut session = self.session.lock().await;
        let Some(session) = session.as_mut() else {
            stop_screen_share_session(&room, screen_share).await;
            bail!("the room ended while screen sharing started");
        };
        session.screen_share = Some(screen_share);
        Ok(ScreenShareInfo { state })
    }

    pub(crate) async fn stop_screen_share(&self) {
        let _operation = self.operation.lock().await;
        let (room, screen_share) = {
            let mut session = self.session.lock().await;
            let Some(session) = session.as_mut() else {
                return;
            };
            (session.room.clone(), session.screen_share.take())
        };
        if let Some(screen_share) = screen_share {
            stop_screen_share_session(&room, screen_share).await;
        }
    }

    pub(crate) async fn start_camera(
        &self,
        expected_hangout_id: Option<&str>,
        expected_camera_id: Option<&str>,
    ) -> anyhow::Result<CameraInfo> {
        let _operation = self.operation.lock().await;
        let (room, generation, hangout_id) = {
            let session = self.session.lock().await;
            let session = session
                .as_ref()
                .context("join a room before starting the camera")?;
            if session.camera.is_some() {
                bail!("the camera is already active");
            }
            (
                session.room.clone(),
                self.generation(),
                session.hangout_id.to_string(),
            )
        };
        let (selected_camera_id, quality, codec) = {
            let preferences = self
                .video_preferences
                .lock()
                .expect("video preferences lock poisoned");
            (
                preferences.selected_camera_id.clone(),
                preferences.quality,
                preferences.codec,
            )
        };
        validate_camera_start_target(
            expected_hangout_id,
            &hangout_id,
            expected_camera_id,
            selected_camera_id.as_deref(),
        )?;
        let (camera, state) = create_camera(
            &room,
            generation,
            selected_camera_id.as_deref(),
            quality,
            codec,
            self.event_tx.clone(),
        )
        .await?;
        let mut session = self.session.lock().await;
        let Some(session) = session.as_mut() else {
            stop_camera_session(&room, camera).await;
            bail!("the room ended while the camera started");
        };
        session.camera = Some(camera);
        Ok(CameraInfo { state })
    }

    pub(crate) async fn stop_camera(&self) {
        let _operation = self.operation.lock().await;
        let (room, camera) = {
            let mut session = self.session.lock().await;
            let Some(session) = session.as_mut() else {
                return;
            };
            (session.room.clone(), session.camera.take())
        };
        if let Some(camera) = camera {
            stop_camera_session(&room, camera).await;
        }
    }

    pub(crate) async fn open_surface(&self, target: RemoteVideoTarget) -> anyhow::Result<()> {
        let (publication, desired_video) = self.remote_video_entry(&target).await?;
        desired_video
            .lock()
            .expect("desired video lock poisoned")
            .insert(target.clone());
        // The initial detached window is medium-sized. Its resize event will
        // request High when it is tiled or fullscreen at a larger size.
        publication.set_video_quality(VideoQuality::Medium);
        publication.set_subscribed(true);
        if !self.video_bridge.contains(&target)
            && let Some(surface) = &self.surface
            && let Err(error) = surface.open(target.clone())
        {
            desired_video
                .lock()
                .expect("desired video lock poisoned")
                .remove(&target);
            publication.set_subscribed(false);
            return Err(error);
        }
        // Report viewing intent immediately. Waiting only for TrackSubscribed is
        // racy when LiveKit has already attached the track by the time the local
        // surface is opened; the later callback remains as an idempotent retry.
        if let Some(room) = self.connected_room().await {
            spawn_video_watch_signal(room, target, true);
        }
        Ok(())
    }

    pub(crate) async fn close_surface(&self, target: RemoteVideoTarget) -> anyhow::Result<()> {
        self.video_bridge.close(&target);
        let (publication, desired_video) = self.remote_video_entry(&target).await?;
        desired_video
            .lock()
            .expect("desired video lock poisoned")
            .remove(&target);
        publication.set_subscribed(false);
        let close_result = self
            .surface
            .as_ref()
            .map_or(Ok(()), |surface| surface.close(target.clone()));
        if let Some(room) = self.connected_room().await {
            spawn_video_watch_signal(room, target, false);
        }
        close_result
    }

    async fn connected_room(&self) -> Option<Arc<Room>> {
        self.session
            .lock()
            .await
            .as_ref()
            .map(|session| session.room.clone())
    }

    pub(crate) async fn first_video_target(&self) -> Option<RemoteVideoTarget> {
        self.session.lock().await.as_ref().and_then(|session| {
            session
                .remote_video
                .lock()
                .expect("remote video lock poisoned")
                .keys()
                .next()
                .cloned()
        })
    }

    async fn remote_video_publication(
        &self,
        target: &RemoteVideoTarget,
    ) -> anyhow::Result<RemoteTrackPublication> {
        self.remote_video_entry(target)
            .await
            .map(|(publication, _)| publication)
    }

    async fn remote_video_entry(
        &self,
        target: &RemoteVideoTarget,
    ) -> anyhow::Result<(
        RemoteTrackPublication,
        Arc<Mutex<HashSet<RemoteVideoTarget>>>,
    )> {
        self.session
            .lock()
            .await
            .as_ref()
            .and_then(|session| {
                session
                    .remote_video
                    .lock()
                    .expect("remote video lock poisoned")
                    .get(target)
                    .cloned()
                    .map(|publication| (publication, session.desired_video.clone()))
            })
            .with_context(|| {
                format!(
                    "{} {} is no longer available",
                    target.participant, target.source
                )
            })
    }

    pub(crate) async fn set_surface_visible(&self, target: &RemoteVideoTarget, visible: bool) {
        if let Ok(publication) = self.remote_video_publication(target).await {
            publication.set_enabled(visible);
            if visible {
                publication.set_video_quality(VideoQuality::High);
            }
        }
    }

    pub(crate) async fn set_surface_dimensions(
        &self,
        target: &RemoteVideoTarget,
        width: u32,
        height: u32,
    ) -> &'static str {
        let quality = surface_quality(width, height);
        if let Ok(publication) = self.remote_video_publication(target).await {
            publication.set_enabled(true);
            publication.set_video_quality(quality);
            publication.update_video_dimensions(livekit::prelude::TrackDimension(width, height));
        }
        video_quality_name(quality)
    }

    pub(crate) fn shutdown_surface(&self) {
        if let Some(surface) = &self.surface {
            surface.shutdown();
        }
    }
}

async fn publish_video_watch_signal(
    room: &Room,
    target: &RemoteVideoTarget,
    watching: bool,
) -> anyhow::Result<()> {
    let target_participant = room
        .remote_participants()
        .into_values()
        .find(|participant| participant.name() == target.participant);
    let participant_identity = target_participant
        .as_ref()
        .map(|participant| participant.identity().to_string());
    let destination_identities = target_participant
        .map(|participant| vec![participant.identity()])
        .unwrap_or_default();
    let payload = serde_json::to_vec(&VideoWatchSignal {
        participant: target.participant.clone(),
        participant_identity,
        source: target.source,
        watching,
    })?;
    room.local_participant()
        .publish_data(DataPacket {
            payload,
            topic: Some(VIDEO_WATCH_TOPIC.into()),
            reliable: true,
            destination_identities,
        })
        .await
        .context("publish the video viewer update")
}

fn spawn_video_watch_signal(room: Arc<Room>, target: RemoteVideoTarget, watching: bool) {
    tokio::spawn(async move {
        match publish_video_watch_signal(&room, &target, watching).await {
            Ok(()) => info!(
                participant = %target.participant,
                source = %target.source,
                watching,
                "sent video viewer update"
            ),
            Err(error) => warn!(
                %error,
                participant = %target.participant,
                source = %target.source,
                watching,
                "could not notify the video publisher about viewing state"
            ),
        }
    });
}

struct RoomEventContext {
    video_bridge: crate::video_bridge::VideoBridge,
    generation: u64,
    event_tx: mpsc::UnboundedSender<MediaEvent>,
    room: Arc<Room>,
    local_name: String,
    local_identity: String,
    remote_audio: Arc<Mutex<HashMap<String, (String, RemoteAudioTrack)>>>,
    participant_volumes: Arc<Mutex<HashMap<String, f64>>>,
    remote_video: Arc<Mutex<HashMap<RemoteVideoTarget, RemoteTrackPublication>>>,
    desired_video: Arc<Mutex<HashSet<RemoteVideoTarget>>>,
    received_frames: Arc<AtomicU64>,
    received_video_frames: Arc<AtomicU64>,
    connected: Arc<AtomicBool>,
    deafened: Arc<AtomicBool>,
    surface: Option<SurfaceController>,
}

impl RoomEventContext {
    fn register_publication(&self, participant: String, publication: &RemoteTrackPublication) {
        match publication.kind() {
            TrackKind::Audio => {
                publication.set_subscribed(true);
                let _ = self.event_tx.send(MediaEvent::RemoteMuteChanged {
                    generation: self.generation,
                    participant,
                    muted: publication.is_muted(),
                });
            }
            TrackKind::Video => {
                let Some(source) = protocol_video_source(publication.source()) else {
                    return;
                };
                let target = RemoteVideoTarget {
                    participant,
                    source,
                };
                if !self
                    .desired_video
                    .lock()
                    .expect("desired video lock poisoned")
                    .contains(&target)
                {
                    publication.set_subscribed(false);
                }
                let is_new = self
                    .remote_video
                    .lock()
                    .expect("remote video lock poisoned")
                    .insert(target.clone(), publication.clone())
                    .is_none();
                if is_new {
                    let _ = self.event_tx.send(MediaEvent::VideoAvailable {
                        generation: self.generation,
                        target,
                        mime_type: publication.mime_type(),
                        simulcasted: publication.simulcasted(),
                    });
                }
            }
        }
    }

    fn active_speakers_changed(&self, speakers: &[Participant], active_speakers: &mut Vec<String>) {
        let mut next_speakers = speakers
            .iter()
            .map(Participant::name)
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>();
        next_speakers.sort();
        next_speakers.dedup();
        if *active_speakers != next_speakers {
            active_speakers.clone_from(&next_speakers);
            let _ = self.event_tx.send(MediaEvent::ActiveSpeakers {
                generation: self.generation,
                speakers: next_speakers,
            });
        }
    }

    fn subscribe_audio(
        &self,
        participant: String,
        track: RemoteAudioTrack,
        track_tasks: &mut JoinSet<()>,
    ) {
        let sid = track.sid().to_string();
        track.rtc_track().set_playout_volume(
            *self
                .participant_volumes
                .lock()
                .expect("volume lock poisoned")
                .get(&participant)
                .unwrap_or(&1.0),
        );
        if self.deafened.load(Ordering::Acquire) {
            track.disable();
        } else {
            track.enable();
        }
        self.remote_audio
            .lock()
            .expect("remote audio lock poisoned")
            .insert(sid, (participant.clone(), track.clone()));
        let _ = self.event_tx.send(MediaEvent::AudioSubscribed {
            generation: self.generation,
            participant: participant.clone(),
        });
        track_tasks.spawn(count_audio_frames(
            self.generation,
            participant,
            track,
            self.received_frames.clone(),
            self.event_tx.clone(),
        ));
    }

    fn subscribe_video(
        &self,
        participant: String,
        track: RemoteVideoTrack,
        track_tasks: &mut JoinSet<()>,
    ) -> Option<RemoteVideoTarget> {
        let source = protocol_video_source(track.source())?;
        let target = RemoteVideoTarget {
            participant,
            source,
        };
        let _ = self.event_tx.send(MediaEvent::VideoSubscribed {
            generation: self.generation,
            target: target.clone(),
        });
        track_tasks.spawn(receive_video_frames(
            self.generation,
            target.clone(),
            track,
            self.received_video_frames.clone(),
            self.surface.clone(),
            self.video_bridge.clone(),
            self.event_tx.clone(),
        ));
        Some(target)
    }

    fn video_is_desired(&self, participant: &str, track: &RemoteVideoTrack) -> bool {
        protocol_video_source(track.source()).is_some_and(|source| {
            self.desired_video
                .lock()
                .expect("desired video lock poisoned")
                .contains(&RemoteVideoTarget {
                    participant: participant.into(),
                    source,
                })
        })
    }
}

#[allow(clippy::too_many_lines)]
async fn run_room_events(
    mut events: tokio::sync::mpsc::UnboundedReceiver<RoomEvent>,
    context: RoomEventContext,
) {
    let mut track_tasks = JoinSet::new();
    let mut active_speakers = Vec::new();
    let mut reconnecting = false;
    while let Some(event) = events.recv().await {
        match event {
            RoomEvent::TrackSubscribed {
                track: RemoteTrack::Audio(track),
                participant,
                ..
            } => {
                context.subscribe_audio(participant.name(), track, &mut track_tasks);
            }
            RoomEvent::TrackSubscribed {
                track: RemoteTrack::Video(track),
                participant,
                ..
            } => {
                let participant = participant.name();
                if context.video_is_desired(&participant, &track) {
                    if let Some(target) =
                        context.subscribe_video(participant, track, &mut track_tasks)
                    {
                        spawn_video_watch_signal(context.room.clone(), target, true);
                    }
                } else if let Some(source) = protocol_video_source(track.source()) {
                    let target = RemoteVideoTarget {
                        participant,
                        source,
                    };
                    if let Some(publication) = context
                        .remote_video
                        .lock()
                        .expect("remote video lock poisoned")
                        .get(&target)
                    {
                        publication.set_subscribed(false);
                    }
                }
            }
            RoomEvent::TrackPublished {
                publication,
                participant,
            } => context.register_publication(participant.name(), &publication),
            RoomEvent::TrackUnpublished {
                publication,
                participant,
            } => {
                if let Some(source) = protocol_video_source(publication.source()) {
                    let target = RemoteVideoTarget {
                        participant: participant.name(),
                        source,
                    };
                    if !reconnecting {
                        context
                            .remote_video
                            .lock()
                            .expect("remote video lock poisoned")
                            .remove(&target);
                        context
                            .desired_video
                            .lock()
                            .expect("desired video lock poisoned")
                            .remove(&target);
                        if let Some(surface) = &context.surface {
                            let _ = surface.close(target.clone());
                        }
                        let _ = context.event_tx.send(MediaEvent::VideoUnavailable {
                            generation: context.generation,
                            target,
                        });
                    }
                }
            }
            RoomEvent::TrackUnsubscribed {
                track: RemoteTrack::Audio(track),
                participant,
                ..
            } => {
                context
                    .remote_audio
                    .lock()
                    .expect("remote audio lock poisoned")
                    .remove(&track.sid().to_string());
                let _ = context.event_tx.send(MediaEvent::AudioUnsubscribed {
                    generation: context.generation,
                    participant: participant.name(),
                });
            }
            RoomEvent::TrackUnsubscribed {
                track: RemoteTrack::Video(track),
                participant,
                ..
            } => {
                if let Some(source) = protocol_video_source(track.source()) {
                    let target = RemoteVideoTarget {
                        participant: participant.name(),
                        source,
                    };
                    let _ = context.event_tx.send(MediaEvent::VideoUnsubscribed {
                        generation: context.generation,
                        target: target.clone(),
                    });
                    spawn_video_watch_signal(context.room.clone(), target, false);
                }
            }
            RoomEvent::TrackMuted {
                participant,
                publication,
            }
            | RoomEvent::TrackUnmuted {
                participant,
                publication,
            } if publication.is_remote() && publication.kind() == TrackKind::Audio => {
                let _ = context.event_tx.send(MediaEvent::RemoteMuteChanged {
                    generation: context.generation,
                    participant: participant.name(),
                    muted: publication.is_muted(),
                });
            }
            RoomEvent::ActiveSpeakersChanged { speakers } => {
                context.active_speakers_changed(&speakers, &mut active_speakers);
            }
            RoomEvent::DataReceived {
                payload,
                topic: Some(topic),
                participant: Some(participant),
                ..
            } if topic == VIDEO_WATCH_TOPIC => {
                match serde_json::from_slice::<VideoWatchSignal>(&payload) {
                    Ok(signal)
                        if video_watch_targets_local(
                            &signal,
                            &context.local_name,
                            &context.local_identity,
                        ) =>
                    {
                        let viewer = participant.name();
                        let participant = if viewer.is_empty() {
                            participant.identity().to_string()
                        } else {
                            viewer
                        };
                        let _ = context.event_tx.send(MediaEvent::VideoViewerChanged {
                            generation: context.generation,
                            source: signal.source,
                            participant,
                            watching: signal.watching,
                        });
                    }
                    Ok(_) => {}
                    Err(error) => debug!(%error, "ignored an invalid video viewer update"),
                }
            }
            RoomEvent::ParticipantDisconnected(participant) => {
                let viewer = participant.name();
                let participant = if viewer.is_empty() {
                    participant.identity().to_string()
                } else {
                    viewer
                };
                let _ = context.event_tx.send(MediaEvent::VideoViewerDisconnected {
                    generation: context.generation,
                    participant,
                });
            }
            RoomEvent::Reconnecting => {
                reconnecting = true;
                let _ = context.event_tx.send(MediaEvent::Reconnecting {
                    generation: context.generation,
                });
            }
            RoomEvent::Reconnected => {
                reconnecting = false;
                context.connected.store(true, Ordering::Release);
                let _ = context.event_tx.send(MediaEvent::Reconnected {
                    generation: context.generation,
                });
            }
            RoomEvent::Disconnected { reason } => {
                context.connected.store(false, Ordering::Release);
                let _ = context.event_tx.send(MediaEvent::Disconnected {
                    generation: context.generation,
                    reason: format!("{reason:?}"),
                });
                break;
            }
            other => debug!(?other, "LiveKit room event"),
        }
    }
    if let Some(surface) = &context.surface {
        for target in context
            .remote_video
            .lock()
            .expect("remote video lock poisoned")
            .keys()
            .cloned()
            .collect::<Vec<_>>()
        {
            let _ = surface.close(target);
        }
    }
    track_tasks.abort_all();
    while track_tasks.join_next().await.is_some() {}
}

fn video_watch_targets_local(
    signal: &VideoWatchSignal,
    local_name: &str,
    local_identity: &str,
) -> bool {
    signal.participant_identity.as_deref().map_or_else(
        || signal.participant == local_name,
        |identity| identity == local_identity,
    )
}

fn recording_device_id(device: &RecordingDeviceInfo) -> String {
    public_device_id(device.id.as_str(), device.index, &device.name)
}

fn protocol_video_source(source: TrackSource) -> Option<VideoSource> {
    match source {
        TrackSource::Screenshare => Some(VideoSource::ScreenShare),
        TrackSource::Camera => Some(VideoSource::Camera),
        _ => None,
    }
}

fn livekit_video_source(source: VideoSource) -> TrackSource {
    match source {
        VideoSource::ScreenShare => TrackSource::Screenshare,
        VideoSource::Camera => TrackSource::Camera,
    }
}

fn livekit_video_codec(codec: VideoCodecPreference) -> VideoCodec {
    match codec {
        VideoCodecPreference::H264 => VideoCodec::H264,
        VideoCodecPreference::Vp8 => VideoCodec::VP8,
        VideoCodecPreference::Av1 => VideoCodec::AV1,
    }
}

fn encoder_backend_name(backend: VideoEncoderBackend) -> &'static str {
    match backend {
        VideoEncoderBackend::Auto => "auto",
        VideoEncoderBackend::Software => "software",
        VideoEncoderBackend::Hardware => "hardware",
        VideoEncoderBackend::Nvenc => "nvenc",
        VideoEncoderBackend::Vaapi => "vaapi",
        VideoEncoderBackend::VideoToolbox => "videotoolbox",
        VideoEncoderBackend::PreEncoded => "pre_encoded",
        _ => "unknown",
    }
}

fn available_video_encoder_backends() -> Vec<VideoEncoderBackend> {
    VideoEncoderBackend::list_available().into_iter().collect()
}

fn selected_video_encoder(backends: &[VideoEncoderBackend]) -> VideoEncoderBackend {
    [
        VideoEncoderBackend::Nvenc,
        VideoEncoderBackend::Vaapi,
        VideoEncoderBackend::Hardware,
        VideoEncoderBackend::VideoToolbox,
    ]
    .into_iter()
    .find(|candidate| backends.contains(candidate))
    .unwrap_or(VideoEncoderBackend::Software)
}

fn publishing_video_encoder(
    backends: &[VideoEncoderBackend],
    _source: VideoSource,
) -> VideoEncoderBackend {
    let selected = selected_video_encoder(backends);
    if selected == VideoEncoderBackend::Vaapi {
        // Intel's iHD VA-API driver has crashed in both the webcam and ultrawide
        // screen-share paths. A driver SIGBUS takes down the entire daemon, so
        // prefer the stable software encoder whenever VA-API is selected.
        VideoEncoderBackend::Software
    } else {
        selected
    }
}

fn detected_video_settings(
    quality: VideoQualityPreset,
    codec: VideoCodecPreference,
) -> VideoSettings {
    let backends = available_video_encoder_backends();
    let selected = publishing_video_encoder(&backends, VideoSource::Camera);
    VideoSettings {
        quality,
        codec,
        available_codecs: vec![
            VideoCodecPreference::H264,
            VideoCodecPreference::Vp8,
            VideoCodecPreference::Av1,
        ],
        encoder_backend: encoder_backend_name(selected).into(),
        available_encoder_backends: backends
            .iter()
            .copied()
            .map(encoder_backend_name)
            .map(str::to_owned)
            .collect(),
        hardware_acceleration: selected != VideoEncoderBackend::Software,
    }
}

#[derive(Clone, Copy)]
struct VideoProfile {
    width: u32,
    height: u32,
    fps: u32,
    max_bitrate: u64,
}

struct CameraCapturePath {
    caps: gst::Caps,
    decoder: Option<gst::Element>,
    description: &'static str,
}

fn video_profile(quality: VideoQualityPreset, source: VideoSource) -> VideoProfile {
    match (quality, source) {
        (VideoQualityPreset::Balanced, VideoSource::ScreenShare) => VideoProfile {
            width: 1280,
            height: 720,
            fps: 30,
            max_bitrate: 3_000_000,
        },
        (VideoQualityPreset::High, VideoSource::ScreenShare) => VideoProfile {
            width: 1920,
            height: 1080,
            fps: 60,
            max_bitrate: 8_000_000,
        },
        (VideoQualityPreset::Ultra, VideoSource::ScreenShare) => VideoProfile {
            width: 2560,
            height: 1440,
            fps: 60,
            max_bitrate: 14_000_000,
        },
        (VideoQualityPreset::Balanced, VideoSource::Camera) => VideoProfile {
            width: 960,
            height: 540,
            fps: 30,
            max_bitrate: 1_500_000,
        },
        (VideoQualityPreset::High, VideoSource::Camera) => VideoProfile {
            width: 1280,
            height: 720,
            fps: 30,
            max_bitrate: 2_500_000,
        },
        (VideoQualityPreset::Ultra, VideoSource::Camera) => VideoProfile {
            width: 1920,
            height: 1080,
            fps: 60,
            max_bitrate: 6_000_000,
        },
    }
}

fn camera_capture_path(device: &gst::Device, profile: VideoProfile) -> CameraCapturePath {
    let jpeg_caps = gst::Caps::builder("image/jpeg")
        .field("width", i32::try_from(profile.width).unwrap_or(i32::MAX))
        .field("height", i32::try_from(profile.height).unwrap_or(i32::MAX))
        .field(
            "framerate",
            gst::Fraction::new(i32::try_from(profile.fps).unwrap_or(i32::MAX), 1),
        )
        .build();
    let decoder = ["jpegdec", "avdec_mjpeg"].into_iter().find_map(|factory| {
        gst::ElementFactory::make(factory)
            .name("wisp-camera-decoder")
            .build()
            .ok()
    });
    if decoder.is_some()
        && device
            .caps()
            .is_some_and(|caps| caps.can_intersect(&jpeg_caps))
    {
        return CameraCapturePath {
            caps: jpeg_caps,
            decoder,
            description: "mjpeg",
        };
    }

    let exact_raw_caps = gst::Caps::builder("video/x-raw")
        .field("width", i32::try_from(profile.width).unwrap_or(i32::MAX))
        .field("height", i32::try_from(profile.height).unwrap_or(i32::MAX))
        .build();
    let caps = if device
        .caps()
        .is_some_and(|caps| caps.can_intersect(&exact_raw_caps))
    {
        exact_raw_caps
    } else {
        gst::Caps::builder("video/x-raw").build()
    };
    CameraCapturePath {
        caps,
        decoder: None,
        description: "raw",
    }
}

fn video_publish_options(
    source: VideoSource,
    quality: VideoQualityPreset,
    codec: VideoCodecPreference,
    encoder: VideoEncoderBackend,
) -> TrackPublishOptions {
    let profile = video_profile(quality, source);
    TrackPublishOptions {
        source: livekit_video_source(source),
        video_codec: livekit_video_codec(codec),
        video_encoding: Some(VideoEncoding {
            max_bitrate: profile.max_bitrate,
            max_framerate: f64::from(profile.fps),
        }),
        video_encoder: encoder,
        simulcast: true,
        ..Default::default()
    }
}

fn surface_quality(width: u32, height: u32) -> VideoQuality {
    match width.saturating_mul(height) {
        0..=230_400 => VideoQuality::Low,
        230_401..=921_600 => VideoQuality::Medium,
        _ => VideoQuality::High,
    }
}

const fn video_quality_name(quality: VideoQuality) -> &'static str {
    match quality {
        VideoQuality::Low => "low",
        VideoQuality::Medium => "medium",
        VideoQuality::High => "high",
    }
}

fn validate_camera_start_target(
    expected_room: Option<&str>,
    room: &str,
    expected_device: Option<&str>,
    device: Option<&str>,
) -> anyhow::Result<()> {
    if expected_room.is_some_and(|expected| expected != room) {
        bail!("the room changed; confirm camera sharing again");
    }
    if expected_device.is_some_and(|expected| Some(expected) != device) {
        bail!("the selected camera changed; confirm camera sharing again");
    }
    Ok(())
}

fn enumerate_camera_devices() -> anyhow::Result<Vec<(VideoDevice, gst::Device)>> {
    gst::init().context("initialize GStreamer for camera discovery")?;
    let monitor = gst::DeviceMonitor::new();
    monitor
        .add_filter(Some("Video/Source"), None)
        .context("configure camera device discovery")?;
    monitor.start().context("start camera device discovery")?;
    let devices = monitor
        .devices()
        .into_iter()
        .enumerate()
        .map(|(index, device)| {
            let name = device.display_name().to_string();
            let id = camera_device_id(&device, index, &name);
            (VideoDevice { id, name }, device)
        })
        .collect();
    monitor.stop();
    Ok(devices)
}

fn camera_device_id(device: &gst::Device, index: usize, name: &str) -> String {
    let property_id = device.properties().and_then(|properties| {
        ["device.path", "api.v4l2.path", "object.path", "node.name"]
            .into_iter()
            .find_map(|key| properties.get::<String>(key).ok())
    });
    property_id.unwrap_or_else(|| format!("camera:{index}:{name}"))
}

fn playout_device_id(device: &PlayoutDeviceInfo) -> String {
    public_device_id(device.id.as_str(), device.index, &device.name)
}

fn protocol_recording_device(device: &RecordingDeviceInfo) -> AudioDevice {
    AudioDevice {
        id: recording_device_id(device),
        name: device.name.clone(),
    }
}

fn protocol_playout_device(device: &PlayoutDeviceInfo) -> AudioDevice {
    AudioDevice {
        id: playout_device_id(device),
        name: device.name.clone(),
    }
}

fn public_device_id(guid: &str, index: usize, name: &str) -> String {
    if guid.is_empty() {
        format!("fallback:{index}:{name}")
    } else {
        guid.to_owned()
    }
}

fn fallback_device_name(id: &str) -> Option<&str> {
    id.strip_prefix("fallback:")
        .and_then(|rest| rest.split_once(':'))
        .map(|(_, name)| name)
}

fn same_logical_device(left: &str, right: &str) -> bool {
    left == right
        || fallback_device_name(left)
            .zip(fallback_device_name(right))
            .is_some_and(|(left, right)| left == right)
}

fn preferred_or_first<'a>(
    devices: &'a [AudioDevice],
    preferred_id: Option<&str>,
) -> Option<&'a AudioDevice> {
    preferred_id
        .and_then(|id| {
            devices
                .iter()
                .find(|device| same_logical_device(&device.id, id))
        })
        .or_else(|| devices.first())
}

fn selected_device_name(devices: &[AudioDevice], selected_id: Option<&str>) -> Option<String> {
    selected_id.and_then(|id| {
        devices
            .iter()
            .find(|device| device.id == id)
            .map(|device| device.name.clone())
    })
}

fn select_playout_device(
    audio: &PlatformAudio,
    device: &PlayoutDeviceInfo,
    active: bool,
) -> anyhow::Result<()> {
    if !device.id.as_str().is_empty() {
        return if active {
            audio.switch_playout_device(&device.id)
        } else {
            audio.set_playout_device(&device.id)
        }
        .map_err(Into::into);
    }

    let runtime = LkRuntime::instance();
    let factory = runtime.pc_factory();
    let index = u16::try_from(device.index).context("speaker index is out of range")?;
    let was_initialized = factory.playout_is_initialized();
    if active && was_initialized && !factory.stop_playout() {
        bail!("stop speaker before switching");
    }
    if !factory.set_playout_device(index) {
        if active && was_initialized {
            let _ = factory.init_playout() && factory.start_playout();
        }
        bail!("select speaker by index {index}");
    }
    if active && (!factory.init_playout() || !factory.start_playout()) {
        bail!("restart speaker after switching");
    }
    Ok(())
}

fn create_microphone_capture_pipeline(
    microphone: &str,
    frames: mpsc::UnboundedSender<Vec<i16>>,
    queued_samples: Arc<AtomicU64>,
) -> anyhow::Result<gst::Pipeline> {
    gst::init().context("initialize GStreamer for microphone capture")?;
    let source = if std::env::var_os("WISP_TEST_MICROPHONE_TONE").is_some() {
        gst::ElementFactory::make("audiotestsrc")
            .property("is-live", true)
            .property("volume", 0.12_f64)
            .build()
            .context("the GStreamer audio test source is not installed")?
    } else {
        microphone_capture_source(microphone)?
    };
    let caps = gst::Caps::builder("audio/x-raw")
        .field("format", "S16LE")
        .field("layout", "interleaved")
        .field(
            "rate",
            i32::try_from(AUDIO_SAMPLE_RATE).expect("audio rate fits i32"),
        )
        .field("channels", 1_i32)
        .build();
    let app_sink = gst_app::AppSink::builder()
        .caps(&caps)
        .max_buffers(8)
        .drop(true)
        .sync(false)
        .enable_last_sample(false)
        .callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let samples = capture_microphone_sample(sink).map_err(|error| {
                        warn!(%error, "microphone capture failed");
                        gst::FlowError::Error
                    })?;
                    queued_samples.fetch_add(samples.len() as u64, Ordering::AcqRel);
                    if let Err(error) = frames.send(samples) {
                        atomic_saturating_sub(&queued_samples, error.0.len() as u64);
                        return Err(gst::FlowError::Eos);
                    }
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        )
        .build();
    let convert = gst::ElementFactory::make("audioconvert")
        .build()
        .context("the GStreamer audio converter is not installed")?;
    let resample = gst::ElementFactory::make("audioresample")
        .build()
        .context("the GStreamer audio resampler is not installed")?;
    let pipeline = gst::Pipeline::default();
    pipeline
        .add_many([&source, &convert, &resample, app_sink.upcast_ref()])
        .context("build the microphone capture pipeline")?;
    gst::Element::link_many([&source, &convert, &resample, app_sink.upcast_ref()])
        .context("link the microphone capture pipeline")?;
    Ok(pipeline)
}

fn atomic_saturating_sub(counter: &AtomicU64, value: u64) {
    let _ = counter.fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
        Some(current.saturating_sub(value))
    });
}

fn microphone_capture_source(microphone: &str) -> anyhow::Result<gst::Element> {
    let device_name = microphone.strip_prefix("default: ").unwrap_or(microphone);
    let monitor = gst::DeviceMonitor::new();
    monitor
        .add_filter(Some("Audio/Source"), None)
        .context("add the microphone device filter")?;
    monitor.start().context("start microphone discovery")?;
    let device = monitor
        .devices()
        .iter()
        .filter(|device| device.display_name() == device_name)
        .max_by_key(|device| {
            device
                .properties()
                .is_some_and(|properties| properties.has_field("node.name"))
        })
        .cloned();
    monitor.stop();
    let device = device.with_context(|| format!("GStreamer cannot capture {microphone}"))?;
    device
        .create_element(Some("wisp-microphone-source"))
        .with_context(|| format!("create the {microphone} capture source"))
}

fn capture_microphone_sample(sink: &gst_app::AppSink) -> anyhow::Result<Vec<i16>> {
    let sample = sink.pull_sample().context("read a microphone frame")?;
    let buffer = sample
        .buffer()
        .context("captured microphone sample has no buffer")?;
    let map = buffer
        .map_readable()
        .context("map the captured microphone buffer")?;
    let bytes = map.as_slice();
    if !bytes.len().is_multiple_of(2) {
        bail!("captured microphone buffer has an incomplete sample");
    }
    Ok(bytes
        .chunks_exact(2)
        .map(|sample| i16::from_le_bytes([sample[0], sample[1]]))
        .collect())
}

#[allow(clippy::too_many_arguments)]
async fn run_microphone_pipeline(
    mut captured_frames: mpsc::UnboundedReceiver<Vec<i16>>,
    publish_source: NativeAudioSource,
    publish_track: LocalAudioTrack,
    neural_enabled: Arc<AtomicBool>,
    denoiser: Arc<DenoiserService>,
    input_level: Arc<AtomicU8>,
    capture_queue_samples: Arc<AtomicU64>,
    event_tx: mpsc::UnboundedSender<MediaEvent>,
    generation: u64,
) {
    const CHANNELS: u32 = 1;

    let mut pending = std::collections::VecDeque::<i16>::with_capacity(AUDIO_FRAME_SAMPLES * 2);
    let mut meter_frames = 0_u8;
    let mut meter_peak = 0_u8;
    let mut processing_peak_us = 0_u32;
    let mut processing_deadline_misses = 0_u64;

    while let Some(samples) = captured_frames.recv().await {
        atomic_saturating_sub(&capture_queue_samples, samples.len() as u64);
        pending.extend(samples);
        while pending.len() >= AUDIO_FRAME_SAMPLES {
            let processing_started = Instant::now();
            let input = pending.drain(..AUDIO_FRAME_SAMPLES).collect::<Vec<_>>();
            let neural = neural_enabled.load(Ordering::Acquire);
            let processed_samples = if neural {
                match denoiser.process(input.clone()).await {
                    Ok(processed) => processed,
                    Err(error) => {
                        warn!(%error, "neural denoiser worker stopped; publishing raw audio");
                        input.clone()
                    }
                }
            } else {
                input.clone()
            };
            // Clear publishes DeepFilterNet directly. No secondary detector,
            // gate, or gain stage is allowed to reshape the processed signal.
            let output = processed_samples;
            meter_peak = meter_peak.max(pcm_level_percent(&output));
            meter_frames += 1;
            let frame = AudioFrame {
                data: Cow::Owned(output),
                sample_rate: AUDIO_SAMPLE_RATE,
                num_channels: CHANNELS,
                samples_per_channel: u32::try_from(AUDIO_FRAME_SAMPLES)
                    .expect("neural denoiser frame length fits u32"),
            };
            if let Err(error) = publish_source.capture_frame(&frame).await {
                warn!(%error, "neural microphone pipeline stopped");
                return;
            }
            let elapsed_us =
                u32::try_from(processing_started.elapsed().as_micros()).unwrap_or(u32::MAX);
            processing_peak_us = processing_peak_us.max(elapsed_us);
            if elapsed_us > AUDIO_FRAME_BUDGET_US {
                processing_deadline_misses = processing_deadline_misses.saturating_add(1);
            }
            if meter_frames == 10 {
                let muted = publish_track.is_muted();
                let level = if muted { 0 } else { meter_peak };
                input_level.store(level, Ordering::Release);
                let queued_samples = capture_queue_samples.load(Ordering::Acquire);
                let capture_queue_ms = u16::try_from(queued_samples / 48).unwrap_or(u16::MAX);
                let _ = event_tx.send(MediaEvent::AudioTelemetry {
                    generation,
                    level,
                    processing_time_us: processing_peak_us,
                    processing_deadline_misses,
                    capture_queue_ms,
                });
                meter_frames = 0;
                meter_peak = 0;
                processing_peak_us = 0;
            }
        }
    }
}

fn preferred_neural_denoiser() -> NeuralDenoiser {
    match create_deepfilter_model() {
        Ok(model) => {
            info!(
                backend = DenoiserBackend::DeepFilterNet.name(),
                latency_ms = DEEPFILTER_LATENCY_MS,
                "neural denoiser ready"
            );
            NeuralDenoiser::deepfilternet(model)
        }
        Err(error) => {
            warn!(%error, "DeepFilterNet initialization failed; falling back to RNNoise");
            NeuralDenoiser::rnnoise()
        }
    }
}

fn create_deepfilter_model() -> anyhow::Result<DfTract> {
    let model = DfTract::new(DfParams::default(), &RuntimeParams::default())
        .context("initialize embedded DeepFilterNet model")?;
    if model.sr != AUDIO_SAMPLE_RATE as usize || model.hop_size != AUDIO_FRAME_SAMPLES {
        bail!(
            "unsupported DeepFilterNet format: {} Hz, {} samples",
            model.sr,
            model.hop_size
        );
    }
    let delay_samples = model.fft_size - model.hop_size + model.lookahead * model.hop_size;
    let delay_ms = delay_samples * 1_000 / model.sr;
    if delay_ms != usize::from(DEEPFILTER_LATENCY_MS) {
        bail!("unexpected DeepFilterNet latency: {delay_ms} ms");
    }
    Ok(model)
}

#[allow(clippy::cast_possible_truncation)]
fn deepfilter_frame(model: &mut DfTract, input: &[i16]) -> anyhow::Result<Vec<i16>> {
    if input.len() != AUDIO_FRAME_SAMPLES {
        bail!(
            "DeepFilterNet needs {AUDIO_FRAME_SAMPLES} samples, received {}",
            input.len()
        );
    }
    let input = input
        .iter()
        .map(|sample| f32::from(*sample) / 32_768.0)
        .collect::<Vec<_>>();
    let mut output = vec![0.0_f32; AUDIO_FRAME_SAMPLES];
    model
        .process(
            ArrayView2::from_shape((1, AUDIO_FRAME_SAMPLES), &input)
                .expect("DeepFilterNet input shape is fixed"),
            ArrayViewMut2::from_shape((1, AUDIO_FRAME_SAMPLES), &mut output)
                .expect("DeepFilterNet output shape is fixed"),
        )
        .context("process DeepFilterNet audio frame")?;
    Ok(output
        .into_iter()
        .map(|sample| {
            (sample * 32_768.0)
                .round()
                .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
        })
        .collect())
}

#[allow(clippy::cast_possible_truncation)]
fn rnnoise_frame(
    denoiser: &mut DenoiseState<'static>,
    input: &[i16],
    first_frame: &mut bool,
) -> Vec<i16> {
    let input = input
        .iter()
        .map(|sample| f32::from(*sample))
        .collect::<Vec<_>>();
    let mut output = [0.0_f32; DenoiseState::FRAME_SIZE];
    denoiser.process_frame(&mut output, &input);
    if std::mem::take(first_frame) {
        return vec![0; DenoiseState::FRAME_SIZE];
    }
    output
        .into_iter()
        .map(|sample| {
            sample
                .round()
                .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
        })
        .collect()
}

fn apply_denoiser_state(state: &mut AudioState, backend: DenoiserBackend) {
    state.denoiser_active = state.preset == AudioPreset::Clear;
    state.denoiser = state.denoiser_active.then(|| backend.name().to_owned());
    state.processing_latency_ms = if state.denoiser_active {
        backend.latency_ms()
    } else {
        0
    };
}

#[allow(clippy::too_many_lines)]
async fn create_camera(
    room: &Room,
    generation: u64,
    selected_camera_id: Option<&str>,
    quality: VideoQualityPreset,
    codec: VideoCodecPreference,
    event_tx: mpsc::UnboundedSender<MediaEvent>,
) -> anyhow::Result<(CameraSession, CameraState)> {
    gst::init().context("initialize GStreamer for camera capture")?;
    let devices = enumerate_camera_devices()?;
    let (protocol_device, device) = selected_camera_id
        .and_then(|id| devices.iter().find(|(candidate, _)| candidate.id == id))
        // Never substitute a different camera after the user previewed one.
        .or_else(|| {
            selected_camera_id
                .is_none()
                .then(|| devices.first())
                .flatten()
        })
        .cloned()
        .context("the selected camera is unavailable; select a camera and confirm again")?;
    let source = device
        .create_element(Some("wisp-camera-source"))
        .with_context(|| format!("open camera {}", protocol_device.name))?;
    let profile = video_profile(quality, VideoSource::Camera);
    let capture_path = camera_capture_path(&device, profile);
    let capture_format = capture_path.description;
    let capture_filter = gst::ElementFactory::make("capsfilter")
        .name("wisp-camera-input-format")
        .property("caps", &capture_path.caps)
        .build()
        .context("create the camera input format filter")?;
    let encoder =
        publishing_video_encoder(&available_video_encoder_backends(), VideoSource::Camera);
    let video_source = NativeVideoSource::new(
        VideoResolution {
            width: profile.width,
            height: profile.height,
        },
        true,
    );
    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "I420")
        .field("width", i32::try_from(profile.width)?)
        .field("height", i32::try_from(profile.height)?)
        .field(
            "framerate",
            gst::Fraction::new(i32::try_from(profile.fps)?, 1),
        )
        .build();
    let published_frames = Arc::new(AtomicU64::new(0));
    let callback_failed = Arc::new(AtomicBool::new(false));
    let callback_source = video_source.clone();
    let callback_frames = published_frames.clone();
    let callback_events = event_tx.clone();
    let callback_failure = callback_failed.clone();
    let preview_every = u64::from((profile.fps / 2).max(1));
    let app_sink = gst_app::AppSink::builder()
        .caps(&caps)
        .max_buffers(2)
        .drop(true)
        .sync(false)
        .enable_last_sample(false)
        .callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let next_frame = callback_frames.load(Ordering::Acquire) + 1;
                    let preview = (next_frame == 1 || next_frame.is_multiple_of(preview_every))
                        .then_some((VideoSource::Camera, next_frame));
                    let result = capture_screen_sample(sink, &callback_source, preview);
                    match result {
                        Ok(()) => {
                            let total = callback_frames.fetch_add(1, Ordering::AcqRel) + 1;
                            if total == 1 || total.is_multiple_of(preview_every) {
                                let _ = callback_events
                                    .send(MediaEvent::CameraFrames { generation, total });
                            }
                            Ok(gst::FlowSuccess::Ok)
                        }
                        Err(error) => {
                            if !callback_failure.swap(true, Ordering::AcqRel) {
                                let _ = callback_events.send(MediaEvent::CameraStopped {
                                    generation,
                                    error: Some(error.to_string()),
                                });
                            }
                            Err(gst::FlowError::Error)
                        }
                    }
                })
                .build(),
        )
        .build();
    let convert = gst::ElementFactory::make("videoconvert")
        .build()
        .context("the GStreamer video converter is not installed")?;
    let scale = gst::ElementFactory::make("videoscale")
        .property("add-borders", true)
        .build()
        .context("the GStreamer video scaler is not installed")?;
    let rate = gst::ElementFactory::make("videorate")
        .build()
        .context("the GStreamer frame-rate converter is not installed")?;
    let pipeline = gst::Pipeline::default();
    if let Some(decoder) = &capture_path.decoder {
        pipeline
            .add_many([
                &source,
                &capture_filter,
                decoder,
                &convert,
                &scale,
                &rate,
                app_sink.upcast_ref(),
            ])
            .context("build the decoded camera capture pipeline")?;
        gst::Element::link_many([
            &source,
            &capture_filter,
            decoder,
            &convert,
            &scale,
            &rate,
            app_sink.upcast_ref(),
        ])
        .context("link the decoded camera capture pipeline")?;
    } else {
        pipeline
            .add_many([
                &source,
                &capture_filter,
                &convert,
                &scale,
                &rate,
                app_sink.upcast_ref(),
            ])
            .context("build the raw camera capture pipeline")?;
        gst::Element::link_many([
            &source,
            &capture_filter,
            &convert,
            &scale,
            &rate,
            app_sink.upcast_ref(),
        ])
        .context("link the raw camera capture pipeline")?;
    }

    let video_track =
        LocalVideoTrack::create_video_track("camera", RtcVideoSource::Native(video_source));
    let publication = room
        .local_participant()
        .publish_track(
            LocalTrack::Video(video_track),
            video_publish_options(VideoSource::Camera, quality, codec, encoder),
        )
        .await
        .context("publish camera to LiveKit")?;
    if let Err(error) = pipeline.set_state(gst::State::Playing) {
        let _ = room
            .local_participant()
            .unpublish_track(&publication.sid())
            .await;
        return Err(error).context("start camera capture");
    }
    let monitor_running = Arc::new(AtomicBool::new(true));
    let monitor_task = monitor_camera(
        pipeline
            .bus()
            .context("camera pipeline has no message bus")?,
        generation,
        monitor_running.clone(),
        event_tx,
    );
    info!(
        camera = %protocol_device.name,
        width = profile.width,
        height = profile.height,
        fps = profile.fps,
        capture_format,
        encoder = encoder_backend_name(encoder),
        codec = %codec,
        quality = %quality,
        "camera published"
    );
    let state = CameraState {
        devices: devices.into_iter().map(|(device, _)| device).collect(),
        selected_device_id: Some(protocol_device.id),
        active: true,
        width: Some(profile.width),
        height: Some(profile.height),
        fps: Some(profile.fps),
        encoder_backend: Some(encoder_backend_name(encoder).into()),
        ..CameraState::default()
    };
    Ok((
        CameraSession {
            publication_sid: publication.sid(),
            pipeline,
            monitor_running,
            monitor_task,
        },
        state,
    ))
}

fn monitor_camera(
    bus: gst::Bus,
    generation: u64,
    running: Arc<AtomicBool>,
    event_tx: mpsc::UnboundedSender<MediaEvent>,
) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        while running.load(Ordering::Acquire) {
            let Some(message) = bus.timed_pop(gst::ClockTime::from_mseconds(250)) else {
                continue;
            };
            let error = match message.view() {
                gst::MessageView::Eos(..) => None,
                gst::MessageView::Error(error) => Some(format!(
                    "camera capture failed: {} ({})",
                    error.error(),
                    error.debug().unwrap_or_default()
                )),
                _ => continue,
            };
            let _ = event_tx.send(MediaEvent::CameraStopped { generation, error });
            break;
        }
    })
}

async fn stop_camera_session(room: &Room, camera: CameraSession) {
    camera.monitor_running.store(false, Ordering::Release);
    if let Err(error) = camera.pipeline.set_state(gst::State::Null) {
        warn!(%error, "camera pipeline did not stop cleanly");
    }
    if let Err(error) = room
        .local_participant()
        .unpublish_track(&camera.publication_sid)
        .await
    {
        warn!(%error, "camera track did not unpublish cleanly");
    }
    let _ = camera.monitor_task.await;
    remove_local_preview(VideoSource::Camera);
}

#[allow(clippy::too_many_lines)]
async fn create_screen_share(
    room: &Room,
    generation: u64,
    quality: VideoQualityPreset,
    codec: VideoCodecPreference,
    event_tx: mpsc::UnboundedSender<MediaEvent>,
) -> anyhow::Result<(ScreenShareSession, ScreenShareState)> {
    gst::init().context("initialize GStreamer for screen sharing")?;
    let portal = Screencast::new()
        .await
        .context("connect to the screen cast portal")?;
    let portal_session = portal
        .create_session(CreateSessionOptions::default())
        .await
        .context("create a screen cast portal session")?;
    portal
        .select_sources(
            &portal_session,
            SelectSourcesOptions::default()
                .set_cursor_mode(CursorMode::Embedded)
                .set_sources(SourceType::Monitor | SourceType::Window)
                .set_multiple(false)
                .set_persist_mode(PersistMode::DoNot),
        )
        .await
        .context("configure the screen cast portal")?;
    let response = portal
        .start(&portal_session, None, StartCastOptions::default())
        .await
        .context("open the screen or window picker")?
        .response()
        .context("screen sharing was not selected")?;
    let stream = response
        .streams()
        .first()
        .cloned()
        .context("the portal returned no screen cast stream")?;
    let pipewire_remote = portal
        .open_pipe_wire_remote(&portal_session, OpenPipeWireRemoteOptions::default())
        .await
        .context("open the portal PipeWire stream")?;

    let profile = video_profile(quality, VideoSource::ScreenShare);
    let source_size = stream.size();
    let (width, height) = screen_share_resolution(source_size, profile.width, profile.height);
    let (source_width, source_height) = source_size
        .filter(|(width, height)| *width > 0 && *height > 0)
        .map_or((None, None), |(width, height)| {
            (u32::try_from(width).ok(), u32::try_from(height).ok())
        });
    let encoder = publishing_video_encoder(
        &available_video_encoder_backends(),
        VideoSource::ScreenShare,
    );
    let source_name = match stream.source_type() {
        Some(SourceType::Monitor) => "monitor",
        Some(SourceType::Window) => "window",
        Some(SourceType::Virtual) => "region",
        None => "screen",
    };
    let video_source = NativeVideoSource::new(VideoResolution { width, height }, true);
    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "I420")
        .field(
            "width",
            i32::try_from(width).context("screen width is too large")?,
        )
        .field(
            "height",
            i32::try_from(height).context("screen height is too large")?,
        )
        .field(
            "framerate",
            gst::Fraction::new(i32::try_from(profile.fps)?, 1),
        )
        .build();
    let published_frames = Arc::new(AtomicU64::new(0));
    let callback_failed = Arc::new(AtomicBool::new(false));
    let callback_source = video_source.clone();
    let callback_frames = published_frames.clone();
    let callback_events = event_tx.clone();
    let callback_failure = callback_failed.clone();
    let preview_every = u64::from((profile.fps / 2).max(1));
    let app_sink = gst_app::AppSink::builder()
        .caps(&caps)
        .max_buffers(2)
        .drop(true)
        .sync(false)
        .enable_last_sample(false)
        .callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let next_frame = callback_frames.load(Ordering::Acquire) + 1;
                    let preview = (next_frame == 1 || next_frame.is_multiple_of(preview_every))
                        .then_some((VideoSource::ScreenShare, next_frame));
                    let result = capture_screen_sample(sink, &callback_source, preview);
                    match result {
                        Ok(()) => {
                            let total = callback_frames.fetch_add(1, Ordering::AcqRel) + 1;
                            if total == 1 || total.is_multiple_of(preview_every) {
                                let _ = callback_events
                                    .send(MediaEvent::ScreenShareFrames { generation, total });
                            }
                            Ok(gst::FlowSuccess::Ok)
                        }
                        Err(error) => {
                            if !callback_failure.swap(true, Ordering::AcqRel) {
                                let _ = callback_events.send(MediaEvent::ScreenShareStopped {
                                    generation,
                                    error: Some(error.to_string()),
                                });
                            }
                            Err(gst::FlowError::Error)
                        }
                    }
                })
                .build(),
        )
        .build();
    let pipewire = gst::ElementFactory::make("pipewiresrc")
        .property("fd", pipewire_remote.as_raw_fd())
        .property("path", stream.pipe_wire_node_id().to_string())
        .property("do-timestamp", true)
        .build()
        .context("the GStreamer PipeWire source is not installed")?;
    let convert = gst::ElementFactory::make("videoconvert")
        .build()
        .context("the GStreamer video converter is not installed")?;
    let scale = gst::ElementFactory::make("videoscale")
        .property("add-borders", true)
        .build()
        .context("the GStreamer video scaler is not installed")?;
    let rate = gst::ElementFactory::make("videorate")
        .build()
        .context("the GStreamer frame-rate converter is not installed")?;
    let pipeline = gst::Pipeline::default();
    pipeline
        .add_many([&pipewire, &convert, &scale, &rate, app_sink.upcast_ref()])
        .context("build the screen capture pipeline")?;
    gst::Element::link_many([&pipewire, &convert, &scale, &rate, app_sink.upcast_ref()])
        .context("link the screen capture pipeline")?;

    let video_track =
        LocalVideoTrack::create_video_track("screen-share", RtcVideoSource::Native(video_source));
    let publication = room
        .local_participant()
        .publish_track(
            LocalTrack::Video(video_track),
            video_publish_options(VideoSource::ScreenShare, quality, codec, encoder),
        )
        .await
        .context("publish the screen share to LiveKit")?;
    if let Err(error) = pipeline.set_state(gst::State::Playing) {
        let _ = room
            .local_participant()
            .unpublish_track(&publication.sid())
            .await;
        let _ = portal_session.close().await;
        return Err(error).context("start the screen capture pipeline");
    }

    let monitor_running = Arc::new(AtomicBool::new(true));
    let monitor_task = monitor_screen_share(
        pipeline
            .bus()
            .context("screen capture pipeline has no message bus")?,
        generation,
        monitor_running.clone(),
        event_tx,
    );
    let state = ScreenShareState {
        active: true,
        source: Some(source_name.into()),
        source_width,
        source_height,
        width: Some(width),
        height: Some(height),
        fps: Some(profile.fps),
        encoder_backend: Some(encoder_backend_name(encoder).into()),
        ..ScreenShareState::default()
    };
    info!(
        source = source_name,
        width,
        height,
        fps = profile.fps,
        max_bitrate = profile.max_bitrate,
        encoder = encoder_backend_name(encoder),
        codec = %codec,
        quality = %quality,
        "screen share published"
    );
    Ok((
        ScreenShareSession {
            publication_sid: publication.sid(),
            pipeline,
            portal_session,
            _pipewire_remote: pipewire_remote,
            monitor_running,
            monitor_task,
        },
        state,
    ))
}

fn capture_screen_sample(
    sink: &gst_app::AppSink,
    video_source: &NativeVideoSource,
    preview_source: Option<(VideoSource, u64)>,
) -> anyhow::Result<()> {
    use gst_video::VideoFrameExt;

    let sample = sink.pull_sample().context("read a captured screen frame")?;
    let caps = sample
        .caps()
        .context("captured screen frame has no format")?;
    let info = gst_video::VideoInfo::from_caps(caps).context("read the screen frame format")?;
    if info.format() != gst_video::VideoFormat::I420 {
        bail!("screen frame is not I420");
    }
    let buffer = sample
        .buffer()
        .context("captured screen sample has no buffer")?;
    let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info)
        .context("map the captured screen frame")?;
    let strides = frame.plane_stride();
    let mut i420 = I420Buffer::with_strides(
        info.width(),
        info.height(),
        u32::try_from(strides[0]).context("invalid screen luma stride")?,
        u32::try_from(strides[1]).context("invalid screen chroma stride")?,
        u32::try_from(strides[2]).context("invalid screen chroma stride")?,
    );
    let source_planes = frame.planes_data();
    let (luma, chroma_u, chroma_v) = i420.data_mut();
    copy_video_plane(luma, source_planes[0])?;
    copy_video_plane(chroma_u, source_planes[1])?;
    copy_video_plane(chroma_v, source_planes[2])?;
    if let Some((source, revision)) = preview_source
        && let Err(error) =
            write_local_preview(&mut i420, source, revision, info.width(), info.height())
    {
        debug!(%error, %source, "could not refresh the local video preview");
    }
    video_source.capture_frame(&VideoFrame::new(VideoRotation::VideoRotation0, i420));
    Ok(())
}

fn local_preview_path(source: VideoSource, revision: Option<u64>) -> Option<PathBuf> {
    let stem = match source {
        VideoSource::Camera => "camera-preview",
        VideoSource::ScreenShare => "screen-share-preview",
    };
    let file_name = revision.map_or_else(
        || format!("{stem}.bmp"),
        |revision| format!("{stem}-{}.bmp", revision % 2),
    );
    std::env::var_os("WISP_PREVIEW_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .map(|runtime| runtime.join("wisp"))
        })
        .map(|directory| directory.join(file_name))
}

fn remove_local_preview(source: VideoSource) {
    for revision in [None, Some(0), Some(1)] {
        if let Some(path) = local_preview_path(source, revision)
            && let Err(error) = fs::remove_file(&path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            debug!(%error, path = %path.display(), "could not remove a local video preview");
        }
    }
}

fn write_local_preview(
    frame: &mut I420Buffer,
    source: VideoSource,
    revision: u64,
    width: u32,
    height: u32,
) -> anyhow::Result<()> {
    let Some(path) = local_preview_path(source, Some(revision)) else {
        return Ok(());
    };
    let source_width = i32::try_from(width).context("preview width is too large")?;
    let source_height = i32::try_from(height).context("preview height is too large")?;
    let (width, height) = screen_share_resolution(
        Some((source_width, source_height)),
        PREVIEW_MAX_WIDTH,
        PREVIEW_MAX_HEIGHT,
    );
    let width_i32 = i32::try_from(width).context("preview width is too large")?;
    let height_i32 = i32::try_from(height).context("preview height is too large")?;
    let stride = width.checked_mul(4).context("preview stride overflow")?;
    let pixel_bytes = stride
        .checked_mul(height)
        .context("preview image size overflow")?;
    let scaled = frame.scale(width_i32, height_i32);
    let mut pixels = vec![0_u8; usize::try_from(pixel_bytes)?];
    // libyuv format names describe register order. ARGB conversion produces
    // BGRA bytes on little-endian Linux, matching a 32-bit BMP pixel.
    scaled.to_argb(
        VideoFormatType::ARGB,
        &mut pixels,
        stride,
        width_i32,
        height_i32,
    );

    let file_size = 54_u32
        .checked_add(pixel_bytes)
        .context("preview file size overflow")?;
    let mut bmp = Vec::with_capacity(usize::try_from(file_size)?);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size.to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&54_u32.to_le_bytes());
    bmp.extend_from_slice(&40_u32.to_le_bytes());
    bmp.extend_from_slice(&width_i32.to_le_bytes());
    bmp.extend_from_slice(&(-height_i32).to_le_bytes());
    bmp.extend_from_slice(&1_u16.to_le_bytes());
    bmp.extend_from_slice(&32_u16.to_le_bytes());
    bmp.extend_from_slice(&0_u32.to_le_bytes());
    bmp.extend_from_slice(&pixel_bytes.to_le_bytes());
    bmp.extend_from_slice(&[0; 16]);
    bmp.extend_from_slice(&pixels);

    let directory = path.parent().context("preview path has no parent")?;
    fs::create_dir_all(directory).context("create the preview directory")?;
    let temporary = path.with_extension("bmp.part");
    fs::write(&temporary, bmp).context("write the local video preview")?;
    fs::rename(&temporary, &path).context("publish the local video preview")?;
    Ok(())
}

fn copy_video_plane(destination: &mut [u8], source: &[u8]) -> anyhow::Result<()> {
    if source.len() < destination.len() {
        bail!("captured screen plane is shorter than its negotiated stride");
    }
    destination.copy_from_slice(&source[..destination.len()]);
    Ok(())
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn screen_share_resolution(
    size: Option<(i32, i32)>,
    max_width: u32,
    max_height: u32,
) -> (u32, u32) {
    let (source_width, source_height) = size
        .filter(|(width, height)| *width > 0 && *height > 0)
        .unwrap_or((1280, 720));
    let scale = (f64::from(max_width) / f64::from(source_width))
        .min(f64::from(max_height) / f64::from(source_height))
        .min(1.0);
    let even = |value: i32| {
        let scaled = (f64::from(value) * scale).round() as u32;
        scaled.max(2) & !1
    };
    (even(source_width), even(source_height))
}

fn monitor_screen_share(
    bus: gst::Bus,
    generation: u64,
    running: Arc<AtomicBool>,
    event_tx: mpsc::UnboundedSender<MediaEvent>,
) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        while running.load(Ordering::Acquire) {
            let Some(message) = bus.timed_pop(gst::ClockTime::from_mseconds(250)) else {
                continue;
            };
            let error = match message.view() {
                gst::MessageView::Eos(..) => None,
                gst::MessageView::Error(error) => {
                    Some(format!("screen capture stopped: {}", error.error()))
                }
                _ => continue,
            };
            if running.swap(false, Ordering::AcqRel) {
                let _ = event_tx.send(MediaEvent::ScreenShareStopped { generation, error });
            }
            return;
        }
    })
}

async fn stop_screen_share_session(room: &Room, screen_share: ScreenShareSession) {
    screen_share.monitor_running.store(false, Ordering::Release);
    if let Err(error) = screen_share.pipeline.set_state(gst::State::Null) {
        warn!(%error, "screen capture pipeline did not stop cleanly");
    }
    if let Err(error) = room
        .local_participant()
        .unpublish_track(&screen_share.publication_sid)
        .await
    {
        debug!(%error, "screen share track was already unpublished");
    }
    if let Err(error) = screen_share.portal_session.close().await {
        debug!(%error, "screen cast portal session was already closed");
    }
    let _ = screen_share.monitor_task.await;
    remove_local_preview(VideoSource::ScreenShare);
    info!("screen share stopped");
}

fn processing_options(preset: AudioPreset) -> AudioProcessingOptions {
    match preset {
        AudioPreset::Natural => AudioProcessingOptions {
            echo_cancellation: true,
            noise_suppression: true,
            auto_gain_control: false,
            prefer_hardware_processing: false,
        },
        AudioPreset::Clear => AudioProcessingOptions {
            echo_cancellation: true,
            // DeepFilterNet handles suppression in the explicit microphone pipeline.
            noise_suppression: false,
            // Raising the denoiser's residual floor makes intermittent artifacts audible.
            auto_gain_control: false,
            prefer_hardware_processing: false,
        },
        AudioPreset::Studio => AudioProcessingOptions {
            echo_cancellation: false,
            noise_suppression: false,
            auto_gain_control: false,
            prefer_hardware_processing: false,
        },
    }
}

fn source_processing_options(preset: AudioPreset) -> AudioSourceOptions {
    let processing = processing_options(preset);
    AudioSourceOptions {
        echo_cancellation: processing.echo_cancellation,
        noise_suppression: processing.noise_suppression,
        auto_gain_control: processing.auto_gain_control,
    }
}

#[allow(clippy::cast_possible_truncation)]
fn pcm_dbfs(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return -90.0;
    }
    let mean_square = samples
        .iter()
        .map(|sample| f64::from(*sample).powi(2))
        .sum::<f64>()
        / f64::from(u32::try_from(samples.len()).expect("audio frame length fits u32"));
    if mean_square < 1.0 {
        return -90.0;
    }
    let full_scale_square = f64::from(i16::MAX).powi(2);
    (10.0 * (mean_square / full_scale_square).log10()).clamp(-90.0, 0.0) as f32
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pcm_level_percent(samples: &[i16]) -> u8 {
    let dbfs = pcm_dbfs(samples);
    if dbfs <= -60.0 {
        return 0;
    }
    (((dbfs + 60.0) / 60.0).clamp(0.0, 1.0) * 100.0).round() as u8
}

async fn receive_video_frames(
    generation: u64,
    target: RemoteVideoTarget,
    track: RemoteVideoTrack,
    received_frames: Arc<AtomicU64>,
    surface: Option<SurfaceController>,
    video_bridge: crate::video_bridge::VideoBridge,
    event_tx: mpsc::UnboundedSender<MediaEvent>,
) {
    let mut stream = NativeVideoStream::new(track.rtc_track());
    let mut track_total = 0_u64;
    while let Some(frame) = stream.next().await {
        let width = frame.buffer.width();
        let height = frame.buffer.height();
        let Some(byte_len) = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .and_then(|bytes| usize::try_from(bytes).ok())
        else {
            warn!(width, height, "remote video frame dimensions overflow");
            continue;
        };
        let Ok(dst_width) = i32::try_from(width) else {
            warn!(width, "remote video frame is too wide");
            continue;
        };
        if byte_len > 128 * 1024 * 1024 || width > 16384 || height > 16384 {
            warn!(width, height, "remote video frame exceeds renderer limits");
            continue;
        }
        let Ok(dst_height) = i32::try_from(height) else {
            warn!(height, "remote video frame is too tall");
            continue;
        };
        let mut rgba = vec![0; byte_len];
        i420_to_rgba_texture(
            &frame.buffer.to_i420(),
            &mut rgba,
            width.saturating_mul(4),
            dst_width,
            dst_height,
        );
        if video_bridge.contains(&target) {
            video_bridge.send(
                &target,
                RgbaFrame {
                    width,
                    height,
                    data: rgba,
                },
            );
        } else if let Some(surface) = &surface
            && let Err(error) = surface.send_frame(
                &target,
                RgbaFrame {
                    width,
                    height,
                    data: rgba,
                },
            )
        {
            let _ = event_tx.send(MediaEvent::SurfaceError {
                target: Some(target.clone()),
                message: error.to_string(),
            });
        }
        let total = received_frames.fetch_add(1, Ordering::AcqRel) + 1;
        track_total = track_total.saturating_add(1);
        if track_total == 1 || track_total.is_multiple_of(30) {
            let _ = event_tx.send(MediaEvent::VideoFrames {
                generation,
                target: target.clone(),
                total,
                track_total,
                width,
                height,
            });
        }
    }
}

fn i420_to_rgba_texture(
    buffer: &I420Buffer,
    destination: &mut [u8],
    stride: u32,
    width: i32,
    height: i32,
) {
    // libyuv format names describe register order. On little-endian Linux its
    // ABGR conversion produces RGBA bytes, which is what the wgpu texture uses.
    buffer.to_argb(VideoFormatType::ABGR, destination, stride, width, height);
}

async fn count_audio_frames(
    generation: u64,
    participant: String,
    track: RemoteAudioTrack,
    received_frames: Arc<AtomicU64>,
    event_tx: mpsc::UnboundedSender<MediaEvent>,
) {
    let mut stream = NativeAudioStream::new(track.rtc_track(), 48_000, 1);
    let mut meter_frames = 0_u8;
    let mut meter_peak = 0_u8;
    while let Some(frame) = stream.next().await {
        meter_peak = meter_peak.max(pcm_level_percent(&frame.data));
        meter_frames += 1;
        let total = received_frames.fetch_add(1, Ordering::AcqRel) + 1;
        if meter_frames == 10 {
            let _ = event_tx.send(MediaEvent::AudioFrames {
                generation,
                participant: participant.clone(),
                total,
                level: meter_peak,
            });
            meter_frames = 0;
            meter_peak = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn camera_confirmation_rejects_changed_room_or_device() {
        use super::validate_camera_start_target;
        assert!(
            validate_camera_start_target(
                Some("test_room"),
                "test_room",
                Some("camera-a"),
                Some("camera-a")
            )
            .is_ok()
        );
        assert!(
            validate_camera_start_target(
                Some("test_room"),
                "other",
                Some("camera-a"),
                Some("camera-a")
            )
            .is_err()
        );
        assert!(
            validate_camera_start_target(
                Some("test_room"),
                "test_room",
                Some("camera-a"),
                Some("camera-b")
            )
            .is_err()
        );
        assert!(
            validate_camera_start_target(Some("test_room"), "test_room", Some("camera-a"), None)
                .is_err()
        );
    }

    use super::{
        AUDIO_FRAME_SAMPLES, NeuralDenoiser, VideoWatchSignal, create_deepfilter_model,
        deepfilter_frame, i420_to_rgba_texture, pcm_level_percent, preferred_or_first,
        processing_options, public_device_id, publishing_video_encoder, same_logical_device,
        screen_share_resolution, source_processing_options, surface_quality, video_profile,
        video_watch_targets_local,
    };
    use livekit::options::VideoEncoderBackend;
    use livekit::track::VideoQuality;
    use livekit::webrtc::video_frame::I420Buffer;
    use wisp_protocol::{AudioDevice, AudioPreset, VideoQualityPreset, VideoSource};

    fn device(id: &str) -> AudioDevice {
        AudioDevice {
            id: id.into(),
            name: id.into(),
        }
    }

    #[test]
    fn video_viewer_updates_use_identity_with_legacy_name_fallback() {
        let identity_signal = VideoWatchSignal {
            participant: "stale display name".into(),
            participant_identity: Some("user-member_a".into()),
            source: VideoSource::Camera,
            watching: true,
        };
        assert!(video_watch_targets_local(
            &identity_signal,
            "MemberA",
            "user-member_a"
        ));

        let wrong_identity_signal = VideoWatchSignal {
            participant: "MemberA".into(),
            participant_identity: Some("user-owner".into()),
            source: VideoSource::ScreenShare,
            watching: true,
        };
        assert!(!video_watch_targets_local(
            &wrong_identity_signal,
            "MemberA",
            "user-member_a"
        ));

        let legacy_signal = VideoWatchSignal {
            participant: "MemberA".into(),
            participant_identity: None,
            source: VideoSource::Camera,
            watching: true,
        };
        assert!(video_watch_targets_local(
            &legacy_signal,
            "MemberA",
            "user-member_a"
        ));
    }

    #[test]
    fn preferred_device_falls_back_and_recovers() {
        let fallback = vec![device("fallback")];
        assert_eq!(
            preferred_or_first(&fallback, Some("preferred")).map(|device| device.id.as_str()),
            Some("fallback")
        );

        let recovered = vec![device("fallback"), device("preferred")];
        assert_eq!(
            preferred_or_first(&recovered, Some("preferred")).map(|device| device.id.as_str()),
            Some("preferred")
        );
    }

    #[test]
    fn empty_platform_guids_get_distinct_recoverable_ids() {
        let first = public_device_id("", 0, "USB microphone");
        let second = public_device_id("", 1, "Built-in microphone");
        assert_ne!(first, second);
        assert!(same_logical_device(
            &first,
            &public_device_id("", 4, "USB microphone")
        ));
        assert_eq!(public_device_id("guid", 0, "ignored"), "guid");
    }

    #[test]
    fn processing_presets_have_distinct_intent() {
        let natural = processing_options(AudioPreset::Natural);
        let clear = processing_options(AudioPreset::Clear);
        let studio = processing_options(AudioPreset::Studio);

        assert!(natural.echo_cancellation && natural.noise_suppression);
        assert!(!natural.auto_gain_control);
        assert!(clear.echo_cancellation && !clear.auto_gain_control && !clear.noise_suppression);
        assert!(!studio.echo_cancellation && !studio.noise_suppression);

        let natural_source = source_processing_options(AudioPreset::Natural);
        let clear_source = source_processing_options(AudioPreset::Clear);
        let studio_source = source_processing_options(AudioPreset::Studio);
        assert!(natural_source.echo_cancellation && natural_source.noise_suppression);
        assert!(clear_source.echo_cancellation && !clear_source.noise_suppression);
        assert!(!studio_source.echo_cancellation && !studio_source.noise_suppression);
    }

    #[test]
    fn deepfilternet_processes_real_ten_millisecond_frames() {
        let mut denoiser = create_deepfilter_model().expect("embedded model should load");
        let mut output = Vec::new();
        let mut input = Vec::new();
        for frame_index in 0..10 {
            input = (0..AUDIO_FRAME_SAMPLES)
                .map(|index| {
                    if ((frame_index * AUDIO_FRAME_SAMPLES + index) / 24).is_multiple_of(2) {
                        12_000
                    } else {
                        -12_000
                    }
                })
                .collect::<Vec<_>>();
            output = deepfilter_frame(&mut denoiser, &input).expect("frame should process");
        }
        assert_eq!(output.len(), AUDIO_FRAME_SAMPLES);
        assert_ne!(output, input);
    }

    #[test]
    fn deepfilternet_strength_zero_bypasses_noise_reduction() {
        let model = create_deepfilter_model().expect("embedded model should load");
        let mut denoiser = NeuralDenoiser::deepfilternet(model);
        denoiser.set_strength(0);
        let input = (0..AUDIO_FRAME_SAMPLES)
            .map(|index| {
                if index.is_multiple_of(2) {
                    8_000
                } else {
                    -8_000
                }
            })
            .collect::<Vec<_>>();

        let output = denoiser
            .process_frame(&input)
            .expect("frame should process");

        assert_eq!(output, input);
    }

    #[test]
    fn screen_share_resolution_is_even_and_bounded() {
        assert_eq!(screen_share_resolution(None, 1920, 1080), (1280, 720));
        assert_eq!(
            screen_share_resolution(Some((1920, 1080)), 1920, 1080),
            (1920, 1080)
        );
        let (width, height) = screen_share_resolution(Some((5120, 1440)), 1920, 1080);
        assert_eq!((width, height), (1920, 540));
        assert!(width.is_multiple_of(2) && height.is_multiple_of(2));
    }

    #[test]
    fn publishing_avoids_the_unstable_vaapi_encoder() {
        let backends = [VideoEncoderBackend::Software, VideoEncoderBackend::Vaapi];
        assert_eq!(
            publishing_video_encoder(&backends, VideoSource::ScreenShare),
            VideoEncoderBackend::Software
        );
        assert_eq!(
            publishing_video_encoder(&backends, VideoSource::Camera),
            VideoEncoderBackend::Software
        );
    }

    #[test]
    fn publishing_profiles_match_the_m3_quality_ladder() {
        let balanced = video_profile(VideoQualityPreset::Balanced, VideoSource::ScreenShare);
        assert_eq!(
            (balanced.width, balanced.height, balanced.fps),
            (1280, 720, 30)
        );

        let high = video_profile(VideoQualityPreset::High, VideoSource::ScreenShare);
        assert_eq!((high.width, high.height, high.fps), (1920, 1080, 60));

        let ultra = video_profile(VideoQualityPreset::Ultra, VideoSource::ScreenShare);
        assert_eq!((ultra.width, ultra.height, ultra.fps), (2560, 1440, 60));

        let camera = video_profile(VideoQualityPreset::High, VideoSource::Camera);
        assert_eq!((camera.width, camera.height, camera.fps), (1280, 720, 30));
    }

    #[test]
    fn surface_dimensions_select_only_the_useful_simulcast_layer() {
        assert_eq!(surface_quality(320, 180), VideoQuality::Low);
        assert_eq!(surface_quality(960, 540), VideoQuality::Medium);
        assert_eq!(surface_quality(1920, 1080), VideoQuality::High);
    }

    #[test]
    fn remote_video_conversion_matches_rgba_texture_order() {
        let mut frame = I420Buffer::new(2, 2);
        let (luma, chroma_u, chroma_v) = frame.data_mut();
        luma.fill(82);
        chroma_u.fill(90);
        chroma_v.fill(240);

        let mut rgba = [0_u8; 16];
        i420_to_rgba_texture(&frame, &mut rgba, 8, 2, 2);

        for pixel in rgba.chunks_exact(4) {
            assert!(pixel[0] > 240, "red channel should be dominant: {pixel:?}");
            assert!(pixel[1] < 16, "green channel should be low: {pixel:?}");
            assert!(pixel[2] < 16, "blue channel should be low: {pixel:?}");
            assert_eq!(pixel[3], 255, "alpha channel should be opaque");
        }
    }

    #[test]
    fn input_level_uses_a_readable_dbfs_scale() {
        assert_eq!(pcm_level_percent(&[0; AUDIO_FRAME_SAMPLES]), 0);
        assert_eq!(pcm_level_percent(&[i16::MAX; AUDIO_FRAME_SAMPLES]), 100);
        assert!((65..=68).contains(&pcm_level_percent(&[3277; AUDIO_FRAME_SAMPLES])));
    }
}
