use anyhow::{Context, bail};
use futures_util::StreamExt;
use livekit::{
    options::TrackPublishOptions,
    prelude::{
        AudioProcessingOptions, LocalAudioTrack, LocalTrack, Participant, PlatformAudio,
        PlayoutDeviceInfo, RecordingDeviceInfo, RemoteAudioTrack, RemoteTrack, RemoteVideoTrack,
        Room, RoomEvent, RoomOptions, TrackSource,
    },
    rtc_engine::lk_runtime::LkRuntime,
    webrtc::{
        audio_stream::native::NativeAudioStream,
        peer_connection_factory::native::PeerConnectionFactoryExt, prelude::VideoFormatType,
        video_frame::native::VideoFrameBufferExt, video_stream::native::NativeVideoStream,
    },
};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
};
use tokio::{
    sync::{Mutex as AsyncMutex, mpsc},
    task::{JoinHandle, JoinSet},
};
use tracing::{debug, info, warn};
use wisp_protocol::{AudioDevice, AudioPreset, AudioState, HangoutId, LiveKitTokenResponse};

use crate::surface::{RgbaFrame, SurfaceController};

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
    AudioFrames {
        generation: u64,
        participant: String,
        total: u64,
    },
    InputLevel {
        generation: u64,
        level: u8,
    },
    ActiveSpeakers {
        generation: u64,
        speakers: Vec<String>,
    },
    VideoSubscribed {
        generation: u64,
        participant: String,
    },
    VideoFrames {
        generation: u64,
        participant: String,
        total: u64,
        width: u32,
        height: u32,
    },
    SurfaceOpened,
    SurfaceClosed,
    SurfaceRendered {
        total: u64,
    },
    SurfaceError {
        message: String,
    },
}

pub(crate) struct ConnectedMedia {
    pub microphone: String,
    pub speaker: String,
    pub audio: AudioState,
}

pub(crate) struct AudioInventory {
    pub state: AudioState,
    pub microphone: Option<String>,
    pub speaker: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Default)]
struct AudioPreferences {
    preferred_input_id: Option<String>,
    preferred_output_id: Option<String>,
    selected_input_id: Option<String>,
    selected_output_id: Option<String>,
    preset: AudioPreset,
}

struct MediaSession {
    hangout_id: HangoutId,
    room: Arc<Room>,
    microphone: LocalAudioTrack,
    _platform_audio: PlatformAudio,
    remote_audio: Arc<Mutex<HashMap<String, RemoteAudioTrack>>>,
    event_task: JoinHandle<()>,
}

pub(crate) struct MediaManager {
    operation: AsyncMutex<()>,
    session: AsyncMutex<Option<MediaSession>>,
    generation: AtomicU64,
    connected: Arc<AtomicBool>,
    deafened: Arc<AtomicBool>,
    received_frames: Arc<AtomicU64>,
    received_video_frames: Arc<AtomicU64>,
    input_level: Arc<AtomicU8>,
    platform_audio: Mutex<Option<PlatformAudio>>,
    audio_preferences: Mutex<AudioPreferences>,
    surface: Option<SurfaceController>,
    event_tx: mpsc::UnboundedSender<MediaEvent>,
}

