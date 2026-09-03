mod media;
mod shortcut;
mod surface;
mod tray;

use anyhow::{Context, anyhow, bail};
use clap::Parser;
use futures_util::StreamExt;
use reqwest::StatusCode;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc, RwLock as StdRwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::{Mutex, RwLock, broadcast, mpsc, watch},
};
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;
use url::Url;
use wisp_protocol::{
    AudioPreset, CameraState, CommandEnvelope, ConnectionState, CreateDirectConversationRequest,
    CreateInviteRequest, DaemonEnvelope, DevSession, DevSessionRequest, DeviceInvite,
    DeviceSession, DeviceSessionRequest, DeviceView, JoinFriendRequest, JoinFriendResult,
    JoinHangoutRequest, JoinSpotRequest, KnockResponse, LiveKitTokenResponse,
    MarkConversationReadRequest, MediaState, PROTOCOL_VERSION, Presence, PushToTalkState,
    RemoteVideoState, RemoteVideoTarget, RespondKnockRequest, RespondKnockResult, ScreenShareState,
    SendMessageRequest, ServerEvent, SetPresenceRequest, Snapshot, VideoCodecPreference,
    VideoQualityPreset, VideoSource,
};

use crate::media::{AudioInventory, MediaEvent, MediaManager};
use crate::shortcut::ShortcutManager;
use crate::tray::TrayAction;

#[derive(Debug, Parser)]
#[command(about = "Wisp's persistent desktop daemon")]
struct Args {
    #[arg(long, env = "WISP_PROFILE")]
    profile: String,
    #[arg(long, env = "WISP_SERVER_URL", default_value = "http://127.0.0.1:8787")]
    server_url: String,
    #[arg(long, env = "WISP_SOCKET")]
    socket: Option<PathBuf>,
    #[arg(long, env = "WISP_DISABLE_MEDIA")]
    disable_media: bool,
    #[arg(long, env = "WISP_DISABLE_SURFACES")]
    disable_surfaces: bool,
    #[arg(long, env = "WISP_PTT_LEASE_MS", default_value_t = 30_000)]
    ptt_lease_ms: u64,
}

#[derive(Clone)]
struct ServerApi {
    client: reqwest::Client,
    base_url: String,
    token: Arc<StdRwLock<String>>,
    auth: AuthMethod,
}

#[derive(Clone)]
enum AuthMethod {
    Development {
        profile: String,
    },
    Device {
        device_id: uuid::Uuid,
        device_token: String,
    },
}

impl ServerApi {
    async fn connect(base_url: String, profile: &str) -> anyhow::Result<(Self, Snapshot)> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;
        let device_id = std::env::var("WISP_DEVICE_ID").ok();
        let device_token = std::env::var("WISP_DEVICE_TOKEN").ok();
        let auth = match (device_id, device_token) {
            (Some(id), Some(token)) => AuthMethod::Device {
                device_id: id.parse().context("WISP_DEVICE_ID is not a UUID")?,
                device_token: token,
            },
            (None, None) => AuthMethod::Development {
                profile: profile.into(),
            },
            _ => bail!("WISP_DEVICE_ID and WISP_DEVICE_TOKEN must be configured together"),
        };
        let token = obtain_session(&client, &base_url, &auth).await?;
        let api = Self {
            client,
            base_url,
            token: Arc::new(StdRwLock::new(token)),
            auth,
        };
        let snapshot = api.snapshot().await?;
        Ok((api, snapshot))
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let token = self
            .token
            .read()
            .expect("session token lock poisoned")
            .clone();
        self.client
            .request(method, format!("{}{}", self.base_url, path))
            .bearer_auth(token)
    }

    async fn renew_session(&self) -> anyhow::Result<()> {
        let token = obtain_session(&self.client, &self.base_url, &self.auth).await?;
        *self.token.write().expect("session token lock poisoned") = token;
        Ok(())
    }

    async fn snapshot(&self) -> anyhow::Result<Snapshot> {
        decode(
            self.request(reqwest::Method::GET, "/v1/snapshot")
                .send()
                .await?,
        )
        .await
    }

    async fn set_presence(&self, presence: Presence) -> anyhow::Result<()> {
        ensure_ok(
            self.request(reqwest::Method::POST, "/v1/presence")
                .json(&SetPresenceRequest { presence })
                .send()
                .await?,
        )
        .await
    }

    async fn join_friend(&self, friend: String) -> anyhow::Result<JoinFriendResult> {
        decode(
            self.request(reqwest::Method::POST, "/v1/hangouts/join-friend")
                .json(&JoinFriendRequest { friend })
                .send()
                .await?,
        )
        .await
    }

    async fn respond_knock(
        &self,
        knock_id: uuid::Uuid,
        response: KnockResponse,
    ) -> anyhow::Result<RespondKnockResult> {
        decode(
            self.request(reqwest::Method::POST, "/v1/knocks/respond")
                .json(&RespondKnockRequest { knock_id, response })
                .send()
                .await?,
        )
        .await
    }

    async fn join_hangout(&self, hangout_id: uuid::Uuid) -> anyhow::Result<()> {
        ensure_ok(
            self.request(reqwest::Method::POST, "/v1/hangouts/join")
                .json(&JoinHangoutRequest { hangout_id })
                .send()
                .await?,
        )
        .await
    }

    async fn leave(&self) -> anyhow::Result<()> {
        ensure_ok(
            self.request(reqwest::Method::POST, "/v1/hangouts/leave")
                .send()
                .await?,
        )
        .await
    }

    async fn livekit_token(&self) -> anyhow::Result<LiveKitTokenResponse> {
        decode(
            self.request(reqwest::Method::POST, "/v1/livekit/token")
                .send()
                .await?,
        )
        .await
    }

    async fn create_direct(
        &self,
        friend: String,
    ) -> anyhow::Result<wisp_protocol::ConversationView> {
        decode(
            self.request(reqwest::Method::POST, "/v1/conversations/direct")
                .json(&CreateDirectConversationRequest { friend })
                .send()
                .await?,
        )
        .await
    }

    async fn send_message(&self, conversation_id: String, text: String) -> anyhow::Result<()> {
        let _: wisp_protocol::Message = decode(
            self.request(reqwest::Method::POST, "/v1/messages")
                .json(&SendMessageRequest {
                    conversation_id,
                    content_type: "text/plain".into(),
                    payload: Value::String(text),
                    encryption_version: 0,
                })
                .send()
                .await?,
        )
        .await?;
        Ok(())
    }

    async fn mark_conversation_read(&self, conversation_id: String) -> anyhow::Result<()> {
        ensure_ok(
            self.request(reqwest::Method::POST, "/v1/conversations/read")
                .json(&MarkConversationReadRequest { conversation_id })
                .send()
                .await?,
        )
        .await
    }

    async fn join_spot(&self, spot_id: String) -> anyhow::Result<()> {
        ensure_ok(
            self.request(reqwest::Method::POST, "/v1/spots/join")
                .json(&JoinSpotRequest { spot_id })
                .send()
                .await?,
        )
        .await
    }

    async fn create_invite(
        &self,
        profile: String,
        expires_in_minutes: Option<u32>,
    ) -> anyhow::Result<DeviceInvite> {
        decode(
            self.request(reqwest::Method::POST, "/v1/admin/invites")
                .json(&CreateInviteRequest {
                    profile,
                    expires_in_minutes,
                })
                .send()
                .await?,
        )
        .await
    }

    async fn devices(&self) -> anyhow::Result<Vec<DeviceView>> {
        decode(
            self.request(reqwest::Method::GET, "/v1/devices")
                .send()
                .await?,
        )
        .await
    }

    async fn revoke_device(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        ensure_ok(
            self.request(reqwest::Method::DELETE, &format!("/v1/devices/{id}"))
                .send()
                .await?,
        )
        .await
    }

    fn events_request(&self) -> anyhow::Result<tokio_tungstenite::tungstenite::http::Request<()>> {
        let mut url = Url::parse(&self.base_url)?;
        url.set_scheme(if url.scheme() == "https" { "wss" } else { "ws" })
            .map_err(|()| anyhow!("unsupported server URL scheme"))?;
        url.set_path("/v1/events");
        url.set_query(None);
        let mut request = url.as_str().into_client_request()?;
        let token = self
            .token
            .read()
            .expect("session token lock poisoned")
            .clone();
        request
            .headers_mut()
            .insert("Authorization", format!("Bearer {token}").parse()?);
        Ok(request)
    }
}

async fn obtain_session(
    client: &reqwest::Client,
    base_url: &str,
    auth: &AuthMethod,
) -> anyhow::Result<String> {
    match auth {
        AuthMethod::Development { profile } => {
            let session: DevSession = decode(
                client
                    .post(format!("{base_url}/v1/dev/session"))
                    .json(&DevSessionRequest {
                        profile: profile.clone(),
                    })
                    .send()
                    .await?,
            )
            .await?;
            if session.protocol_version != PROTOCOL_VERSION {
                bail!(
                    "protocol mismatch: daemon is v{PROTOCOL_VERSION}, server is v{}",
                    session.protocol_version
                );
            }
            Ok(session.token)
        }
        AuthMethod::Device {
            device_id,
            device_token,
        } => {
            let session: DeviceSession = decode(
                client
                    .post(format!("{base_url}/v1/sessions"))
                    .json(&DeviceSessionRequest {
                        device_id: *device_id,
                        device_token: device_token.clone(),
                        protocol_version: PROTOCOL_VERSION,
                    })
                    .send()
                    .await?,
            )
            .await?;
            if session.protocol_version != PROTOCOL_VERSION {
                bail!(
                    "protocol mismatch: daemon is v{PROTOCOL_VERSION}, server is v{}",
                    session.protocol_version
                );
            }
            Ok(session.token)
        }
    }
}

async fn decode<T: serde::de::DeserializeOwned>(response: reqwest::Response) -> anyhow::Result<T> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        if let Ok(error) = serde_json::from_str::<wisp_protocol::ProtocolError>(&body) {
            bail!("{}: {}", error.code, error.message);
        }
        bail!("server returned {status}: {body}");
    }
    Ok(response.json().await?)
}

async fn ensure_ok(response: reqwest::Response) -> anyhow::Result<()> {
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let body = response.text().await.unwrap_or_default();
    if status == StatusCode::BAD_REQUEST
        && let Ok(error) = serde_json::from_str::<wisp_protocol::ProtocolError>(&body)
    {
        bail!("{}: {}", error.code, error.message);
    }
    bail!("server returned {status}: {body}")
}

