//! Mutable server-channel visibility, independent of the signed identity ledger.
use super::{ApiError, ConversationKind, ConversationView, Row, SqlitePool, UserId, Utc};
use std::collections::BTreeMap;

pub(super) async fn queue_admissions(db: &mut sqlx::SqliteConnection) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM pending_room_admissions WHERE conversation_id IN (SELECT conversation_id FROM server_channels) AND NOT EXISTS(SELECT 1 FROM channel_access a WHERE a.conversation_id=pending_room_admissions.conversation_id AND a.user_id=pending_room_admissions.user_id)")
        .execute(&mut *db).await.map_err(ApiError::internal)?;
    sqlx::query("INSERT OR IGNORE INTO pending_room_admissions(conversation_id,user_id,invited_by,created_at,automatic) SELECT a.conversation_id,a.user_id,si.owner_user_id,?,1 FROM channel_access a CROSS JOIN server_identity si WHERE NOT EXISTS(SELECT 1 FROM conversation_members cm WHERE cm.conversation_id=a.conversation_id AND cm.user_id=a.user_id)")
        .bind(Utc::now().to_rfc3339()).execute(db).await.map_err(ApiError::internal)?;
    Ok(())
}

pub(super) async fn preview(
    pool: &SqlitePool,
    id: &str,
    user: UserId,
) -> Result<Option<ConversationView>, ApiError> {
    let row=sqlx::query("SELECT c.id,c.label,sc.category_id,cc.name category_name FROM channel_access a JOIN server_channels sc ON sc.conversation_id=a.conversation_id JOIN conversations c ON c.id=sc.conversation_id LEFT JOIN channel_categories cc ON cc.id=sc.category_id WHERE c.id=? AND a.user_id=?")
        .bind(id).bind(user.to_string()).fetch_optional(pool).await.map_err(ApiError::internal)?;
    Ok(row.map(|row| ConversationView {
        id: row.get("id"),
        kind: ConversationKind::Circle,
        label: row.get("label"),
        spot_id: None,
        members: Vec::new(),
        last_message: None,
        unread_count: 0,
        tab_closed: false,
        history_cleared_at: None,
        can_clear_for_everyone: false,
        self_role: String::new(),
        member_roles: BTreeMap::new(),
        server_channel: true,
        category_id: row.get("category_id"),
        category_name: row.get("category_name"),
        pending_access: true,
    }))
}

