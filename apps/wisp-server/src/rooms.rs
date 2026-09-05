use super::{
    ApiError, AppState, ConversationView, HeaderMap, Json, SqlitePool, UserId, Utc, Uuid, Value,
    authenticate_headers, ensure_spot_conversation, load_conversation,
};
use axum::extract::State;
use serde_json::json;
use wisp_protocol::{CreateRoomRequest, RoomMemberRequest, SetRoomAdminRequest};

pub(super) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateRoomRequest>,
) -> Result<Json<ConversationView>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 60 || name.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "invalid_name",
            "Room names must have 1–60 characters",
        ));
    }
    let spot_id = Uuid::new_v4().to_string();
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    sqlx::query("INSERT INTO spots(id, name, created_at, private) VALUES (?, ?, ?, 1)")
        .bind(&spot_id)
        .bind(name)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            if error
                .as_database_error()
                .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
            {
                ApiError::conflict("room_name_taken", "A room already uses that name")
            } else {
                ApiError::internal(error)
            }
        })?;
    let conversation_id = ensure_spot_conversation(&mut tx, &spot_id, name)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query("INSERT INTO conversation_members(conversation_id, user_id, joined_at, role) VALUES (?, ?, ?, 'host')")
        .bind(&conversation_id).bind(user.to_string()).bind(Utc::now().to_rfc3339()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    state.emit("room_created", json!({"changed":true})).await;
    Ok(Json(
        load_conversation(&state.pool, user, &conversation_id).await?,
    ))
}

async fn role(pool: &SqlitePool, user: UserId, conversation_id: &str) -> Result<String, ApiError> {
    sqlx::query_scalar("SELECT cm.role FROM conversation_members cm JOIN conversations c ON c.id = cm.conversation_id WHERE cm.user_id = ? AND cm.conversation_id = ? AND c.spot_id IS NOT NULL")
        .bind(user.to_string()).bind(conversation_id).fetch_optional(pool).await.map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::forbidden("Room membership required"))
}

