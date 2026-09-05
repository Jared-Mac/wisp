use super::{
    ApiError, AppState, ConversationView, HeaderMap, Json, SqlitePool, Utc, Uuid, Value,
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

pub(super) async fn require_room(pool: &SqlitePool, conversation: &str) -> Result<(), ApiError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM conversations WHERE id=? AND spot_id IS NOT NULL)",
    )
    .bind(conversation)
    .fetch_one(pool)
    .await
    .map_err(ApiError::internal)?;
    if exists {
        Ok(())
    } else {
        Err(ApiError::not_found("Room does not exist"))
    }
}

pub(super) async fn invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RoomMemberRequest>,
) -> Result<Json<Value>, ApiError> {
    let user = super::server_management::require_manager(&state, &headers).await?;
    require_room(&state.pool, &request.conversation_id).await?;
    super::ensure_friendship(&state.pool, user, request.user_id).await?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    let signed: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM chat_rosters WHERE conversation_id=?)")
            .bind(&request.conversation_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    if signed {
        // Server administration authorizes access; an existing client still
        // signs the encrypted membership chain. Never forge or reset it.
        super::queue_encrypted_room_admission(
            &mut tx,
            &request.conversation_id,
            request.user_id,
            user,
            &Utc::now().to_rfc3339(),
        )
        .await?;
    } else {
        sqlx::query("INSERT OR IGNORE INTO conversation_members(conversation_id, user_id, joined_at, role) VALUES (?, ?, ?, 'member')")
            .bind(&request.conversation_id).bind(request.user_id.to_string()).bind(Utc::now().to_rfc3339())
            .execute(&mut *tx).await.map_err(ApiError::internal)?;
    }
    tx.commit().await.map_err(ApiError::internal)?;
    state.emit("room_invited", json!({"changed":true})).await;
    Ok(Json(json!({"ok":true,"pending_encrypted_access":signed})))
}

