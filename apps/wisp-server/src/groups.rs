use super::{
    ApiError, AppState, ConversationView, HeaderMap, Json, Utc, authenticate_headers,
    load_conversation,
};
use axum::extract::State;
use serde_json::json;
use sqlx::Row;
use wisp_protocol::CreateGroupConversationRequest;

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
        let friend: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM circle_members a JOIN circle_members b ON b.circle_id = a.circle_id WHERE a.user_id = ? AND b.user_id = ?)")
            .bind(actor.to_string()).bind(member.to_string()).fetch_one(&mut *tx).await.map_err(ApiError::internal)?;
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
    use crate::{CHARLIE_ID, JACK_ID, JARED_ID, TYLER_ID, ensure_conversation_member};
    use uuid::Uuid;

    #[tokio::test]
    async fn groups_are_private_atomic_and_retry_safe() {
        let state = AppState::new(test_config()).await.unwrap();
        let req = CreateGroupConversationRequest {
            request_id: Uuid::new_v4(),
            name: "Games".into(),
            members: vec![
                Uuid::parse_str(JARED_ID).unwrap(),
                Uuid::parse_str(CHARLIE_ID).unwrap(),
            ],
        };
        let group = create(
            State(state.clone()),
            chat_headers(TYLER_ID),
            Json(req.clone()),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(group.members.len(), 3);
        assert!(group.spot_id.is_none());
        assert!(group.can_clear_for_everyone);
        assert!(
            !load_conversation(&state.pool, Uuid::parse_str(JARED_ID).unwrap(), &group.id)
                .await
                .unwrap()
                .can_clear_for_everyone
        );
        assert!(
            ensure_conversation_member(&state.pool, &group.id, Uuid::parse_str(JACK_ID).unwrap())
                .await
                .is_err()
        );
        let retry = create(
            State(state.clone()),
            chat_headers(TYLER_ID),
            Json(req.clone()),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(group.id, retry.id);
        let mut changed = req.clone();
        changed.name = "Different".into();
        assert!(
            create(State(state.clone()), chat_headers(TYLER_ID), Json(changed))
                .await
                .is_err()
        );
        let mut invalid = req.clone();
        invalid.request_id = Uuid::new_v4();
        invalid.members[0] = Uuid::new_v4();
        assert!(
            create(State(state.clone()), chat_headers(TYLER_ID), Json(invalid))
                .await
                .is_err()
        );
        let mut self_member = req.clone();
        self_member.members[0] = Uuid::parse_str(TYLER_ID).unwrap();
        assert!(
            create(
                State(state.clone()),
                chat_headers(TYLER_ID),
                Json(self_member)
            )
            .await
            .is_err()
        );
        let mut empty = req;
        empty.name = " \n".into();
        assert!(
            create(State(state.clone()), chat_headers(TYLER_ID), Json(empty))
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
}
