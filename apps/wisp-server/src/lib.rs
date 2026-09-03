use axum::{
    Json, Router,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message as WsMessage, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration as ChronoDuration, Utc};
use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Sha256;
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow},
};
use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};
use tokio::sync::{RwLock, broadcast};
use tower_http::trace::TraceLayer;
use tracing::{debug, info, warn};
use uuid::Uuid;
use wisp_protocol::{
    BootstrapDeviceRequest, ConnectionState, ConversationKind, ConversationView,
    CreateDirectConversationRequest, CreateInviteRequest, DevSession, DevSessionRequest,
    DeviceCredential, DeviceInvite, DeviceSession, DeviceSessionRequest, DeviceView, FriendState,
    HangoutId, HangoutView, JoinFriendRequest, JoinFriendResult, JoinHangoutRequest,
    JoinSpotRequest, KnockId, KnockRequestView, KnockResponse, LiveKitTokenResponse,
    MarkConversationReadRequest, Message, PROTOCOL_VERSION, Presence, ProtocolError,
    RegisterDeviceRequest, RespondKnockRequest, RespondKnockResult, SendMessageRequest,
    ServerEvent, SetPresenceRequest, Snapshot, SpotView, UserId, UserSummary,
};

const JARED_ID: &str = "00000000-0000-4000-8000-000000000001";
const TYLER_ID: &str = "00000000-0000-4000-8000-000000000002";
const JACK_ID: &str = "00000000-0000-4000-8000-000000000003";
const CHARLIE_ID: &str = "00000000-0000-4000-8000-000000000004";
const CIRCLE_ID: &str = "00000000-0000-4000-8000-000000000010";
const CIRCLE_CONVERSATION_ID: &str = "00000000-0000-4000-8000-000000000011";
const PORCH_ID: &str = "00000000-0000-4000-8000-000000000020";
const SESSION_TTL_HOURS: i64 = 12;
const INVITE_TTL_MINUTES: u32 = 30;
const HANGOUT_MESSAGE_RETENTION_HOURS: i64 = 24;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub livekit_url: String,
    pub livekit_api_key: String,
    pub livekit_api_secret: String,
    pub knock_ttl: Duration,
    pub allow_dev_sessions: bool,
    pub bootstrap_token: Option<String>,
}

#[derive(Clone)]
pub struct AppState {
    pool: SqlitePool,
    runtime: Arc<RwLock<RuntimeState>>,
    events: broadcast::Sender<ServerEvent>,
    config: Arc<AppConfig>,
}

#[derive(Debug, Default)]
struct RuntimeState {
    users: HashMap<UserId, RuntimeUser>,
    seq: u64,
    connected_clients: usize,
    knocks: HashMap<KnockId, PendingKnock>,
}

#[derive(Debug, Clone)]
struct PendingKnock {
    id: KnockId,
    from: UserSummary,
    to: UserId,
    expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct RuntimeUser {
    presence: Presence,
    connections: usize,
}

impl Default for RuntimeUser {
    fn default() -> Self {
        Self {
            presence: Presence::Open,
            connections: 0,
        }
    }
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: message.into(),
        }
    }

    fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: message.into(),
        }
    }

    fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: message.into(),
        }
    }

    fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code,
            message: message.into(),
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        warn!(%error, "request failed");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal",
            message: "internal server error".into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ProtocolError {
                code: self.code.into(),
                message: self.message,
            }),
        )
            .into_response()
    }
}

impl AppState {
    pub async fn new(config: AppConfig) -> anyhow::Result<Self> {
        let options = SqliteConnectOptions::from_str(&config.database_url)?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));
        let max_connections = if config.database_url.contains(":memory:") {
            1
        } else {
            5
        };
        let pool = SqlitePoolOptions::new()
            .max_connections(max_connections)
            .connect_with(options)
            .await?;
        sqlx::migrate!("../../migrations").run(&pool).await?;
        cleanup_stale_sessions(&pool).await?;
        seed_development_users(&pool).await?;
        cleanup_expired_data(&pool).await?;
        let (events, _) = broadcast::channel(256);
        Ok(Self {
            pool,
            runtime: Arc::new(RwLock::new(RuntimeState::default())),
            events,
            config: Arc::new(config),
        })
    }

    async fn emit(&self, name: &str, payload: Value) {
        let seq = {
            let mut runtime = self.runtime.write().await;
            runtime.seq += 1;
            runtime.seq
        };
        let event = ServerEvent {
            seq,
            name: name.into(),
            occurred_at: Utc::now(),
            payload,
        };
        let _ = self.events.send(event);
    }

    async fn set_connected(&self, user_id: UserId, connected: bool) {
        let mut runtime = self.runtime.write().await;
        let user = runtime.users.entry(user_id).or_default();
        if connected {
            user.connections += 1;
            runtime.connected_clients += 1;
        } else {
            user.connections = user.connections.saturating_sub(1);
            runtime.connected_clients = runtime.connected_clients.saturating_sub(1);
        }
        drop(runtime);
        self.emit(
            "presence_changed",
            json!({"user_id": user_id, "online": connected}),
        )
        .await;
    }

    async fn snapshot(&self, self_id: UserId) -> Result<Snapshot, ApiError> {
        let knocks = self.incoming_knocks(self_id).await;
        let users =
            sqlx::query("SELECT id, display_name FROM users ORDER BY display_name COLLATE NOCASE")
                .fetch_all(&self.pool)
                .await
                .map_err(ApiError::internal)?;
        let runtime = self.runtime.read().await;
        let seq = runtime.seq;
        let self_runtime = runtime.users.get(&self_id).cloned().unwrap_or_default();
        let mut friends = Vec::new();
        let mut self_user = None;
        for row in users {
            let id = parse_uuid(&row.get::<String, _>("id"))?;
            let user = UserSummary {
                id,
                display_name: row.get("display_name"),
            };
            if id == self_id {
                self_user = Some(user);
                continue;
            }
            let state = runtime.users.get(&id).cloned().unwrap_or_default();
            friends.push(FriendState {
                user,
                presence: state.presence,
                online: state.connections > 0,
                hangout_id: active_hangout_for(&self.pool, id).await?,
                activity: None,
            });
        }
        drop(runtime);
        let self_user = self_user.ok_or_else(|| ApiError::not_found("user does not exist"))?;
        let self_hangout = active_hangout_for(&self.pool, self_id).await?;
        let hangouts = load_hangouts(&self.pool).await?;
        let conversations = load_conversations(&self.pool, self_id).await?;
        let messages = load_recent_messages(&self.pool, self_id).await?;
        let spots = load_spots(&self.pool).await?;
        let devices = load_devices(&self.pool, self_id).await?;
        Ok(Snapshot {
            seq,
            self_state: wisp_protocol::SelfState {
                user: self_user,
                presence: self_runtime.presence,
                connection: if self_hangout.is_some() {
                    ConnectionState::Connected
                } else {
                    ConnectionState::Available
                },
                muted: false,
                deafened: false,
                sharing: false,
                hangout_id: self_hangout,
                push_to_talk: wisp_protocol::PushToTalkState::default(),
                media: wisp_protocol::MediaState::default(),
            },
            friends,
            hangouts,
            knocks,
            conversations,
            messages,
            spots,
            devices,
            last_invite: None,
        })
    }

    async fn incoming_knocks(&self, user_id: UserId) -> Vec<KnockRequestView> {
        let now = Utc::now();
        let mut runtime = self.runtime.write().await;
        runtime.knocks.retain(|_, knock| knock.expires_at > now);
        let mut knocks = runtime
            .knocks
            .values()
            .filter(|knock| knock.to == user_id)
            .map(|knock| KnockRequestView {
                id: knock.id,
                from: knock.from.clone(),
                expires_at: knock.expires_at,
            })
            .collect::<Vec<_>>();
        knocks.sort_by_key(|knock| knock.expires_at);
        knocks
    }

    async fn create_knock(&self, from: UserSummary, to: UserId) -> (KnockRequestView, bool) {
        let now = Utc::now();
        let mut runtime = self.runtime.write().await;
        runtime.knocks.retain(|_, knock| knock.expires_at > now);
        if let Some(existing) = runtime
            .knocks
            .values()
            .find(|knock| knock.from.id == from.id && knock.to == to)
        {
            return (
                KnockRequestView {
                    id: existing.id,
                    from: existing.from.clone(),
                    expires_at: existing.expires_at,
                },
                false,
            );
        }
        let id = Uuid::new_v4();
        let expires_at = now
            + ChronoDuration::from_std(self.config.knock_ttl)
                .unwrap_or_else(|_| ChronoDuration::seconds(30));
        runtime.knocks.insert(
            id,
            PendingKnock {
                id,
                from: from.clone(),
                to,
                expires_at,
            },
        );
        (
            KnockRequestView {
                id,
                from,
                expires_at,
            },
            true,
        )
    }

    async fn pending_knock(&self, id: KnockId, recipient: UserId) -> Option<PendingKnock> {
        let now = Utc::now();
        let mut runtime = self.runtime.write().await;
        runtime.knocks.retain(|_, knock| knock.expires_at > now);
        runtime
            .knocks
            .get(&id)
            .filter(|knock| knock.to == recipient)
            .cloned()
    }

    async fn remove_knock(&self, id: KnockId) -> bool {
        self.runtime.write().await.knocks.remove(&id).is_some()
    }

    fn schedule_knock_expiry(&self, id: KnockId, ttl: Duration) {
        let state = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(ttl).await;
            if state.remove_knock(id).await {
                state.emit("knock_expired", json!({"knock_id": id})).await;
            }
        });
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/v1/dev/session", post(dev_session))
        .route("/v1/sessions", post(device_session))
        .route("/v1/devices/bootstrap", post(bootstrap_device))
        .route("/v1/devices/register", post(register_device))
        .route("/v1/devices", get(list_devices))
        .route("/v1/devices/{id}", delete(revoke_device))
        .route("/v1/admin/invites", post(create_invite))
        .route("/v1/snapshot", get(snapshot))
        .route("/v1/events", get(events))
        .route("/v1/presence", post(set_presence))
        .route("/v1/hangouts/join-friend", post(join_friend))
        .route("/v1/hangouts/join", post(join_hangout))
        .route("/v1/knocks/respond", post(respond_knock))
        .route("/v1/hangouts/leave", post(leave_hangout))
        .route("/v1/spots/join", post(join_spot))
        .route("/v1/livekit/token", post(livekit_token))
        .route("/v1/messages", get(list_messages).post(send_message))
        .route("/v1/conversations/direct", post(create_direct_conversation))
        .route("/v1/conversations/read", post(mark_conversation_read))
        .route("/v1/users/{id}", get(user_by_id))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Response {
    let connected_clients = state.runtime.read().await.connected_clients;
    let database = sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();
    let status = if database {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(json!({
            "ok": database,
            "database": database,
            "connected_clients": connected_clients,
            "protocol_version": PROTOCOL_VERSION
        })),
    )
        .into_response()
}

