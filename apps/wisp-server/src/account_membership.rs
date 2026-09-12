//! Authentication is independent of admission to this community. No private
//! chat/media key or share-link secret is accepted by the account service.
use super::{
    ApiError, AppState, ChronoDuration, DeviceCredential, HeaderMap, Instant, IntoResponse, Json,
    LOGIN_FAILURE_WINDOW, Next, Path, Request, Response, Row, SqlitePool, State, StatusCode,
    UserId, Utc, Uuid, Value, authenticate_headers, create_device, find_user, hash_password,
    leave_active_in_transaction, login_rate_key, parse_uuid, random_token, require_protocol,
    room_access, server_management, token_hash, validate_device_name, validate_display_name,
    validate_password, validate_username,
};
use serde::Deserialize;
use serde_json::json;

pub(super) async fn is_member(pool: &SqlitePool, user: UserId) -> Result<bool, ApiError> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id=? AND server_member=1)")
        .bind(user.to_string())
        .fetch_one(pool)
        .await
        .map_err(ApiError::internal)
}

pub(super) async fn require_member(pool: &SqlitePool, user: UserId) -> Result<(), ApiError> {
    if !is_member(pool, user).await? {
        return Err(ApiError::forbidden(
            "Join this server before accessing its rooms or members",
        ));
    }
    Ok(())
}

pub(super) async fn gate(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let path = request.uri().path();
    let community = path.starts_with("/v1/server/")
        || path.starts_with("/v1/rooms")
        || path.starts_with("/v1/room-invitations")
        || path.starts_with("/v1/spots/")
        || path.starts_with("/v1/livekit/")
        || path.starts_with("/v1/soundboard")
        || path == "/v1/account-invites"
        || path == "/v1/admin/invites"
        || (path.starts_with("/v1/hangouts/") && path != "/v1/hangouts/leave")
        || path.starts_with("/v1/knocks/");
    if community {
        let user = authenticate_headers(&state, request.headers()).await?;
        require_member(&state.pool, user).await?;
    }
    Ok(next.run(request).await)
}

fn limited() -> ApiError {
    ApiError {
        status: StatusCode::TOO_MANY_REQUESTS,
        code: "rate_limited",
        message: "Too many attempts. Please try again shortly.".into(),
    }
}

// Count successful operations too; expensive public signup and directory
// discovery cannot be limited only by failed password attempts.
pub(super) async fn rate(state: &AppState, key: String, limit: usize) -> Result<(), ApiError> {
    let now = Instant::now();
    let mut entries = state.login_failures.lock().await;
    entries.retain(|_, attempts| {
        attempts.retain(|at| now.duration_since(*at) < LOGIN_FAILURE_WINDOW);
        !attempts.is_empty()
    });
    if entries.len() >= 10_000 && !entries.contains_key(&key) {
        return Err(limited());
    }
    let attempts = entries.entry(key).or_default();
    if attempts.len() >= limit {
        return Err(limited());
    }
    attempts.push_back(now);
    Ok(())
}

pub(super) async fn capabilities() -> Json<Value> {
    Json(json!({"accounts_without_membership":true,"server_invites":2,"public_handles":true}))
}

