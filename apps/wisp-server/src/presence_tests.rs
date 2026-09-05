use super::*;
use tests::{chat_headers, test_config};
use text_tests::{request, value};

async fn save(app: &Router, id: &str, presence: Presence) {
    value(
        request(
            app,
            "POST",
            "/v1/presence",
            id,
            json!({"presence":presence}),
        )
        .await,
    )
    .await;
}

#[tokio::test]
async fn saved_presence_survives_restart_and_controls_joins_before_any_client_restores_it() {
    let path = std::env::temp_dir().join(format!("wisp-presence-{}.sqlite3", Uuid::new_v4()));
    let mut config = test_config();
    config.database_url = format!("sqlite://{}", path.display());
    let members = [
        TEST_OWNER_ID,
        TEST_MEMBER_A_ID,
        TEST_MEMBER_B_ID,
        TEST_MEMBER_C_ID,
    ];
    let choices = [
        Presence::Closed,
        Presence::Knock,
        Presence::Away,
        Presence::Open,
    ];
    let state = AppState::new(config.clone()).await.unwrap();
    let app = router(state.clone());
    for (id, presence) in members.iter().zip(choices) {
        assert_eq!(
            state
                .snapshot(id.parse().unwrap())
                .await
                .unwrap()
                .self_state
                .presence,
            Presence::Open
        );
        save(&app, id, presence).await;
    }
    // A later choice replaces the previous choice, including returning to Open.
    save(&app, TEST_MEMBER_C_ID, Presence::Closed).await;
    save(&app, TEST_MEMBER_C_ID, Presence::Open).await;
    drop(app);
    state.pool.close().await;
    drop(state);

    let state = AppState::new(config).await.unwrap();
    for (id, presence) in members.iter().zip(choices) {
        let id = id.parse().unwrap();
        let snapshot = state.snapshot(id).await.unwrap();
        assert_eq!(snapshot.self_state.presence, presence);
        assert!(snapshot.self_state.hangout_id.is_none());
        assert!(snapshot.knocks.is_empty());
        state.set_connected(id, true).await;
        state.set_connected(id, false).await;
        state.set_connected(id, true).await;
        assert_eq!(
            state.snapshot(id).await.unwrap().self_state.presence,
            presence
        );
    }
    let snapshot = state
        .snapshot(TEST_OWNER_ID.parse().unwrap())
        .await
        .unwrap();
    for (id, presence) in members.iter().zip(choices).skip(1) {
        let friend = snapshot
            .friends
            .iter()
            .find(|friend| friend.user.id.to_string() == *id)
            .unwrap();
        assert!(friend.online);
        assert_eq!(friend.presence, presence);
    }
    for id in [TEST_OWNER_ID, TEST_MEMBER_B_ID] {
        let error = join_friend(
            State(state.clone()),
            chat_headers(TEST_MEMBER_C_ID),
            Json(JoinFriendRequest { friend: id.into() }),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "friend_unavailable");
    }
    let knock = join_friend(
        State(state.clone()),
        chat_headers(TEST_OWNER_ID),
        Json(JoinFriendRequest {
            friend: TEST_MEMBER_A_ID.into(),
        }),
    )
    .await
    .unwrap()
    .0;
    assert!(matches!(knock, JoinFriendResult::KnockSent { .. }));
    let open = join_friend(
        State(state.clone()),
        chat_headers(TEST_OWNER_ID),
        Json(JoinFriendRequest {
            friend: TEST_MEMBER_C_ID.into(),
        }),
    )
    .await
    .unwrap()
    .0;
    assert!(matches!(open, JoinFriendResult::Joined { .. }));
    state.pool.close().await;
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn failed_or_invalid_presence_updates_do_not_change_the_saved_choice() {
    let state = AppState::new(test_config()).await.unwrap();
    let app = router(state.clone());
    value(
        request(
            &app,
            "POST",
            "/v1/presence",
            TEST_OWNER_ID,
            json!({"presence":"knock"}),
        )
        .await,
    )
    .await;
    assert!(
        !request(
            &app,
            "POST",
            "/v1/presence",
            TEST_OWNER_ID,
            json!({"presence":"invalid"})
        )
        .await
        .status()
        .is_success()
    );
    sqlx::query("CREATE TRIGGER fail_presence BEFORE UPDATE OF presence ON users BEGIN SELECT RAISE(FAIL, 'test write failure'); END")
        .execute(&state.pool).await.unwrap();
    assert_eq!(
        request(
            &app,
            "POST",
            "/v1/presence",
            TEST_OWNER_ID,
            json!({"presence":"open"})
        )
        .await
        .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        state
            .snapshot(TEST_OWNER_ID.parse().unwrap())
            .await
            .unwrap()
            .self_state
            .presence,
        Presence::Knock
    );
}