async fn dev_session(
    State(state): State<AppState>,
    Json(request): Json<DevSessionRequest>,
) -> Result<Json<DevSession>, ApiError> {
    if !state.config.allow_dev_sessions {
        return Err(ApiError::forbidden(
            "development sessions are disabled; register this device with an invite",
        ));
    }
    let row =
        sqlx::query("SELECT id, display_name FROM users WHERE display_name = ? COLLATE NOCASE")
            .bind(request.profile.trim())
            .fetch_optional(&state.pool)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(|| {
                ApiError::not_found(format!("unknown development profile: {}", request.profile))
            })?;
    let id = parse_uuid(&row.get::<String, _>("id"))?;
    let user = UserSummary {
        id,
        display_name: row.get("display_name"),
    };
    Ok(Json(DevSession {
        token: format!("dev:{id}"),
        user,
        protocol_version: PROTOCOL_VERSION,
    }))
}

async fn create_invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateInviteRequest>,
) -> Result<Json<DeviceInvite>, ApiError> {
    let creator = authenticate_headers(&state, &headers).await?;
    if creator.to_string() != JARED_ID {
        return Err(ApiError::forbidden(
            "only the circle administrator can create invites",
        ));
    }
    let user = find_user(&state.pool, request.profile.trim()).await?;
    let id = Uuid::new_v4();
    let code = random_token("wisp-invite");
    let expires_at = Utc::now()
        + ChronoDuration::minutes(
            i64::from(request.expires_in_minutes.unwrap_or(INVITE_TTL_MINUTES)).clamp(1, 24 * 60),
        );
    sqlx::query(
        "INSERT INTO device_invites(id, code_hash, user_id, created_by, created_at, expires_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(token_hash(&code))
    .bind(user.id.to_string())
    .bind(creator.to_string())
    .bind(Utc::now().to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .execute(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    info!(invite_id = %id, profile = %user.display_name, "device invite created");
    Ok(Json(DeviceInvite {
        id,
        code,
        profile: user.display_name,
        expires_at,
    }))
}

async fn bootstrap_device(
    State(state): State<AppState>,
    Json(request): Json<BootstrapDeviceRequest>,
) -> Result<Json<DeviceCredential>, ApiError> {
    require_protocol(request.protocol_version)?;
    let expected = state
        .config
        .bootstrap_token
        .as_deref()
        .ok_or_else(|| ApiError::forbidden("device bootstrap is disabled"))?;
    if token_hash(expected) != token_hash(&request.bootstrap_token) {
        return Err(ApiError::unauthorized("bootstrap token is invalid"));
    }
    let user = find_user(&state.pool, request.profile.trim()).await?;
    if user.id.to_string() != JARED_ID {
        return Err(ApiError::forbidden(
            "only the administrator profile may bootstrap",
        ));
    }
    let name = request.device_name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(ApiError::bad_request(
            "invalid_device_name",
            "device name must contain 1–80 characters",
        ));
    }
    let device_id = Uuid::new_v4();
    let device_token = random_token("wisp-device");
    let now = Utc::now().to_rfc3339();
    let inserted = sqlx::query(
        "INSERT INTO devices(id, user_id, name, token_hash, created_at, last_seen_at) SELECT ?, ?, ?, ?, ?, ? WHERE NOT EXISTS (SELECT 1 FROM devices WHERE user_id = ? AND revoked_at IS NULL)",
    )
    .bind(device_id.to_string())
    .bind(user.id.to_string())
    .bind(name)
    .bind(token_hash(&device_token))
    .bind(&now)
    .bind(&now)
    .bind(user.id.to_string())
    .execute(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    if inserted.rows_affected() != 1 {
        return Err(ApiError::conflict(
            "already_bootstrapped",
            "an administrator device is already registered",
        ));
    }
    info!(%device_id, user_id = %user.id, "administrator device bootstrapped");
    Ok(Json(DeviceCredential {
        device_id,
        device_token,
        user,
    }))
}

async fn register_device(
    State(state): State<AppState>,
    Json(request): Json<RegisterDeviceRequest>,
) -> Result<Json<DeviceCredential>, ApiError> {
    require_protocol(request.protocol_version)?;
    let name = request.device_name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(ApiError::bad_request(
            "invalid_device_name",
            "device name must contain 1–80 characters",
        ));
    }
    let now = Utc::now();
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    let invite = sqlx::query(
        "SELECT id, user_id FROM device_invites WHERE code_hash = ? AND used_at IS NULL AND expires_at > ?",
    )
    .bind(token_hash(&request.invite_code))
    .bind(now.to_rfc3339())
    .fetch_optional(&mut *tx)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(|| ApiError::bad_request("invalid_invite", "invite is invalid, expired, or already used"))?;
    let invite_id: String = invite.get("id");
    let user_id = parse_uuid(&invite.get::<String, _>("user_id"))?;
    let consumed =
        sqlx::query("UPDATE device_invites SET used_at = ? WHERE id = ? AND used_at IS NULL")
            .bind(now.to_rfc3339())
            .bind(&invite_id)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    if consumed.rows_affected() != 1 {
        return Err(ApiError::conflict("invite_used", "invite was already used"));
    }
    let device_id = Uuid::new_v4();
    let device_token = random_token("wisp-device");
    sqlx::query(
        "INSERT INTO devices(id, user_id, name, token_hash, created_at, last_seen_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(device_id.to_string())
    .bind(user_id.to_string())
    .bind(name)
    .bind(token_hash(&device_token))
    .bind(now.to_rfc3339())
    .bind(now.to_rfc3339())
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    let user = find_user(&state.pool, &user_id.to_string()).await?;
    info!(%device_id, user_id = %user_id, "device registered");
    Ok(Json(DeviceCredential {
        device_id,
        device_token,
        user,
    }))
}

async fn device_session(
    State(state): State<AppState>,
    Json(request): Json<DeviceSessionRequest>,
) -> Result<Json<DeviceSession>, ApiError> {
    require_protocol(request.protocol_version)?;
    let row = sqlx::query(
        "SELECT d.user_id, u.display_name FROM devices d JOIN users u ON u.id = d.user_id WHERE d.id = ? AND d.token_hash = ? AND d.revoked_at IS NULL",
    )
    .bind(request.device_id.to_string())
    .bind(token_hash(&request.device_token))
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(|| ApiError::unauthorized("device credential is invalid or revoked"))?;
    let user_id = parse_uuid(&row.get::<String, _>("user_id"))?;
    let now = Utc::now();
    let expires_at = now + ChronoDuration::hours(SESSION_TTL_HOURS);
    let token = random_token("wisp-session");
    sqlx::query(
        "INSERT INTO sessions(id, device_id, token_hash, created_at, expires_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(request.device_id.to_string())
    .bind(token_hash(&token))
    .bind(now.to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .execute(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    sqlx::query("UPDATE devices SET last_seen_at = ? WHERE id = ?")
        .bind(now.to_rfc3339())
        .bind(request.device_id.to_string())
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DeviceSession {
        token,
        expires_at,
        user: UserSummary {
            id: user_id,
            display_name: row.get("display_name"),
        },
        device_id: request.device_id,
        protocol_version: PROTOCOL_VERSION,
    }))
}

async fn list_devices(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DeviceView>>, ApiError> {
    let user_id = authenticate_headers(&state, &headers).await?;
    Ok(Json(load_devices(&state.pool, user_id).await?))
}

async fn revoke_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let user_id = authenticate_headers(&state, &headers).await?;
    let device_id = Uuid::parse_str(&id)
        .map_err(|_| ApiError::bad_request("invalid_device_id", "device ID is invalid"))?;
    let now = Utc::now().to_rfc3339();
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    let result = sqlx::query(
        "UPDATE devices SET revoked_at = ? WHERE id = ? AND user_id = ? AND revoked_at IS NULL",
    )
    .bind(&now)
    .bind(device_id.to_string())
    .bind(user_id.to_string())
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;
    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("active device does not exist"));
    }
    sqlx::query("UPDATE sessions SET revoked_at = ? WHERE device_id = ? AND revoked_at IS NULL")
        .bind(&now)
        .bind(device_id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    info!(%device_id, user_id = %user_id, "device revoked");
    Ok(Json(json!({"ok": true})))
}

async fn snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Snapshot>, ApiError> {
    let user_id = authenticate_headers(&state, &headers).await?;
    Ok(Json(state.snapshot(user_id).await?))
}

async fn set_presence(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetPresenceRequest>,
) -> Result<Json<Value>, ApiError> {
    let user_id = authenticate_headers(&state, &headers).await?;
    state
        .runtime
        .write()
        .await
        .users
        .entry(user_id)
        .or_default()
        .presence = request.presence;
    sqlx::query("UPDATE users SET last_seen_at = ? WHERE id = ?")
        .bind(Utc::now().to_rfc3339())
        .bind(user_id.to_string())
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    state
        .emit(
            "presence_changed",
            json!({"user_id": user_id, "presence": request.presence}),
        )
        .await;
    Ok(Json(json!({"ok": true})))
}

async fn join_friend(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<JoinFriendRequest>,
) -> Result<Json<JoinFriendResult>, ApiError> {
    let self_id = authenticate_headers(&state, &headers).await?;
    let self_user = find_user(&state.pool, &self_id.to_string()).await?;
    let friend = find_user(&state.pool, &request.friend).await?;
    if friend.id == self_id {
        return Err(ApiError::bad_request(
            "cannot_join_self",
            "choose a friend to join",
        ));
    }
    let friend_runtime = state
        .runtime
        .read()
        .await
        .users
        .get(&friend.id)
        .cloned()
        .unwrap_or_default();
    match join_policy(friend_runtime.presence, friend_runtime.connections > 0) {
        JoinPolicy::Unavailable => {
            return Err(ApiError::bad_request(
                "friend_unavailable",
                format!("{} is not open to joins", friend.display_name),
            ));
        }
        JoinPolicy::Offline => {
            return Err(ApiError::bad_request(
                "friend_offline",
                format!("{} is offline", friend.display_name),
            ));
        }
        JoinPolicy::Knock => {
            let (knock, created) = state.create_knock(self_user, friend.id).await;
            if created {
                state
                    .emit(
                        "knock_requested",
                        json!({
                            "knock_id": knock.id,
                            "from": knock.from.id,
                            "to": friend.id,
                            "expires_at": knock.expires_at,
                        }),
                    )
                    .await;
                state.schedule_knock_expiry(knock.id, state.config.knock_ttl);
            }
            return Ok(Json(JoinFriendResult::KnockSent {
                knock_id: knock.id,
                expires_at: knock.expires_at,
            }));
        }
        JoinPolicy::Direct => {}
    }

    let hangout_id = join_users(&state, self_id, friend.id).await?;
    state
        .emit("hangout_changed", json!({"hangout_id": hangout_id}))
        .await;
    Ok(Json(JoinFriendResult::Joined { hangout_id }))
}

async fn respond_knock(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RespondKnockRequest>,
) -> Result<Json<RespondKnockResult>, ApiError> {
    let recipient_id = authenticate_headers(&state, &headers).await?;
    let Some(knock) = state.pending_knock(request.knock_id, recipient_id).await else {
        state
            .emit(
                "knock_unavailable",
                json!({"knock_id": request.knock_id, "to": recipient_id}),
            )
            .await;
        return Err(ApiError::bad_request(
            "knock_unavailable",
            "this knock has expired or was dismissed",
        ));
    };
    match request.response {
        KnockResponse::Later => {
            state.remove_knock(knock.id).await;
            state
                .emit(
                    "knock_dismissed",
                    json!({"knock_id": knock.id, "from": knock.from.id, "to": recipient_id}),
                )
                .await;
            Ok(Json(RespondKnockResult::Later))
        }
        KnockResponse::Accept => {
            let requester_online = state
                .runtime
                .read()
                .await
                .users
                .get(&knock.from.id)
                .is_some_and(|user| user.connections > 0);
            if !requester_online {
                state.remove_knock(knock.id).await;
                state
                    .emit(
                        "knock_cancelled",
                        json!({
                            "knock_id": knock.id,
                            "from": knock.from.id,
                            "to": recipient_id,
                            "reason": "requester_offline",
                        }),
                    )
                    .await;
                return Err(ApiError::bad_request(
                    "knock_requester_offline",
                    format!("{} is no longer online", knock.from.display_name),
                ));
            }
            let hangout_id = join_users(&state, recipient_id, knock.from.id).await?;
            state.remove_knock(knock.id).await;
            state
                .emit(
                    "knock_accepted",
                    json!({
                        "knock_id": knock.id,
                        "from": knock.from.id,
                        "to": recipient_id,
                        "hangout_id": hangout_id,
                    }),
                )
                .await;
            Ok(Json(RespondKnockResult::Accepted { hangout_id }))
        }
    }
}

async fn join_users(
    state: &AppState,
    self_id: UserId,
    friend_id: UserId,
) -> Result<HangoutId, ApiError> {
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    let previous_hangout = active_hangout_for_tx(&mut tx, self_id).await?;
    leave_active_in_transaction(&mut tx, self_id).await?;
    let hangout_id = if let Some(id) = active_hangout_for_tx(&mut tx, friend_id).await? {
        id
    } else {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO hangouts(id, livekit_room, created_at) VALUES (?, ?, ?)")
            .bind(id.to_string())
            .bind(format!("wisp-{id}"))
            .bind(Utc::now().to_rfc3339())
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
        create_hangout_conversation(&mut tx, id, None).await?;
        add_member(&mut tx, id, friend_id).await?;
        id
    };
    add_member(&mut tx, hangout_id, self_id).await?;
    if let Some(previous_hangout) = previous_hangout {
        end_if_empty(&mut tx, previous_hangout).await?;
    }
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(hangout_id)
}

async fn join_hangout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<JoinHangoutRequest>,
) -> Result<Json<Value>, ApiError> {
    let self_id = authenticate_headers(&state, &headers).await?;
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM hangouts WHERE id = ? AND ended_at IS NULL",
    )
    .bind(request.hangout_id.to_string())
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::internal)?
        > 0;
    if !exists {
        return Err(ApiError::not_found("room is no longer active"));
    }
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    let previous_hangout = active_hangout_for_tx(&mut tx, self_id).await?;
    leave_active_in_transaction(&mut tx, self_id).await?;
    add_member(&mut tx, request.hangout_id, self_id).await?;
    if let Some(previous_hangout) = previous_hangout {
        end_if_empty(&mut tx, previous_hangout).await?;
    }
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .emit("hangout_changed", json!({"hangout_id": request.hangout_id}))
        .await;
    Ok(Json(json!({"hangout_id": request.hangout_id})))
}

