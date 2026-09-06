use super::*;
use tests::{chat_headers, test_config};
use text_tests::{request, value};

#[tokio::test]
async fn migration_opens_implicit_rooms_once_and_preserves_later_privacy_choices() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let current = sqlx::migrate!("../../migrations");
    let previous = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            current.iter().filter(|m| m.version < 23).cloned().collect(),
        ),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    previous.run(&pool).await.unwrap();
    seed_development_users(&pool).await.unwrap();
    sqlx::query("INSERT INTO server_identity(id,owner_user_id) VALUES (1,?)")
        .bind(TEST_OWNER_ID)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE spots SET private=1 WHERE id=?")
        .bind(TEST_ROOM_ID)
        .execute(&pool)
        .await
        .unwrap();
    let conversation = format!("spot:{TEST_ROOM_ID}");
    sqlx::query("INSERT INTO pending_room_admissions(conversation_id,user_id,invited_by,created_at) VALUES (?,?,?,?)")
        .bind(&conversation).bind(TEST_MEMBER_A_ID).bind(TEST_OWNER_ID).bind(Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
    current.run(&pool).await.unwrap();
    let schema: i64 = sqlx::query_scalar("SELECT MAX(version) FROM schema_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(schema, 23);
    let private: bool = sqlx::query_scalar("SELECT private FROM spots WHERE id=?")
        .bind(TEST_ROOM_ID)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!private);
    let automatic: bool = sqlx::query_scalar(
        "SELECT automatic FROM pending_room_admissions WHERE conversation_id=? AND user_id=?",
    )
    .bind(&conversation)
    .bind(TEST_MEMBER_A_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !automatic,
        "Existing explicit invitations must remain explicit"
    );
    sqlx::query("UPDATE spots SET private=1 WHERE id=?")
        .bind(TEST_ROOM_ID)
        .execute(&pool)
        .await
        .unwrap();
    current.run(&pool).await.unwrap();
    let private: bool = sqlx::query_scalar("SELECT private FROM spots WHERE id=?")
        .bind(TEST_ROOM_ID)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        private,
        "Restarting must preserve the admin's privacy choice"
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap()
            .is_empty()
    );
}

