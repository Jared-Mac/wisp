//! Community membership moderation never deletes an account or its friendships.
use super::{ApiError, AppState, HeaderMap, Json, Row, State, Utc, Uuid, Value};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::json;
use sha2::Sha256;

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Action {
    Kick,
    Ban,
    Unban,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    user_id: Uuid,
    action: Action,
    #[serde(default)]
    reason: String,
}

pub(super) async fn ensure_not_banned(
    db: &mut sqlx::SqliteConnection,
    user: Uuid,
) -> Result<(), ApiError> {
    let banned: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_bans WHERE user_id=?)")
            .bind(user.to_string())
            .fetch_one(db)
            .await
            .map_err(ApiError::internal)?;
    if banned {
        return Err(ApiError::forbidden("You are banned from this server"));
    }
    Ok(())
}

pub(super) async fn bans(state: &AppState) -> Result<Vec<Value>, ApiError> {
    Ok(sqlx::query("SELECT u.id,u.display_name,b.banned_at,b.reason FROM server_bans b JOIN users u ON u.id=b.user_id ORDER BY u.display_name COLLATE NOCASE,u.id")
        .fetch_all(&state.pool).await.map_err(ApiError::internal)?.into_iter()
        .map(|r| json!({"id":r.get::<String,_>("id"),"display_name":r.get::<String,_>("display_name"),"banned_at":r.get::<String,_>("banned_at"),"reason":r.get::<String,_>("reason")})).collect())
}

#[allow(clippy::too_many_lines)] // Authorization, removal and pending disconnect commit together.
pub(super) async fn moderate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Request>,
) -> Result<Json<Value>, ApiError> {
    let actor = super::server_management::require_manager(&state, &headers).await?;
    let reason = request.reason.trim();
    if reason.chars().count() > 280 || reason.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "invalid_reason",
            "Use up to 280 printable characters for the reason",
        ));
    }
    if actor == request.user_id {
        return Err(ApiError::forbidden("You cannot kick or ban yourself"));
    }
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(ApiError::internal)?;
    let roles = sqlx::query("SELECT EXISTS(SELECT 1 FROM server_identity WHERE owner_user_id=?1) actor_owner,EXISTS(SELECT 1 FROM server_admins a JOIN users u ON u.id=a.user_id WHERE a.user_id=?1 AND u.server_member=1) actor_admin,EXISTS(SELECT 1 FROM server_identity WHERE owner_user_id=?2) target_owner,EXISTS(SELECT 1 FROM server_admins WHERE user_id=?2) target_admin")
        .bind(actor.to_string()).bind(request.user_id.to_string()).fetch_one(&mut *tx).await.map_err(ApiError::internal)?;
    if !roles.get::<bool, _>("actor_owner") && !roles.get::<bool, _>("actor_admin") {
        return Err(ApiError::forbidden("Server administrator required"));
    }
    if roles.get::<bool, _>("target_owner") {
        return Err(ApiError::forbidden(
            "The server owner cannot be kicked or banned",
        ));
    }
    if roles.get::<bool, _>("target_admin") && !roles.get::<bool, _>("actor_owner") {
        return Err(ApiError::forbidden(
            "Only the owner can kick or ban an administrator",
        ));
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id=?)")
        .bind(request.user_id.to_string())
        .fetch_one(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    if !exists {
        return Err(ApiError::not_found("Account does not exist"));
    }
    if request.action == Action::Unban {
        sqlx::query("DELETE FROM server_bans WHERE user_id=?")
            .bind(request.user_id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    } else {
        let user = request.user_id.to_string();
        if request.action == Action::Ban {
            sqlx::query("INSERT INTO server_bans(user_id,banned_by,banned_at,reason) VALUES(?,?,?,?) ON CONFLICT(user_id) DO NOTHING")
                .bind(&user).bind(actor.to_string()).bind(Utc::now().to_rfc3339()).bind(reason).execute(&mut *tx).await.map_err(ApiError::internal)?;
        }
        sqlx::query("INSERT OR IGNORE INTO server_member_disconnects(user_id,room) SELECT hm.user_id,h.livekit_room FROM hangout_members hm JOIN hangouts h ON h.id=hm.hangout_id WHERE hm.user_id=? AND hm.left_at IS NULL AND h.ended_at IS NULL")
            .bind(&user).execute(&mut *tx).await.map_err(ApiError::internal)?;
        sqlx::query("UPDATE users SET server_member=0 WHERE id=?")
            .bind(&user)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
        sqlx::query("DELETE FROM server_admins WHERE user_id=?")
            .bind(&user)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
        sqlx::query("DELETE FROM pending_room_admissions WHERE user_id=? OR invited_by=?")
            .bind(&user)
            .bind(&user)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
        sqlx::query("DELETE FROM channel_allowed_users WHERE user_id=?")
            .bind(&user)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
        sqlx::query("UPDATE server_invites SET revoked_at=?,envelope=NULL WHERE created_by=? AND used_at IS NULL").bind(Utc::now().to_rfc3339()).bind(&user).execute(&mut *tx).await.map_err(ApiError::internal)?;
        sqlx::query("DELETE FROM account_invites WHERE created_by=? AND used_at IS NULL")
            .bind(&user)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
        sqlx::query("UPDATE room_invitations SET status='dismissed',encrypted_membership=NULL WHERE (sender_id=? OR recipient_id=?) AND status='pending'")
            .bind(&user).bind(&user).execute(&mut *tx).await.map_err(ApiError::internal)?;
        super::leave_active_in_transaction(&mut tx, request.user_id).await?;
    }
    tx.commit().await.map_err(ApiError::internal)?;
    if request.action != Action::Unban {
        state
            .runtime
            .write()
            .await
            .knocks
            .retain(|_, knock| knock.from.id != request.user_id && knock.to != request.user_id);
    }
    state
        .emit("server_membership_changed", json!({"changed":true}))
        .await;
    state
        .emit("server_settings_changed", json!({"changed":true}))
        .await;
    // Membership is already revoked even if the media service is temporarily down.
    let _ = disconnect_pending(&state, Some(request.user_id)).await;
    let media_pending: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM server_member_disconnects WHERE user_id=?)",
    )
    .bind(request.user_id.to_string())
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    Ok(Json(json!({"ok":true,"media_pending":media_pending})))
}