async fn leave_hangout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let self_id = authenticate_headers(&state, &headers).await?;
    let hangout = active_hangout_for(&state.pool, self_id).await?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    leave_active_in_transaction(&mut tx, self_id).await?;
    if let Some(hangout_id) = hangout {
        end_if_empty(&mut tx, hangout_id).await?;
    }
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .emit("hangout_changed", json!({"hangout_id": hangout}))
        .await;
    Ok(Json(json!({"ok": true})))
}

async fn join_spot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<JoinSpotRequest>,
) -> Result<Json<Value>, ApiError> {
    let self_id = authenticate_headers(&state, &headers).await?;
    let spot = sqlx::query("SELECT id, name FROM spots WHERE id = ? OR name = ? COLLATE NOCASE")
        .bind(request.spot_id.trim())
        .bind(request.spot_id.trim())
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("spot does not exist"))?;
    let spot_id: String = spot.get("id");
    let spot_name: String = spot.get("name");
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    ensure_spot_conversation(&mut tx, &spot_id, &spot_name)
        .await
        .map_err(ApiError::internal)?;
    let previous_hangout = active_hangout_for_tx(&mut tx, self_id).await?;
    leave_active_in_transaction(&mut tx, self_id).await?;
    let active = sqlx::query_scalar::<_, String>(
        "SELECT id FROM hangouts WHERE spot_id = ? AND ended_at IS NULL ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&spot_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ApiError::internal)?;
    let hangout_id = if let Some(id) = active {
        parse_uuid(&id)?
    } else {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO hangouts(id, livekit_room, label, spot_id, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id.to_string())
        .bind(format!("wisp-{id}"))
        .bind(&spot_name)
        .bind(&spot_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
        id
    };
    add_member(&mut tx, hangout_id, self_id).await?;
    if let Some(previous) = previous_hangout.filter(|previous| *previous != hangout_id) {
        end_if_empty(&mut tx, previous).await?;
    }
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .emit("hangout_changed", json!({"hangout_id": hangout_id}))
        .await;
    Ok(Json(json!({"hangout_id": hangout_id})))
}

async fn livekit_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LiveKitTokenResponse>, ApiError> {
    let user_id = authenticate_headers(&state, &headers).await?;
    let user = find_user(&state.pool, &user_id.to_string()).await?;
    let hangout_id = active_hangout_for(&state.pool, user_id)
        .await?
        .ok_or_else(|| ApiError::bad_request("not_in_hangout", "join a room first"))?;
    let room: String = sqlx::query_scalar("SELECT livekit_room FROM hangouts WHERE id = ?")
        .bind(hangout_id.to_string())
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    let token = issue_livekit_token(&state.config, &user, &room)?;
    Ok(Json(LiveKitTokenResponse {
        url: state.config.livekit_url.clone(),
        room,
        token,
    }))
}

#[derive(Debug, Deserialize)]
struct MessageQuery {
    conversation_id: String,
    #[serde(default)]
    after: Option<String>,
}

async fn list_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<MessageQuery>,
) -> Result<Json<Vec<Message>>, ApiError> {
    let user_id = authenticate_headers(&state, &headers).await?;
    ensure_conversation_member(&state.pool, &query.conversation_id, user_id).await?;
    cleanup_expired_messages(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    let rows = sqlx::query(
        "SELECT m.id, m.conversation_id, m.created_at, m.content_type, m.payload, m.encryption_version, u.id AS sender_id, u.display_name FROM messages m JOIN users u ON u.id = m.sender_id WHERE m.conversation_id = ? AND (? IS NULL OR m.created_at > ?) ORDER BY m.created_at, m.id LIMIT 200",
    )
    .bind(&query.conversation_id)
    .bind(query.after.as_deref())
    .bind(query.after.as_deref())
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    let mut messages = Vec::with_capacity(rows.len());
    for row in rows {
        messages.push(message_from_row(&row)?);
    }
    Ok(Json(messages))
}

async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SendMessageRequest>,
) -> Result<Json<Message>, ApiError> {
    let sender_id = authenticate_headers(&state, &headers).await?;
    ensure_conversation_member(&state.pool, &request.conversation_id, sender_id).await?;
    validate_message(&request)?;
    let sender = find_user(&state.pool, &sender_id.to_string()).await?;
    let message = Message {
        id: Uuid::new_v4(),
        conversation_id: request.conversation_id,
        sender,
        created_at: Utc::now(),
        content_type: request.content_type,
        payload: request.payload,
        encryption_version: request.encryption_version,
    };
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    sqlx::query("INSERT INTO messages(id, conversation_id, sender_id, created_at, content_type, payload, encryption_version) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(message.id.to_string()).bind(&message.conversation_id).bind(sender_id.to_string())
        .bind(message.created_at.to_rfc3339()).bind(&message.content_type)
        .bind(serde_json::to_string(&message.payload).map_err(ApiError::internal)?)
        .bind(message.encryption_version).execute(&mut *tx).await.map_err(ApiError::internal)?;
    sqlx::query("INSERT INTO message_reads(conversation_id, user_id, last_read_at) VALUES (?, ?, ?) ON CONFLICT(conversation_id, user_id) DO UPDATE SET last_read_at = excluded.last_read_at")
        .bind(&message.conversation_id)
        .bind(sender_id.to_string())
        .bind(message.created_at.to_rfc3339())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    // The row is committed before the acknowledgement and notification.
    state
        .emit("message_created", json!({"changed": true}))
        .await;
    Ok(Json(message))
}

async fn create_direct_conversation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateDirectConversationRequest>,
) -> Result<Json<ConversationView>, ApiError> {
    let self_id = authenticate_headers(&state, &headers).await?;
    let friend = find_user(&state.pool, request.friend.trim()).await?;
    if friend.id == self_id {
        return Err(ApiError::bad_request(
            "cannot_message_self",
            "choose a friend to message",
        ));
    }
    let conversation_id = find_or_create_direct(&state.pool, self_id, friend.id).await?;
    let conversation = load_conversation(&state.pool, self_id, &conversation_id).await?;
    state
        .emit("conversation_changed", json!({"changed": true}))
        .await;
    Ok(Json(conversation))
}