struct Daemon {
    profile: String,
    api: ServerApi,
    state: RwLock<Snapshot>,
    seq: AtomicU64,
    events: broadcast::Sender<DaemonEnvelope>,
    media: MediaManager,
    media_reconcile: Mutex<()>,
    ptt_operation: Mutex<()>,
    ptt_lease_tx: watch::Sender<Option<Instant>>,
    ptt_lease_duration: Duration,
    shortcut: ShortcutManager,
    shortcut_operation: Mutex<()>,
    media_enabled: bool,
}

impl Daemon {
    fn new(
        profile: String,
        api: ServerApi,
        snapshot: Snapshot,
        media: MediaManager,
        media_enabled: bool,
        ptt_lease_duration: Duration,
        shortcut: ShortcutManager,
    ) -> Self {
        let seq = snapshot.seq;
        let (events, _) = broadcast::channel(256);
        let (ptt_lease_tx, _) = watch::channel::<Option<Instant>>(None);
        Self {
            profile,
            api,
            state: RwLock::new(snapshot),
            seq: AtomicU64::new(seq),
            events,
            media,
            media_reconcile: Mutex::new(()),
            ptt_operation: Mutex::new(()),
            ptt_lease_tx,
            ptt_lease_duration,
            shortcut,
            shortcut_operation: Mutex::new(()),
            media_enabled,
        }
    }

    async fn envelope_snapshot(&self) -> DaemonEnvelope {
        DaemonEnvelope::Snapshot {
            v: PROTOCOL_VERSION,
            snapshot: Box::new(self.state.read().await.clone()),
        }
    }

    async fn merge_server_snapshot(&self, mut incoming: Snapshot, event_name: &str) {
        {
            let current = self.state.read().await;
            incoming.self_state.muted = current.self_state.muted;
            incoming.self_state.deafened = current.self_state.deafened;
            incoming.self_state.sharing = current.self_state.sharing;
            incoming.self_state.push_to_talk = current.self_state.push_to_talk.clone();
            incoming.self_state.media = current.self_state.media.clone();
            incoming.last_invite = current.last_invite.clone();
            incoming.self_state.connection = if incoming.self_state.hangout_id.is_none() {
                ConnectionState::Available
            } else if current.self_state.media.livekit_connected {
                ConnectionState::Connected
            } else if incoming.self_state.hangout_id == current.self_state.hangout_id {
                current.self_state.connection
            } else {
                ConnectionState::Joining
            };
        }
        let seq = self.next_seq(incoming.seq);
        incoming.seq = seq;
        *self.state.write().await = incoming.clone();
        self.emit(event_name, json!({"snapshot": incoming}), seq);
    }