pub(super) async fn validate_recipients(
    db: &mut sqlx::SqliteConnection,
    id: &str,
    sender: UserId,
    recipients: &[UserId],
) -> Result<(), ApiError> {
    let channel: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_channels WHERE conversation_id=?)")
            .bind(id)
            .fetch_one(&mut *db)
            .await
            .map_err(ApiError::internal)?;
    if !channel {
        return Ok(());
    }
    if !recipients.contains(&sender) {
        return Err(ApiError::bad_request(
            "invalid_recipients",
            "Message must include its sender",
        ));
    }
    for recipient in recipients {
        let allowed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM channel_access WHERE conversation_id=? AND user_id=?)",
        )
        .bind(id)
        .bind(recipient.to_string())
        .fetch_one(&mut *db)
        .await
        .map_err(ApiError::internal)?;
        if !allowed {
            return Err(ApiError::conflict(
                "channel_access_changed",
                "Channel access changed. Refresh or update Wisp before sending.",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AppState, TEST_MEMBER_A_ID, TEST_MEMBER_B_ID, TEST_MEMBER_C_ID, TEST_OWNER_ID, router,
        tests::test_config,
        text_tests::{request, value},
    };
    use axum::http::StatusCode;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use serde_json::json;
    use uuid::Uuid;
    use wisp_crypto::{
        Identity,
        roster::{Member, Role, Roster},
    };

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn channel_audiences_are_editable_and_filter_history_and_encryption() {
        let state = AppState::new(test_config()).await.unwrap();
        let ids = [TEST_OWNER_ID, TEST_MEMBER_A_ID, TEST_MEMBER_B_ID];
        let keys = [
            Identity::generate().unwrap(),
            Identity::generate().unwrap(),
            Identity::generate().unwrap(),
        ];
        for (i, id) in ids.iter().enumerate() {
            sqlx::query("UPDATE users SET username=? WHERE id=?")
                .bind(format!("account-{i}"))
                .bind(id)
                .execute(&state.pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO chat_identities(user_id,public_identity) VALUES (?,?)")
                .bind(id)
                .bind(serde_json::to_string(&keys[i].public()).unwrap())
                .execute(&state.pool)
                .await
                .unwrap();
        }
        sqlx::query("INSERT INTO server_identity(id,owner_user_id) VALUES (1,?)")
            .bind(TEST_OWNER_ID)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM friendships")
            .execute(&state.pool)
            .await
            .unwrap();
        let app = router(state.clone());
        let channel = value(
            request(
                &app,
                "POST",
                "/v1/server/channels",
                TEST_OWNER_ID,
                json!({"name":"General"}),
            )
            .await,
        )
        .await;
        let id = channel["id"].as_str().unwrap();
        assert_eq!(channel["members"].as_array().unwrap().len(), 3);
        for account in ids {
            assert!(
                state
                    .snapshot(account.parse().unwrap())
                    .await
                    .unwrap()
                    .conversations
                    .iter()
                    .any(|c| c.id == id)
            );
        }
        let roster = Roster {
            network: crate::chat_identity::network(&state).await.unwrap(),
            conversation: id.into(),
            revision: 0,
            previous: None,
            actor: TEST_OWNER_ID.parse().unwrap(),
            members: ids
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    (
                        id.parse().unwrap(),
                        Member {
                            identity: keys[i].public(),
                            role: if i == 0 { Role::Admin } else { Role::Member },
                        },
                    )
                })
                .collect(),
        }
        .sign(&keys[0])
        .unwrap();
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/e2ee/roster",
                TEST_OWNER_ID,
                serde_json::to_value(&roster).unwrap()
            )
            .await
            .status(),
            StatusCode::OK
        );
        let directory =
            value(request(&app, "GET", "/v1/e2ee/state", TEST_MEMBER_A_ID, json!({})).await).await;
        assert!(
            directory["channel_identities"]
                .get(TEST_MEMBER_B_ID)
                .is_some()
        );
        assert!(directory["identities"].get(TEST_MEMBER_B_ID).is_none());
        let public_message = Uuid::new_v4();
        let envelope = |id: Uuid, recipients: &[usize], declared: bool| json!({"id":id,"conversation_id":channel["id"],"roster_hash":roster.hash().unwrap(),"recipient_ids":if declared {Some(recipients.iter().map(|i|ids[*i]).collect::<Vec<_>>())}else{None},"ciphertext":STANDARD.encode(keys[0].seal("test",b"private message",&recipients.iter().map(|i|keys[*i].public()).collect::<Vec<_>>()).unwrap())});
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/e2ee/messages",
                TEST_OWNER_ID,
                envelope(public_message, &[0, 1, 2], false)
            )
            .await
            .status(),
            StatusCode::OK
        );
        let edit = |visibility: &str, members: Vec<&str>| json!({"name":"General","visibility":visibility,"member_ids":members});
        let path = format!("/v1/server/channels/{id}");
        assert_eq!(
            request(
                &app,
                "PATCH",
                &path,
                TEST_OWNER_ID,
                edit("members", vec![TEST_MEMBER_A_ID])
            )
            .await
            .status(),
            StatusCode::OK
        );
        let denied = state
            .snapshot(TEST_MEMBER_B_ID.parse().unwrap())
            .await
            .unwrap();
        assert!(!denied.conversations.iter().any(|c| c.id == id));
        assert!(!denied.messages.iter().any(|m| m.conversation_id == id));
        assert_eq!(
            request(
                &app,
                "GET",
                &format!("/v1/messages/{public_message}"),
                TEST_MEMBER_B_ID,
                json!({})
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        // Old clients cannot keep encrypting to an account whose access ended.
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/e2ee/messages",
                TEST_OWNER_ID,
                envelope(Uuid::new_v4(), &[0, 1, 2], false)
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/e2ee/messages",
                TEST_OWNER_ID,
                envelope(Uuid::new_v4(), &[0, 1, 2], true)
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );
        let restricted_message = Uuid::new_v4();
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/e2ee/messages",
                TEST_OWNER_ID,
                envelope(restricted_message, &[0, 1], true)
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            request(
                &app,
                "PATCH",
                &path,
                TEST_OWNER_ID,
                edit("everyone", vec![])
            )
            .await
            .status(),
            StatusCode::OK
        );
        let restored = state
            .snapshot(TEST_MEMBER_B_ID.parse().unwrap())
            .await
            .unwrap();
        assert!(restored.messages.iter().any(|m| m.id == public_message));
        assert!(!restored.messages.iter().any(|m| m.id == restricted_message));
        assert_eq!(
            request(
                &app,
                "GET",
                &format!("/v1/messages/{restricted_message}"),
                TEST_MEMBER_B_ID,
                json!({})
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        // A future account sees the channel before enrolling its encryption key.
        sqlx::query("UPDATE users SET username='late-account' WHERE id=?")
            .bind(TEST_MEMBER_C_ID)
            .execute(&state.pool)
            .await
            .unwrap();
        queue_admissions(&mut state.pool.acquire().await.unwrap())
            .await
            .unwrap();
        assert!(
            state
                .snapshot(TEST_MEMBER_C_ID.parse().unwrap())
                .await
                .unwrap()
                .conversations
                .iter()
                .any(|c| c.id == id && c.pending_access)
        );
        assert_eq!(
            request(&app, "PATCH", &path, TEST_OWNER_ID, edit("admins", vec![]))
                .await
                .status(),
            StatusCode::OK
        );
        assert!(
            !state
                .snapshot(TEST_MEMBER_A_ID.parse().unwrap())
                .await
                .unwrap()
                .conversations
                .iter()
                .any(|c| c.id == id)
        );
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/server/admins",
                TEST_OWNER_ID,
                json!({"user_id":TEST_MEMBER_A_ID,"admin":true})
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert!(
            state
                .snapshot(TEST_MEMBER_A_ID.parse().unwrap())
                .await
                .unwrap()
                .conversations
                .iter()
                .any(|c| c.id == id)
        );
        assert_eq!(
            request(
                &app,
                "PATCH",
                &path,
                TEST_MEMBER_B_ID,
                edit("everyone", vec![])
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(&app, "PATCH", &path, TEST_OWNER_ID, edit("invalid", vec![]))
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
        let settings =
            value(request(&app, "GET", "/v1/server/settings", TEST_OWNER_ID, json!({})).await)
                .await;
        assert_eq!(settings["channels"][0]["visibility"], "admins");
    }
}
