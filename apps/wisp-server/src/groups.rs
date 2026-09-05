use super::{
    ApiError, AppState, ConversationView, HeaderMap, Json, Utc, authenticate_headers,
    load_conversation,
};
use axum::extract::{Path, State};
use serde_json::json;
use sqlx::Row;
use wisp_protocol::{CreateGroupConversationRequest, GroupMemberRequest};

async fn group_role(
    state: &AppState,
    conversation_id: &str,
    user_id: uuid::Uuid,
) -> Result<String, ApiError> {
    sqlx::query_scalar("SELECT cm.role FROM conversation_members cm JOIN conversations c ON c.id=cm.conversation_id LEFT JOIN server_channels sc ON sc.conversation_id=c.id WHERE cm.conversation_id=? AND cm.user_id=? AND c.kind='circle' AND c.spot_id IS NULL AND sc.conversation_id IS NULL")
        .bind(conversation_id)
        .bind(user_id.to_string())
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::forbidden("Group membership required"))
}

pub(super) async fn add_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<GroupMemberRequest>,
) -> Result<Json<ConversationView>, ApiError> {
    let actor = authenticate_headers(&state, &headers).await?;
    if group_role(&state, &id, actor).await? != "host" {
        return Err(ApiError::forbidden("Only the group owner can add people"));
    }
    super::privacy::require_unsigned_room(&state.pool, &id).await?;
    super::ensure_friendship(&state.pool, actor, request.user_id).await?;
    sqlx::query("INSERT OR IGNORE INTO conversation_members(conversation_id,user_id,joined_at,role) VALUES (?,?,?,'member')")
        .bind(&id).bind(request.user_id.to_string()).bind(Utc::now().to_rfc3339())
        .execute(&state.pool).await.map_err(ApiError::internal)?;
    state
        .emit("group_members_changed", json!({"changed":true}))
        .await;
    Ok(Json(load_conversation(&state.pool, actor, &id).await?))
}