async fn mark_conversation_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<MarkConversationReadRequest>,
) -> Result<Json<Value>, ApiError> {
    let user_id = authenticate_headers(&state, &headers).await?;
    ensure_conversation_member(&state.pool, &request.conversation_id, user_id).await?;
    sqlx::query("INSERT INTO message_reads(conversation_id, user_id, last_read_at) VALUES (?, ?, ?) ON CONFLICT(conversation_id, user_id) DO UPDATE SET last_read_at = excluded.last_read_at")
        .bind(&request.conversation_id)
        .bind(user_id.to_string())
        .bind(Utc::now().to_rfc3339())
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"ok": true})))
}

async fn user_by_id(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<UserSummary>, ApiError> {
    let _user_id = authenticate_headers(&state, &headers).await?;
    Ok(Json(find_user(&state.pool, &id).await?))
}

#[derive(Debug, Deserialize)]
struct EventQuery {
    #[serde(default)]
    token: Option<String>,
}

async fn events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<EventQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let user_id = if headers.get("authorization").is_some() {
        authenticate_headers(&state, &headers).await?
    } else {
        let token = query
            .token
            .as_deref()
            .ok_or_else(|| ApiError::unauthorized("missing bearer token"))?;
        if !state.config.allow_dev_sessions || !token.starts_with("dev:") {
            return Err(ApiError::unauthorized(
                "query tokens are restricted to local development sessions",
            ));
        }
        authenticate_token(&state, token).await?
    };
    Ok(ws.on_upgrade(move |socket| event_socket(state, user_id, socket)))
}