    fn next_seq(&self, minimum: u64) -> u64 {
        let mut current = self.seq.load(Ordering::Relaxed);
        loop {
            let next = current.max(minimum).saturating_add(1);
            match self.seq.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return next,
                Err(actual) => current = actual,
            }
        }
    }

    fn emit(&self, name: &str, payload: Value, seq: u64) {
        let _ = self.events.send(DaemonEnvelope::Event {
            v: PROTOCOL_VERSION,
            seq,
            name: name.into(),
            payload,
        });
    }

    async fn set_connection(&self, connection: ConnectionState, message: Option<&str>) {
        let snapshot = {
            let mut state = self.state.write().await;
            state.self_state.connection = connection;
            let seq = self.next_seq(state.seq);
            state.seq = seq;
            state.clone()
        };
        self.emit(
            "connection_state_changed",
            json!({"snapshot": snapshot, "message": message}),
            snapshot.seq,
        );
    }

    async fn refresh(&self, event_name: &str) -> anyhow::Result<()> {
        let snapshot = self.api.snapshot().await?;
        self.merge_server_snapshot(snapshot, event_name).await;
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    async fn reconcile_media(&self) -> anyhow::Result<()> {
        let _reconcile = self.media_reconcile.lock().await;
        let hangout_id = self.state.read().await.self_state.hangout_id;
        if !self.media_enabled {
            let connection = if hangout_id.is_some() {
                ConnectionState::Connected
            } else {
                ConnectionState::Available
            };
            self.set_media_state(MediaState::default(), connection, None)
                .await;
            return Ok(());
        }
        let Some(hangout_id) = hangout_id else {
            self.release_push_to_talk("push_to_talk_released").await;
            let (audio, camera, video) = {
                let state = self.state.read().await;
                (
                    state.self_state.media.audio.clone(),
                    CameraState {
                        devices: state.self_state.media.camera.devices.clone(),
                        selected_device_id: state
                            .self_state
                            .media
                            .camera
                            .selected_device_id
                            .clone(),
                        ..CameraState::default()
                    },
                    state.self_state.media.video.clone(),
                )
            };
            self.media.disconnect().await;
            self.set_media_state(
                MediaState {
                    audio,
                    camera,
                    video,
                    ..MediaState::default()
                },
                ConnectionState::Available,
                None,
            )
            .await;
            return Ok(());
        };
        if self.media.is_connected_to(hangout_id).await {
            return Ok(());
        }

        self.release_push_to_talk("push_to_talk_released").await;
        let (muted, deafened) = {
            let state = self.state.read().await;
            (
                effective_muted(
                    state.self_state.muted,
                    state.self_state.deafened,
                    &state.self_state.push_to_talk,
                ),
                state.self_state.deafened,
            )
        };
        self.set_connection(ConnectionState::Joining, None).await;
        let result = async {
            let credentials = self.api.livekit_token().await?;
            self.media
                .connect(hangout_id, credentials, muted, deafened)
                .await
        }
        .await;
        match result {
            Ok(connected) => {
                let (camera, video) = {
                    let state = self.state.read().await;
                    (
                        state.self_state.media.camera.clone(),
                        state.self_state.media.video.clone(),
                    )
                };
                let mut media = MediaState {
                    livekit_connected: true,
                    microphone_published: true,
                    e2ee_enabled: connected.e2ee_enabled,
                    microphone: Some(connected.microphone),
                    speaker: Some(connected.speaker),
                    audio: connected.audio,
                    remote_audio_participants: connected.remote_audio_participants,
                    remote_muted_participants: connected.remote_muted_participants,
                    remote_videos: connected.remote_videos,
                    camera,
                    video,
                    ..MediaState::default()
                };
                refresh_legacy_video_state(&mut media);
                self.set_media_state(media, ConnectionState::Connected, None)
                    .await;
                Ok(())
            }
            Err(error) => {
                let (code, message) = describe_media_failure(&error);
                let audio = self.state.read().await.self_state.media.audio.clone();
                self.set_media_state(
                    MediaState {
                        error_code: Some(code),
                        error: Some(message.clone()),
                        audio,
                        ..MediaState::default()
                    },
                    ConnectionState::Failed,
                    Some(&message),
                )
                .await;
                Err(error)
            }
        }
    }

    async fn set_media_state(
        &self,
        media: MediaState,
        connection: ConnectionState,
        message: Option<&str>,
    ) {
        let snapshot = {
            let mut state = self.state.write().await;
            if !media.livekit_connected {
                state.self_state.sharing = false;
            }
            state.self_state.media = media;
            state.self_state.connection = connection;
            let seq = self.next_seq(state.seq);
            state.seq = seq;
            state.clone()
        };
        self.emit(
            "media_state_changed",
            json!({"snapshot": snapshot, "message": message}),
            snapshot.seq,
        );
    }

    async fn update_media_state(
        &self,
        connection: Option<ConnectionState>,
        event_name: &str,
        update: impl FnOnce(&mut MediaState),
    ) {
        let snapshot = {
            let mut state = self.state.write().await;
            update(&mut state.self_state.media);
            if let Some(connection) = connection {
                state.self_state.connection = connection;
            }
            let seq = self.next_seq(state.seq);
            state.seq = seq;
            state.clone()
        };
        self.emit(event_name, json!({"snapshot": snapshot}), snapshot.seq);
    }

    async fn apply_audio_inventory(
        &self,
        mut inventory: AudioInventory,
        event_name: &str,
    ) -> wisp_protocol::AudioState {
        let snapshot = {
            let mut state = self.state.write().await;
            inventory.state.input_level = state.self_state.media.audio.input_level;
            let next_error = inventory
                .error
                .as_ref()
                .map(|error| format!("Audio device error: {error}"));
            let clears_audio_error = next_error.is_none()
                && state.self_state.media.error_code.as_deref() == Some("audio_device");
            let changes_error = next_error.as_ref().is_some_and(|error| {
                state.self_state.media.error_code.as_deref() != Some("audio_device")
                    || state.self_state.media.error.as_ref() != Some(error)
            });
            let changed = state.self_state.media.audio != inventory.state
                || state.self_state.media.microphone != inventory.microphone
                || state.self_state.media.speaker != inventory.speaker
                || clears_audio_error
                || changes_error;
            if !changed {
                return inventory.state;
            }
            state.self_state.media.audio = inventory.state.clone();
            state.self_state.media.microphone = inventory.microphone;
            state.self_state.media.speaker = inventory.speaker;
            if let Some(error) = next_error {
                state.self_state.media.error_code = Some("audio_device".into());
                state.self_state.media.error = Some(error);
            } else if state.self_state.media.error_code.as_deref() == Some("audio_device") {
                state.self_state.media.error_code = None;
                state.self_state.media.error = None;
            }
            let seq = self.next_seq(state.seq);
            state.seq = seq;
            state.clone()
        };
        self.emit(event_name, json!({"snapshot": snapshot}), snapshot.seq);
        inventory.state
    }

    async fn update_manual_mute(&self, requested: Option<bool>) -> Value {
        let _operation = self.ptt_operation.lock().await;
        let (muted, undeafened, push_to_talk, effective) = {
            let mut state = self.state.write().await;
            let was_deafened = state.self_state.deafened;
            let (muted, deafened) =
                mute_transition(state.self_state.muted, state.self_state.deafened, requested);
            state.self_state.muted = muted;
            state.self_state.deafened = deafened;
            if muted {
                state.self_state.push_to_talk.active = false;
                self.ptt_lease_tx.send_replace(None);
            }
            let push_to_talk = state.self_state.push_to_talk.clone();
            let effective = effective_muted(muted, deafened, &push_to_talk);
            if effective {
                clear_local_speaker(&mut state, &self.profile);
            }
            (muted, was_deafened && !deafened, push_to_talk, effective)
        };
        if undeafened {
            self.media.set_deafened(false).await;
        }
        self.media.set_muted(effective).await;
        self.publish_current("self_state_changed").await;
        json!({
            "muted": muted,
            "effective_muted": effective,
            "undeafened": undeafened,
            "push_to_talk": push_to_talk,
        })
    }

    async fn update_deafened(&self, requested: Option<bool>) -> Value {
        let _operation = self.ptt_operation.lock().await;
        let (muted, deafened, push_to_talk, effective) = {
            let mut state = self.state.write().await;
            let deafened = requested.unwrap_or(!state.self_state.deafened);
            let (muted, deafened) = deafen_transition(state.self_state.muted, deafened);
            state.self_state.muted = muted;
            state.self_state.deafened = deafened;
            if deafened {
                state.self_state.push_to_talk.active = false;
                self.ptt_lease_tx.send_replace(None);
            }
            let push_to_talk = state.self_state.push_to_talk.clone();
            let effective = effective_muted(muted, deafened, &push_to_talk);
            if effective {
                clear_local_speaker(&mut state, &self.profile);
            }
            (muted, deafened, push_to_talk, effective)
        };
        self.media.set_muted(effective).await;
        self.media.set_deafened(deafened).await;
        self.publish_current("self_state_changed").await;
        json!({
            "muted": muted,
            "deafened": deafened,
            "effective_muted": effective,
            "push_to_talk": push_to_talk,
        })
    }

    async fn set_push_to_talk(&self, enabled: bool) -> Value {
        let _operation = self.ptt_operation.lock().await;
        let (muted, push_to_talk, effective, changed) = {
            let mut state = self.state.write().await;
            let changed = state.self_state.push_to_talk.enabled != enabled
                || state.self_state.push_to_talk.active;
            state.self_state.push_to_talk.enabled = enabled;
            state.self_state.push_to_talk.active = false;
            self.ptt_lease_tx.send_replace(None);
            let muted = state.self_state.muted;
            let push_to_talk = state.self_state.push_to_talk.clone();
            let effective = effective_muted(muted, state.self_state.deafened, &push_to_talk);
            if effective {
                clear_local_speaker(&mut state, &self.profile);
            }
            (muted, push_to_talk, effective, changed)
        };
        if changed {
            self.media.set_muted(effective).await;
            self.publish_current("push_to_talk_changed").await;
        }
        voice_gate_value(muted, &push_to_talk, effective, false)
    }

    async fn set_push_to_talk_shortcut(&self, shortcut: Option<&str>) -> anyhow::Result<Value> {
        let _operation = self.shortcut_operation.lock().await;
        let update = self.shortcut.set(shortcut).await?;
        {
            let mut state = self.state.write().await;
            state
                .self_state
                .push_to_talk
                .shortcut
                .clone_from(&update.shortcut);
            state.self_state.push_to_talk.shortcut_backend = Some(update.backend.clone());
            state
                .self_state
                .push_to_talk
                .shortcut_replaced
                .clone_from(&update.replaced);
        }
        self.publish_current("push_to_talk_shortcut_changed").await;
        Ok(json!({
            "shortcut": update.shortcut,
            "shortcut_backend": update.backend,
            "replaced": update.replaced,
        }))
    }

    async fn press_push_to_talk(&self) -> anyhow::Result<Value> {
        let _operation = self.ptt_operation.lock().await;
        let (muted, push_to_talk, effective, changed, blocked) = {
            let mut state = self.state.write().await;
            if !state.self_state.push_to_talk.enabled {
                bail!("push-to-talk is disabled");
            }
            if state.self_state.hangout_id.is_none() || !state.self_state.media.livekit_connected {
                bail!("push-to-talk requires an active voice connection");
            }
            let muted = state.self_state.muted;
            let blocked = muted;
            let changed = !muted && !state.self_state.push_to_talk.active;
            state.self_state.push_to_talk.active = !muted;
            if muted {
                self.ptt_lease_tx.send_replace(None);
            } else {
                self.ptt_lease_tx
                    .send_replace(Some(Instant::now() + self.ptt_lease_duration));
            }
            let push_to_talk = state.self_state.push_to_talk.clone();
            let effective = effective_muted(muted, state.self_state.deafened, &push_to_talk);
            (muted, push_to_talk, effective, changed, blocked)
        };
        if changed {
            self.media.set_muted(effective).await;
            self.publish_current("push_to_talk_pressed").await;
        }
        Ok(voice_gate_value(muted, &push_to_talk, effective, blocked))
    }

    async fn release_push_to_talk(&self, event_name: &str) -> Value {
        let _operation = self.ptt_operation.lock().await;
        let (muted, push_to_talk, effective, changed) = {
            let mut state = self.state.write().await;
            let changed = state.self_state.push_to_talk.active;
            state.self_state.push_to_talk.active = false;
            self.ptt_lease_tx.send_replace(None);
            let muted = state.self_state.muted;
            let push_to_talk = state.self_state.push_to_talk.clone();
            let effective = effective_muted(muted, state.self_state.deafened, &push_to_talk);
            if effective {
                clear_local_speaker(&mut state, &self.profile);
            }
            (muted, push_to_talk, effective, changed)
        };
        if changed {
            self.media.set_muted(effective).await;
            self.publish_current(event_name).await;
        }
        voice_gate_value(muted, &push_to_talk, effective, false)
    }

    async fn expire_push_to_talk(&self, deadline: Instant) {
        let _operation = self.ptt_operation.lock().await;
        if *self.ptt_lease_tx.borrow() != Some(deadline) {
            return;
        }
        self.ptt_lease_tx.send_replace(None);
        let (effective, changed) = {
            let mut state = self.state.write().await;
            let changed = state.self_state.push_to_talk.active;
            state.self_state.push_to_talk.active = false;
            let effective = effective_muted(
                state.self_state.muted,
                state.self_state.deafened,
                &state.self_state.push_to_talk,
            );
            if effective {
                clear_local_speaker(&mut state, &self.profile);
            }
            (effective, changed)
        };
        if changed {
            self.media.set_muted(effective).await;
            self.publish_current("push_to_talk_expired").await;
        }
    }

    async fn handle_command(&self, command: CommandEnvelope) -> Vec<DaemonEnvelope> {
        if let Err(code) = command.validate() {
            return vec![DaemonEnvelope::failure(
                command.id,
                code,
                "unsupported IPC envelope",
            )];
        }
        let result = self.run_command(&command).await;
        match result {
            Ok(value) => {
                let mut envelopes = vec![DaemonEnvelope::success(command.id, value)];
                if command.name == "hello" {
                    envelopes.insert(
                        0,
                        DaemonEnvelope::Hello {
                            v: PROTOCOL_VERSION,
                            daemon: env!("CARGO_PKG_VERSION").into(),
                            profile: self.profile.clone(),
                        },
                    );
                    envelopes.push(self.envelope_snapshot().await);
                }
                envelopes
            }
            Err(error) => {
                warn!(command = %command.name, %error, "IPC command failed");
                vec![DaemonEnvelope::failure(
                    command.id,
                    "command_failed",
                    error.to_string(),
                )]
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    async fn run_command(&self, command: &CommandEnvelope) -> anyhow::Result<Option<Value>> {
        match command.name.as_str() {
            "hello" => Ok(None),
            "status" => Ok(Some(serde_json::to_value(self.state.read().await.clone())?)),
            "set_muted" => {
                let muted = boolean_arg(&command.args, "muted")?;
                Ok(Some(self.update_manual_mute(Some(muted)).await))
            }
            "toggle_muted" => Ok(Some(self.update_manual_mute(None).await)),
            "set_push_to_talk" => {
                let enabled = boolean_arg(&command.args, "enabled")?;
                Ok(Some(self.set_push_to_talk(enabled).await))
            }
            "set_push_to_talk_shortcut" => {
                let shortcut = match command.args.get("shortcut") {
                    None | Some(Value::Null) => None,
                    Some(Value::String(shortcut)) => Some(shortcut.as_str()),
                    Some(_) => bail!("shortcut must be a string or null"),
                };
                Ok(Some(self.set_push_to_talk_shortcut(shortcut).await?))
            }
            "push_to_talk_press" => Ok(Some(self.press_push_to_talk().await?)),
            "push_to_talk_release" => Ok(Some(
                self.release_push_to_talk("push_to_talk_released").await,
            )),
            "set_deafened" => Ok(Some(
                self.update_deafened(Some(boolean_arg(&command.args, "deafened")?))
                    .await,
            )),
            "toggle_deafened" => Ok(Some(self.update_deafened(None).await)),
            "refresh_audio_devices"
            | "set_input_device"
            | "set_output_device"
            | "set_audio_preset" => self.audio_command(command).await,
            "refresh_video_devices"
            | "set_camera_device"
            | "set_video_quality"
            | "set_video_codec" => self.video_command(command).await,
            "set_presence" => {
                let presence = command
                    .args
                    .get("presence")
                    .and_then(Value::as_str)
                    .context("presence is required")?
                    .parse()
                    .map_err(anyhow::Error::msg)?;
                self.api.set_presence(presence).await?;
                self.refresh("presence_changed").await?;
                Ok(Some(json!({"presence": presence})))
            }
            "join_friend" => self.join_friend_command(&command.args).await,
            "open_direct" => {
                let friend = string_arg(&command.args, "friend")?;
                let conversation = self.api.create_direct(friend).await?;
                self.refresh("conversation_changed").await?;
                Ok(Some(serde_json::to_value(conversation)?))
            }
            "send_direct" => {
                let friend = string_arg(&command.args, "friend")?;
                let text = string_arg(&command.args, "text")?;
                let conversation = self.api.create_direct(friend).await?;
                self.api.send_message(conversation.id.clone(), text).await?;
                self.refresh("message_created").await?;
                Ok(Some(json!({"conversation_id": conversation.id})))
            }
            "send_message" => {
                let conversation_id = string_arg(&command.args, "conversation_id")?;
                let text = string_arg(&command.args, "text")?;
                self.api.send_message(conversation_id, text).await?;
                self.refresh("message_created").await?;
                Ok(None)
            }
            "mark_conversation_read" => {
                let conversation_id = string_arg(&command.args, "conversation_id")?;
                self.api.mark_conversation_read(conversation_id).await?;
                self.refresh("conversation_read").await?;
                Ok(None)
            }
            "join_spot" => {
                let spot_id = string_arg(&command.args, "spot_id")?;
                self.set_connection(ConnectionState::Joining, None).await;
                self.api.join_spot(spot_id).await?;
                self.refresh("hangout_changed").await?;
                self.reconcile_media().await?;
                Ok(None)
            }
            "create_invite" => {
                let profile = string_arg(&command.args, "profile")?;
                let expires = command
                    .args
                    .get("expires_in_minutes")
                    .and_then(Value::as_u64)
                    .map(u32::try_from)
                    .transpose()
                    .context("expires_in_minutes is too large")?;
                let invite = self.api.create_invite(profile, expires).await?;
                {
                    let mut state = self.state.write().await;
                    state.last_invite = Some(invite.clone());
                }
                self.publish_current("invite_created").await;
                Ok(Some(serde_json::to_value(invite)?))
            }
            "list_devices" => {
                let devices = self.api.devices().await?;
                self.refresh("devices_changed").await?;
                Ok(Some(serde_json::to_value(devices)?))
            }
            "revoke_device" => {
                let id = string_arg(&command.args, "device_id")?.parse()?;
                self.api.revoke_device(id).await?;
                self.refresh("devices_changed").await?;
                Ok(None)
            }
            "respond_knock" => self.respond_knock_command(&command.args).await,
            "join_hangout" => {
                let id = string_arg(&command.args, "hangout_id")?.parse()?;
                self.set_connection(ConnectionState::Joining, None).await;
                self.api.join_hangout(id).await?;
                self.refresh("hangout_changed").await?;
                self.reconcile_media().await?;
                Ok(None)
            }
            "leave" => {
                self.api.leave().await?;
                self.refresh("hangout_changed").await?;
                self.reconcile_media().await?;
                Ok(None)
            }
            "open_surface" => self.surface_command(true).await,
            "close_surface" => self.surface_command(false).await,
            "watch_video" => self.watch_video_command(&command.args).await,
            "share" => self.screen_share_command(&command.args).await,
            "camera" => self.camera_command(&command.args).await,
            _ => bail!("unknown command: {}", command.name),
        }
    }

    async fn audio_command(&self, command: &CommandEnvelope) -> anyhow::Result<Option<Value>> {
        if !self.media_enabled {
            bail!("media is disabled");
        }
        let (inventory, event_name) = match command.name.as_str() {
            "refresh_audio_devices" => (
                self.media.refresh_audio_devices().await,
                "audio_devices_changed",
            ),
            "set_input_device" => {
                let id = opaque_string_arg(&command.args, "id")?;
                (
                    self.media.select_input_device(&id).await?,
                    "audio_input_changed",
                )
            }
            "set_output_device" => {
                let id = opaque_string_arg(&command.args, "id")?;
                (
                    self.media.select_output_device(&id).await?,
                    "audio_output_changed",
                )
            }
            "set_audio_preset" => {
                let preset = string_arg(&command.args, "preset")?
                    .parse::<AudioPreset>()
                    .map_err(anyhow::Error::msg)?;
                (
                    self.media.set_audio_preset(preset).await?,
                    "audio_preset_changed",
                )
            }
            _ => unreachable!("only audio commands are dispatched here"),
        };
        let audio = self.apply_audio_inventory(inventory, event_name).await;
        Ok(Some(serde_json::to_value(audio)?))
    }

    async fn video_command(&self, command: &CommandEnvelope) -> anyhow::Result<Option<Value>> {
        if !self.media_enabled {
            bail!("media is disabled");
        }
        match command.name.as_str() {
            "refresh_video_devices" => {
                let refreshed = self.media.refresh_camera_devices().await?;
                self.set_local_media(
                    |state| merge_camera_inventory(&mut state.self_state.media.camera, refreshed),
                    "camera_devices_changed",
                )
                .await;
                Ok(Some(serde_json::to_value(
                    &self.state.read().await.self_state.media.camera,
                )?))
            }
            "set_camera_device" => {
                let id = opaque_string_arg(&command.args, "id")?;
                let refreshed = self.media.select_camera_device(&id).await?;
                self.set_local_media(
                    |state| merge_camera_inventory(&mut state.self_state.media.camera, refreshed),
                    "camera_device_changed",
                )
                .await;
                Ok(Some(serde_json::to_value(
                    &self.state.read().await.self_state.media.camera,
                )?))
            }
            "set_video_quality" => {
                let quality = string_arg(&command.args, "quality")?
                    .parse::<VideoQualityPreset>()
                    .map_err(anyhow::Error::msg)?;
                let settings = self.media.set_video_quality(quality).await?;
                self.set_local_media(
                    |state| state.self_state.media.video = settings.clone(),
                    "video_quality_changed",
                )
                .await;
                Ok(Some(serde_json::to_value(settings)?))
            }
            "set_video_codec" => {
                let codec = string_arg(&command.args, "codec")?
                    .parse::<VideoCodecPreference>()
                    .map_err(anyhow::Error::msg)?;
                let settings = self.media.set_video_codec(codec).await?;
                self.set_local_media(
                    |state| state.self_state.media.video = settings.clone(),
                    "video_codec_changed",
                )
                .await;
                Ok(Some(serde_json::to_value(settings)?))
            }
            _ => unreachable!("only video settings commands are dispatched here"),
        }
    }

    async fn screen_share_command(&self, args: &Value) -> anyhow::Result<Option<Value>> {
        if !self.media_enabled {
            bail!("media is disabled");
        }
        let enabled = args.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        if !enabled {
            self.media.stop_screen_share().await;
            self.set_local_media(
                |state| {
                    state.self_state.sharing = false;
                    state.self_state.media.screen_share = ScreenShareState::default();
                },
                "screen_share_stopped",
            )
            .await;
            return Ok(Some(json!({"sharing": false})));
        }

        self.set_local_media(
            |state| {
                state.self_state.sharing = false;
                state.self_state.media.screen_share.starting = true;
                state.self_state.media.screen_share.error = None;
            },
            "screen_share_starting",
        )
        .await;
        match self.media.start_screen_share().await {
            Ok(info) => {
                let result = serde_json::to_value(&info.state)?;
                self.set_local_media(
                    |state| {
                        state.self_state.sharing = true;
                        state.self_state.media.screen_share = info.state;
                    },
                    "screen_share_started",
                )
                .await;
                Ok(Some(result))
            }
            Err(error) => {
                let message = error.to_string();
                self.set_local_media(
                    |state| {
                        state.self_state.sharing = false;
                        state.self_state.media.screen_share.starting = false;
                        state.self_state.media.screen_share.active = false;
                        state.self_state.media.screen_share.error = Some(message.clone());
                    },
                    "screen_share_failed",
                )
                .await;
                Err(error)
            }
        }
    }

    async fn camera_command(&self, args: &Value) -> anyhow::Result<Option<Value>> {
        if !self.media_enabled {
            bail!("media is disabled");
        }
        let enabled = args.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        if !enabled {
            self.media.stop_camera().await;
            self.set_local_media(
                |state| {
                    let devices = state.self_state.media.camera.devices.clone();
                    let selected_device_id =
                        state.self_state.media.camera.selected_device_id.clone();
                    state.self_state.media.camera = CameraState {
                        devices,
                        selected_device_id,
                        ..CameraState::default()
                    };
                },
                "camera_stopped",
            )
            .await;
            return Ok(Some(json!({"camera": false})));
        }
        self.set_local_media(
            |state| {
                state.self_state.media.camera.starting = true;
                state.self_state.media.camera.error = None;
            },
            "camera_starting",
        )
        .await;
        match self.media.start_camera().await {
            Ok(info) => {
                let result = serde_json::to_value(&info.state)?;
                self.set_local_media(
                    |state| state.self_state.media.camera = info.state,
                    "camera_started",
                )
                .await;
                Ok(Some(result))
            }
            Err(error) => {
                let message = error.to_string();
                self.set_local_media(
                    |state| {
                        state.self_state.media.camera.starting = false;
                        state.self_state.media.camera.active = false;
                        state.self_state.media.camera.error = Some(message.clone());
                    },
                    "camera_failed",
                )
                .await;
                Err(error)
            }
        }
    }

    async fn join_friend_command(&self, args: &Value) -> anyhow::Result<Option<Value>> {
        let friend = string_arg(args, "friend")?;
        self.set_connection(ConnectionState::Joining, None).await;
        match self.api.join_friend(friend).await {
            Ok(JoinFriendResult::Joined { .. }) => {
                self.refresh("hangout_changed").await?;
                self.reconcile_media().await?;
                Ok(None)
            }
            Ok(result @ JoinFriendResult::KnockSent { .. }) => {
                self.set_connection(ConnectionState::Available, None).await;
                Ok(Some(serde_json::to_value(result)?))
            }
            Err(error) => {
                self.set_connection(ConnectionState::Available, Some(&error.to_string()))
                    .await;
                Err(error)
            }
        }
    }

    async fn respond_knock_command(&self, args: &Value) -> anyhow::Result<Option<Value>> {
        let knock_id = string_arg(args, "knock_id")?.parse()?;
        let response = serde_json::from_value(
            args.get("response")
                .cloned()
                .context("response is required")?,
        )?;
        let result = self.api.respond_knock(knock_id, response).await?;
        self.refresh("knock_responded").await?;
        if matches!(result, RespondKnockResult::Accepted { .. }) {
            self.reconcile_media().await?;
        }
        Ok(Some(serde_json::to_value(result)?))
    }

    async fn surface_command(&self, open: bool) -> anyhow::Result<Option<Value>> {
        let target = if open {
            self.media
                .first_video_target()
                .await
                .context("no remote video is available")?
        } else {
            let active = self
                .state
                .read()
                .await
                .self_state
                .media
                .remote_videos
                .iter()
                .find(|video| video.surface_open || video.subscribed)
                .map(|video| video.target.clone());
            match active {
                Some(target) => target,
                None => self
                    .media
                    .first_video_target()
                    .await
                    .context("no remote video is available")?,
            }
        };
        self.set_video_watched(target.clone(), open).await?;
        Ok(Some(json!({
            "surface": if open { "opening" } else { "closing" },
            "participant": target.participant,
            "source": target.source,
        })))
    }

    async fn watch_video_command(&self, args: &Value) -> anyhow::Result<Option<Value>> {
        let participant = opaque_string_arg(args, "participant")?;
        let source = string_arg(args, "source")?
            .parse::<VideoSource>()
            .map_err(anyhow::Error::msg)?;
        let open = args.get("open").and_then(Value::as_bool).unwrap_or(true);
        let target = RemoteVideoTarget {
            participant,
            source,
        };
        self.set_video_watched(target.clone(), open).await?;
        Ok(Some(json!({
            "participant": target.participant,
            "source": target.source,
            "watched": open,
        })))
    }

    async fn set_video_watched(&self, target: RemoteVideoTarget, open: bool) -> anyhow::Result<()> {
        if !self.media_enabled {
            bail!("media is disabled");
        }
        if open {
            self.media.open_surface(target.clone()).await?;
        } else {
            self.media.close_surface(target.clone()).await?;
        }
        self.update_media_state(None, "video_watch_changed", |media| {
            if let Some(video) = find_remote_video_mut(media, &target) {
                video.subscribed = open;
                if !open {
                    video.surface_open = false;
                    video.surface_visible = false;
                    video.requested_quality = None;
                }
            }
            refresh_legacy_video_state(media);
        })
        .await;
        Ok(())
    }

    async fn set_local_media(&self, update: impl FnOnce(&mut Snapshot), event_name: &str) {
        {
            let mut state = self.state.write().await;
            update(&mut state);
        }
        self.publish_current(event_name).await;
    }

    async fn publish_current(&self, event_name: &str) {
        let snapshot = {
            let mut state = self.state.write().await;
            let seq = self.next_seq(state.seq);
            state.seq = seq;
            state.clone()
        };
        self.emit(event_name, json!({"snapshot": snapshot}), snapshot.seq);
    }
}

fn boolean_arg(args: &Value, name: &str) -> anyhow::Result<bool> {
    args.get(name)
        .and_then(Value::as_bool)
        .with_context(|| format!("{name} must be a boolean"))
}

fn string_arg(args: &Value, name: &str) -> anyhow::Result<String> {
    args.get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .with_context(|| format!("{name} must be a non-empty string"))
}

fn opaque_string_arg(args: &Value, name: &str) -> anyhow::Result<String> {
    args.get(name)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .with_context(|| format!("{name} must be a string"))
}

fn mute_transition(current: bool, deafened: bool, requested: Option<bool>) -> (bool, bool) {
    let muted = requested.unwrap_or(!current);
    if deafened && !muted {
        (false, false)
    } else {
        (muted, deafened)
    }
}

fn deafen_transition(current_muted: bool, deafened: bool) -> (bool, bool) {
    (current_muted || deafened, deafened)
}

fn effective_muted(muted: bool, deafened: bool, push_to_talk: &PushToTalkState) -> bool {
    muted || deafened || (push_to_talk.enabled && !push_to_talk.active)
}

fn clear_local_speaker(snapshot: &mut Snapshot, profile: &str) {
    snapshot
        .self_state
        .media
        .active_speakers
        .retain(|name| name != profile);
    snapshot.self_state.media.audio.input_level = 0;
}

fn update_remote_mute_state(media: &mut MediaState, participant: &str, muted: bool) {
    if muted {
        if !media
            .remote_muted_participants
            .iter()
            .any(|name| name == participant)
        {
            media.remote_muted_participants.push(participant.into());
            media.remote_muted_participants.sort();
        }
        media.active_speakers.retain(|name| name != participant);
    } else {
        media
            .remote_muted_participants
            .retain(|name| name != participant);
    }
}

fn voice_gate_value(
    muted: bool,
    push_to_talk: &PushToTalkState,
    effective: bool,
    blocked: bool,
) -> Value {
    json!({
        "muted": muted,
        "effective_muted": effective,
        "push_to_talk": push_to_talk,
        "blocked_by_mute": blocked,
    })
}

fn runtime_socket_path() -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(
        || {
            PathBuf::from(format!(
                "/run/user/{}",
                std::process::Command::new("id")
                    .arg("-u")
                    .output()
                    .ok()
                    .and_then(|o| String::from_utf8(o.stdout).ok())
                    .unwrap_or_default()
                    .trim()
            ))
        },
        PathBuf::from,
    );
    runtime.join("wisp/wispd.sock")
}

async fn bind_socket(path: &Path) -> anyhow::Result<UnixListener> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    if tokio::fs::try_exists(path).await? {
        if UnixStream::connect(path).await.is_ok() {
            bail!("wispd is already listening at {}", path.display());
        }
        tokio::fs::remove_file(path)
            .await
            .context("remove stale wispd socket")?;
    }
    let listener = UnixListener::bind(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).await?;
    }
    Ok(listener)
}