pub(super) async fn remove_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, user_id)): Path<(String, uuid::Uuid)>,
) -> Result<Json<ConversationView>, ApiError> {
    let actor = authenticate_headers(&state, &headers).await?;
    if group_role(&state, &id, actor).await? != "host" {
        return Err(ApiError::forbidden(
            "Only the group owner can remove people",
        ));
    }
    if actor == user_id {
        return Err(ApiError::forbidden(
            "The group owner cannot leave without transferring ownership",
        ));
    }
    super::privacy::require_unsigned_room(&state.pool, &id).await?;
    sqlx::query(
        "DELETE FROM conversation_members WHERE conversation_id=? AND user_id=? AND role!='host'",
    )
    .bind(&id)
    .bind(user_id.to_string())
    .execute(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    state
        .emit("group_members_changed", json!({"changed":true}))
        .await;
    Ok(Json(load_conversation(&state.pool, actor, &id).await?))
}

pub(super) async fn leave(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = authenticate_headers(&state, &headers).await?;
    if group_role(&state, &id, actor).await? == "host" {
        return Err(ApiError::forbidden(
            "Transfer ownership before leaving this group",
        ));
    }
    super::privacy::require_unsigned_room(&state.pool, &id).await?;
    sqlx::query("DELETE FROM conversation_members WHERE conversation_id=? AND user_id=?")
        .bind(&id)
        .bind(actor.to_string())
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    state
        .emit("group_members_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateGroupConversationRequest>,
) -> Result<Json<ConversationView>, ApiError> {
    let actor = authenticate_headers(&state, &headers).await?;
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 60 || name.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "invalid_name",
            "Group names must have 1–60 characters",
        ));
    }
    let mut members = request.members;
    members.sort_unstable();
    members.dedup();
    if members.len() < 2 || members.len() > 31 || members.contains(&actor) {
        return Err(ApiError::bad_request(
            "invalid_members",
            "Choose 2–31 friends; you are included automatically",
        ));
    }
    let id = format!("group:{}", request.request_id);
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    for member in &members {
        let friend: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM friendships WHERE (first_user_id=? AND second_user_id=?) OR (first_user_id=? AND second_user_id=?))")
            .bind(actor.to_string()).bind(member.to_string()).bind(member.to_string()).bind(actor.to_string()).fetch_one(&mut *tx).await.map_err(ApiError::internal)?;
        if !friend {
            return Err(ApiError::forbidden(
                "Only friends can be added to a group chat",
            ));
        }
    }
    members.push(actor);
    members.sort_unstable();
    let existing = sqlx::query("SELECT c.label, cm.role FROM conversations c LEFT JOIN conversation_members cm ON cm.conversation_id = c.id AND cm.user_id = ? WHERE c.id = ?")
        .bind(actor.to_string()).bind(&id).fetch_optional(&mut *tx).await.map_err(ApiError::internal)?;
    if let Some(row) = existing {
        let stored: Vec<String> = sqlx::query_scalar(
            "SELECT user_id FROM conversation_members WHERE conversation_id = ? ORDER BY user_id",
        )
        .bind(&id)
        .fetch_all(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
        if row.get::<Option<String>, _>("role").as_deref() != Some("host")
            || row.get::<String, _>("label") != name
            || stored != members.iter().map(ToString::to_string).collect::<Vec<_>>()
        {
            return Err(ApiError::conflict(
                "request_id_used",
                "This group request was already used; start a new chat",
            ));
        }
    } else {
        // Circle is the existing wire kind for group text chat. A NULL circle_id
        // keeps this private conversation separate from the global friend roster.
        sqlx::query(
            "INSERT INTO conversations(id, kind, label, created_at) VALUES (?, 'circle', ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
        for member in members {
            sqlx::query("INSERT INTO conversation_members(conversation_id, user_id, joined_at, role) VALUES (?, ?, ?, ?)")
                .bind(&id).bind(member.to_string()).bind(Utc::now().to_rfc3339())
                .bind(if member == actor { "host" } else { "member" }).execute(&mut *tx).await.map_err(ApiError::internal)?;
        }
    }
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .emit("conversation_changed", json!({"changed":true}))
        .await;
    Ok(Json(load_conversation(&state.pool, actor, &id).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{chat_headers, test_config};
    use crate::{
        TEST_MEMBER_A_ID, TEST_MEMBER_B_ID, TEST_MEMBER_C_ID, TEST_OWNER_ID,
        ensure_conversation_member,
    };
    use uuid::Uuid;

    #[tokio::test]
    async fn groups_are_private_atomic_and_retry_safe() {
        let state = AppState::new(test_config()).await.unwrap();
        let req = CreateGroupConversationRequest {
            request_id: Uuid::new_v4(),
            name: "Games".into(),
            members: vec![
                Uuid::parse_str(TEST_OWNER_ID).unwrap(),
                Uuid::parse_str(TEST_MEMBER_C_ID).unwrap(),
            ],
        };
        let group = create(
            State(state.clone()),
            chat_headers(TEST_MEMBER_A_ID),
            Json(req.clone()),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(group.members.len(), 3);
        assert!(group.spot_id.is_none());
        assert!(group.can_clear_for_everyone);
        assert!(
            !load_conversation(
                &state.pool,
                Uuid::parse_str(TEST_OWNER_ID).unwrap(),
                &group.id
            )
            .await
            .unwrap()
            .can_clear_for_everyone
        );
        assert!(
            ensure_conversation_member(
                &state.pool,
                &group.id,
                Uuid::parse_str(TEST_MEMBER_B_ID).unwrap()
            )
            .await
            .is_err()
        );
        let retry = create(
            State(state.clone()),
            chat_headers(TEST_MEMBER_A_ID),
            Json(req.clone()),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(group.id, retry.id);
        let mut changed = req.clone();
        changed.name = "Different".into();
        assert!(
            create(
                State(state.clone()),
                chat_headers(TEST_MEMBER_A_ID),
                Json(changed)
            )
            .await
            .is_err()
        );
        let mut invalid = req.clone();
        invalid.request_id = Uuid::new_v4();
        invalid.members[0] = Uuid::new_v4();
        assert!(
            create(
                State(state.clone()),
                chat_headers(TEST_MEMBER_A_ID),
                Json(invalid)
            )
            .await
            .is_err()
        );
        let mut self_member = req.clone();
        self_member.members[0] = Uuid::parse_str(TEST_MEMBER_A_ID).unwrap();
        assert!(
            create(
                State(state.clone()),
                chat_headers(TEST_MEMBER_A_ID),
                Json(self_member)
            )
            .await
            .is_err()
        );
        let mut empty = req;
        empty.name = " \n".into();
        assert!(
            create(
                State(state.clone()),
                chat_headers(TEST_MEMBER_A_ID),
                Json(empty)
            )
            .await
            .is_err()
        );
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM conversations WHERE id LIKE 'group:%'")
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn owner_controls_members_and_members_can_leave() {
        let state = AppState::new(test_config()).await.unwrap();
        let member = Uuid::parse_str(TEST_OWNER_ID).unwrap();
        let leaving = Uuid::parse_str(TEST_MEMBER_C_ID).unwrap();
        let added = Uuid::parse_str(TEST_MEMBER_B_ID).unwrap();
        let group = create(
            State(state.clone()),
            chat_headers(TEST_MEMBER_A_ID),
            Json(CreateGroupConversationRequest {
                request_id: Uuid::new_v4(),
                name: "Private group".into(),
                members: vec![member, leaving],
            }),
        )
        .await
        .unwrap()
        .0;
        let _ = add_member(
            State(state.clone()),
            chat_headers(TEST_MEMBER_A_ID),
            Path(group.id.clone()),
            Json(GroupMemberRequest { user_id: added }),
        )
        .await
        .unwrap();
        assert!(
            load_conversation(&state.pool, added, &group.id)
                .await
                .is_ok()
        );
        assert!(
            remove_member(
                State(state.clone()),
                chat_headers(TEST_OWNER_ID),
                Path((group.id.clone(), added)),
            )
            .await
            .is_err()
        );
        let _ = remove_member(
            State(state.clone()),
            chat_headers(TEST_MEMBER_A_ID),
            Path((group.id.clone(), added)),
        )
        .await
        .unwrap();
        assert!(
            load_conversation(&state.pool, added, &group.id)
                .await
                .is_err()
        );
        let _ = leave(
            State(state.clone()),
            chat_headers(TEST_MEMBER_C_ID),
            Path(group.id.clone()),
        )
        .await
        .unwrap();
        assert!(
            load_conversation(&state.pool, leaving, &group.id)
                .await
                .is_err()
        );
        assert!(
            leave(
                State(state.clone()),
                chat_headers(TEST_MEMBER_A_ID),
                Path(group.id),
            )
            .await
            .is_err()
        );
    }
}
