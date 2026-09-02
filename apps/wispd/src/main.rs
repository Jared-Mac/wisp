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
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::{Mutex, RwLock, broadcast, mpsc, watch},
};
use tokio_tungstenite::connect_async;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;
use url::Url;
use wisp_protocol::{
    AudioPreset, CommandEnvelope, ConnectionState, DaemonEnvelope, DevSession, DevSessionRequest,
    JoinFriendRequest, JoinFriendResult, JoinHangoutRequest, KnockResponse, LiveKitTokenResponse,
    MediaState, PROTOCOL_VERSION, Presence, PushToTalkState, RespondKnockRequest,
    RespondKnockResult, ServerEvent, SetPresenceRequest, Snapshot,
};

use crate::media::{AudioInventory, MediaEvent, MediaManager};
use crate::shortcut::ShortcutManager;
use crate::tray::TrayAction;

#[derive(Debug, Parser)]
#[command(about = "Wisp's persistent desktop daemon")]
struct Args {
    #[arg(long, default_value = "Jared")]
    profile: String,
    #[arg(long, env = "WISP_SERVER_URL", default_value = "http://127.0.0.1:8787")]
    server_url: String,
    #[arg(long, env = "WISP_SOCKET")]
    socket: Option<PathBuf>,
    #[arg(long, env = "WISP_DISABLE_MEDIA")]
    disable_media: bool,
    #[arg(long, env = "WISP_PTT_LEASE_MS", default_value_t = 30_000)]
    ptt_lease_ms: u64,
}

#[derive(Clone)]
struct ServerApi {
    client: reqwest::Client,
    base_url: String,
    token: String,
}