async fn serve_client(stream: UnixStream, daemon: Arc<Daemon>) -> anyhow::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    let mut events = daemon.events.subscribe();
    loop {
        tokio::select! {
            line = lines.next_line() => match line? {
                Some(line) => {
                    let command = match serde_json::from_str::<CommandEnvelope>(&line) {
                        Ok(command) => command,
                        Err(error) => {
                            write_envelope(&mut writer, &DaemonEnvelope::failure("", "invalid_json", error.to_string())).await?;
                            continue;
                        }
                    };
                    for envelope in daemon.handle_command(command).await { write_envelope(&mut writer, &envelope).await?; }
                }
                None => return Ok(()),
            },
            event = events.recv() => match event {
                Ok(event) => write_envelope(&mut writer, &event).await?,
                Err(broadcast::error::RecvError::Lagged(_)) => write_envelope(&mut writer, &daemon.envelope_snapshot().await).await?,
                Err(broadcast::error::RecvError::Closed) => return Ok(()),
            }
        }
    }
}

async fn write_envelope(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    envelope: &DaemonEnvelope,
) -> anyhow::Result<()> {
    let mut bytes = serde_json::to_vec(envelope)?;
    bytes.push(b'\n');
    writer.write_all(&bytes).await?;
    Ok(())
}