pub(super) async fn invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RoomMemberRequest>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    let actor_role = role(&state.pool, user, &request.conversation_id).await?;
    super::privacy::require_unsigned_room(&state.pool, &request.conversation_id).await?;
    if !matches!(actor_role.as_str(), "host" | "admin") {
        return Err(ApiError::forbidden(
            "Only room owners and admins can invite friends",
        ));
    }
    super::ensure_friendship(&state.pool, user, request.user_id).await?;
    sqlx::query("INSERT OR IGNORE INTO conversation_members(conversation_id, user_id, joined_at, role) VALUES (?, ?, ?, 'member')")
        .bind(&request.conversation_id).bind(request.user_id.to_string()).bind(Utc::now().to_rfc3339())
        .execute(&state.pool).await.map_err(ApiError::internal)?;
    state.emit("room_invited", json!({"changed":true})).await;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn set_admin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetRoomAdminRequest>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    if role(&state.pool, user, &request.conversation_id).await? != "host" {
        return Err(ApiError::forbidden(
            "Only the room owner can manage admin access",
        ));
    }
    super::privacy::require_unsigned_room(&state.pool, &request.conversation_id).await?;
    if role(&state.pool, request.user_id, &request.conversation_id).await? == "host" {
        return Err(ApiError::forbidden("The owner cannot be demoted"));
    }
    sqlx::query("UPDATE conversation_members SET role = ? WHERE conversation_id = ? AND user_id = ? AND role != 'host'")
        .bind(if request.admin { "admin" } else { "member" }).bind(&request.conversation_id).bind(request.user_id.to_string())
        .execute(&state.pool).await.map_err(ApiError::internal)?;
    state
        .emit("room_permissions_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{chat_headers, test_config};
    use crate::{
        JoinHangoutRequest, JoinSpotRequest, SendMessageRequest, TEST_MEMBER_A_ID,
        TEST_MEMBER_B_ID, TEST_MEMBER_C_ID, TEST_OWNER_ID, TEST_ROOM_ID, active_hangout_for,
        can_clear_room, clear_room_history, join_hangout, join_spot, join_users, load_hangouts,
        load_recent_messages, load_spots, seed_development_users, send_message,
    };

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn ownership_invites_admin_permissions_and_private_room_access() {
        let state = AppState::new(test_config()).await.unwrap();
        let member_a = Uuid::parse_str(TEST_MEMBER_A_ID).unwrap();
        let member_c = Uuid::parse_str(TEST_MEMBER_C_ID).unwrap();
        let member_b = Uuid::parse_str(TEST_MEMBER_B_ID).unwrap();
        let owner = Uuid::parse_str(TEST_OWNER_ID).unwrap();
        assert_eq!(
            role(&state.pool, owner, &format!("spot:{TEST_ROOM_ID}"))
                .await
                .unwrap(),
            "host"
        );
        let room = create(
            State(state.clone()),
            chat_headers(TEST_MEMBER_A_ID),
            Json(CreateRoomRequest {
                name: "Test room".into(),
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(room.self_role, "host");
        assert_eq!(room.members.len(), 1);
        assert!(room.can_clear_for_everyone);
        assert!(
            active_hangout_for(&state.pool, member_a)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(load_spots(&state.pool, owner).await.unwrap().len(), 1);
        assert!(
            join_spot(
                State(state.clone()),
                chat_headers(TEST_OWNER_ID),
                Json(JoinSpotRequest {
                    spot_id: room.spot_id.clone().unwrap()
                })
            )
            .await
            .is_err()
        );
        let _ = invite(
            State(state.clone()),
            chat_headers(TEST_MEMBER_A_ID),
            Json(RoomMemberRequest {
                conversation_id: room.id.clone(),
                user_id: member_c,
            }),
        )
        .await
        .unwrap();
        assert!(
            active_hangout_for(&state.pool, member_c)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            invite(
                State(state.clone()),
                chat_headers(TEST_MEMBER_C_ID),
                Json(RoomMemberRequest {
                    conversation_id: room.id.clone(),
                    user_id: member_b
                })
            )
            .await
            .is_err()
        );
        assert!(
            set_admin(
                State(state.clone()),
                chat_headers(TEST_MEMBER_C_ID),
                Json(SetRoomAdminRequest {
                    conversation_id: room.id.clone(),
                    user_id: member_c,
                    admin: true
                })
            )
            .await
            .is_err()
        );
        let _ = set_admin(
            State(state.clone()),
            chat_headers(TEST_MEMBER_A_ID),
            Json(SetRoomAdminRequest {
                conversation_id: room.id.clone(),
                user_id: member_c,
                admin: true,
            }),
        )
        .await
        .unwrap();
        let _ = invite(
            State(state.clone()),
            chat_headers(TEST_MEMBER_C_ID),
            Json(RoomMemberRequest {
                conversation_id: room.id.clone(),
                user_id: member_b,
            }),
        )
        .await
        .unwrap();
        assert!(
            can_clear_room(&state.pool, member_c, &room.id)
                .await
                .unwrap()
        );
        assert!(
            !can_clear_room(&state.pool, member_b, &room.id)
                .await
                .unwrap()
        );
        assert!(!can_clear_room(&state.pool, owner, &room.id).await.unwrap());
        let _ = join_spot(
            State(state.clone()),
            chat_headers(TEST_MEMBER_A_ID),
            Json(JoinSpotRequest {
                spot_id: room.spot_id.clone().unwrap(),
            }),
        )
        .await
        .unwrap();
        let hangout = active_hangout_for(&state.pool, member_a)
            .await
            .unwrap()
            .unwrap();
        assert!(load_hangouts(&state.pool, owner).await.unwrap().is_empty());
        assert!(
            join_hangout(
                State(state.clone()),
                chat_headers(TEST_OWNER_ID),
                Json(JoinHangoutRequest {
                    hangout_id: hangout
                })
            )
            .await
            .is_err()
        );
        assert!(join_users(&state, owner, member_a).await.is_err());
        assert!(
            active_hangout_for(&state.pool, owner)
                .await
                .unwrap()
                .is_none()
        );
        let _ = send_message(
            State(state.clone()),
            chat_headers(TEST_MEMBER_A_ID),
            Json(SendMessageRequest {
                conversation_id: room.id.clone(),
                content_type: "text/plain".into(),
                payload: json!("Room history"),
                encryption_version: 0,
            }),
        )
        .await
        .unwrap();
        assert!(
            clear_room_history(&state.pool, member_b, &room.id)
                .await
                .is_err()
        );
        clear_room_history(&state.pool, member_c, &room.id)
            .await
            .unwrap();
        assert!(
            load_recent_messages(&state.pool, member_a)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            load_conversation(&state.pool, member_b, &room.id)
                .await
                .unwrap()
                .history_cleared_at
                .is_some()
        );
        seed_development_users(&state.pool).await.unwrap();
        assert_eq!(
            role(&state.pool, member_c, &room.id).await.unwrap(),
            "admin"
        );
        assert!(role(&state.pool, owner, &room.id).await.is_err());
        let _ = set_admin(
            State(state.clone()),
            chat_headers(TEST_MEMBER_A_ID),
            Json(SetRoomAdminRequest {
                conversation_id: room.id.clone(),
                user_id: member_c,
                admin: false,
            }),
        )
        .await
        .unwrap();
        assert!(
            !can_clear_room(&state.pool, member_c, &room.id)
                .await
                .unwrap()
        );
        assert!(
            set_admin(
                State(state.clone()),
                chat_headers(TEST_MEMBER_A_ID),
                Json(SetRoomAdminRequest {
                    conversation_id: room.id,
                    user_id: member_a,
                    admin: false
                })
            )
            .await
            .is_err()
        );
    }
}
