//! Authorize media signaling at the reverse proxy, including cached JWT replays.
use super::{ApiError, AppState, HeaderMap, State, StatusCode, Uuid};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;

#[derive(Deserialize)]
struct Claims {
    iss: String,
    sub: Option<String>,
    #[serde(default)]
    nbf: i64,
    exp: i64,
    video: Grant,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)] // LiveKit's signed grants are independent capability flags.
struct Grant {
    room: String,
    room_join: bool,
    room_admin: bool,
    room_list: bool,
    room_create: bool,
}

pub(super) async fn authorize(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let token = headers
        .get("x-wisp-media-token")
        .and_then(|h| h.to_str().ok())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            headers
                .get("authorization")
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer "))
        })
        .ok_or_else(|| ApiError::unauthorized("Media token required"))?;
    let invalid = || ApiError::unauthorized("Invalid media token");
    if token.len() > 16384 {
        return Err(invalid());
    }
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(invalid());
    }
    let header: serde_json::Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).map_err(|_| invalid())?)
            .map_err(|_| invalid())?;
    if header["alg"] != "HS256" {
        return Err(invalid());
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(state.config.livekit_api_secret.as_bytes())
        .map_err(ApiError::internal)?;
    mac.update(format!("{}.{}", parts[0], parts[1]).as_bytes());
    mac.verify_slice(&URL_SAFE_NO_PAD.decode(parts[2]).map_err(|_| invalid())?)
        .map_err(|_| invalid())?;
    let claims: Claims =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).map_err(|_| invalid())?)
            .map_err(|_| invalid())?;
    let now = chrono::Utc::now().timestamp();
    if claims.iss != state.config.livekit_api_key || claims.exp <= now || claims.nbf > now + 5 {
        return Err(invalid());
    }
    // Signed backend control grants still reach LiveKit, which checks the
    // specific room-service permission. Participant grants cannot set these.
    if claims.video.room_admin || claims.video.room_list || claims.video.room_create {
        return Ok(StatusCode::NO_CONTENT);
    }
    if !claims.video.room_join {
        return Err(invalid());
    }
    let user =
        Uuid::parse_str(claims.sub.as_deref().ok_or_else(invalid)?).map_err(|_| invalid())?;
    let allowed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users u JOIN hangout_members hm ON hm.user_id=u.id JOIN hangouts h ON h.id=hm.hangout_id WHERE u.id=? AND u.server_member=1 AND hm.left_at IS NULL AND h.ended_at IS NULL AND h.livekit_room=? AND NOT EXISTS(SELECT 1 FROM server_bans b WHERE b.user_id=u.id) AND NOT EXISTS(SELECT 1 FROM server_member_disconnects d WHERE d.user_id=u.id AND d.room=h.livekit_room))")
        .bind(user.to_string()).bind(&claims.video.room).fetch_one(&state.pool).await.map_err(ApiError::internal)?;
    if !allowed {
        return Err(ApiError::forbidden(
            "Voice access was removed; join an available room in Wisp",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
#[path = "media_admission_tests.rs"]
mod tests;