async fn event_socket(state: AppState, user_id: UserId, socket: WebSocket) {
    state.set_connected(user_id, true).await;
    let (mut sender, mut receiver) = socket.split();
    let mut events = state.events.subscribe();
    if let Ok(snapshot) = state.snapshot(user_id).await {
        let initial = ServerEvent {
            seq: snapshot.seq,
            name: "snapshot".into(),
            occurred_at: Utc::now(),
            payload: json!(snapshot),
        };
        if sender
            .send(WsMessage::Text(
                serde_json::to_string(&initial).unwrap_or_default().into(),
            ))
            .await
            .is_err()
        {
            state.set_connected(user_id, false).await;
            return;
        }
    }
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Ok(event) => {
                    let Ok(text) = serde_json::to_string(&event) else { continue };
                    if sender.send(WsMessage::Text(text.into())).await.is_err() { break; }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => debug!(skipped, "event client lagged"),
                Err(broadcast::error::RecvError::Closed) => break,
            },
            message = receiver.next() => match message {
                Some(Ok(WsMessage::Close(_)) | Err(_)) | None => break,
                Some(Ok(WsMessage::Ping(bytes))) => {
                    if sender.send(WsMessage::Pong(bytes)).await.is_err() { break; }
                }
                _ => {}
            }
        }
    }
    state.set_connected(user_id, false).await;
}

async fn seed_development_users(pool: &SqlitePool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT OR IGNORE INTO circles(id, name) VALUES (?, 'Friends')")
        .bind(CIRCLE_ID)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT OR IGNORE INTO conversations(id, kind, label, circle_id, created_at) VALUES (?, 'circle', 'Friends', ?, ?)")
        .bind(CIRCLE_CONVERSATION_ID)
        .bind(CIRCLE_ID)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *tx)
        .await?;
    for (id, display_name) in [
        (JARED_ID, "Jared"),
        (TYLER_ID, "Tyler"),
        (JACK_ID, "Jack"),
        (CHARLIE_ID, "Charlie"),
    ] {
        sqlx::query("INSERT OR IGNORE INTO users(id, display_name) VALUES (?, ?)")
            .bind(id)
            .bind(display_name)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT OR IGNORE INTO circle_members(circle_id, user_id) VALUES (?, ?)")
            .bind(CIRCLE_ID)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT OR IGNORE INTO conversation_members(conversation_id, user_id, joined_at) VALUES (?, ?, ?)")
            .bind(CIRCLE_CONVERSATION_ID)
            .bind(id)
            .bind(Utc::now().to_rfc3339())
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("INSERT OR IGNORE INTO spots(id, name, created_at) VALUES (?, 'Porch', ?)")
        .bind(PORCH_ID)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *tx)
        .await?;
    ensure_spot_conversation(&mut tx, PORCH_ID, "Porch").await?;
    tx.commit().await?;
    info!("development profiles ready");
    Ok(())
}

async fn cleanup_expired_messages(pool: &SqlitePool) -> anyhow::Result<()> {
    let cutoff = (Utc::now() - ChronoDuration::hours(HANGOUT_MESSAGE_RETENTION_HOURS)).to_rfc3339();
    sqlx::query(
        "DELETE FROM messages WHERE created_at < ? AND conversation_id IN (SELECT id FROM conversations WHERE kind = 'hangout' AND spot_id IS NULL)",
    )
    .bind(cutoff)
    .execute(pool)
    .await?;
    Ok(())
}

async fn cleanup_expired_data(pool: &SqlitePool) -> anyhow::Result<()> {
    let now = Utc::now().to_rfc3339();
    cleanup_expired_messages(pool).await?;
    sqlx::query("DELETE FROM sessions WHERE expires_at <= ? OR revoked_at IS NOT NULL")
        .bind(&now)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM device_invites WHERE expires_at <= ? OR used_at IS NOT NULL")
        .bind(&now)
        .execute(pool)
        .await?;
    Ok(())
}

async fn cleanup_stale_sessions(pool: &SqlitePool) -> anyhow::Result<()> {
    let now = Utc::now().to_rfc3339();
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE hangout_members SET left_at = ? WHERE left_at IS NULL")
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE hangouts SET ended_at = ? WHERE ended_at IS NULL")
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JoinPolicy {
    Direct,
    Knock,
    Unavailable,
    Offline,
}

fn join_policy(presence: Presence, online: bool) -> JoinPolicy {
    if !online {
        return JoinPolicy::Offline;
    }
    match presence {
        Presence::Open => JoinPolicy::Direct,
        Presence::Knock => JoinPolicy::Knock,
        Presence::Closed | Presence::Away => JoinPolicy::Unavailable,
    }
}

async fn authenticate_headers(state: &AppState, headers: &HeaderMap) -> Result<UserId, ApiError> {
    let value = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::unauthorized("missing bearer token"))?;
    let token = value
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
        .ok_or_else(|| ApiError::unauthorized("authorization must use Bearer authentication"))?;
    authenticate_token(state, token).await
}

async fn authenticate_token(state: &AppState, token: &str) -> Result<UserId, ApiError> {
    if state.config.allow_dev_sessions
        && let Some(id) = token.strip_prefix("dev:")
    {
        return Uuid::parse_str(id)
            .map_err(|_| ApiError::unauthorized("invalid development token"));
    }
    let row = sqlx::query(
        "SELECT d.user_id FROM sessions s JOIN devices d ON d.id = s.device_id WHERE s.token_hash = ? AND s.expires_at > ? AND s.revoked_at IS NULL AND d.revoked_at IS NULL",
    )
    .bind(token_hash(token))
    .bind(Utc::now().to_rfc3339())
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(|| ApiError::unauthorized("session is invalid or expired"))?;
    parse_uuid(&row.get::<String, _>("user_id"))
}

fn parse_uuid(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value).map_err(ApiError::internal)
}

fn require_protocol(version: u8) -> Result<(), ApiError> {
    if version == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            "unsupported_protocol_version",
            format!("server requires protocol version {PROTOCOL_VERSION}"),
        ))
    }
}