async fn synchronize_server(daemon: Arc<Daemon>) {
    let mut attempt = 0_u32;
    loop {
        let request = match daemon.api.events_request() {
            Ok(request) => request,
            Err(error) => {
                error!(%error, "invalid server events request");
                return;
            }
        };
        match connect_async(request).await {
            Ok((stream, _)) => {
                attempt = 0;
                info!("connected to wisp-server events");
                if let Err(error) = daemon.refresh("server_reconnected").await {
                    warn!(%error, "initial server refresh failed");
                }
                if let Err(error) = daemon.reconcile_media().await {
                    warn!(%error, "media reconciliation failed after server reconnect");
                }
                let (_, mut incoming) = stream.split();
                while let Some(message) = incoming.next().await {
                    match message {
                        Ok(message) if message.is_text() => {
                            if let Ok(event) = serde_json::from_str::<ServerEvent>(
                                message.to_text().unwrap_or_default(),
                            ) {
                                debug!(name = %event.name, seq = event.seq, "server event");
                            }
                            if let Err(error) = daemon.refresh("server_state_changed").await {
                                warn!(%error, "server snapshot refresh failed");
                                break;
                            }
                            if let Err(error) = daemon.reconcile_media().await {
                                warn!(%error, "media reconciliation failed after server event");
                            }
                        }
                        Ok(message) if message.is_close() => break,
                        Ok(_) => {}
                        Err(error) => {
                            warn!(%error, "server event stream failed");
                            break;
                        }
                    }
                }
            }
            Err(error) => {
                warn!(%error, "cannot connect to wisp-server events");
                if let Err(renew_error) = daemon.api.renew_session().await {
                    warn!(%renew_error, "cannot renew wisp-server session");
                }
            }
        }
        daemon
            .set_connection(
                ConnectionState::Reconnecting,
                Some("coordination server unavailable"),
            )
            .await;
        attempt = attempt.saturating_add(1).min(6);
        let base = 250 * 2_u64.pow(attempt);
        let jitter = u64::from(daemon.profile.bytes().fold(0_u8, u8::wrapping_add)) * 3;
        tokio::time::sleep(Duration::from_millis((base + jitter).min(20_000))).await;
    }
}