impl ServerApi {
    async fn connect(base_url: String, profile: &str) -> anyhow::Result<(Self, Snapshot)> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;
        let response = client
            .post(format!("{base_url}/v1/dev/session"))
            .json(&DevSessionRequest {
                profile: profile.into(),
            })
            .send()
            .await?;
        let session: DevSession = decode(response).await?;
        let api = Self {
            client,
            base_url,
            token: session.token,
        };
        let snapshot = api.snapshot().await?;
        Ok((api, snapshot))
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, format!("{}{}", self.base_url, path))
            .bearer_auth(&self.token)
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

    fn events_url(&self) -> anyhow::Result<Url> {
        let mut url = Url::parse(&self.base_url)?;
        url.set_scheme(if url.scheme() == "https" { "wss" } else { "ws" })
            .map_err(|()| anyhow!("unsupported server URL scheme"))?;
        url.set_path("/v1/events");
        url.query_pairs_mut()
            .clear()
            .append_pair("token", &self.token);
        Ok(url)
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
            let audio = self.state.read().await.self_state.media.audio.clone();
            self.media.disconnect().await;
            self.set_media_state(
                MediaState {
                    audio,
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
                effective_muted(state.self_state.muted, &state.self_state.push_to_talk),
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
                self.set_media_state(
                    MediaState {
                        livekit_connected: true,
                        microphone_published: true,
                        microphone: Some(connected.microphone),
                        speaker: Some(connected.speaker),
                        audio: connected.audio,
                        ..MediaState::default()
                    },
                    ConnectionState::Connected,
                    None,
                )
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
        let (muted, push_to_talk, effective) = {
            let mut state = self.state.write().await;
            let muted = requested.unwrap_or(!state.self_state.muted);
            state.self_state.muted = muted;
            if muted {
                state.self_state.push_to_talk.active = false;
                self.ptt_lease_tx.send_replace(None);
            }
            let push_to_talk = state.self_state.push_to_talk.clone();
            let effective = effective_muted(muted, &push_to_talk);
            if effective {
                clear_local_speaker(&mut state, &self.profile);
            }
            (muted, push_to_talk, effective)
        };
        self.media.set_muted(effective).await;
        self.publish_current("self_state_changed").await;
        json!({
            "muted": muted,
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
            let effective = effective_muted(muted, &push_to_talk);
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
            let effective = effective_muted(muted, &push_to_talk);
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
            let effective = effective_muted(muted, &push_to_talk);
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
            let effective = effective_muted(state.self_state.muted, &state.self_state.push_to_talk);
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
            "set_deafened" => {
                let deafened = boolean_arg(&command.args, "deafened")?;
                self.media.set_deafened(deafened).await;
                self.set_local_media(
                    |state| state.self_state.deafened = deafened,
                    "self_state_changed",
                )
                .await;
                Ok(Some(json!({"deafened": deafened})))
            }
            "toggle_deafened" => {
                let deafened = {
                    let mut state = self.state.write().await;
                    state.self_state.deafened = !state.self_state.deafened;
                    state.self_state.deafened
                };
                self.media.set_deafened(deafened).await;
                self.publish_current("self_state_changed").await;
                Ok(Some(json!({"deafened": deafened})))
            }
            "refresh_audio_devices"
            | "set_input_device"
            | "set_output_device"
            | "set_audio_preset" => self.audio_command(command).await,
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
            "open_surface" => self.surface_command(true),
            "close_surface" => self.surface_command(false),
            "share" => {
                let sharing = command
                    .args
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                self.set_local_media(
                    |state| state.self_state.sharing = sharing,
                    "self_state_changed",
                )
                .await;
                Ok(Some(
                    json!({"sharing": sharing, "source": command.args.get("source")}),
                ))
            }
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

    fn surface_command(&self, open: bool) -> anyhow::Result<Option<Value>> {
        if open {
            self.media.open_surface()?;
        } else {
            self.media.close_surface()?;
        }
        Ok(Some(
            json!({"surface": if open { "opening" } else { "closing" }}),
        ))
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

fn effective_muted(muted: bool, push_to_talk: &PushToTalkState) -> bool {
    muted || (push_to_talk.enabled && !push_to_talk.active)
}

fn clear_local_speaker(snapshot: &mut Snapshot, profile: &str) {
    snapshot
        .self_state
        .media
        .active_speakers
        .retain(|name| name != profile);
    snapshot.self_state.media.audio.input_level = 0;
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
        let url = match daemon.api.events_url() {
            Ok(url) => url,
            Err(error) => {
                error!(%error, "invalid server events URL");
                return;
            }
        };
        match connect_async(url.as_str()).await {
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
            Err(error) => warn!(%error, "cannot connect to wisp-server events"),
        }
        daemon
            .set_connection(
                ConnectionState::Reconnecting,
                Some("coordination server unavailable"),
            )
            .await;
        attempt = attempt.saturating_add(1).min(6);
        tokio::time::sleep(Duration::from_millis(250 * 2_u64.pow(attempt))).await;
    }
}

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
            | MediaEvent::AudioFrames { generation, .. }
            | MediaEvent::InputLevel { generation, .. }
            | MediaEvent::ActiveSpeakers { generation, .. }
            | MediaEvent::VideoSubscribed { generation, .. }
            | MediaEvent::VideoFrames { generation, .. } => Some(*generation),
            MediaEvent::SurfaceOpened
            | MediaEvent::SurfaceClosed
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
            | MediaEvent::AudioFrames { .. }
            | MediaEvent::InputLevel { .. }
            | MediaEvent::ActiveSpeakers { .. }
            | MediaEvent::VideoSubscribed { .. }
            | MediaEvent::VideoFrames { .. }) => {
                synchronize_track_event(&daemon, track_event).await;
            }
            surface_event @ (MediaEvent::SurfaceOpened
            | MediaEvent::SurfaceClosed
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
                    media.active_speakers.retain(|name| name != &participant);
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
                effective_muted(state.self_state.muted, &state.self_state.push_to_talk)
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
        MediaEvent::VideoSubscribed { participant, .. } => {
            info!(%participant, "subscribed to remote video");
            daemon
                .update_media_state(None, "remote_video_subscribed", |media| {
                    media.last_video_from = Some(participant);
                })
                .await;
        }
        MediaEvent::VideoFrames {
            participant,
            total,
            width,
            height,
            ..
        } => {
            daemon
                .update_media_state(None, "remote_video_received", |media| {
                    media.received_video_frames = total;
                    media.last_video_from = Some(participant);
                    media.video_width = Some(width);
                    media.video_height = Some(height);
                })
                .await;
        }
        _ => unreachable!("only remote track events are dispatched here"),
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
        MediaEvent::SurfaceOpened => {
            daemon
                .update_media_state(None, "video_surface_opened", |media| {
                    media.surface_open = true;
                    media.surface_error = None;
                    media.rendered_video_frames = 0;
                })
                .await;
        }
        MediaEvent::SurfaceClosed => {
            daemon
                .update_media_state(None, "video_surface_closed", |media| {
                    media.surface_open = false;
                })
                .await;
        }
        MediaEvent::SurfaceRendered { total } => {
            daemon
                .update_media_state(None, "video_surface_rendered", |media| {
                    media.rendered_video_frames = total;
                })
                .await;
        }
        MediaEvent::SurfaceError { message } => {
            warn!(%message, "video surface unavailable");
            daemon
                .update_media_state(None, "video_surface_error", |media| {
                    media.surface_open = false;
                    media.surface_error = Some(message);
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

async fn handle_tray_action(action: TrayAction, daemon: &Arc<Daemon>) -> bool {
    match action {
        TrayAction::Activate { x, y } => {
            tokio::spawn(control_ui(vec![
                "activate".into(),
                x.to_string(),
                y.to_string(),
            ]));
        }
        TrayAction::Show => {
            tokio::spawn(control_ui(vec!["open".into()]));
        }
        TrayAction::Hide => {
            tokio::spawn(control_ui(vec!["hide".into()]));
        }
        TrayAction::SetAnchor(anchor) => {
            tokio::spawn(control_ui(vec!["anchor".into(), anchor.into()]));
        }
        TrayAction::ToggleMuted => {
            daemon.update_manual_mute(None).await;
        }
        TrayAction::ToggleDeafened => {
            let deafened = {
                let mut state = daemon.state.write().await;
                state.self_state.deafened = !state.self_state.deafened;
                state.self_state.deafened
            };
            daemon.media.set_deafened(deafened).await;
            daemon.publish_current("self_state_changed").await;
        }
        TrayAction::Exit => {
            info!("exit requested from system tray");
            quit_all_ui_instances().await;
            return false;
        }
    }
    true
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
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "wispd=info".into()))
        .init();
    let args = Args::parse();
    let socket_path = args.socket.unwrap_or_else(runtime_socket_path);
    let listener = bind_socket(&socket_path).await?;
    let (api, mut snapshot) = ServerApi::connect(args.server_url, &args.profile)
        .await
        .with_context(|| format!("connect profile {} to wisp-server", args.profile))?;
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
    let (media, media_events) = MediaManager::new(!args.disable_media);
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
    tokio::spawn(synchronize_push_to_talk_lease(daemon.clone()));
    let initial_media = daemon.clone();
    tokio::spawn(async move {
        if let Err(error) = initial_media.reconcile_media().await {
            warn!(%error, "initial media connection failed");
        }
    });

    let (mut tray_actions, _tray_handle) = if std::env::var_os("WISP_DISABLE_TRAY").is_some() {
        (None, None)
    } else {
        match tray::spawn().await {
            Ok((receiver, handle)) => (Some(receiver), Some(handle)),
            Err(error) => {
                warn!(%error, "system tray is unavailable; continuing without a tray icon");
                (None, None)
            }
        }
    };
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
    use super::{describe_media_failure, effective_muted};
    use wisp_protocol::PushToTalkState;

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

        assert!(!effective_muted(false, &disabled));
        assert!(effective_muted(false, &waiting));
        assert!(!effective_muted(false, &talking));
        assert!(effective_muted(true, &talking));
    }
}