pub(super) async fn overview(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    let handle: Option<String> = sqlx::query_scalar("SELECT public_handle FROM users WHERE id=?")
        .bind(user.to_string())
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    let blocked=sqlx::query("SELECT u.id,u.display_name FROM account_blocks b JOIN users u ON u.id=b.blocked_id WHERE b.user_id=? ORDER BY u.display_name COLLATE NOCASE").bind(user.to_string()).fetch_all(&state.pool).await.map_err(ApiError::internal)?.into_iter().map(|r|json!({"id":r.get::<String,_>("id"),"display_name":r.get::<String,_>("display_name")})).collect::<Vec<_>>();
    Ok(Json(
        json!({"handle":handle,"server_member":is_member(&state.pool,user).await?,"blocked":blocked}),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Registration {
    username: String,
    display_name: String,
    password: String,
    device_name: String,
    protocol_version: u8,
}

pub(super) async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Registration>,
) -> Result<Json<DeviceCredential>, ApiError> {
    require_protocol(request.protocol_version)?;
    let handle = validate_username(&request.username)?.to_ascii_lowercase();
    let name = validate_display_name(&request.display_name)?;
    validate_password(&request.password)?;
    let device = validate_device_name(&request.device_name)?;
    rate(&state, login_rate_key(&headers, "public-registration"), 8).await?;
    let _permit = state.password_work.try_acquire().map_err(|_| limited())?;
    let password = hash_password(request.password.clone()).await?;
    let user = Uuid::new_v4();
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    sqlx::query("INSERT INTO users(id,display_name,username,public_handle,password_hash,created_at,server_member) VALUES(?,?,?,?,?,?,0)")
        .bind(user.to_string()).bind(name).bind(&handle).bind(&handle).bind(password).bind(Utc::now().to_rfc3339())
        .execute(&mut *tx).await.map_err(|error| {
            if error.as_database_error().is_some_and(sqlx::error::DatabaseError::is_unique_violation) {
                ApiError::conflict("account_exists", "That username is unavailable. Choose another or sign in.")
            } else { ApiError::internal(error) }
        })?;
    let credential = create_device(&mut tx, user, device).await?;
    tx.commit().await.map_err(ApiError::internal)?;
    // No public admission, friendship, voice, or global signup event.
    Ok(Json(credential))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Handle {
    handle: String,
}

pub(super) async fn set_handle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Handle>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    rate(&state, format!("handle:{user}"), 8).await?;
    let handle = if request.handle.trim().is_empty() {
        None
    } else {
        Some(validate_username(request.handle.trim().trim_start_matches('@'))?.to_ascii_lowercase())
    };
    sqlx::query("UPDATE users SET public_handle=? WHERE id=?")
        .bind(&handle)
        .bind(user.to_string())
        .execute(&state.pool)
        .await
        .map_err(|error| {
            if error
                .as_database_error()
                .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
            {
                ApiError::conflict("handle_unavailable", "That public username is unavailable")
            } else {
                ApiError::internal(error)
            }
        })?;
    Ok(Json(json!({"handle":handle})))
}

pub(super) async fn blocked(
    pool: &SqlitePool,
    user: UserId,
    other: UserId,
) -> Result<bool, ApiError> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_blocks WHERE (user_id=?1 AND blocked_id=?2) OR (user_id=?2 AND blocked_id=?1))")
        .bind(user.to_string()).bind(other.to_string()).fetch_one(pool).await.map_err(ApiError::internal)
}

pub(super) async fn lookup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Handle>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    rate(&state, format!("lookup:{user}"), 60).await?;
    let handle = validate_username(request.handle.trim().trim_start_matches('@'))?;
    let row = sqlx::query(
        "SELECT id,display_name,public_handle FROM users WHERE public_handle=? COLLATE NOCASE",
    )
    .bind(handle)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    if let Some(row) = row {
        let other = parse_uuid(&row.get::<String, _>("id"))?;
        if other != user && !blocked(&state.pool, user, other).await? {
            return Ok(Json(
                json!({"person":{"id":other,"display_name":row.get::<String,_>("display_name"),"handle":row.get::<String,_>("public_handle")}}),
            ));
        }
    }
    Ok(Json(json!({"person":null})))
}

pub(super) async fn can_view_person(
    pool: &SqlitePool,
    user: UserId,
    other: UserId,
) -> Result<bool, ApiError> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users u WHERE u.id=?2 AND (u.id=?1 OR (u.server_member=1 AND EXISTS(SELECT 1 FROM users a WHERE a.id=?1 AND a.server_member=1)) OR u.public_handle IS NOT NULL OR EXISTS(SELECT 1 FROM friendships f WHERE (f.first_user_id=?1 AND f.second_user_id=?2) OR (f.first_user_id=?2 AND f.second_user_id=?1)) OR EXISTS(SELECT 1 FROM friend_requests r WHERE (r.sender_id=?1 AND r.recipient_id=?2) OR (r.sender_id=?2 AND r.recipient_id=?1))))")
        .bind(user.to_string()).bind(other.to_string()).fetch_one(pool).await.map_err(ApiError::internal)
}

