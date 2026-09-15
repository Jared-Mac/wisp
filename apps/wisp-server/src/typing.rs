//! Ephemeral chat activity. Never persist drafts or turn typing into a snapshot refresh.
use super::{
    ApiError, AppState, account_membership, authenticate_headers, ensure_conversation_member,
    find_user,
};
use axum::{Json, extract::State, http::HeaderMap};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use wisp_protocol::{ServerEvent, UserId};

const TTL: Duration = Duration::from_secs(8);
const THROTTLE: Duration = Duration::from_secs(2);

#[derive(Debug, Deserialize)]
pub(super) struct Request {
    conversation_id: String,
    active: bool,
}

pub(super) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Request>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    ensure_conversation_member(&state.pool, &request.conversation_id, user).await?;
    account_membership::require_unblocked_chat(&state.pool, user, &request.conversation_id).await?;
    if request.active {
        let sender = find_user(&state.pool, &user.to_string()).await?;
        let now = Instant::now();
        let mut runtime = state.runtime.write().await;
        runtime
            .typing
            .retain(|_, sent| now.duration_since(*sent) < TTL);
        let key = (user, request.conversation_id.clone());
        if runtime
            .typing
            .get(&key)
            .is_some_and(|sent| now.duration_since(*sent) < THROTTLE)
        {
            return Ok(Json(json!({"ok":true})));
        }
        // A user cannot keep an unbounded number of conversations active.
        if !runtime.typing.contains_key(&key)
            && runtime.typing.keys().filter(|(id, _)| *id == user).count() >= 16
        {
            return Ok(Json(json!({"ok":true})));
        }
        runtime.typing.insert(key, now);
        drop(runtime);
        state
            .emit(
                "chat_typing",
                json!({
                    "conversation_id":request.conversation_id,"user_id":user,
                    "display_name":sender.display_name,"active":true,"timeout_ms":8000
                }),
            )
            .await;
    } else {
        clear(&state, user, &request.conversation_id).await;
    }
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn clear(state: &AppState, user: UserId, conversation: &str) {
    let removed = state
        .runtime
        .write()
        .await
        .typing
        .remove(&(user, conversation.to_owned()))
        .is_some();
    if removed {
        state
            .emit(
                "chat_typing",
                json!({
                    "conversation_id":conversation,"user_id":user,"active":false,"timeout_ms":0
                }),
            )
            .await;
    }
}

pub(super) async fn disconnected(state: &AppState, user: UserId) {
    let runtime = state.runtime.read().await;
    if runtime
        .users
        .get(&user)
        .is_some_and(|user| user.connections > 0)
    {
        return;
    }
    let conversations: Vec<_> = runtime
        .typing
        .keys()
        .filter(|(id, _)| *id == user)
        .map(|(_, conversation)| conversation.clone())
        .collect();
    drop(runtime);
    for conversation in conversations {
        clear(state, user, &conversation).await;
    }
}

pub(super) async fn visible(state: &AppState, event: &ServerEvent, viewer: UserId) -> bool {
    let Some(conversation) = event.payload["conversation_id"].as_str() else {
        return false;
    };
    let Some(sender) = event.payload["user_id"]
        .as_str()
        .and_then(|id| id.parse::<UserId>().ok())
    else {
        return false;
    };
    viewer != sender
        && ensure_conversation_member(&state.pool, conversation, viewer)
            .await
            .is_ok()
        && ensure_conversation_member(&state.pool, conversation, sender)
            .await
            .is_ok()
        && account_membership::require_unblocked_chat(&state.pool, sender, conversation)
            .await
            .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router;
    use crate::{
        TEST_MEMBER_A_ID, TEST_MEMBER_B_ID, TEST_OWNER_ID, find_or_create_direct,
        tests::test_config, text_tests::request,
    };
    use axum::http::StatusCode;

    #[tokio::test]
    async fn typing_is_private_throttled_and_stops_on_send_and_disconnect() {
        let state = AppState::new(test_config()).await.unwrap();
        let owner = TEST_OWNER_ID.parse().unwrap();
        let member = TEST_MEMBER_A_ID.parse().unwrap();
        let other = TEST_MEMBER_B_ID.parse().unwrap();
        let id = find_or_create_direct(&state.pool, owner, member)
            .await
            .unwrap();
        let app = router(state.clone());
        let mut events = state.events.subscribe();
        let body = json!({"conversation_id":id,"active":true});
        assert_eq!(
            request(&app, "POST", "/v1/typing", TEST_MEMBER_B_ID, body.clone())
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
        assert!(events.try_recv().is_err());
        assert_eq!(
            request(&app, "POST", "/v1/typing", TEST_OWNER_ID, body.clone())
                .await
                .status(),
            StatusCode::OK
        );
        let event = events.try_recv().unwrap();
        assert_eq!(event.name, "chat_typing");
        assert_eq!(event.payload["timeout_ms"], 8000);
        assert!(visible(&state, &event, member).await);
        assert!(!visible(&state, &event, other).await);
        assert!(!visible(&state, &event, owner).await);
        // Account-only friends still receive their DM activity.
        sqlx::query("UPDATE users SET server_member=0 WHERE id=?")
            .bind(TEST_MEMBER_A_ID)
            .execute(&state.pool)
            .await
            .unwrap();
        assert!(visible(&state, &event, member).await);
        request(&app, "POST", "/v1/typing", TEST_OWNER_ID, body.clone()).await;
        assert!(events.try_recv().is_err());
        let message = json!({"conversation_id":id,"content_type":"text/plain","payload":"Hello"});
        assert_eq!(
            request(&app, "POST", "/v1/messages", TEST_OWNER_ID, message)
                .await
                .status(),
            StatusCode::OK
        );
        assert_eq!(events.try_recv().unwrap().payload["active"], false);
        assert_eq!(events.try_recv().unwrap().name, "message_created");
        request(&app, "POST", "/v1/typing", TEST_OWNER_ID, body).await;
        events.try_recv().unwrap();
        disconnected(&state, owner).await;
        assert_eq!(events.try_recv().unwrap().payload["active"], false);
        assert!(state.runtime.read().await.typing.is_empty());
        sqlx::query("INSERT INTO account_blocks(user_id,blocked_id) VALUES (?,?)")
            .bind(TEST_MEMBER_A_ID)
            .bind(TEST_OWNER_ID)
            .execute(&state.pool)
            .await
            .unwrap();
        assert!(!visible(&state, &event, member).await);
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/typing",
                TEST_OWNER_ID,
                json!({"conversation_id":id,"active":true})
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        sqlx::query("DELETE FROM account_blocks")
            .execute(&state.pool)
            .await
            .unwrap();
        // Revoking access invalidates even an already-queued activity event.
        sqlx::query("DELETE FROM conversation_members WHERE conversation_id=? AND user_id=?")
            .bind(&id)
            .bind(TEST_MEMBER_A_ID)
            .execute(&state.pool)
            .await
            .unwrap();
        assert!(!visible(&state, &event, member).await);
    }
}