// Keep a clear response for old clients; room roles are no longer assignable.
pub(super) async fn set_admin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(_request): Json<SetRoomAdminRequest>,
) -> Result<Json<Value>, ApiError> {
    authenticate_headers(&state, &headers).await?;
    Err(ApiError::bad_request(
        "server_roles_required",
        "Manage administrators in Server settings; rooms inherit server permissions",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        TEST_MEMBER_A_ID, TEST_MEMBER_B_ID, TEST_MEMBER_C_ID, TEST_OWNER_ID, can_clear_room,
        load_spots, server_management,
        tests::{chat_headers, test_config},
    };

    #[tokio::test]
    async fn server_authority_replaces_room_roles_without_granting_private_chat_access() {
        let state = AppState::new(test_config()).await.unwrap();
        sqlx::query("INSERT INTO server_identity(id,owner_user_id) VALUES (1,?)")
            .bind(TEST_OWNER_ID)
            .execute(&state.pool)
            .await
            .unwrap();
        let creator: Uuid = TEST_MEMBER_A_ID.parse().unwrap();
        let owner: Uuid = TEST_OWNER_ID.parse().unwrap();
        let admin: Uuid = TEST_MEMBER_C_ID.parse().unwrap();
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
        assert!(!room.can_clear_for_everyone);
        assert!(
            !can_clear_room(&state.pool, creator, &room.id)
                .await
                .unwrap()
        );
        // Encryption custody does not grant app administration.
        sqlx::query("UPDATE conversation_members SET role='admin' WHERE conversation_id=?")
            .bind(&room.id)
            .execute(&state.pool)
            .await
            .unwrap();
        assert!(
            !can_clear_room(&state.pool, creator, &room.id)
                .await
                .unwrap()
        );
        assert!(can_clear_room(&state.pool, owner, &room.id).await.unwrap());
        assert_eq!(load_spots(&state.pool, owner).await.unwrap().len(), 1);
        assert!(
            invite(
                State(state.clone()),
                chat_headers(TEST_MEMBER_A_ID),
                Json(RoomMemberRequest {
                    conversation_id: room.id.clone(),
                    user_id: admin
                })
            )
            .await
            .is_err()
        );
        sqlx::query("INSERT INTO server_admins(user_id,granted_by,granted_at) VALUES (?,?,?)")
            .bind(admin.to_string())
            .bind(owner.to_string())
            .bind(Utc::now().to_rfc3339())
            .execute(&state.pool)
            .await
            .unwrap();
        assert!(
            server_management::is_manager(&state.pool, admin)
                .await
                .unwrap()
        );
        assert!(can_clear_room(&state.pool, admin, &room.id).await.unwrap());
        let _ = invite(
            State(state.clone()),
            chat_headers(TEST_MEMBER_C_ID),
            Json(RoomMemberRequest {
                conversation_id: room.id.clone(),
                user_id: TEST_MEMBER_B_ID.parse().unwrap(),
            }),
        )
        .await
        .unwrap();
        assert!(
            set_admin(
                State(state.clone()),
                chat_headers(TEST_OWNER_ID),
                Json(SetRoomAdminRequest {
                    conversation_id: room.id.clone(),
                    user_id: creator,
                    admin: true
                })
            )
            .await
            .is_err()
        );
        sqlx::query("DELETE FROM server_admins WHERE user_id=?")
            .bind(admin.to_string())
            .execute(&state.pool)
            .await
            .unwrap();
        assert!(!can_clear_room(&state.pool, admin, &room.id).await.unwrap());
    }
    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn encrypted_custody_cannot_grant_room_roles_or_bypass_server_invitation_authority() {
        use std::collections::BTreeMap;
        use wisp_crypto::{
            Identity,
            roster::{Member, Role, Roster},
        };
        let state = AppState::new(test_config()).await.unwrap();
        let custodian: Uuid = TEST_MEMBER_A_ID.parse().unwrap();
        let recipient: Uuid = TEST_MEMBER_B_ID.parse().unwrap();
        let manager: Uuid = TEST_MEMBER_C_ID.parse().unwrap();
        let key = Identity::generate().unwrap();
        let other = Identity::generate().unwrap();
        for (id, identity) in [(custodian, key.public()), (recipient, other.public())] {
            sqlx::query("INSERT INTO chat_identities(user_id,public_identity) VALUES (?,?)")
                .bind(id.to_string())
                .bind(serde_json::to_string(&identity).unwrap())
                .execute(&state.pool)
                .await
                .unwrap();
        }
        let room = create(
            State(state.clone()),
            chat_headers(TEST_MEMBER_A_ID),
            Json(CreateRoomRequest {
                name: "Encrypted server room".into(),
            }),
        )
        .await
        .unwrap()
        .0;
        let network = crate::chat_identity::network(&state).await.unwrap();
        let first = Roster {
            network,
            conversation: room.id.clone(),
            revision: 0,
            previous: None,
            actor: custodian,
            members: BTreeMap::from([(
                custodian,
                Member {
                    identity: key.public(),
                    role: Role::Host,
                },
            )]),
        }
        .sign(&key)
        .unwrap();
        let _ = crate::chat_identity::update_roster(
            State(state.clone()),
            chat_headers(TEST_MEMBER_A_ID),
            Json(first.clone()),
        )
        .await
        .unwrap();
        let mut next = first.roster.clone();
        next.revision = 1;
        next.previous = Some(first.hash().unwrap());
        next.members.insert(
            recipient,
            Member {
                identity: other.public(),
                role: Role::Member,
            },
        );
        let signed = next.clone().sign(&key).unwrap();
        assert!(
            crate::chat_identity::update_roster(
                State(state.clone()),
                chat_headers(TEST_MEMBER_A_ID),
                Json(signed.clone())
            )
            .await
            .is_err()
        );
        sqlx::query("INSERT INTO server_admins(user_id,granted_by,granted_at) VALUES (?,?,?)")
            .bind(manager.to_string())
            .bind(TEST_OWNER_ID)
            .bind(Utc::now().to_rfc3339())
            .execute(&state.pool)
            .await
            .unwrap();
        let response = invite(
            State(state.clone()),
            chat_headers(TEST_MEMBER_C_ID),
            Json(RoomMemberRequest {
                conversation_id: room.id.clone(),
                user_id: recipient,
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(response["pending_encrypted_access"], true);
        sqlx::query("DELETE FROM server_admins WHERE user_id=?")
            .bind(manager.to_string())
            .execute(&state.pool)
            .await
            .unwrap();
        assert!(
            crate::chat_identity::update_roster(
                State(state.clone()),
                chat_headers(TEST_MEMBER_A_ID),
                Json(signed.clone())
            )
            .await
            .is_err()
        );
        sqlx::query("INSERT INTO server_admins(user_id,granted_by,granted_at) VALUES (?,?,?)")
            .bind(manager.to_string())
            .bind(TEST_OWNER_ID)
            .bind(Utc::now().to_rfc3339())
            .execute(&state.pool)
            .await
            .unwrap();
        let _ = crate::chat_identity::update_roster(
            State(state.clone()),
            chat_headers(TEST_MEMBER_A_ID),
            Json(signed.clone()),
        )
        .await
        .unwrap();
        assert!(
            crate::ensure_conversation_member(&state.pool, &room.id, recipient)
                .await
                .is_ok()
        );
        next.revision = 2;
        next.previous = Some(signed.hash().unwrap());
        next.members.get_mut(&recipient).unwrap().role = Role::Admin;
        assert!(
            crate::chat_identity::update_roster(
                State(state),
                chat_headers(TEST_MEMBER_A_ID),
                Json(next.sign(&key).unwrap())
            )
            .await
            .is_err()
        );
    }
}