async fn remove_participant(state: &AppState, room: &str, user: &str) -> Result<(), ApiError> {
    let now = Utc::now().timestamp();
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
    let claims = json!({"iss":state.config.livekit_api_key,"nbf":now-5,"exp":now+60,"video":{"roomAdmin":true,"room":room}});
    let input = format!("{header}.{}", URL_SAFE_NO_PAD.encode(claims.to_string()));
    let mut mac = Hmac::<Sha256>::new_from_slice(state.config.livekit_api_secret.as_bytes())
        .map_err(ApiError::internal)?;
    mac.update(input.as_bytes());
    let token = format!(
        "{input}.{}",
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    );
    let origin = state
        .config
        .livekit_url
        .replacen("wss://", "https://", 1)
        .replacen("ws://", "http://", 1);
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(ApiError::internal)?
        .post(format!(
            "{}/twirp/livekit.RoomService/RemoveParticipant",
            origin.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .json(&json!({"room":room,"identity":user}))
        .send()
        .await
        .map_err(|_| ApiError::conflict("media_unavailable", "Voice disconnect is pending"))?;
    if response.status().is_success() || response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(());
    }
    Err(ApiError::conflict(
        "media_unavailable",
        "Voice disconnect is pending",
    ))
}

async fn disconnect_pending(state: &AppState, user: Option<Uuid>) -> Result<(), ApiError> {
    // The request path and retry worker must not race a stale removal against
    // a newly admitted connection. Busy requests report the durable pending job.
    let Ok(_worker) = state.member_disconnects.try_lock() else {
        return Ok(());
    };
    let rows=sqlx::query("SELECT user_id,room FROM server_member_disconnects WHERE (?1 IS NULL OR user_id=?1) LIMIT 32").bind(user.map(|id|id.to_string())).fetch_all(&state.pool).await.map_err(ApiError::internal)?;
    for row in rows {
        let user: String = row.get("user_id");
        let room: String = row.get("room");
        if remove_participant(state, &room, &user).await.is_ok() {
            sqlx::query("DELETE FROM server_member_disconnects WHERE user_id=? AND room=?")
                .bind(user)
                .bind(room)
                .execute(&state.pool)
                .await
                .map_err(ApiError::internal)?;
        }
    }
    Ok(())
}

impl AppState {
    pub async fn maintain_member_disconnects(self) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let _ = disconnect_pending(&self, None).await;
        }
    }
}

#[cfg(test)]
mod tests;