async fn setup() -> (AppState, Router) {
    let state = AppState::new(test_config()).await.unwrap();
    sqlx::query("INSERT INTO server_identity(id,owner_user_id) VALUES (1,?)")
        .bind(TEST_OWNER_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    for id in [TEST_OWNER_ID, TEST_MEMBER_A_ID, TEST_MEMBER_B_ID] {
        sqlx::query("UPDATE users SET username=? WHERE id=?")
            .bind(id)
            .bind(id)
            .execute(&state.pool)
            .await
            .unwrap();
    }
    let app = router(state.clone());
    (state, app)
}

#[tokio::test]
async fn automatic_admissions_only_offer_identities_the_signer_can_resolve() {
    let (state, app) = setup().await;
    value(
        request(
            &app,
            "POST",
            "/v1/rooms",
            TEST_OWNER_ID,
            json!({"name":"Public lobby"}),
        )
        .await,
    )
    .await;
    let directory = chat_identity::directory(State(state.clone()), chat_headers(TEST_OWNER_ID))
        .await
        .unwrap()
        .0;
    assert!(
        directory["pending_admissions"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    for user in [TEST_MEMBER_A_ID, TEST_MEMBER_B_ID] {
        let identity = wisp_crypto::Identity::generate().unwrap();
        sqlx::query("INSERT INTO chat_identities(user_id,public_identity) VALUES (?,?)")
            .bind(user)
            .bind(serde_json::to_string(&identity.public()).unwrap())
            .execute(&state.pool)
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM friendships WHERE (first_user_id=? AND second_user_id=?) OR (first_user_id=? AND second_user_id=?)")
        .bind(TEST_OWNER_ID).bind(TEST_MEMBER_B_ID).bind(TEST_MEMBER_B_ID).bind(TEST_OWNER_ID).execute(&state.pool).await.unwrap();
    let directory = chat_identity::directory(State(state), chat_headers(TEST_OWNER_ID))
        .await
        .unwrap()
        .0;
    let admissions = directory["pending_admissions"].as_array().unwrap();
    assert_eq!(admissions.len(), 1);
    assert_eq!(admissions[0]["user_id"], TEST_MEMBER_A_ID);
}

#[tokio::test]
async fn public_rooms_are_discoverable_and_voice_accessible_without_exposing_chat_history() {
    let (state, app) = setup().await;
    let room = value(
        request(
            &app,
            "POST",
            "/v1/rooms",
            TEST_OWNER_ID,
            json!({"name":"Public room"}),
        )
        .await,
    )
    .await;
    let id = room["id"].as_str().unwrap();
    let spot = room["spot_id"].as_str().unwrap();
    let outsider = TEST_MEMBER_A_ID.parse().unwrap();
    assert!(
        !load_spots(&state.pool, outsider)
            .await
            .unwrap()
            .iter()
            .find(|s| s.id == spot)
            .unwrap()
            .private
    );
    let preview = load_conversation(&state.pool, outsider, id).await.unwrap();
    assert!(preview.pending_access);
    assert!(
        preview.last_message.is_none() && preview.members.is_empty() && preview.unread_count == 0
    );
    assert!(
        ensure_conversation_member(&state.pool, id, outsider)
            .await
            .is_err()
    );
    sqlx::query("INSERT INTO messages(id,conversation_id,sender_id,created_at,content_type,payload) VALUES (?,?,?,?, 'text/plain','\"Existing history\"')")
        .bind(Uuid::new_v4().to_string()).bind(id).bind(TEST_OWNER_ID).bind(Utc::now().to_rfc3339()).execute(&state.pool).await.unwrap();
    let snapshot = state.snapshot(outsider).await.unwrap();
    assert!(
        snapshot
            .conversations
            .iter()
            .any(|c| c.id == id && c.pending_access)
    );
    assert!(!snapshot.messages.iter().any(|m| m.conversation_id == id));
    assert!(
        load_conversation(&state.pool, outsider, id)
            .await
            .unwrap()
            .last_message
            .is_none()
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/v1/messages",
            TEST_MEMBER_A_ID,
            json!({"conversation_id":id,"content_type":"text/plain","payload":"No access"})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let _ = join_spot(
        State(state.clone()),
        chat_headers(TEST_MEMBER_A_ID),
        Json(JoinSpotRequest {
            spot_id: spot.into(),
        }),
    )
    .await
    .unwrap();
    assert!(
        active_hangout_for(&state.pool, outsider)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        ensure_conversation_member(&state.pool, id, outsider)
            .await
            .is_err()
    );
    let queued: i64=sqlx::query_scalar("SELECT COUNT(*) FROM pending_room_admissions WHERE conversation_id=? AND user_id=? AND automatic=1").bind(id).bind(TEST_MEMBER_A_ID).fetch_one(&state.pool).await.unwrap();
    assert_eq!(queued, 1);
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Exercise the full privacy transition, including legacy clients.
async fn only_admins_can_make_rooms_invite_only_and_private_rooms_require_access() {
    let (state, app) = setup().await;
    let room = value(
        request(
            &app,
            "POST",
            "/v1/rooms",
            TEST_OWNER_ID,
            json!({"name":"Restricted room"}),
        )
        .await,
    )
    .await;
    let id = room["id"].as_str().unwrap();
    let spot = room["spot_id"].as_str().unwrap();
    let outsider = TEST_MEMBER_A_ID.parse().unwrap();
    let path = format!("/v1/server/rooms/{id}");
    assert_eq!(
        request(
            &app,
            "PATCH",
            &path,
            TEST_MEMBER_A_ID,
            json!({"name":"Restricted room","private":true})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    value(
        request(
            &app,
            "PATCH",
            &path,
            TEST_OWNER_ID,
            json!({"name":"Restricted room","private":true}),
        )
        .await,
    )
    .await;
    assert!(
        !load_spots(&state.pool, outsider)
            .await
            .unwrap()
            .iter()
            .any(|s| s.id == spot)
    );
    assert!(
        !load_conversations(&state.pool, outsider)
            .await
            .unwrap()
            .iter()
            .any(|c| c.id == id)
    );
    assert!(load_conversation(&state.pool, outsider, id).await.is_err());
    assert!(
        room_access::ensure_voice_access(&state.pool, spot, outsider)
            .await
            .is_err()
    );
    let queued: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_room_admissions WHERE conversation_id=? AND automatic=1",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(queued, 0);
    // Legacy rename requests omit privacy and must preserve the admin's choice.
    value(
        request(
            &app,
            "PATCH",
            &path,
            TEST_OWNER_ID,
            json!({"name":"Still restricted"}),
        )
        .await,
    )
    .await;
    assert!(
        room_access::ensure_voice_access(&state.pool, spot, outsider)
            .await
            .is_err()
    );
    value(
        request(
            &app,
            "POST",
            "/v1/rooms/invite",
            TEST_OWNER_ID,
            json!({"conversation_id":id,"user_id":TEST_MEMBER_A_ID}),
        )
        .await,
    )
    .await;
    assert!(
        load_spots(&state.pool, outsider)
            .await
            .unwrap()
            .iter()
            .any(|s| s.id == spot && s.private)
    );
    assert!(
        !load_conversation(&state.pool, outsider, id)
            .await
            .unwrap()
            .pending_access
    );
    room_access::ensure_voice_access(&state.pool, spot, outsider)
        .await
        .unwrap();
    value(
        request(
            &app,
            "PATCH",
            &path,
            TEST_OWNER_ID,
            json!({"name":"Open again","private":false}),
        )
        .await,
    )
    .await;
    let other = TEST_MEMBER_B_ID.parse().unwrap();
    assert!(
        load_conversation(&state.pool, other, id)
            .await
            .unwrap()
            .pending_access
    );
    room_access::ensure_voice_access(&state.pool, spot, other)
        .await
        .unwrap();
}

#[tokio::test]
async fn ordinary_server_invitation_discovers_public_rooms_but_not_private_rooms() {
    let (state, app) = setup().await;
    assert_eq!(
        request(
            &app,
            "POST",
            "/v1/rooms",
            TEST_MEMBER_A_ID,
            json!({"name":"Unauthorized private room","private":true})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let public = value(
        request(
            &app,
            "POST",
            "/v1/rooms",
            TEST_OWNER_ID,
            json!({"name":"Lobby"}),
        )
        .await,
    )
    .await;
    let private = value(
        request(
            &app,
            "POST",
            "/v1/rooms",
            TEST_OWNER_ID,
            json!({"name":"Invite only","private":true}),
        )
        .await,
    )
    .await;
    let invite = create_account_invite(
        State(state.clone()),
        chat_headers(TEST_OWNER_ID),
        Json(CreateAccountInviteRequest {
            kind: AccountInviteKind::Friend,
            conversation_id: None,
            expires_in_minutes: Some(30),
        }),
    )
    .await
    .unwrap()
    .0;
    let credential = register_account(
        State(state.clone()),
        Json(RegisterAccountRequest {
            protocol_version: PROTOCOL_VERSION,
            invite_code: invite.code,
            username: "new-member".into(),
            display_name: "New member".into(),
            password: "long-test-passphrase".into(),
            device_name: "Synthetic client".into(),
        }),
    )
    .await
    .unwrap()
    .0;
    let snapshot = state.snapshot(credential.user.id).await.unwrap();
    assert!(
        snapshot
            .spots
            .iter()
            .any(|s| Some(s.id.as_str()) == public["spot_id"].as_str())
    );
    assert!(
        !snapshot
            .spots
            .iter()
            .any(|s| Some(s.id.as_str()) == private["spot_id"].as_str())
    );
    assert!(
        snapshot
            .conversations
            .iter()
            .any(|c| Some(c.id.as_str()) == public["id"].as_str() && c.pending_access)
    );
    let queued: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pending_room_admissions WHERE user_id=? AND automatic=1",
    )
    .bind(credential.user.id.to_string())
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(queued, 2); // Lobby and seeded public test room.
}
