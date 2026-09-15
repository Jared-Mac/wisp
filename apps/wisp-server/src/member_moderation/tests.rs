use super::*;
use crate::{
    TEST_MEMBER_A_ID, TEST_MEMBER_B_ID, TEST_OWNER_ID, router,
    tests::test_config,
    text_tests::{request, value},
};
use axum::{Router, http::StatusCode, routing::post};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

async fn setup() -> (AppState, Router) {
    let state = AppState::new(test_config()).await.unwrap();
    for (id, name) in [
        (TEST_OWNER_ID, "Owner"),
        (TEST_MEMBER_A_ID, "Admin"),
        (TEST_MEMBER_B_ID, "OfflineMember"),
    ] {
        sqlx::query("UPDATE users SET username=? WHERE id=?")
            .bind(name)
            .bind(id)
            .execute(&state.pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO server_identity(id,owner_user_id) VALUES(1,?)")
        .bind(TEST_OWNER_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO server_admins(user_id,granted_by,granted_at) VALUES(?,?,?)")
        .bind(TEST_MEMBER_A_ID)
        .bind(TEST_OWNER_ID)
        .bind(Utc::now().to_rfc3339())
        .execute(&state.pool)
        .await
        .unwrap();
    (state.clone(), router(state))
}

async fn change(app: &Router, actor: &str, target: &str, action: &str) -> axum::response::Response {
    request(
        app,
        "POST",
        "/v1/server/members/moderate",
        actor,
        json!({"user_id":target,"action":action,"reason":"Test moderation"}),
    )
    .await
}

async fn invitation(state: &AppState, code: &str) {
    sqlx::query("INSERT INTO server_invites(id,code_hash,created_by,created_at,expires_at) VALUES(?,?,?,?,?)")
        .bind(Uuid::new_v4().to_string()).bind(crate::token_hash(code)).bind(TEST_OWNER_ID).bind(Utc::now().to_rfc3339())
        .bind((Utc::now()+chrono::Duration::hours(1)).to_rfc3339()).execute(&state.pool).await.unwrap();
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // One lifecycle: ban, rename, rejected admission, unban and reinvite.
async fn offline_ban_is_persistent_and_unban_requires_a_new_admission() {
    let (state, app) = setup().await;
    let target = Uuid::parse_str(TEST_MEMBER_B_ID).unwrap();
    let dm =
        crate::find_or_create_direct(&state.pool, Uuid::parse_str(TEST_OWNER_ID).unwrap(), target)
            .await
            .unwrap();
    let before_friends: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM friendships")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert!(
        state
            .runtime
            .read()
            .await
            .users
            .get(&target)
            .is_none_or(|u| u.connections == 0)
    );
    let settings = value(
        request(
            &app,
            "GET",
            "/v1/server/settings",
            TEST_MEMBER_A_ID,
            json!({}),
        )
        .await,
    )
    .await;
    assert!(
        settings["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == TEST_MEMBER_B_ID)
    );
    let result = value(change(&app, TEST_MEMBER_A_ID, TEST_MEMBER_B_ID, "ban").await).await;
    assert_eq!(result["media_pending"], false);
    assert!(
        !crate::account_membership::is_member(&state.pool, target)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM friendships")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        before_friends
    );
    crate::ensure_conversation_member(&state.pool, &dm, target)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET display_name='Renamed member',username='NewLogin' WHERE id=?")
        .bind(TEST_MEMBER_B_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        change(&app, TEST_OWNER_ID, TEST_MEMBER_B_ID, "ban")
            .await
            .status(),
        StatusCode::OK
    );
    let settings =
        value(request(&app, "GET", "/v1/server/settings", TEST_OWNER_ID, json!({})).await).await;
    assert_eq!(settings["bans"].as_array().unwrap().len(), 1);
    assert_eq!(settings["bans"][0]["display_name"], "Renamed member");
    assert!(
        !settings["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == TEST_MEMBER_B_ID)
    );
    assert_eq!(
        request(
            &app,
            "GET",
            "/v1/server/settings",
            TEST_MEMBER_B_ID,
            json!({})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/v1/livekit/token",
            TEST_MEMBER_B_ID,
            json!({})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    invitation(&state, "moderation-test-invite").await;
    assert_eq!(
        request(
            &app,
            "POST",
            "/v2/server/join",
            TEST_MEMBER_B_ID,
            json!({"code":"moderation-test-invite"})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM server_invites WHERE used_at IS NOT NULL"
        )
        .fetch_one(&state.pool)
        .await
        .unwrap(),
        0
    );
    assert!(
        sqlx::query("UPDATE users SET server_member=1 WHERE id=?")
            .bind(TEST_MEMBER_B_ID)
            .execute(&state.pool)
            .await
            .is_err()
    );
    assert_eq!(
        change(&app, TEST_MEMBER_A_ID, TEST_MEMBER_B_ID, "unban")
            .await
            .status(),
        StatusCode::OK
    );
    assert!(
        !crate::account_membership::is_member(&state.pool, target)
            .await
            .unwrap()
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/v2/server/join",
            TEST_MEMBER_B_ID,
            json!({"code":"moderation-test-invite"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert!(
        crate::account_membership::is_member(&state.pool, target)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn moderation_permissions_and_kick_reinvite() {
    let (state, app) = setup().await;
    sqlx::query("INSERT INTO server_admins(user_id,granted_by,granted_at) VALUES(?,?,?)")
        .bind(crate::TEST_MEMBER_C_ID)
        .bind(TEST_OWNER_ID)
        .bind(Utc::now().to_rfc3339())
        .execute(&state.pool)
        .await
        .unwrap();
    for (actor, target) in [
        (TEST_MEMBER_B_ID, TEST_MEMBER_A_ID),
        (TEST_MEMBER_A_ID, TEST_OWNER_ID),
        (TEST_OWNER_ID, TEST_OWNER_ID),
        (TEST_MEMBER_A_ID, TEST_MEMBER_A_ID),
        (TEST_MEMBER_A_ID, crate::TEST_MEMBER_C_ID),
    ] {
        assert_eq!(
            change(&app, actor, target, "kick").await.status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        change(&app, TEST_OWNER_ID, &Uuid::new_v4().to_string(), "ban")
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/v1/server/members/moderate",
            TEST_OWNER_ID,
            json!({"user_id":TEST_MEMBER_B_ID,"action":"ban","reason":"x".repeat(281)})
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        change(&app, TEST_OWNER_ID, TEST_MEMBER_A_ID, "kick")
            .await
            .status(),
        StatusCode::OK
    );
    assert!(
        !crate::server_management::is_manager(
            &state.pool,
            Uuid::parse_str(TEST_MEMBER_A_ID).unwrap()
        )
        .await
        .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM server_bans")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        change(&app, TEST_MEMBER_A_ID, TEST_MEMBER_B_ID, "ban")
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    invitation(&state, "reinvite-test").await;
    assert_eq!(
        request(
            &app,
            "POST",
            "/v2/server/join",
            TEST_MEMBER_A_ID,
            json!({"code":"reinvite-test"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert!(
        !crate::server_management::is_manager(
            &state.pool,
            Uuid::parse_str(TEST_MEMBER_A_ID).unwrap()
        )
        .await
        .unwrap()
    );
}

#[tokio::test]
async fn voice_disconnect_retries_without_restoring_membership() {
    let healthy = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let health = healthy.clone();
    let count = calls.clone();
    let media = Router::new().route(
        "/twirp/livekit.RoomService/RemoveParticipant",
        post(move |Json(body): Json<Value>| {
            let health = health.clone();
            let count = count.clone();
            async move {
                assert_eq!(body["identity"], TEST_MEMBER_B_ID);
                assert_eq!(body["room"], "moderation-test-room");
                count.fetch_add(1, Ordering::Relaxed);
                if health.load(Ordering::Relaxed) {
                    StatusCode::OK
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("ws://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, media).await.unwrap() });
    let (mut state, _) = setup().await;
    Arc::make_mut(&mut state.config).livekit_url = origin;
    let app = router(state.clone());
    let room = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO hangouts(id,livekit_room,created_at) VALUES(?,'moderation-test-room',?)",
    )
    .bind(&room)
    .bind(Utc::now().to_rfc3339())
    .execute(&state.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO hangout_members(hangout_id,user_id,joined_at) VALUES(?,?,?)")
        .bind(&room)
        .bind(TEST_MEMBER_B_ID)
        .bind(Utc::now().to_rfc3339())
        .execute(&state.pool)
        .await
        .unwrap();
    let result = value(change(&app, TEST_OWNER_ID, TEST_MEMBER_B_ID, "ban").await).await;
    assert_eq!(result["media_pending"], true);
    assert!(
        crate::active_hangout_for(&state.pool, Uuid::parse_str(TEST_MEMBER_B_ID).unwrap())
            .await
            .unwrap()
            .is_none()
    );
    healthy.store(true, Ordering::Relaxed);
    disconnect_pending(&state, None).await.unwrap();
    assert!(calls.load(Ordering::Relaxed) >= 2);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM server_member_disconnects")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        0
    );
    assert!(
        !crate::account_membership::is_member(
            &state.pool,
            Uuid::parse_str(TEST_MEMBER_B_ID).unwrap()
        )
        .await
        .unwrap()
    );
    task.abort();
}