#[allow(clippy::too_many_lines)]
async fn synchronize_media_events(
    daemon: Arc<Daemon>,
    mut events: mpsc::UnboundedReceiver<MediaEvent>,
) {
    while let Some(event) = events.recv().await {
        let generation = match &event {
            MediaEvent::Reconnecting { generation }
            | MediaEvent::Reconnected { generation }
            | MediaEvent::Disconnected { generation, .. }
            | MediaEvent::AudioSubscribed { generation, .. }
            | MediaEvent::AudioUnsubscribed { generation, .. }
            | MediaEvent::RemoteMuteChanged { generation, .. }
            | MediaEvent::AudioFrames { generation, .. }
            | MediaEvent::InputLevel { generation, .. }
            | MediaEvent::ActiveSpeakers { generation, .. }
            | MediaEvent::VideoAvailable { generation, .. }
            | MediaEvent::VideoUnavailable { generation, .. }
            | MediaEvent::VideoSubscribed { generation, .. }
            | MediaEvent::VideoUnsubscribed { generation, .. }
            | MediaEvent::VideoFrames { generation, .. }
            | MediaEvent::ScreenShareFrames { generation, .. }
            | MediaEvent::ScreenShareStopped { generation, .. }
            | MediaEvent::CameraFrames { generation, .. }
            | MediaEvent::CameraStopped { generation, .. }
            | MediaEvent::VideoViewerChanged { generation, .. }
            | MediaEvent::VideoViewerDisconnected { generation, .. } => Some(*generation),
            MediaEvent::SurfaceOpened { .. }
            | MediaEvent::SurfaceClosed { .. }
            | MediaEvent::SurfaceVisibilityChanged { .. }
            | MediaEvent::SurfaceResized { .. }
            | MediaEvent::SurfaceRendered { .. }
            | MediaEvent::SurfaceError { .. } => None,
        };
        if generation.is_some_and(|generation| generation != daemon.media.generation()) {
            continue;
        }
        match event {
            MediaEvent::Reconnecting { .. } => {
                warn!("LiveKit media reconnecting");
                daemon
                    .update_media_state(
                        Some(ConnectionState::Reconnecting),
                        "media_reconnecting",
                        |media| {
                            media.livekit_connected = false;
                            media.active_speakers.clear();
                            media.audio.input_level = 0;
                        },
                    )
                    .await;
            }
            MediaEvent::Reconnected { .. } => {
                info!("LiveKit media reconnected");
                daemon
                    .update_media_state(
                        Some(ConnectionState::Connected),
                        "media_reconnected",
                        |media| {
                            media.livekit_connected = true;
                            media.error_code = None;
                            media.error = None;
                        },
                    )
                    .await;
            }
            track_event @ (MediaEvent::AudioSubscribed { .. }
            | MediaEvent::AudioUnsubscribed { .. }
            | MediaEvent::RemoteMuteChanged { .. }
            | MediaEvent::AudioFrames { .. }
            | MediaEvent::InputLevel { .. }
            | MediaEvent::ActiveSpeakers { .. }
            | MediaEvent::VideoAvailable { .. }
            | MediaEvent::VideoUnavailable { .. }
            | MediaEvent::VideoSubscribed { .. }
            | MediaEvent::VideoUnsubscribed { .. }
            | MediaEvent::VideoFrames { .. }
            | MediaEvent::ScreenShareFrames { .. }
            | MediaEvent::CameraFrames { .. }
            | MediaEvent::VideoViewerChanged { .. }
            | MediaEvent::VideoViewerDisconnected { .. }) => {
                synchronize_track_event(&daemon, track_event).await;
            }
            MediaEvent::ScreenShareStopped { error, .. } => {
                daemon.media.stop_screen_share().await;
                if let Some(message) = &error {
                    warn!(%message, "screen share stopped unexpectedly");
                }
                daemon
                    .set_local_media(
                        |state| {
                            state.self_state.sharing = false;
                            state.self_state.media.screen_share = ScreenShareState::default();
                            state.self_state.media.screen_share.error = error;
                        },
                        "screen_share_stopped",
                    )
                    .await;
            }
            MediaEvent::CameraStopped { error, .. } => {
                daemon.media.stop_camera().await;
                if let Some(message) = &error {
                    warn!(%message, "camera stopped unexpectedly");
                }
                daemon
                    .set_local_media(
                        |state| {
                            let devices = state.self_state.media.camera.devices.clone();
                            let selected_device_id =
                                state.self_state.media.camera.selected_device_id.clone();
                            state.self_state.media.camera = CameraState {
                                devices,
                                selected_device_id,
                                error,
                                ..CameraState::default()
                            };
                        },
                        "camera_stopped",
                    )
                    .await;
            }
            surface_event @ (MediaEvent::SurfaceOpened { .. }
            | MediaEvent::SurfaceClosed { .. }
            | MediaEvent::SurfaceVisibilityChanged { .. }
            | MediaEvent::SurfaceResized { .. }
            | MediaEvent::SurfaceRendered { .. }
            | MediaEvent::SurfaceError { .. }) => {
                synchronize_surface_event(&daemon, surface_event).await;
            }
            MediaEvent::Disconnected { reason, .. } => {
                warn!(%reason, "LiveKit media disconnected; reconnecting");
                daemon
                    .update_media_state(
                        Some(ConnectionState::Reconnecting),
                        "media_disconnected",
                        |media| {
                            media.livekit_connected = false;
                            media.remote_audio_participants.clear();
                            media.remote_muted_participants.clear();
                            media.remote_video_participants.clear();
                            media.remote_videos.clear();
                            media.active_speakers.clear();
                            media.audio.input_level = 0;
                            media.error_code = Some("livekit_disconnected".into());
                            media.error = Some(format!("LiveKit disconnected: {reason}"));
                        },
                    )
                    .await;
                tokio::time::sleep(Duration::from_millis(500)).await;
                if let Err(error) = daemon.reconcile_media().await {
                    warn!(%error, "LiveKit media reconnect failed");
                }
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
async fn synchronize_track_event(daemon: &Daemon, event: MediaEvent) {
    match event {
        MediaEvent::AudioSubscribed { participant, .. } => {
            info!(%participant, "subscribed to remote audio");
            daemon
                .update_media_state(None, "remote_audio_subscribed", |media| {
                    media.last_audio_from = Some(participant.clone());
                    if !media.remote_audio_participants.contains(&participant) {
                        media.remote_audio_participants.push(participant);
                        media.remote_audio_participants.sort();
                    }
                })
                .await;
        }
        MediaEvent::AudioUnsubscribed { participant, .. } => {
            info!(%participant, "unsubscribed from remote audio");
            daemon
                .update_media_state(None, "remote_audio_unsubscribed", |media| {
                    media
                        .remote_audio_participants
                        .retain(|name| name != &participant);
                    media
                        .remote_muted_participants
                        .retain(|name| name != &participant);
                    media.active_speakers.retain(|name| name != &participant);
                })
                .await;
        }
        MediaEvent::RemoteMuteChanged {
            participant, muted, ..
        } => {
            info!(%participant, muted, "remote microphone mute changed");
            daemon
                .update_media_state(None, "remote_mute_changed", |media| {
                    update_remote_mute_state(media, &participant, muted);
                })
                .await;
        }
        MediaEvent::AudioFrames {
            participant, total, ..
        } => {
            daemon
                .update_media_state(None, "remote_audio_received", |media| {
                    media.received_audio_frames = total;
                    media.last_audio_from = Some(participant);
                })
                .await;
        }
        MediaEvent::InputLevel { level, .. } => {
            daemon
                .update_media_state(None, "audio_input_level_changed", |media| {
                    media.audio.input_level = level;
                })
                .await;
        }
        MediaEvent::ActiveSpeakers { mut speakers, .. } => {
            let effective = {
                let state = daemon.state.read().await;
                effective_muted(
                    state.self_state.muted,
                    state.self_state.deafened,
                    &state.self_state.push_to_talk,
                )
            };
            if effective {
                speakers.retain(|name| name != &daemon.profile);
            }
            daemon
                .update_media_state(None, "active_speakers_changed", |media| {
                    media.active_speakers = speakers;
                })
                .await;
        }
        video_event @ (MediaEvent::VideoAvailable { .. }
        | MediaEvent::VideoUnavailable { .. }
        | MediaEvent::VideoSubscribed { .. }
        | MediaEvent::VideoUnsubscribed { .. }
        | MediaEvent::VideoFrames { .. }) => {
            synchronize_video_track_event(daemon, video_event).await;
        }
        MediaEvent::ScreenShareFrames { total, .. } => {
            daemon
                .update_media_state(None, "screen_share_frame_published", |media| {
                    media.screen_share.published_frames = total;
                })
                .await;
        }
        MediaEvent::CameraFrames { total, .. } => {
            daemon
                .update_media_state(None, "camera_frame_published", |media| {
                    media.camera.published_frames = total;
                })
                .await;
        }
        viewer_event @ (MediaEvent::VideoViewerChanged { .. }
        | MediaEvent::VideoViewerDisconnected { .. }) => {
            synchronize_local_video_viewers(daemon, viewer_event).await;
        }
        _ => unreachable!("only remote track events are dispatched here"),
    }
}

async fn synchronize_local_video_viewers(daemon: &Daemon, event: MediaEvent) {
    match event {
        MediaEvent::VideoViewerChanged {
            source,
            participant,
            watching,
            ..
        } => {
            info!(%participant, %source, watching, "local video viewer changed");
            daemon
                .update_media_state(None, "video_viewers_changed", |media| {
                    let viewers = match source {
                        VideoSource::Camera => &mut media.camera.viewers,
                        VideoSource::ScreenShare => &mut media.screen_share.viewers,
                    };
                    viewers.retain(|viewer| viewer != &participant);
                    if watching {
                        viewers.push(participant);
                        viewers.sort();
                        viewers.dedup();
                    }
                })
                .await;
        }
        MediaEvent::VideoViewerDisconnected { participant, .. } => {
            daemon
                .update_media_state(None, "video_viewers_changed", |media| {
                    media.camera.viewers.retain(|viewer| viewer != &participant);
                    media
                        .screen_share
                        .viewers
                        .retain(|viewer| viewer != &participant);
                })
                .await;
        }
        _ => unreachable!("only local video viewer events are dispatched here"),
    }
}

async fn synchronize_video_track_event(daemon: &Daemon, event: MediaEvent) {
    match event {
        MediaEvent::VideoAvailable {
            target,
            mime_type,
            simulcasted,
            ..
        } => {
            info!(participant = %target.participant, source = %target.source, "remote video available");
            daemon
                .update_media_state(None, "remote_video_available", |media| {
                    if find_remote_video_mut(media, &target).is_none() {
                        media.remote_videos.push(RemoteVideoState {
                            target,
                            mime_type,
                            simulcasted,
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
                        sort_remote_videos(media);
                    }
                    refresh_legacy_video_state(media);
                })
                .await;
        }
        MediaEvent::VideoUnavailable { target, .. } => {
            info!(participant = %target.participant, source = %target.source, "remote video unavailable");
            daemon
                .update_media_state(None, "remote_video_unavailable", |media| {
                    media.remote_videos.retain(|video| video.target != target);
                    refresh_legacy_video_state(media);
                })
                .await;
        }
        MediaEvent::VideoSubscribed { target, .. } => {
            info!(participant = %target.participant, source = %target.source, "subscribed to remote video");
            daemon
                .update_media_state(None, "remote_video_subscribed", |media| {
                    if let Some(video) = find_remote_video_mut(media, &target) {
                        video.subscribed = true;
                        video.requested_quality = Some("high".into());
                    }
                    refresh_legacy_video_state(media);
                })
                .await;
        }
        MediaEvent::VideoUnsubscribed { target, .. } => {
            info!(participant = %target.participant, source = %target.source, "unsubscribed from remote video");
            daemon
                .update_media_state(None, "remote_video_unsubscribed", |media| {
                    if let Some(video) = find_remote_video_mut(media, &target) {
                        video.subscribed = false;
                        video.requested_quality = None;
                    }
                    refresh_legacy_video_state(media);
                })
                .await;
        }
        MediaEvent::VideoFrames {
            target,
            total,
            track_total,
            width,
            height,
            ..
        } => {
            daemon
                .update_media_state(None, "remote_video_received", |media| {
                    if let Some(video) = find_remote_video_mut(media, &target) {
                        video.received_frames = track_total;
                        video.width = Some(width);
                        video.height = Some(height);
                    }
                    media.received_video_frames = total;
                    refresh_legacy_video_state(media);
                })
                .await;
        }
        _ => unreachable!("only remote video events are dispatched here"),
    }
}

async fn synchronize_audio_devices(daemon: Arc<Daemon>) {
    if !daemon.media_enabled {
        return;
    }
    let inventory = daemon.media.refresh_audio_devices().await;
    daemon
        .apply_audio_inventory(inventory, "audio_devices_changed")
        .await;

    let mut interval = tokio::time::interval(Duration::from_secs(2));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    interval.tick().await;
    loop {
        interval.tick().await;
        if daemon.media.is_active().await {
            let inventory = daemon.media.refresh_audio_devices().await;
            daemon
                .apply_audio_inventory(inventory, "audio_devices_changed")
                .await;
        }
    }
}

async fn synchronize_video_devices(daemon: Arc<Daemon>) {
    if !daemon.media_enabled {
        return;
    }
    let mut interval = tokio::time::interval(Duration::from_secs(3));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let Ok(refreshed) = daemon.media.refresh_camera_devices().await else {
            continue;
        };
        let changed = {
            let state = daemon.state.read().await;
            state.self_state.media.camera.devices != refreshed.devices
                || state.self_state.media.camera.selected_device_id != refreshed.selected_device_id
        };
        if changed {
            daemon
                .set_local_media(
                    |state| merge_camera_inventory(&mut state.self_state.media.camera, refreshed),
                    "camera_devices_changed",
                )
                .await;
        }
    }
}

fn merge_camera_inventory(current: &mut CameraState, refreshed: CameraState) {
    current.devices = refreshed.devices;
    current.selected_device_id = refreshed.selected_device_id;
}

fn find_remote_video_mut<'a>(
    media: &'a mut MediaState,
    target: &RemoteVideoTarget,
) -> Option<&'a mut RemoteVideoState> {
    media
        .remote_videos
        .iter_mut()
        .find(|video| &video.target == target)
}

fn sort_remote_videos(media: &mut MediaState) {
    media.remote_videos.sort_by(|left, right| {
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
}

fn refresh_legacy_video_state(media: &mut MediaState) {
    let mut participants = media
        .remote_videos
        .iter()
        .map(|video| video.target.participant.clone())
        .collect::<Vec<_>>();
    participants.sort();
    participants.dedup();
    media.remote_video_participants = participants;
    media.surface_open = media.remote_videos.iter().any(|video| video.surface_open);
    media.rendered_video_frames = media
        .remote_videos
        .iter()
        .map(|video| video.rendered_frames)
        .sum();
    if let Some(video) = media
        .remote_videos
        .iter()
        .filter(|video| video.received_frames > 0)
        .max_by_key(|video| video.received_frames)
    {
        media.last_video_from = Some(video.target.participant.clone());
        media.video_width = video.width;
        media.video_height = video.height;
    } else {
        media.last_video_from = None;
        media.video_width = None;
        media.video_height = None;
    }
}

async fn synchronize_push_to_talk_lease(daemon: Arc<Daemon>) {
    let mut leases = daemon.ptt_lease_tx.subscribe();
    loop {
        let deadline = *leases.borrow_and_update();
        let Some(deadline) = deadline else {
            if leases.changed().await.is_err() {
                return;
            }
            continue;
        };
        tokio::select! {
            result = leases.changed() => {
                if result.is_err() {
                    return;
                }
            }
            () = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {
                daemon.expire_push_to_talk(deadline).await;
            }
        }
    }
}

async fn synchronize_surface_event(daemon: &Daemon, event: MediaEvent) {
    match event {
        MediaEvent::SurfaceOpened { target } => {
            daemon
                .update_media_state(None, "video_surface_opened", |media| {
                    if let Some(video) = find_remote_video_mut(media, &target) {
                        video.surface_open = true;
                        video.surface_visible = true;
                        video.requested_quality = Some("high".into());
                        video.error = None;
                    }
                    media.surface_error = None;
                    refresh_legacy_video_state(media);
                })
                .await;
        }
        MediaEvent::SurfaceClosed { target } => {
            let _ = daemon.media.close_surface(target.clone()).await;
            daemon
                .update_media_state(None, "video_surface_closed", |media| {
                    if let Some(video) = find_remote_video_mut(media, &target) {
                        video.surface_open = false;
                        video.surface_visible = false;
                        video.subscribed = false;
                        video.requested_quality = None;
                    }
                    refresh_legacy_video_state(media);
                })
                .await;
        }
        MediaEvent::SurfaceVisibilityChanged { target, visible } => {
            daemon.media.set_surface_visible(&target, visible).await;
            daemon
                .update_media_state(None, "video_surface_visibility_changed", |media| {
                    if let Some(video) = find_remote_video_mut(media, &target) {
                        video.surface_visible = visible;
                        video.requested_quality =
                            Some(if visible { "high" } else { "paused" }.into());
                    }
                    refresh_legacy_video_state(media);
                })
                .await;
        }
        MediaEvent::SurfaceResized {
            target,
            width,
            height,
        } => {
            let quality = daemon
                .media
                .set_surface_dimensions(&target, width, height)
                .await;
            daemon
                .update_media_state(None, "video_surface_quality_changed", |media| {
                    if let Some(video) = find_remote_video_mut(media, &target) {
                        video.requested_quality = Some(quality.into());
                    }
                })
                .await;
        }
        MediaEvent::SurfaceRendered { target, total } => {
            daemon
                .update_media_state(None, "video_surface_rendered", |media| {
                    if let Some(video) = find_remote_video_mut(media, &target) {
                        video.rendered_frames = total;
                    }
                    refresh_legacy_video_state(media);
                })
                .await;
        }
        MediaEvent::SurfaceError { target, message } => {
            warn!(%message, "video surface unavailable");
            daemon
                .update_media_state(None, "video_surface_error", |media| {
                    if let Some(target) = target
                        && let Some(video) = find_remote_video_mut(media, &target)
                    {
                        video.surface_open = false;
                        video.surface_visible = false;
                        video.error = Some(message.clone());
                    }
                    media.surface_error = Some(message);
                    refresh_legacy_video_state(media);
                })
                .await;
        }
        _ => unreachable!("only surface events are dispatched here"),
    }
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("wispd manifest is nested under the repository root")
        .to_path_buf()
}

fn installed_ui_script() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("WISP_UI_BIN") {
        return Some(PathBuf::from(path));
    }
    let bin_root = std::env::var_os("XDG_BIN_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/bin")))?;
    let path = bin_root.join("wisp-ui");
    path.is_file().then_some(path)
}

async fn run_ui_script(script: &Path, selector: Option<&Path>, arguments: &[String]) -> bool {
    let mut command = tokio::process::Command::new(script);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(path) = selector {
        command.env("WISP_QUICKSHELL_PATH", path);
    }
    match command.status().await {
        Ok(status) => status.success(),
        Err(error) => {
            debug!(script = %script.display(), %error, "could not run Wisp UI control command");
            false
        }
    }
}

async fn control_ui(arguments: Vec<String>) {
    let root = repository_root();
    let repository_script = root.join("scripts/wisp-ui.sh");
    let repository_ui = root.join("quickshell/app");
    if let Some(selector) = std::env::var_os("WISP_QUICKSHELL_PATH").map(PathBuf::from)
        && repository_script.is_file()
        && run_ui_script(&repository_script, Some(&selector), &arguments).await
    {
        return;
    }
    if let Some(script) = installed_ui_script()
        && run_ui_script(&script, None, &arguments).await
    {
        return;
    }
    if repository_script.is_file() {
        let _ = run_ui_script(&repository_script, Some(&repository_ui), &arguments).await;
    }
}

async fn quit_all_ui_instances() {
    let root = repository_root();
    let arguments = vec!["quit".into()];
    let repository_script = root.join("scripts/wisp-ui.sh");
    let repository_ui = root.join("quickshell/app");
    if let Some(selector) = std::env::var_os("WISP_QUICKSHELL_PATH").map(PathBuf::from)
        && repository_script.is_file()
    {
        let _ = run_ui_script(&repository_script, Some(&selector), &arguments).await;
    }
    if repository_script.is_file() {
        let _ = run_ui_script(&repository_script, Some(&repository_ui), &arguments).await;
    }
    if let Some(script) = installed_ui_script() {
        let _ = run_ui_script(&script, None, &arguments).await;
    }
}

async fn next_tray_action(
    receiver: &mut Option<tokio::sync::mpsc::UnboundedReceiver<TrayAction>>,
) -> Option<TrayAction> {
    match receiver {
        Some(receiver) => receiver.recv().await,
        None => std::future::pending().await,
    }
}

async fn synchronize_tray_state(daemon: Arc<Daemon>, handle: ksni::Handle<tray::WispTray>) {
    let mut events = daemon.events.subscribe();
    let mut previous = None;
    loop {
        let current = {
            let state = daemon.state.read().await;
            tray::TrayState::new(
                (state.self_state.muted, state.self_state.deafened),
                (
                    state.self_state.media.screen_share.active,
                    state.self_state.media.camera.active,
                ),
                state
                    .conversations
                    .iter()
                    .map(|conversation| conversation.unread_count)
                    .sum(),
            )
        };
        if previous != Some(current) {
            if handle
                .update(|tray| tray.set_state(current))
                .await
                .is_none()
            {
                return;
            }
            previous = Some(current);
        }
        match events.recv().await {
            Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => return,
        }
    }
}

async fn handle_tray_action(action: TrayAction, daemon: &Arc<Daemon>) -> bool {
    match action {
        TrayAction::Activate { x, y } => {
            tokio::spawn(control_ui(vec![
                "panel".into(),
                "activate".into(),
                x.to_string(),
                y.to_string(),
            ]));
        }
        TrayAction::Show => {
            tokio::spawn(control_ui(vec!["panel".into(), "open".into()]));
        }
        TrayAction::Hide => {
            tokio::spawn(control_ui(vec!["panel".into(), "hide".into()]));
        }
        TrayAction::OpenApp => {
            tokio::spawn(control_ui(vec!["app".into(), "open".into()]));
        }
        TrayAction::SetAnchor(anchor) => {
            tokio::spawn(control_ui(vec![
                "panel".into(),
                "anchor".into(),
                anchor.into(),
            ]));
        }
        TrayAction::ToggleMuted => {
            daemon.update_manual_mute(None).await;
        }
        TrayAction::ToggleDeafened => {
            daemon.update_deafened(None).await;
        }
        TrayAction::ToggleShare => {
            let enabled = !daemon
                .state
                .read()
                .await
                .self_state
                .media
                .screen_share
                .active;
            if let Err(error) = daemon
                .screen_share_command(&json!({"enabled": enabled}))
                .await
            {
                warn!(%error, "tray screen sharing action failed");
            }
        }
        TrayAction::ToggleCamera => {
            let enabled = !daemon.state.read().await.self_state.media.camera.active;
            if let Err(error) = daemon.camera_command(&json!({"enabled": enabled})).await {
                warn!(%error, "tray camera action failed");
            }
        }
        TrayAction::Exit => {
            info!("exit requested from system tray");
            quit_all_ui_instances().await;
            return false;
        }
    }
    true
}

async fn handle_connecting_tray_action(
    action: TrayAction,
    audio_state: &mut (bool, bool),
    audio_changed: &mut bool,
    tray_handle: Option<&ksni::Handle<tray::WispTray>>,
) -> bool {
    match action {
        TrayAction::Activate { x, y } => {
            tokio::spawn(control_ui(vec![
                "panel".into(),
                "activate".into(),
                x.to_string(),
                y.to_string(),
            ]));
        }
        TrayAction::Show => {
            tokio::spawn(control_ui(vec!["panel".into(), "open".into()]));
        }
        TrayAction::Hide => {
            tokio::spawn(control_ui(vec!["panel".into(), "hide".into()]));
        }
        TrayAction::OpenApp => {
            tokio::spawn(control_ui(vec!["app".into(), "open".into()]));
        }
        TrayAction::SetAnchor(anchor) => {
            tokio::spawn(control_ui(vec![
                "panel".into(),
                "anchor".into(),
                anchor.into(),
            ]));
        }
        TrayAction::ToggleMuted => {
            *audio_state = mute_transition(audio_state.0, audio_state.1, None);
            *audio_changed = true;
        }
        TrayAction::ToggleDeafened => {
            *audio_state = deafen_transition(audio_state.0, !audio_state.1);
            *audio_changed = true;
        }
        TrayAction::ToggleShare | TrayAction::ToggleCamera => {
            debug!("video publishing is unavailable while connecting to the server");
        }
        TrayAction::Exit => {
            info!("exit requested from system tray while connecting");
            quit_all_ui_instances().await;
            return false;
        }
    }
    if let Some(handle) = tray_handle {
        let (muted, deafened) = *audio_state;
        let _ = handle
            .update(|tray_service| {
                tray_service.set_state(tray::TrayState::new((muted, deafened), (false, false), 0));
            })
            .await;
    }
    true
}

async fn connect_with_tray(
    server_url: String,
    profile: &str,
    tray_actions: &mut Option<tokio::sync::mpsc::UnboundedReceiver<TrayAction>>,
    tray_handle: Option<&ksni::Handle<tray::WispTray>>,
    audio_state: &mut (bool, bool),
    audio_changed: &mut bool,
) -> anyhow::Result<Option<(ServerApi, Snapshot)>> {
    let mut attempt = 0_u32;
    loop {
        let connection = ServerApi::connect(server_url.clone(), profile);
        tokio::pin!(connection);
        let result = loop {
            tokio::select! {
                result = &mut connection => break result,
                result = tokio::signal::ctrl_c() => {
                    result?;
                    info!("shutting down while connecting");
                    return Ok(None);
                }
                action = next_tray_action(tray_actions) => {
                    let Some(action) = action else {
                        *tray_actions = None;
                        continue;
                    };
                    if !handle_connecting_tray_action(
                        action,
                        audio_state,
                        audio_changed,
                        tray_handle,
                    ).await {
                        return Ok(None);
                    }
                }
            }
        };
        match result {
            Ok(connected) => return Ok(Some(connected)),
            Err(error) => warn!(%error, %server_url, "cannot establish initial server session"),
        }

        attempt = attempt.saturating_add(1).min(6);
        let delay = tokio::time::sleep(Duration::from_millis(250 * 2_u64.pow(attempt)));
        tokio::pin!(delay);
        loop {
            tokio::select! {
                () = &mut delay => break,
                result = tokio::signal::ctrl_c() => {
                    result?;
                    info!("shutting down while waiting to reconnect");
                    return Ok(None);
                }
                action = next_tray_action(tray_actions) => {
                    let Some(action) = action else {
                        *tray_actions = None;
                        continue;
                    };
                    if !handle_connecting_tray_action(
                        action,
                        audio_state,
                        audio_changed,
                        tray_handle,
                    ).await {
                        return Ok(None);
                    }
                }
            }
        }
    }
}

async fn start_connected_daemon(
    args: Args,
    api: ServerApi,
    mut snapshot: Snapshot,
    connecting_audio: Option<(bool, bool)>,
) -> Arc<Daemon> {
    if let Some((muted, deafened)) = connecting_audio {
        snapshot.self_state.muted = muted;
        snapshot.self_state.deafened = deafened;
    }
    if snapshot.self_state.deafened {
        snapshot.self_state.muted = true;
    }
    snapshot.self_state.connection = if snapshot.connected() && !args.disable_media {
        ConnectionState::Joining
    } else if snapshot.connected() {
        ConnectionState::Connected
    } else {
        ConnectionState::Available
    };
    let shortcut = ShortcutManager::from_environment();
    snapshot.self_state.push_to_talk.shortcut = shortcut.load_shortcut().await;
    snapshot.self_state.push_to_talk.shortcut_backend = shortcut.backend().map(str::to_owned);
    let e2ee_key = std::env::var("WISP_E2EE_KEY")
        .ok()
        .filter(|key| !key.is_empty());
    let (media, media_events) =
        MediaManager::new(!args.disable_media && !args.disable_surfaces, e2ee_key);
    snapshot.self_state.media.video = media.video_settings();
    let daemon = Arc::new(Daemon::new(
        args.profile,
        api,
        snapshot,
        media,
        !args.disable_media,
        Duration::from_millis(args.ptt_lease_ms.max(100)),
        shortcut,
    ));
    tokio::spawn(synchronize_server(daemon.clone()));
    tokio::spawn(synchronize_media_events(daemon.clone(), media_events));
    tokio::spawn(synchronize_audio_devices(daemon.clone()));
    tokio::spawn(synchronize_video_devices(daemon.clone()));
    tokio::spawn(synchronize_push_to_talk_lease(daemon.clone()));
    let initial_media = daemon.clone();
    tokio::spawn(async move {
        if let Err(error) = initial_media.reconcile_media().await {
            warn!(%error, "initial media connection failed");
        }
    });
    daemon
}

fn describe_media_failure(error: &anyhow::Error) -> (String, String) {
    let detail = error.to_string();
    let (code, label) = if detail.contains("publish microphone") {
        ("microphone_publication", "Microphone publication failed")
    } else if detail.contains("LiveKit") {
        ("livekit_connection", "LiveKit connection failed")
    } else if detail.contains("microphone")
        || detail.contains("speaker")
        || detail.contains("audio processing")
    {
        ("audio_device", "Audio device error")
    } else {
        ("media_connection", "Media connection failed")
    };
    (code.into(), format!("{label}: {detail}"))
}

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "wispd=info".into()))
        .init();
    let args = Args::parse();
    if !args.disable_media
        && std::env::var_os("WISP_DEVICE_ID").is_some()
        && std::env::var_os("WISP_E2EE_KEY").is_none()
    {
        bail!("WISP_E2EE_KEY is required for device-authenticated media");
    }
    let socket_path = args.socket.clone().unwrap_or_else(runtime_socket_path);
    let listener = bind_socket(&socket_path).await?;
    let mut connecting_audio_state = (false, false);
    let mut connecting_audio_changed = false;
    let (mut tray_actions, tray_handle) = if std::env::var_os("WISP_DISABLE_TRAY").is_some() {
        (None, None)
    } else {
        match tray::spawn(tray::TrayState::default()).await {
            Ok((receiver, handle)) => (Some(receiver), Some(handle)),
            Err(error) => {
                warn!(%error, "system tray is unavailable; continuing without a tray icon");
                (None, None)
            }
        }
    };
    info!(socket = %socket_path.display(), "wispd ready; connecting to server");
    let Some((api, snapshot)) = connect_with_tray(
        args.server_url.clone(),
        &args.profile,
        &mut tray_actions,
        tray_handle.as_ref(),
        &mut connecting_audio_state,
        &mut connecting_audio_changed,
    )
    .await
    .with_context(|| format!("connect profile {} to wisp-server", args.profile))?
    else {
        drop(listener);
        if let Err(error) = tokio::fs::remove_file(&socket_path).await {
            warn!(%error, "could not remove IPC socket");
        }
        return Ok(());
    };
    let daemon = start_connected_daemon(
        args,
        api,
        snapshot,
        connecting_audio_changed.then_some(connecting_audio_state),
    )
    .await;

    let initial_tray_state = {
        let state = daemon.state.read().await;
        tray::TrayState::new(
            (state.self_state.muted, state.self_state.deafened),
            (
                state.self_state.media.screen_share.active,
                state.self_state.media.camera.active,
            ),
            state
                .conversations
                .iter()
                .map(|conversation| conversation.unread_count)
                .sum(),
        )
    };
    if let Some(handle) = tray_handle.as_ref() {
        let _ = handle
            .update(|tray| tray.set_state(initial_tray_state))
            .await;
    }
    if let Some(handle) = tray_handle {
        tokio::spawn(synchronize_tray_state(daemon.clone(), handle));
    }
    info!(socket = %socket_path.display(), "wispd ready");

    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            result = listener.accept() => {
                let (stream, _) = result?;
                let daemon = daemon.clone();
                tokio::spawn(async move { if let Err(error) = serve_client(stream, daemon).await { debug!(%error, "IPC client disconnected"); } });
            }
            result = &mut shutdown => {
                result?;
                info!("shutting down");
                break;
            }
            action = next_tray_action(&mut tray_actions) => {
                let Some(action) = action else {
                    tray_actions = None;
                    continue;
                };
                if !handle_tray_action(action, &daemon).await {
                    break;
                }
            }
        }
    }
    daemon.media.disconnect().await;
    daemon.media.shutdown_surface();
    drop(listener);
    if let Err(error) = tokio::fs::remove_file(&socket_path).await {
        warn!(%error, "could not remove IPC socket");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        deafen_transition, describe_media_failure, effective_muted, mute_transition,
        update_remote_mute_state,
    };
    use wisp_protocol::{MediaState, PushToTalkState};

    #[test]
    fn remote_mute_state_is_sorted_deduplicated_and_not_speaking() {
        let mut media = MediaState {
            active_speakers: vec!["Tyler".into(), "Jared".into()],
            ..MediaState::default()
        };

        update_remote_mute_state(&mut media, "Tyler", true);
        update_remote_mute_state(&mut media, "Aaron", true);
        update_remote_mute_state(&mut media, "Tyler", true);
        assert_eq!(media.remote_muted_participants, ["Aaron", "Tyler"]);
        assert_eq!(media.active_speakers, ["Jared"]);

        update_remote_mute_state(&mut media, "Tyler", false);
        assert_eq!(media.remote_muted_participants, ["Aaron"]);
    }

    #[test]
    fn media_failures_have_stable_codes_and_clear_labels() {
        let (code, message) =
            describe_media_failure(&anyhow::anyhow!("no microphone is available"));
        assert_eq!(code, "audio_device");
        assert!(message.starts_with("Audio device error:"));

        let (code, message) =
            describe_media_failure(&anyhow::anyhow!("connect to LiveKit room wisp-test"));
        assert_eq!(code, "livekit_connection");
        assert!(message.starts_with("LiveKit connection failed:"));
    }

    #[test]
    fn manual_mute_has_precedence_over_push_to_talk() {
        let disabled = PushToTalkState::default();
        let waiting = PushToTalkState {
            enabled: true,
            active: false,
            ..PushToTalkState::default()
        };
        let talking = PushToTalkState {
            enabled: true,
            active: true,
            ..PushToTalkState::default()
        };

        assert!(!effective_muted(false, false, &disabled));
        assert!(effective_muted(false, false, &waiting));
        assert!(!effective_muted(false, false, &talking));
        assert!(effective_muted(true, false, &talking));
        assert!(effective_muted(false, true, &talking));
    }

    #[test]
    fn unmuting_while_deafened_clears_both_states() {
        assert_eq!(mute_transition(true, true, Some(false)), (false, false));
        assert_eq!(mute_transition(true, true, None), (false, false));
        assert_eq!(mute_transition(true, false, Some(false)), (false, false));
        assert_eq!(mute_transition(false, true, Some(true)), (true, true));
    }

    #[test]
    fn headset_toggle_keeps_microphone_muted_after_undeafen() {
        assert_eq!(deafen_transition(false, true), (true, true));
        assert_eq!(deafen_transition(true, false), (true, false));
    }
}
