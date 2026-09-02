use axum::{
    Json, Router,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message as WsMessage, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
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
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};
use tokio::sync::{RwLock, broadcast};
use tower_http::trace::TraceLayer;
use tracing::{debug, info, warn};
use uuid::Uuid;
use wisp_protocol::{
    ConnectionState, DevSession, DevSessionRequest, FriendState, HangoutId, HangoutView,
    JoinFriendRequest, JoinFriendResult, JoinHangoutRequest, KnockId, KnockRequestView,
    KnockResponse, LiveKitTokenResponse, Message, Presence, ProtocolError, RespondKnockRequest,
    RespondKnockResult, SendMessageRequest, ServerEvent, SetPresenceRequest, Snapshot, UserId,
    UserSummary,
};

const JARED_ID: &str = "00000000-0000-4000-8000-000000000001";
const TYLER_ID: &str = "00000000-0000-4000-8000-000000000002";
const JACK_ID: &str = "00000000-0000-4000-8000-000000000003";
const CHARLIE_ID: &str = "00000000-0000-4000-8000-000000000004";
const CIRCLE_ID: &str = "00000000-0000-4000-8000-000000000010";

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub livekit_url: String,
    pub livekit_api_key: String,
    pub livekit_api_secret: String,
    pub knock_ttl: Duration,
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
            .journal_mode(SqliteJournalMode::Wal);
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
        .route("/v1/snapshot", get(snapshot))
        .route("/v1/events", get(events))
        .route("/v1/presence", post(set_presence))
        .route("/v1/hangouts/join-friend", post(join_friend))
        .route("/v1/hangouts/join", post(join_hangout))
        .route("/v1/knocks/respond", post(respond_knock))
        .route("/v1/hangouts/leave", post(leave_hangout))
        .route("/v1/livekit/token", post(livekit_token))
        .route("/v1/messages", get(list_messages).post(send_message))
        .route("/v1/users/{id}", get(user_by_id))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<Value> {
    let connected_clients = state.runtime.read().await.connected_clients;
    Json(json!({"ok": true, "connected_clients": connected_clients}))
}

async fn dev_session(
    State(state): State<AppState>,
    Json(request): Json<DevSessionRequest>,
) -> Result<Json<DevSession>, ApiError> {
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
    }))
}

async fn snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Snapshot>, ApiError> {
    let user_id = authenticate_headers(&headers)?;
    Ok(Json(state.snapshot(user_id).await?))
}

async fn set_presence(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetPresenceRequest>,
) -> Result<Json<Value>, ApiError> {
    let user_id = authenticate_headers(&headers)?;
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
    let self_id = authenticate_headers(&headers)?;
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
    let recipient_id = authenticate_headers(&headers)?;
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
    let self_id = authenticate_headers(&headers)?;
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM hangouts WHERE id = ? AND ended_at IS NULL",
    )
    .bind(request.hangout_id.to_string())
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::internal)?
        > 0;
    if !exists {
        return Err(ApiError::not_found("hangout is no longer active"));
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
    let self_id = authenticate_headers(&headers)?;
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

async fn livekit_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LiveKitTokenResponse>, ApiError> {
    let user_id = authenticate_headers(&headers)?;
    let user = find_user(&state.pool, &user_id.to_string()).await?;
    let hangout_id = active_hangout_for(&state.pool, user_id)
        .await?
        .ok_or_else(|| ApiError::bad_request("not_in_hangout", "join a hangout first"))?;
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
}

async fn list_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<MessageQuery>,
) -> Result<Json<Vec<Message>>, ApiError> {
    let _user_id = authenticate_headers(&headers)?;
    let rows = sqlx::query(
        "SELECT m.id, m.conversation_id, m.created_at, m.content_type, m.payload, m.encryption_version, u.id AS sender_id, u.display_name FROM messages m JOIN users u ON u.id = m.sender_id WHERE m.conversation_id = ? ORDER BY m.created_at, m.id LIMIT 200",
    ).bind(&query.conversation_id).fetch_all(&state.pool).await.map_err(ApiError::internal)?;
    let mut messages = Vec::with_capacity(rows.len());
    for row in rows {
        messages.push(Message {
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
        });
    }
    Ok(Json(messages))
}

async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SendMessageRequest>,
) -> Result<Json<Message>, ApiError> {
    let sender_id = authenticate_headers(&headers)?;
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
    sqlx::query("INSERT INTO messages(id, conversation_id, sender_id, created_at, content_type, payload, encryption_version) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(message.id.to_string()).bind(&message.conversation_id).bind(sender_id.to_string())
        .bind(message.created_at.to_rfc3339()).bind(&message.content_type)
        .bind(serde_json::to_string(&message.payload).map_err(ApiError::internal)?)
        .bind(message.encryption_version).execute(&state.pool).await.map_err(ApiError::internal)?;
    state
        .emit(
            "message_created",
            json!({"message_id": message.id, "conversation_id": message.conversation_id}),
        )
        .await;
    Ok(Json(message))
}

async fn user_by_id(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<UserSummary>, ApiError> {
    let _user_id = authenticate_headers(&headers)?;
    Ok(Json(find_user(&state.pool, &id).await?))
}

#[derive(Debug, Deserialize)]
struct EventQuery {
    token: String,
}

async fn events(
    State(state): State<AppState>,
    Query(query): Query<EventQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let user_id = authenticate_token(&query.token)?;
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
    }
    tx.commit().await?;
    info!("development profiles ready");
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

fn authenticate_headers(headers: &HeaderMap) -> Result<UserId, ApiError> {
    let value = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::unauthorized("missing bearer token"))?;
    authenticate_token(value.strip_prefix("Bearer ").unwrap_or(value))
}

fn authenticate_token(token: &str) -> Result<UserId, ApiError> {
    let id = token
        .strip_prefix("dev:")
        .ok_or_else(|| ApiError::unauthorized("invalid development token"))?;
    Uuid::parse_str(id).map_err(|_| ApiError::unauthorized("invalid development token"))
}

fn parse_uuid(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value).map_err(ApiError::internal)
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
    Ok(())
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