impl MediaManager {
    pub(crate) fn new(surface_enabled: bool) -> (Self, mpsc::UnboundedReceiver<MediaEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let surface = if surface_enabled {
            match SurfaceController::spawn(event_tx.clone()) {
                Ok(surface) => Some(surface),
                Err(error) => {
                    let _ = event_tx.send(MediaEvent::SurfaceError {
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
                operation: AsyncMutex::new(()),
                session: AsyncMutex::new(None),
                generation: AtomicU64::new(0),
                connected: Arc::new(AtomicBool::new(false)),
                deafened: Arc::new(AtomicBool::new(false)),
                received_frames: Arc::new(AtomicU64::new(0)),
                received_video_frames: Arc::new(AtomicU64::new(0)),
                input_level: Arc::new(AtomicU8::new(0)),
                platform_audio: Mutex::new(None),
                audio_preferences: Mutex::new(AudioPreferences::default()),
                surface,
                event_tx,
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
            *audio = Some(PlatformAudio::new().context("initialize microphone and speaker")?);
        }
        Ok(audio
            .as_ref()
            .expect("platform audio was initialized")
            .clone())
    }

    pub(crate) async fn is_active(&self) -> bool {
        self.session.lock().await.is_some()
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
        select_recording_device(&audio, &device, active)
            .with_context(|| format!("select microphone {}", device.name))?;
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
        self.audio_preferences
            .lock()
            .expect("audio preferences lock poisoned")
            .preset = preset;
        Ok(self.reconcile_audio_devices(&audio, self.is_active().await, false))
    }

    fn unavailable_audio_inventory(&self, error: String) -> AudioInventory {
        let preset = self
            .audio_preferences
            .lock()
            .expect("audio preferences lock poisoned")
            .preset;
        AudioInventory {
            state: AudioState {
                preset,
                ..AudioState::default()
            },
            microphone: None,
            speaker: None,
            error: Some(error),
        }
    }

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
            && let Some(device) = recording_devices
                .iter()
                .find(|device| recording_device_id(device) == *id)
        {
            let result = select_recording_device(audio, device, active);
            match result {
                Ok(()) => preferences.selected_input_id.clone_from(&next_input),
                Err(error) => {
                    errors.push(format!("select microphone {}: {error}", device.name));
                    preferences.selected_input_id = preferences
                        .selected_input_id
                        .take()
                        .filter(|id| input_devices.iter().any(|device| &device.id == id));
                }
            }
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
        AudioInventory {
            state: AudioState {
                input_devices,
                output_devices,
                selected_input_id,
                selected_output_id,
                preset: preferences.preset,
                input_level: 0,
            },
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

    pub(crate) async fn connect(
        &self,
        hangout_id: HangoutId,
        credentials: LiveKitTokenResponse,
        muted: bool,
        deafened: bool,
    ) -> anyhow::Result<ConnectedMedia> {
        let _operation = self.operation.lock().await;
        if self.is_connected_to(hangout_id).await {
            bail!("already connected to this hangout");
        }
        self.disconnect_session().await;

        let platform_audio = self.platform_audio()?;
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

        let (room, events) =
            Room::connect(&credentials.url, &credentials.token, RoomOptions::default())
                .await
                .with_context(|| format!("connect to LiveKit room {}", credentials.room))?;
        let room = Arc::new(room);
        let microphone_track =
            LocalAudioTrack::create_audio_track("microphone", platform_audio.rtc_source());
        if muted {
            microphone_track.mute();
        }
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

        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.received_frames.store(0, Ordering::Release);
        self.received_video_frames.store(0, Ordering::Release);
        self.input_level.store(0, Ordering::Release);
        self.connected.store(true, Ordering::Release);
        self.deafened.store(deafened, Ordering::Release);
        let remote_audio = Arc::new(Mutex::new(HashMap::new()));
        let event_task = tokio::spawn(run_room_events(
            events,
            RoomEventContext {
                generation,
                event_tx: self.event_tx.clone(),
                remote_audio: remote_audio.clone(),
                received_frames: self.received_frames.clone(),
                received_video_frames: self.received_video_frames.clone(),
                input_level: self.input_level.clone(),
                connected: self.connected.clone(),
                deafened: self.deafened.clone(),
                surface: self.surface.clone(),
            },
        ));
        *self.session.lock().await = Some(MediaSession {
            hangout_id,
            room,
            microphone: microphone_track,
            _platform_audio: platform_audio,
            remote_audio,
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
        })
    }

    pub(crate) async fn disconnect(&self) {
        let _operation = self.operation.lock().await;
        self.disconnect_session().await;
    }

    async fn disconnect_session(&self) {
        self.connected.store(false, Ordering::Release);
        self.input_level.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        if let Some(surface) = &self.surface {
            let _ = surface.close();
        }
        let Some(session) = self.session.lock().await.take() else {
            return;
        };
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
            for track in remote_audio
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

    pub(crate) fn open_surface(&self) -> anyhow::Result<()> {
        self.surface
            .as_ref()
            .context("the video surface is unavailable")?
            .open()
    }

    pub(crate) fn close_surface(&self) -> anyhow::Result<()> {
        self.surface
            .as_ref()
            .context("the video surface is unavailable")?
            .close()
    }

    pub(crate) fn shutdown_surface(&self) {
        if let Some(surface) = &self.surface {
            surface.shutdown();
        }
    }
}

struct RoomEventContext {
    generation: u64,
    event_tx: mpsc::UnboundedSender<MediaEvent>,
    remote_audio: Arc<Mutex<HashMap<String, RemoteAudioTrack>>>,
    received_frames: Arc<AtomicU64>,
    received_video_frames: Arc<AtomicU64>,
    input_level: Arc<AtomicU8>,
    connected: Arc<AtomicBool>,
    deafened: Arc<AtomicBool>,
    surface: Option<SurfaceController>,
}

impl RoomEventContext {
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

        let level = speakers
            .iter()
            .find_map(|participant| match participant {
                Participant::Local(_) => Some(audio_level_percent(participant.audio_level())),
                Participant::Remote(_) => None,
            })
            .unwrap_or(0);
        let previous = self.input_level.swap(level, Ordering::AcqRel);
        if previous.abs_diff(level) >= 2 || (previous != 0 && level == 0) {
            let _ = self.event_tx.send(MediaEvent::InputLevel {
                generation: self.generation,
                level,
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
        if self.deafened.load(Ordering::Acquire) {
            track.disable();
        } else {
            track.enable();
        }
        self.remote_audio
            .lock()
            .expect("remote audio lock poisoned")
            .insert(sid, track.clone());
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
    ) {
        let _ = self.event_tx.send(MediaEvent::VideoSubscribed {
            generation: self.generation,
            participant: participant.clone(),
        });
        if let Some(surface) = &self.surface
            && let Err(error) = surface.open()
        {
            let _ = self.event_tx.send(MediaEvent::SurfaceError {
                message: error.to_string(),
            });
        }
        track_tasks.spawn(receive_video_frames(
            self.generation,
            participant,
            track,
            self.received_video_frames.clone(),
            self.surface.clone(),
            self.event_tx.clone(),
        ));
    }
}

async fn run_room_events(
    mut events: tokio::sync::mpsc::UnboundedReceiver<RoomEvent>,
    context: RoomEventContext,
) {
    let mut track_tasks = JoinSet::new();
    let mut active_speakers = Vec::new();
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
                context.subscribe_video(participant.name(), track, &mut track_tasks);
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
                track: RemoteTrack::Video(_),
                ..
            } => {
                if let Some(surface) = &context.surface {
                    let _ = surface.close();
                }
            }
            RoomEvent::ActiveSpeakersChanged { speakers } => {
                context.active_speakers_changed(&speakers, &mut active_speakers);
            }
            RoomEvent::Reconnecting => {
                let _ = context.event_tx.send(MediaEvent::Reconnecting {
                    generation: context.generation,
                });
            }
            RoomEvent::Reconnected => {
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
        let _ = surface.close();
    }
    track_tasks.abort_all();
    while track_tasks.join_next().await.is_some() {}
}

fn recording_device_id(device: &RecordingDeviceInfo) -> String {
    public_device_id(device.id.as_str(), device.index, &device.name)
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

fn select_recording_device(
    audio: &PlatformAudio,
    device: &RecordingDeviceInfo,
    active: bool,
) -> anyhow::Result<()> {
    if !device.id.as_str().is_empty() {
        return if active {
            audio.switch_recording_device(&device.id)
        } else {
            audio.set_recording_device(&device.id)
        }
        .map_err(Into::into);
    }

    let runtime = LkRuntime::instance();
    let factory = runtime.pc_factory();
    let index = u16::try_from(device.index).context("microphone index is out of range")?;
    let was_initialized = factory.recording_is_initialized();
    if active && was_initialized && !factory.stop_recording() {
        bail!("stop microphone before switching");
    }
    if !factory.set_recording_device(index) {
        if active && was_initialized {
            let _ = factory.init_recording() && factory.start_recording();
        }
        bail!("select microphone by index {index}");
    }
    if active && (!factory.init_recording() || !factory.start_recording()) {
        bail!("restart microphone after switching");
    }
    Ok(())
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
            noise_suppression: true,
            auto_gain_control: true,
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

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn audio_level_percent(level: f32) -> u8 {
    (level.clamp(0.0, 1.0) * 100.0).round() as u8
}

async fn receive_video_frames(
    generation: u64,
    participant: String,
    track: RemoteVideoTrack,
    received_frames: Arc<AtomicU64>,
    surface: Option<SurfaceController>,
    event_tx: mpsc::UnboundedSender<MediaEvent>,
) {
    let mut stream = NativeVideoStream::new(track.rtc_track());
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
        let Ok(dst_height) = i32::try_from(height) else {
            warn!(height, "remote video frame is too tall");
            continue;
        };
        let mut rgba = vec![0; byte_len];
        frame.buffer.to_i420().to_argb(
            VideoFormatType::RGBA,
            &mut rgba,
            width.saturating_mul(4),
            dst_width,
            dst_height,
        );
        if let Some(surface) = &surface
            && let Err(error) = surface.send_frame(RgbaFrame {
                width,
                height,
                data: rgba,
            })
        {
            let _ = event_tx.send(MediaEvent::SurfaceError {
                message: error.to_string(),
            });
        }
        let total = received_frames.fetch_add(1, Ordering::AcqRel) + 1;
        if total == 1 || total.is_multiple_of(30) {
            let _ = event_tx.send(MediaEvent::VideoFrames {
                generation,
                participant: participant.clone(),
                total,
                width,
                height,
            });
        }
    }
}

async fn count_audio_frames(
    generation: u64,
    participant: String,
    track: RemoteAudioTrack,
    received_frames: Arc<AtomicU64>,
    event_tx: mpsc::UnboundedSender<MediaEvent>,
) {
    let mut stream = NativeAudioStream::new(track.rtc_track(), 48_000, 1);
    while stream.next().await.is_some() {
        let total = received_frames.fetch_add(1, Ordering::AcqRel) + 1;
        if total == 1 || total.is_multiple_of(100) {
            let _ = event_tx.send(MediaEvent::AudioFrames {
                generation,
                participant: participant.clone(),
                total,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        audio_level_percent, preferred_or_first, processing_options, public_device_id,
        same_logical_device,
    };
    use wisp_protocol::{AudioDevice, AudioPreset};

    fn device(id: &str) -> AudioDevice {
        AudioDevice {
            id: id.into(),
            name: id.into(),
        }
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
        assert!(clear.auto_gain_control);
        assert!(!studio.echo_cancellation && !studio.noise_suppression);
    }

    #[test]
    fn input_level_is_clamped_to_a_percentage() {
        assert_eq!(audio_level_percent(-1.0), 0);
        assert_eq!(audio_level_percent(0.426), 43);
        assert_eq!(audio_level_percent(2.0), 100);
    }
}