pub(super) async fn block(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(other): Path<UserId>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    if user == other || !can_view_person(&state.pool, user, other).await? {
        return Err(ApiError::not_found("Person unavailable"));
    }
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    sqlx::query("INSERT OR IGNORE INTO account_blocks(user_id,blocked_id) VALUES(?,?)")
        .bind(user.to_string())
        .bind(other.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query("DELETE FROM friend_requests WHERE (sender_id=?1 AND recipient_id=?2) OR (sender_id=?2 AND recipient_id=?1)").bind(user.to_string()).bind(other.to_string()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    sqlx::query("DELETE FROM friendships WHERE (first_user_id=?1 AND second_user_id=?2) OR (first_user_id=?2 AND second_user_id=?1)").bind(user.to_string()).bind(other.to_string()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .emit("friendship_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn unblock(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(other): Path<UserId>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    sqlx::query("DELETE FROM account_blocks WHERE user_id=? AND blocked_id=?")
        .bind(user.to_string())
        .bind(other.to_string())
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"ok":true})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InviteOptions {
    expires_in_minutes: Option<u32>,
}

pub(super) async fn create_invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<InviteOptions>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    require_member(&state.pool, user).await?;
    rate(&state, format!("invite:{user}"), 30).await?;
    let id = Uuid::new_v4();
    let code = random_token("wisp-server-invite");
    let expires = Utc::now()
        + ChronoDuration::minutes(
            i64::from(request.expires_in_minutes.unwrap_or(720)).clamp(1, 720),
        );
    sqlx::query("INSERT INTO server_invites(id,code_hash,created_by,created_at,expires_at) VALUES(?,?,?,?,?)")
        .bind(id.to_string()).bind(token_hash(&code)).bind(user.to_string()).bind(Utc::now().to_rfc3339()).bind(expires.to_rfc3339()).execute(&state.pool).await.map_err(ApiError::internal)?;
    let short_label = super::short_invitations::reserve(&state, id).await?;
    let short_origin = state
        .config
        .invite_url
        .as_ref()
        .or(state.config.public_url.as_ref());
    let name: String = sqlx::query_scalar("SELECT name FROM server_identity WHERE id=1")
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    let inviter = find_user(&state.pool, &user.to_string()).await?;
    Ok(Json(
        json!({"id":id,"code":code,"expires_at":expires,"server_name":name,"inviter":inviter,"kind":"server","short_label":short_label,"short_origin":short_origin}),
    ))
}

pub(super) async fn list_invites(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    require_member(&state.pool, user).await?;
    let manager = server_management::is_manager(&state.pool, user).await?;
    let rows = sqlx::query("SELECT id,created_at,expires_at FROM server_invites WHERE (created_by=? OR ?=1) AND used_at IS NULL AND revoked_at IS NULL AND expires_at>? ORDER BY created_at DESC LIMIT 100")
        .bind(user.to_string()).bind(manager).bind(Utc::now().to_rfc3339()).fetch_all(&state.pool).await.map_err(ApiError::internal)?;
    Ok(Json(
        json!({"invites":rows.into_iter().map(|r|json!({"id":r.get::<String,_>("id"),"created_at":r.get::<String,_>("created_at"),"expires_at":r.get::<String,_>("expires_at")})).collect::<Vec<_>>()}),
    ))
}

pub(super) async fn revoke_invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    let manager = server_management::is_manager(&state.pool, user).await?;
    let changed = sqlx::query(
        "UPDATE server_invites SET revoked_at=?,envelope=NULL WHERE id=? AND (created_by=? OR ?=1)",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(id.to_string())
    .bind(user.to_string())
    .bind(manager)
    .execute(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    if changed.rows_affected() == 0 {
        return Err(ApiError::not_found("Invitation unavailable"));
    }
    Ok(Json(json!({"ok":true})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Envelope {
    lookup_id: String,
    envelope: String,
}

pub(super) fn encoded(value: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&value.len())
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}

pub(super) async fn store_envelope(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<Envelope>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    require_member(&state.pool, user).await?;
    if !encoded(&request.lookup_id, 43, 43) || !encoded(&request.envelope, 40, 12000) {
        return Err(ApiError::bad_request(
            "invalid_envelope",
            "Invalid encrypted invitation",
        ));
    }
    // One upload only: an already shared invitation cannot be replaced.
    let changed = sqlx::query("UPDATE server_invites SET lookup_id=?,envelope=? WHERE id=? AND created_by=? AND envelope IS NULL AND used_at IS NULL AND revoked_at IS NULL AND expires_at>?")
        .bind(&request.lookup_id).bind(&request.envelope).bind(id.to_string()).bind(user.to_string()).bind(Utc::now().to_rfc3339()).execute(&state.pool).await.map_err(ApiError::internal)?;
    if changed.rows_affected() != 1 {
        return Err(ApiError::conflict(
            "invite_unavailable",
            "Invitation unavailable",
        ));
    }
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn resolve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    rate(&state, login_rate_key(&headers, "resolve-invitation"), 120).await?;
    if !encoded(&id, 43, 43) {
        return Err(ApiError::not_found("Invitation unavailable"));
    }
    let envelope: Option<String> = sqlx::query_scalar("SELECT envelope FROM server_invites i JOIN users u ON u.id=i.created_by WHERE lookup_id=? AND u.server_member=1 AND used_at IS NULL AND revoked_at IS NULL AND expires_at>? AND envelope IS NOT NULL")
        .bind(id).bind(Utc::now().to_rfc3339()).fetch_optional(&state.pool).await.map_err(ApiError::internal)?;
    Ok(Json(
        json!({"envelope":envelope.ok_or_else(||ApiError::not_found("This invitation has expired, was used, or was revoked"))?}),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Acceptance {
    code: String,
}

pub(super) async fn join(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Acceptance>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    rate(&state, format!("accept:{user}"), 30).await?;
    if request.code.len() > 256 {
        return Err(ApiError::bad_request(
            "invalid_invite",
            "Invalid invitation",
        ));
    }
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    // Begin with a write so concurrent redemption cannot both observe unused.
    let changed = sqlx::query("UPDATE server_invites SET used_at=?,used_by=?,envelope=NULL WHERE code_hash=? AND used_at IS NULL AND revoked_at IS NULL AND expires_at>? AND EXISTS(SELECT 1 FROM users u WHERE u.id=server_invites.created_by AND u.server_member=1)")
        .bind(Utc::now().to_rfc3339()).bind(user.to_string()).bind(token_hash(&request.code)).bind(Utc::now().to_rfc3339()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    if changed.rows_affected() != 1 {
        let retry: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_invites i JOIN users u ON u.id=i.used_by WHERE i.code_hash=? AND i.used_by=? AND u.server_member=1 AND i.revoked_at IS NULL)")
            .bind(token_hash(&request.code)).bind(user.to_string()).fetch_one(&mut *tx).await.map_err(ApiError::internal)?;
        if !retry {
            return Err(ApiError::bad_request(
                "invalid_invite",
                "This invitation has expired, was used, or was revoked",
            ));
        }
    }
    sqlx::query("UPDATE users SET server_member=1 WHERE id=?")
        .bind(user.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    room_access::queue_public_admissions(&mut tx).await?;
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .emit("server_membership_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true,"server_member":true})))
}

pub(super) async fn leave(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    if server_management::is_owner(&state.pool, user).await? {
        return Err(ApiError::forbidden(
            "The server owner cannot leave their server",
        ));
    }
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    sqlx::query("UPDATE users SET server_member=0 WHERE id=?")
        .bind(user.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query("DELETE FROM server_admins WHERE user_id=?")
        .bind(user.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query("DELETE FROM pending_room_admissions WHERE user_id=?")
        .bind(user.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query("UPDATE server_invites SET revoked_at=?,envelope=NULL WHERE created_by=? AND used_at IS NULL").bind(Utc::now().to_rfc3339()).bind(user.to_string()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    leave_active_in_transaction(&mut tx, user).await?;
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .emit("server_membership_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true,"server_member":false})))
}

pub(super) async fn invite_page() -> Response {
    invite_asset("text/html; charset=utf-8", include_str!("invite_page.html"))
}
pub(super) async fn invite_script() -> Response {
    invite_asset(
        "text/javascript; charset=utf-8",
        include_str!("invite_page.js"),
    )
}
pub(super) async fn invite_style() -> Response {
    invite_asset("text/css; charset=utf-8", include_str!("invite_page.css"))
}
fn invite_asset(content_type: &'static str, body: &'static str) -> Response {
    ([("content-type",content_type),("cache-control","no-store"),("referrer-policy","no-referrer"),("x-content-type-options","nosniff"),("content-security-policy","default-src 'none'; script-src 'self'; style-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'")],body).into_response()
}

pub(super) async fn require_unblocked_chat(
    pool: &SqlitePool,
    user: UserId,
    conversation: &str,
) -> Result<(), ApiError> {
    let mut connection = pool.acquire().await.map_err(ApiError::internal)?;
    require_unblocked_chat_tx(&mut connection, user, conversation).await
}

pub(super) async fn require_unblocked_chat_tx(
    connection: &mut sqlx::SqliteConnection,
    user: UserId,
    conversation: &str,
) -> Result<(), ApiError> {
    let blocked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM conversations c JOIN conversation_members cm ON cm.conversation_id=c.id JOIN account_blocks b ON (b.user_id=?1 AND b.blocked_id=cm.user_id) OR (b.blocked_id=?1 AND b.user_id=cm.user_id) WHERE c.id=?2 AND c.kind='direct' AND cm.user_id!=?1)")
        .bind(user.to_string()).bind(conversation).fetch_one(connection).await.map_err(ApiError::internal)?;
    if blocked {
        return Err(ApiError::forbidden(
            "This conversation is unavailable for new messages",
        ));
    }
    Ok(())
}
