use super::{ApiError, AppState, HeaderMap, Json, State, Utc, Uuid, Value};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde_json::json;
use sha2::Sha256;
use sqlx::{Row, SqlitePool};
use std::collections::BTreeMap;
use wisp_protocol::{ModerateVoiceRequest, VoiceModeration};

pub(super) async fn load(pool: &SqlitePool) -> Result<BTreeMap<Uuid, VoiceModeration>, ApiError> {
    sqlx::query("SELECT user_id,muted,deafened FROM voice_moderation WHERE muted=1 OR deafened=1")
        .fetch_all(pool)
        .await
        .map_err(ApiError::internal)?
        .into_iter()
        .map(|row| {
            Ok((
                super::parse_uuid(&row.get::<String, _>("user_id"))?,
                VoiceModeration {
                    muted: row.get("muted"),
                    deafened: row.get("deafened"),
                },
            ))
        })
        .collect()
}

pub(super) async fn for_user(pool: &SqlitePool, user: Uuid) -> Result<VoiceModeration, ApiError> {
    Ok(load(pool).await?.remove(&user).unwrap_or_default())
}

fn permission(value: &VoiceModeration) -> Value {
    json!({"canSubscribe": !value.deafened, "canPublish": true, "canPublishData": true,
        "canPublishSources": if value.muted || value.deafened { vec![1,3] } else { vec![1,2,3,4] }})
}