fn random_token(prefix: &str) -> String {
    format!(
        "{prefix}:{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

fn token_hash(token: &str) -> String {
    use sha2::Digest;
    let digest = Sha256::digest(token.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn device_view(row: &SqliteRow) -> Result<DeviceView, ApiError> {
    Ok(DeviceView {
        id: parse_uuid(&row.get::<String, _>("id"))?,
        name: row.get("name"),
        created_at: row
            .get::<String, _>("created_at")
            .parse()
            .map_err(ApiError::internal)?,
        last_seen_at: row
            .get::<Option<String>, _>("last_seen_at")
            .map(|value| value.parse().map_err(ApiError::internal))
            .transpose()?,
        revoked: row.get::<Option<String>, _>("revoked_at").is_some(),
    })
}

async fn load_devices(pool: &SqlitePool, user_id: UserId) -> Result<Vec<DeviceView>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, name, created_at, last_seen_at, revoked_at FROM devices WHERE user_id = ? ORDER BY created_at",
    )
    .bind(user_id.to_string())
    .fetch_all(pool)
    .await
    .map_err(ApiError::internal)?;
    rows.iter().map(device_view).collect()
}

fn message_from_row(row: &SqliteRow) -> Result<Message, ApiError> {
    Ok(Message {
        id: parse_uuid(&row.get::<String, _>("id"))?,
        conversation_id: row.get("conversation_id"),
        sender: UserSummary {
            id: parse_uuid(&row.get::<String, _>("sender_id"))?,
            display_name: row.get("display_name"),
        },
        created_at: row
            .get::<String, _>("created_at")
            .parse()
            .map_err(ApiError::internal)?,
        content_type: row.get("content_type"),
        payload: serde_json::from_str(&row.get::<String, _>("payload"))
            .map_err(ApiError::internal)?,
        encryption_version: row.get("encryption_version"),
    })
}

fn validate_message(request: &SendMessageRequest) -> Result<(), ApiError> {
    if request.content_type != "text/plain" {
        return Err(ApiError::bad_request(
            "unsupported_content_type",
            "private alpha supports text/plain messages only",
        ));
    }
    let text = request
        .payload
        .as_str()
        .ok_or_else(|| ApiError::bad_request("invalid_message", "message payload must be text"))?;
    let length = text.trim().chars().count();
    if length == 0 || length > 4_000 {
        return Err(ApiError::bad_request(
            "invalid_message",
            "message must contain 1–4000 characters",
        ));
    }
    if request.encryption_version < 0 {
        return Err(ApiError::bad_request(
            "invalid_encryption_version",
            "encryption version cannot be negative",
        ));
    }
    Ok(())
}

async fn ensure_conversation_member(
    pool: &SqlitePool,
    conversation_id: &str,
    user_id: UserId,
) -> Result<(), ApiError> {
    let allowed = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM conversation_members WHERE conversation_id = ? AND user_id = ?",
    )
    .bind(conversation_id)
    .bind(user_id.to_string())
    .fetch_one(pool)
    .await
    .map_err(ApiError::internal)?
        > 0;
    if allowed {
        Ok(())
    } else {
        Err(ApiError::forbidden(
            "conversation is not available to this user",
        ))
    }
}

async fn find_or_create_direct(
    pool: &SqlitePool,
    self_id: UserId,
    friend_id: UserId,
) -> Result<String, ApiError> {
    if let Some(id) = sqlx::query_scalar::<_, String>(
        "SELECT c.id FROM conversations c WHERE c.kind = 'direct' AND (SELECT COUNT(*) FROM conversation_members cm WHERE cm.conversation_id = c.id) = 2 AND EXISTS (SELECT 1 FROM conversation_members cm WHERE cm.conversation_id = c.id AND cm.user_id = ?) AND EXISTS (SELECT 1 FROM conversation_members cm WHERE cm.conversation_id = c.id AND cm.user_id = ?) LIMIT 1",
    )
    .bind(self_id.to_string())
    .bind(friend_id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(ApiError::internal)?
    {
        return Ok(id);
    }
    let self_key = self_id.to_string();
    let friend_key = friend_id.to_string();
    let (first, second) = if self_key < friend_key {
        (&self_key, &friend_key)
    } else {
        (&friend_key, &self_key)
    };
    let id = format!("dm:{first}:{second}");
    let now = Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(ApiError::internal)?;
    sqlx::query(
        "INSERT OR IGNORE INTO conversations(id, kind, label, created_at) VALUES (?, 'direct', 'Direct message', ?)",
    )
    .bind(&id)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;
    for user_id in [self_id, friend_id] {
        sqlx::query(
            "INSERT OR IGNORE INTO conversation_members(conversation_id, user_id, joined_at) VALUES (?, ?, ?)",
        )
        .bind(&id)
        .bind(user_id.to_string())
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    }
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(id)
}

async fn load_conversations(
    pool: &SqlitePool,
    user_id: UserId,
) -> Result<Vec<ConversationView>, ApiError> {
    let ids = sqlx::query_scalar::<_, String>(
        "SELECT c.id FROM conversations c JOIN conversation_members cm ON cm.conversation_id = c.id WHERE cm.user_id = ? ORDER BY COALESCE((SELECT MAX(created_at) FROM messages WHERE conversation_id = c.id), c.created_at) DESC",
    )
    .bind(user_id.to_string())
    .fetch_all(pool)
    .await
    .map_err(ApiError::internal)?;
    let mut conversations = Vec::with_capacity(ids.len());
    for id in ids {
        conversations.push(load_conversation(pool, user_id, &id).await?);
    }
    Ok(conversations)
}

async fn load_recent_messages(
    pool: &SqlitePool,
    user_id: UserId,
) -> Result<Vec<Message>, ApiError> {
    cleanup_expired_messages(pool)
        .await
        .map_err(ApiError::internal)?;
    let rows = sqlx::query(
        "SELECT recent.id, recent.conversation_id, recent.created_at, recent.content_type, recent.payload, recent.encryption_version, u.id AS sender_id, u.display_name FROM (SELECT m.* FROM messages m JOIN conversation_members cm ON cm.conversation_id = m.conversation_id WHERE cm.user_id = ? ORDER BY m.created_at DESC, m.id DESC LIMIT 500) recent JOIN users u ON u.id = recent.sender_id ORDER BY recent.created_at, recent.id",
    )
    .bind(user_id.to_string())
    .fetch_all(pool)
    .await
    .map_err(ApiError::internal)?;
    rows.iter().map(message_from_row).collect()
}

async fn load_conversation(
    pool: &SqlitePool,
    user_id: UserId,
    id: &str,
) -> Result<ConversationView, ApiError> {
    ensure_conversation_member(pool, id, user_id).await?;
    let row = sqlx::query("SELECT kind, label, spot_id FROM conversations WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("conversation does not exist"))?;
    let kind_text: String = row.get("kind");
    let kind = match kind_text.as_str() {
        "direct" => ConversationKind::Direct,
        "circle" => ConversationKind::Circle,
        "hangout" => ConversationKind::Hangout,
        _ => return Err(ApiError::internal("unknown conversation kind")),
    };
    let member_rows = sqlx::query(
        "SELECT u.id, u.display_name FROM conversation_members cm JOIN users u ON u.id = cm.user_id WHERE cm.conversation_id = ? ORDER BY u.display_name COLLATE NOCASE",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(ApiError::internal)?;
    let members = member_rows
        .into_iter()
        .map(|member| {
            Ok(UserSummary {
                id: parse_uuid(&member.get::<String, _>("id"))?,
                display_name: member.get("display_name"),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let message_row = sqlx::query(
        "SELECT m.id, m.conversation_id, m.created_at, m.content_type, m.payload, m.encryption_version, u.id AS sender_id, u.display_name FROM messages m JOIN users u ON u.id = m.sender_id WHERE m.conversation_id = ? ORDER BY m.created_at DESC, m.id DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::internal)?;
    let last_message = message_row
        .as_ref()
        .map(message_from_row)
        .transpose()?
        .map(Box::new);
    let unread_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM messages m WHERE m.conversation_id = ? AND m.sender_id != ? AND m.created_at > COALESCE((SELECT last_read_at FROM message_reads WHERE conversation_id = ? AND user_id = ?), '0000-01-01T00:00:00Z')",
    )
    .bind(id)
    .bind(user_id.to_string())
    .bind(id)
    .bind(user_id.to_string())
    .fetch_one(pool)
    .await
    .map_err(ApiError::internal)?;
    let stored_label: String = row.get("label");
    let spot_id: Option<String> = row.get("spot_id");
    let label = if kind == ConversationKind::Direct {
        members
            .iter()
            .find(|member| member.id != user_id)
            .map_or(stored_label, |member| member.display_name.clone())
    } else {
        stored_label
    };
    Ok(ConversationView {
        id: id.into(),
        kind,
        label,
        spot_id,
        members,
        last_message,
        unread_count: u64::try_from(unread_count).unwrap_or(0),
    })
}

async fn load_spots(pool: &SqlitePool) -> Result<Vec<SpotView>, ApiError> {
    let rows = sqlx::query("SELECT id, name FROM spots ORDER BY name COLLATE NOCASE")
        .fetch_all(pool)
        .await
        .map_err(ApiError::internal)?;
    let mut spots = Vec::with_capacity(rows.len());
    for row in rows {
        let id: String = row.get("id");
        let active_hangout = sqlx::query_scalar::<_, String>(
            "SELECT id FROM hangouts WHERE spot_id = ? AND ended_at IS NULL ORDER BY created_at DESC LIMIT 1",
        )
        .bind(&id)
        .fetch_optional(pool)
        .await
        .map_err(ApiError::internal)?;
        let active_hangout_id = active_hangout.as_deref().map(parse_uuid).transpose()?;
        let members = if let Some(hangout_id) = active_hangout_id {
            let member_rows = sqlx::query(
                "SELECT u.id, u.display_name FROM hangout_members hm JOIN users u ON u.id = hm.user_id WHERE hm.hangout_id = ? AND hm.left_at IS NULL ORDER BY hm.joined_at",
            )
            .bind(hangout_id.to_string())
            .fetch_all(pool)
            .await
            .map_err(ApiError::internal)?;
            member_rows
                .into_iter()
                .map(|member| {
                    Ok(UserSummary {
                        id: parse_uuid(&member.get::<String, _>("id"))?,
                        display_name: member.get("display_name"),
                    })
                })
                .collect::<Result<Vec<_>, ApiError>>()?
        } else {
            Vec::new()
        };
        spots.push(SpotView {
            id,
            name: row.get("name"),
            active_hangout_id,
            members,
        });
    }
    Ok(spots)
}

async fn find_user(pool: &SqlitePool, selector: &str) -> Result<UserSummary, ApiError> {
    let row = if Uuid::parse_str(selector).is_ok() {
        sqlx::query("SELECT id, display_name FROM users WHERE id = ?")
            .bind(selector)
            .fetch_optional(pool)
            .await
            .map_err(ApiError::internal)?
    } else {
        sqlx::query("SELECT id, display_name FROM users WHERE display_name = ? COLLATE NOCASE")
            .bind(selector.trim())
            .fetch_optional(pool)
            .await
            .map_err(ApiError::internal)?
    }
    .ok_or_else(|| ApiError::not_found(format!("unknown user: {selector}")))?;
    Ok(UserSummary {
        id: parse_uuid(&row.get::<String, _>("id"))?,
        display_name: row.get("display_name"),
    })
}

async fn active_hangout_for(
    pool: &SqlitePool,
    user_id: UserId,
) -> Result<Option<HangoutId>, ApiError> {
    let value = sqlx::query_scalar::<_, String>(
        "SELECT hm.hangout_id FROM hangout_members hm JOIN hangouts h ON h.id = hm.hangout_id WHERE hm.user_id = ? AND hm.left_at IS NULL AND h.ended_at IS NULL ORDER BY hm.joined_at DESC LIMIT 1",
    ).bind(user_id.to_string()).fetch_optional(pool).await.map_err(ApiError::internal)?;
    value.as_deref().map(parse_uuid).transpose()
}

async fn active_hangout_for_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: UserId,
) -> Result<Option<HangoutId>, ApiError> {
    let value = sqlx::query_scalar::<_, String>(
        "SELECT hm.hangout_id FROM hangout_members hm JOIN hangouts h ON h.id = hm.hangout_id WHERE hm.user_id = ? AND hm.left_at IS NULL AND h.ended_at IS NULL ORDER BY hm.joined_at DESC LIMIT 1",
    ).bind(user_id.to_string()).fetch_optional(&mut **tx).await.map_err(ApiError::internal)?;
    value.as_deref().map(parse_uuid).transpose()
}

async fn load_hangouts(pool: &SqlitePool) -> Result<Vec<HangoutView>, ApiError> {
    let rows =
        sqlx::query("SELECT id, label FROM hangouts WHERE ended_at IS NULL ORDER BY created_at")
            .fetch_all(pool)
            .await
            .map_err(ApiError::internal)?;
    let mut hangouts = Vec::with_capacity(rows.len());
    for row in rows {
        let id = parse_uuid(&row.get::<String, _>("id"))?;
        let member_rows = sqlx::query(
            "SELECT u.id, u.display_name FROM hangout_members hm JOIN users u ON u.id = hm.user_id WHERE hm.hangout_id = ? AND hm.left_at IS NULL ORDER BY hm.joined_at",
        ).bind(id.to_string()).fetch_all(pool).await.map_err(ApiError::internal)?;
        let mut members = Vec::with_capacity(member_rows.len());
        for member in member_rows {
            members.push(UserSummary {
                id: parse_uuid(&member.get::<String, _>("id"))?,
                display_name: member.get("display_name"),
            });
        }
        if !members.is_empty() {
            hangouts.push(HangoutView {
                id,
                label: row.get("label"),
                members,
                sharing: vec![],
            });
        }
    }
    Ok(hangouts)
}

async fn leave_active_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: UserId,
) -> Result<(), ApiError> {
    sqlx::query("UPDATE hangout_members SET left_at = ? WHERE user_id = ? AND left_at IS NULL")
        .bind(Utc::now().to_rfc3339())
        .bind(user_id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(ApiError::internal)?;
    Ok(())
}

async fn add_member(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    hangout_id: HangoutId,
    user_id: UserId,
) -> Result<(), ApiError> {
    let already_member = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM hangout_members WHERE hangout_id = ? AND user_id = ? AND left_at IS NULL",
    ).bind(hangout_id.to_string()).bind(user_id.to_string()).fetch_one(&mut **tx).await.map_err(ApiError::internal)? > 0;
    if !already_member {
        sqlx::query("INSERT INTO hangout_members(hangout_id, user_id, joined_at) VALUES (?, ?, ?)")
            .bind(hangout_id.to_string())
            .bind(user_id.to_string())
            .bind(Utc::now().to_rfc3339())
            .execute(&mut **tx)
            .await
            .map_err(ApiError::internal)?;
    }
    sqlx::query(
        "INSERT OR IGNORE INTO conversation_members(conversation_id, user_id, joined_at) SELECT id, ?, ? FROM conversations WHERE hangout_id = ?",
    )
    .bind(user_id.to_string())
    .bind(Utc::now().to_rfc3339())
    .bind(hangout_id.to_string())
    .execute(&mut **tx)
    .await
    .map_err(ApiError::internal)?;
    Ok(())
}

async fn create_hangout_conversation(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    hangout_id: HangoutId,
    label: Option<&str>,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO conversations(id, kind, label, hangout_id, created_at) VALUES (?, 'hangout', ?, ?, ?)",
    )
    .bind(format!("hangout:{hangout_id}"))
    .bind(label.unwrap_or("Room"))
    .bind(hangout_id.to_string())
    .bind(Utc::now().to_rfc3339())
    .execute(&mut **tx)
    .await
    .map_err(ApiError::internal)?;
    Ok(())
}

async fn ensure_spot_conversation(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    spot_id: &str,
    label: &str,
) -> anyhow::Result<String> {
    let conversation_id = format!("spot:{spot_id}");
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO conversations(id, kind, label, spot_id, created_at) VALUES (?, 'hangout', ?, ?, ?)",
    )
    .bind(&conversation_id)
    .bind(label)
    .bind(spot_id)
    .bind(&now)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT OR IGNORE INTO conversation_members(conversation_id, user_id, joined_at) SELECT ?, user_id, ? FROM circle_members",
    )
    .bind(&conversation_id)
    .bind(&now)
    .execute(&mut **tx)
    .await?;
    Ok(conversation_id)
}

async fn end_if_empty(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    hangout_id: HangoutId,
) -> Result<(), ApiError> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM hangout_members WHERE hangout_id = ? AND left_at IS NULL",
    )
    .bind(hangout_id.to_string())
    .fetch_one(&mut **tx)
    .await
    .map_err(ApiError::internal)?;
    if count == 0 {
        sqlx::query("UPDATE hangouts SET ended_at = ? WHERE id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(hangout_id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(ApiError::internal)?;
    }
    Ok(())
}

#[derive(Serialize)]
struct LiveKitClaims<'a> {
    iss: &'a str,
    sub: String,
    name: &'a str,
    nbf: i64,
    exp: i64,
    video: LiveKitVideoGrant<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveKitVideoGrant<'a> {
    room_join: bool,
    room: &'a str,
    can_publish: bool,
    can_subscribe: bool,
}

fn issue_livekit_token(
    config: &AppConfig,
    user: &UserSummary,
    room: &str,
) -> Result<String, ApiError> {
    let now = Utc::now();
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
    let claims = LiveKitClaims {
        iss: &config.livekit_api_key,
        sub: user.id.to_string(),
        name: &user.display_name,
        nbf: (now - ChronoDuration::seconds(5)).timestamp(),
        exp: (now + ChronoDuration::minutes(15)).timestamp(),
        video: LiveKitVideoGrant {
            room_join: true,
            room,
            can_publish: true,
            can_subscribe: true,
        },
    };
    let body = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).map_err(ApiError::internal)?);
    let signing_input = format!("{header}.{body}");
    let mut mac = Hmac::<Sha256>::new_from_slice(config.livekit_api_secret.as_bytes())
        .map_err(ApiError::internal)?;
    mac.update(signing_input.as_bytes());
    let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    Ok(format!("{signing_input}.{signature}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> AppConfig {
        AppConfig {
            database_url: "sqlite::memory:".into(),
            livekit_url: "ws://127.0.0.1:7880".into(),
            livekit_api_key: "devkey".into(),
            livekit_api_secret: "wisp-local-development-secret-32".into(),
            knock_ttl: Duration::from_secs(30),
            allow_dev_sessions: true,
            bootstrap_token: Some("test-bootstrap-token".into()),
        }
    }

    #[tokio::test]
    async fn migrations_seed_four_stable_profiles() {
        let state = AppState::new(test_config()).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        assert_eq!(count, 4);
        let jared = find_user(&state.pool, "Jared").await.unwrap();
        assert_eq!(jared.id.to_string(), JARED_ID);
        let conversations = load_conversations(&state.pool, jared.id).await.unwrap();
        assert!(conversations.iter().any(|conversation| {
            conversation.kind == ConversationKind::Circle && conversation.label == "Friends"
        }));
        let porch = conversations
            .iter()
            .find(|conversation| conversation.label == "Porch")
            .expect("Porch conversation");
        assert_eq!(porch.id, format!("spot:{PORCH_ID}"));
        assert_eq!(porch.spot_id.as_deref(), Some(PORCH_ID));
        assert_eq!(porch.members.len(), 4);
        let spots = load_spots(&state.pool).await.unwrap();
        assert_eq!(spots.len(), 1);
        assert_eq!(spots[0].name, "Porch");
        assert!(spots[0].active_hangout_id.is_none());
    }

    #[test]
    fn livekit_token_has_three_segments() {
        let user = UserSummary {
            id: Uuid::parse_str(JARED_ID).unwrap(),
            display_name: "Jared".into(),
        };
        let token = issue_livekit_token(&test_config(), &user, "wisp-test").unwrap();
        assert_eq!(token.split('.').count(), 3);
    }

    #[test]
    fn join_permissions_follow_presence_and_connectivity() {
        assert_eq!(join_policy(Presence::Open, true), JoinPolicy::Direct);
        assert_eq!(join_policy(Presence::Knock, true), JoinPolicy::Knock);
        assert_eq!(join_policy(Presence::Closed, true), JoinPolicy::Unavailable);
        assert_eq!(join_policy(Presence::Away, true), JoinPolicy::Unavailable);
        assert_eq!(join_policy(Presence::Open, false), JoinPolicy::Offline);
    }

    #[test]
    fn credentials_are_hashed_and_protocol_versions_are_strict() {
        let secret = random_token("test");
        assert!(!token_hash(&secret).contains(&secret));
        assert!(require_protocol(PROTOCOL_VERSION).is_ok());
        assert_eq!(
            require_protocol(PROTOCOL_VERSION + 1).unwrap_err().code,
            "unsupported_protocol_version"
        );
    }

    #[tokio::test]
    async fn direct_conversations_are_scoped_to_their_members() {
        let state = AppState::new(test_config()).await.unwrap();
        let jared = find_user(&state.pool, "Jared").await.unwrap();
        let tyler = find_user(&state.pool, "Tyler").await.unwrap();
        let charlie = find_user(&state.pool, "Charlie").await.unwrap();
        let id = find_or_create_direct(&state.pool, jared.id, tyler.id)
            .await
            .unwrap();
        assert!(
            ensure_conversation_member(&state.pool, &id, jared.id)
                .await
                .is_ok()
        );
        assert!(
            ensure_conversation_member(&state.pool, &id, tyler.id)
                .await
                .is_ok()
        );
        assert!(
            ensure_conversation_member(&state.pool, &id, charlie.id)
                .await
                .is_err()
        );
        assert_eq!(
            find_or_create_direct(&state.pool, tyler.id, jared.id)
                .await
                .unwrap(),
            id
        );
    }

    #[tokio::test]
    async fn spot_messages_survive_transient_hangout_retention() {
        let state = AppState::new(test_config()).await.unwrap();
        let old = "2000-01-01T00:00:00Z";
        let jared = find_user(&state.pool, "Jared").await.unwrap();
        let transient_hangout = Uuid::new_v4();
        let transient_conversation = format!("hangout:{transient_hangout}");

        sqlx::query(
            "INSERT INTO hangouts(id, livekit_room, created_at, ended_at) VALUES (?, ?, ?, ?)",
        )
        .bind(transient_hangout.to_string())
        .bind(format!("wisp-{transient_hangout}"))
        .bind(old)
        .bind(old)
        .execute(&state.pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO conversations(id, kind, label, hangout_id, created_at) VALUES (?, 'hangout', 'Old room', ?, ?)")
            .bind(&transient_conversation)
            .bind(transient_hangout.to_string())
            .bind(old)
            .execute(&state.pool)
            .await
            .unwrap();
        for (id, conversation_id, payload) in [
            (
                Uuid::new_v4().to_string(),
                format!("spot:{PORCH_ID}"),
                "persistent",
            ),
            (
                Uuid::new_v4().to_string(),
                transient_conversation.clone(),
                "temporary",
            ),
        ] {
            sqlx::query("INSERT INTO messages(id, conversation_id, sender_id, created_at, content_type, payload, encryption_version) VALUES (?, ?, ?, ?, 'text/plain', ?, 0)")
                .bind(id)
                .bind(conversation_id)
                .bind(jared.id.to_string())
                .bind(old)
                .bind(payload)
                .execute(&state.pool)
                .await
                .unwrap();
        }

        cleanup_expired_messages(&state.pool).await.unwrap();

        let porch_messages: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE conversation_id = ?")
                .bind(format!("spot:{PORCH_ID}"))
                .fetch_one(&state.pool)
                .await
                .unwrap();
        let transient_messages: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE conversation_id = ?")
                .bind(transient_conversation)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(porch_messages, 1);
        assert_eq!(transient_messages, 0);
    }

    #[tokio::test]
    async fn empty_hangout_is_ended() {
        let state = AppState::new(test_config()).await.unwrap();
        let jared = find_user(&state.pool, "Jared").await.unwrap();
        let hangout_id = Uuid::new_v4();
        let mut tx = state.pool.begin().await.unwrap();
        sqlx::query("INSERT INTO hangouts(id, livekit_room, created_at) VALUES (?, ?, ?)")
            .bind(hangout_id.to_string())
            .bind(format!("wisp-{hangout_id}"))
            .bind(Utc::now().to_rfc3339())
            .execute(&mut *tx)
            .await
            .unwrap();
        add_member(&mut tx, hangout_id, jared.id).await.unwrap();
        leave_active_in_transaction(&mut tx, jared.id)
            .await
            .unwrap();
        end_if_empty(&mut tx, hangout_id).await.unwrap();
        tx.commit().await.unwrap();

        let ended: Option<String> =
            sqlx::query_scalar("SELECT ended_at FROM hangouts WHERE id = ?")
                .bind(hangout_id.to_string())
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert!(ended.is_some());
    }
}