async fn enforce(
    state: &AppState,
    room: &str,
    user: Uuid,
    value: &VoiceModeration,
) -> Result<(), ApiError> {
    let now = Utc::now().timestamp();
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
    let claims = json!({"iss":state.config.livekit_api_key,"nbf":now-5,"exp":now+60,
        "video":{"roomAdmin":true,"room":room}});
    let input = format!("{header}.{}", URL_SAFE_NO_PAD.encode(claims.to_string()));
    let mut mac = Hmac::<Sha256>::new_from_slice(state.config.livekit_api_secret.as_bytes())
        .map_err(ApiError::internal)?;
    mac.update(input.as_bytes());
    let token = format!(
        "{input}.{}",
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    );
    let url = state
        .config
        .livekit_url
        .replacen("wss://", "https://", 1)
        .replacen("ws://", "http://", 1);
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(ApiError::internal)?
        .post(format!(
            "{}/twirp/livekit.RoomService/UpdateParticipant",
            url.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .json(&json!({"room":room,"identity":user.to_string(),"permission":permission(value)}))
        .send()
        .await
        .map_err(|_| {
            ApiError::conflict(
                "media_unavailable",
                "Couldn't apply server moderation to voice; try again",
            )
        })?;
    if response.status().is_success() || response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(());
    }
    Err(ApiError::conflict(
        "media_moderation_failed",
        "Voice rejected the moderation change; try again",
    ))
}

pub(super) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ModerateVoiceRequest>,
) -> Result<Json<Value>, ApiError> {
    let actor = super::server_management::require_manager(&state, &headers).await?;
    if request.muted.is_none() && request.deafened.is_none() {
        return Err(ApiError::bad_request(
            "missing_action",
            "Choose mute or deafen",
        ));
    }
    let owner: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_identity WHERE owner_user_id=?)")
            .bind(request.user_id.to_string())
            .fetch_one(&state.pool)
            .await
            .map_err(ApiError::internal)?;
    if owner && actor != request.user_id {
        return Err(ApiError::forbidden(
            "Only the server owner can change their own moderation state",
        ));
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id=?)")
        .bind(request.user_id.to_string())
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    if !exists {
        return Err(ApiError::not_found("Account does not exist"));
    }
    // Serialize concurrent changes and room moves. Never persist a success if
    // the live media service rejected it. New join tokens read the same state.
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    sqlx::query("UPDATE users SET id=id WHERE id=?")
        .bind(request.user_id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    let still_manager: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_identity WHERE owner_user_id=?) OR EXISTS(SELECT 1 FROM server_admins WHERE user_id=?)")
        .bind(actor.to_string()).bind(actor.to_string()).fetch_one(&mut *tx).await.map_err(ApiError::internal)?;
    if !still_manager {
        return Err(ApiError::forbidden("Server administrator required"));
    }
    let current = sqlx::query("SELECT muted,deafened FROM voice_moderation WHERE user_id=?")
        .bind(request.user_id.to_string())
        .fetch_optional(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    let value = VoiceModeration {
        muted: request
            .muted
            .unwrap_or_else(|| current.as_ref().is_some_and(|row| row.get("muted"))),
        deafened: request
            .deafened
            .unwrap_or_else(|| current.as_ref().is_some_and(|row| row.get("deafened"))),
    };
    let room: Option<String> = sqlx::query_scalar("SELECT h.livekit_room FROM hangouts h JOIN hangout_members hm ON hm.hangout_id=h.id WHERE hm.user_id=? AND hm.left_at IS NULL AND h.ended_at IS NULL")
        .bind(request.user_id.to_string()).fetch_optional(&mut *tx).await.map_err(ApiError::internal)?;
    if let Some(room) = room {
        enforce(&state, &room, request.user_id, &value).await?;
    }
    sqlx::query("INSERT INTO voice_moderation(user_id,muted,deafened,changed_by,changed_at) VALUES (?,?,?,?,?) ON CONFLICT(user_id) DO UPDATE SET muted=excluded.muted,deafened=excluded.deafened,changed_by=excluded.changed_by,changed_at=excluded.changed_at")
        .bind(request.user_id.to_string()).bind(value.muted).bind(value.deafened).bind(actor.to_string()).bind(Utc::now().to_rfc3339())
        .execute(&mut *tx).await.map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    state.emit("voice_moderated", json!({"changed":true})).await;
    Ok(Json(json!({"ok":true,"moderation":value})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        TEST_MEMBER_A_ID, TEST_MEMBER_B_ID, TEST_OWNER_ID, router,
        tests::test_config,
        text_tests::{request, value},
    };
    use axum::{Router, http::StatusCode, routing::post};
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn moderation_is_server_scoped_persistent_and_enforced_in_media_grants() {
        let calls = Arc::new(Mutex::new(Vec::<Value>::new()));
        let captured = calls.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut config = test_config();
        config.livekit_url = format!("ws://{}", listener.local_addr().unwrap());
        let media = Router::new().route(
            "/twirp/livekit.RoomService/UpdateParticipant",
            post(move |Json(body): Json<Value>| {
                captured.lock().unwrap().push(body);
                async { Json(json!({})) }
            }),
        );
        let task = tokio::spawn(async move {
            axum::serve(listener, media).await.unwrap();
        });
        let state = AppState::new(config).await.unwrap();
        sqlx::query("INSERT INTO server_identity(id,owner_user_id) VALUES (1,?)")
            .bind(TEST_OWNER_ID)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO server_admins(user_id,granted_by,granted_at) VALUES (?,?,?)")
            .bind(TEST_MEMBER_A_ID)
            .bind(TEST_OWNER_ID)
            .bind(Utc::now().to_rfc3339())
            .execute(&state.pool)
            .await
            .unwrap();
        let app = router(state.clone());
        let body = json!({"user_id":TEST_MEMBER_B_ID,"muted":true});
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/server/voice",
                TEST_MEMBER_B_ID,
                body.clone()
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/server/voice",
                TEST_MEMBER_A_ID,
                json!({"user_id":TEST_OWNER_ID,"muted":true})
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        value(
            request(
                &app,
                "POST",
                "/v1/spots/join",
                TEST_MEMBER_B_ID,
                json!({"spot_id":"TestRoom"}),
            )
            .await,
        )
        .await;
        value(request(&app, "POST", "/v1/server/voice", TEST_MEMBER_A_ID, body).await).await;
        assert_eq!(calls.lock().unwrap()[0]["identity"], TEST_MEMBER_B_ID);
        assert_eq!(
            calls.lock().unwrap()[0]["permission"]["canPublishSources"],
            json!([1, 3])
        );
        assert_eq!(calls.lock().unwrap()[0]["permission"]["canSubscribe"], true);
        value(
            request(
                &app,
                "POST",
                "/v1/server/voice",
                TEST_MEMBER_A_ID,
                json!({"user_id":TEST_MEMBER_B_ID,"deafened":true}),
            )
            .await,
        )
        .await;
        value(
            request(
                &app,
                "POST",
                "/v1/server/voice",
                TEST_MEMBER_A_ID,
                json!({"user_id":TEST_MEMBER_B_ID,"muted":false}),
            )
            .await,
        )
        .await;
        let saved = for_user(&state.pool, TEST_MEMBER_B_ID.parse().unwrap())
            .await
            .unwrap();
        assert!(!saved.muted && saved.deafened);
        let token = value(
            request(
                &app,
                "POST",
                "/v1/livekit/token",
                TEST_MEMBER_B_ID,
                json!({}),
            )
            .await,
        )
        .await;
        let claims: Value = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(token["token"].as_str().unwrap().split('.').nth(1).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(claims["video"]["canSubscribe"], false);
        assert_eq!(
            claims["video"]["canPublishSources"],
            json!(["camera", "screen_share"])
        );
        sqlx::query("DELETE FROM server_admins WHERE user_id=?")
            .bind(TEST_MEMBER_A_ID)
            .execute(&state.pool)
            .await
            .unwrap();
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/server/voice",
                TEST_MEMBER_A_ID,
                json!({"user_id":TEST_MEMBER_B_ID,"deafened":false})
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(calls.lock().unwrap().len(), 3);
        task.abort();
    }

    #[tokio::test]
    async fn unavailable_media_does_not_save_a_successful_moderation_change() {
        let mut config = test_config();
        config.livekit_url = "ws://127.0.0.1:1".into();
        let state = AppState::new(config).await.unwrap();
        sqlx::query("INSERT INTO server_identity(id,owner_user_id) VALUES (1,?)")
            .bind(TEST_OWNER_ID)
            .execute(&state.pool)
            .await
            .unwrap();
        let app = router(state.clone());
        value(
            request(
                &app,
                "POST",
                "/v1/spots/join",
                TEST_MEMBER_B_ID,
                json!({"spot_id":"TestRoom"}),
            )
            .await,
        )
        .await;
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/server/voice",
                TEST_OWNER_ID,
                json!({"user_id":TEST_MEMBER_B_ID,"muted":true})
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            for_user(&state.pool, TEST_MEMBER_B_ID.parse().unwrap())
                .await
                .unwrap(),
            VoiceModeration::default()
        );
    }
}
