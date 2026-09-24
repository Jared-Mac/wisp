use super::*;
use text_tests::{request, value};

async fn setup() -> (AppState, Router) {
    let state = AppState::new(tests::test_config()).await.unwrap();
    sqlx::query("DELETE FROM friendships")
        .execute(&state.pool)
        .await
        .unwrap();
    (state.clone(), router(state))
}

async fn list(app: &Router, user: &str) -> Value {
    value(request(app, "GET", "/v1/people", user, json!({})).await).await
}
fn relation(people: &Value, user: &str) -> String {
    people["people"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == user)
        .unwrap()["relationship"]
        .as_str()
        .unwrap()
        .into()
}
fn path(user: &str) -> String {
    format!("/v1/friend-requests/{user}")
}

#[tokio::test]
async fn members_are_visible_without_friendship_but_private_details_are_not() {
    let (_, app) = setup().await;
    let directory = list(&app, TEST_OWNER_ID).await;
    assert_eq!(directory["people"].as_array().unwrap().len(), 4);
    for person in directory["people"].as_array().unwrap() {
        assert_eq!(person.as_object().unwrap().len(), 4);
        assert_eq!(person["server_member"], true);
        assert!(person["id"].is_string() && person["display_name"].is_string());
    }
    assert_eq!(relation(&directory, TEST_OWNER_ID), "self");
    assert_eq!(relation(&directory, TEST_MEMBER_A_ID), "none");
    assert_eq!(
        request(&app, "GET", "/v1/people", "not-authenticated", json!({}))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/v1/conversations/direct",
            TEST_OWNER_ID,
            json!({"friend":TEST_MEMBER_A_ID})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Follow one request through retries, crossed sends, and mutual consent.
async fn friend_requests_require_recipient_consent_and_support_idempotent_retries() {
    let (state, app) = setup().await;
    let mut events = state.events.subscribe();
    let sent = value(
        request(
            &app,
            "POST",
            &path(TEST_MEMBER_A_ID),
            TEST_OWNER_ID,
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(relation(&sent, TEST_MEMBER_A_ID), "outgoing");
    let event = events.try_recv().unwrap();
    assert_eq!(event.name, "friend_requests_changed");
    assert_eq!(event.payload, json!({"changed":true}));
    value(
        request(
            &app,
            "POST",
            &path(TEST_MEMBER_A_ID),
            TEST_OWNER_ID,
            json!({}),
        )
        .await,
    )
    .await;
    assert!(events.try_recv().is_err());
    assert_eq!(
        relation(&list(&app, TEST_MEMBER_A_ID).await, TEST_OWNER_ID),
        "incoming"
    );
    let snapshot = state
        .snapshot(TEST_OWNER_ID.parse().unwrap())
        .await
        .unwrap();
    assert!(
        !snapshot
            .friends
            .iter()
            .any(|p| p.user.id.to_string() == TEST_MEMBER_A_ID)
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("{}/accept", path(TEST_MEMBER_A_ID)),
            TEST_OWNER_ID,
            json!({})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("{}/accept", path(TEST_OWNER_ID)),
            TEST_MEMBER_B_ID,
            json!({})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    // A crossed send still requires an explicit acceptance.
    let crossed = value(
        request(
            &app,
            "POST",
            &path(TEST_OWNER_ID),
            TEST_MEMBER_A_ID,
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(relation(&crossed, TEST_OWNER_ID), "incoming");
    for _ in 0..2 {
        let accepted = value(
            request(
                &app,
                "POST",
                &format!("{}/accept", path(TEST_OWNER_ID)),
                TEST_MEMBER_A_ID,
                json!({}),
            )
            .await,
        )
        .await;
        assert_eq!(relation(&accepted, TEST_OWNER_ID), "friend");
    }
    assert_eq!(
        relation(&list(&app, TEST_OWNER_ID).await, TEST_MEMBER_A_ID),
        "friend"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM friend_requests")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        0
    );
    value(
        request(
            &app,
            "POST",
            "/v1/conversations/direct",
            TEST_OWNER_ID,
            json!({"friend":TEST_MEMBER_A_ID}),
        )
        .await,
    )
    .await;
    value(
        request(
            &app,
            "POST",
            &path(TEST_MEMBER_A_ID),
            TEST_OWNER_ID,
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM friendships")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        1
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&state.pool)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn requests_can_be_declined_or_cancelled_without_affecting_other_members() {
    let (state, app) = setup().await;
    for remover in [TEST_OWNER_ID, TEST_MEMBER_A_ID] {
        value(
            request(
                &app,
                "POST",
                &path(TEST_MEMBER_A_ID),
                TEST_OWNER_ID,
                json!({}),
            )
            .await,
        )
        .await;
        value(
            request(
                &app,
                "DELETE",
                &path(TEST_OWNER_ID),
                TEST_MEMBER_B_ID,
                json!({}),
            )
            .await,
        )
        .await;
        assert_eq!(
            relation(&list(&app, TEST_OWNER_ID).await, TEST_MEMBER_A_ID),
            "outgoing"
        );
        let target = if remover == TEST_OWNER_ID {
            TEST_MEMBER_A_ID
        } else {
            TEST_OWNER_ID
        };
        value(request(&app, "DELETE", &path(target), remover, json!({})).await).await;
        value(request(&app, "DELETE", &path(target), remover, json!({})).await).await;
        assert_eq!(
            relation(&list(&app, TEST_OWNER_ID).await, TEST_MEMBER_A_ID),
            "none"
        );
    }
    for action in ["POST", "DELETE"] {
        assert_eq!(
            request(&app, action, &path(TEST_OWNER_ID), TEST_OWNER_ID, json!({}))
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            request(
                &app,
                action,
                &path(&Uuid::new_v4().to_string()),
                TEST_OWNER_ID,
                json!({})
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM friendships")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn concurrent_crossed_requests_leave_one_pending_invitation() {
    let (state, app) = setup().await;
    let left = path(TEST_MEMBER_A_ID);
    let right = path(TEST_OWNER_ID);
    let (a, b) = tokio::join!(
        request(&app, "POST", &left, TEST_OWNER_ID, json!({})),
        request(&app, "POST", &right, TEST_MEMBER_A_ID, json!({}))
    );
    value(a).await;
    value(b).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM friend_requests")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM friendships")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn contacts_outside_the_server_keep_relationships_without_appearing_as_members() {
    let (state, app) = setup().await;
    value(
        request(
            &app,
            "POST",
            &path(TEST_MEMBER_A_ID),
            TEST_OWNER_ID,
            json!({}),
        )
        .await,
    )
    .await;
    value(
        request(
            &app,
            "POST",
            &format!("{}/accept", path(TEST_OWNER_ID)),
            TEST_MEMBER_A_ID,
            json!({}),
        )
        .await,
    )
    .await;
    value(
        request(
            &app,
            "POST",
            &path(TEST_OWNER_ID),
            TEST_MEMBER_B_ID,
            json!({}),
        )
        .await,
    )
    .await;
    sqlx::query("UPDATE users SET server_member=0 WHERE id IN (?,?)")
        .bind(TEST_MEMBER_A_ID)
        .bind(TEST_MEMBER_B_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    let directory = list(&app, TEST_OWNER_ID).await;
    assert_eq!(relation(&directory, TEST_MEMBER_A_ID), "friend");
    assert_eq!(relation(&directory, TEST_MEMBER_B_ID), "incoming");
    for person in directory["people"].as_array().unwrap() {
        if person["id"] == TEST_MEMBER_A_ID || person["id"] == TEST_MEMBER_B_ID {
            assert_eq!(person["server_member"], false);
        }
    }
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Follow removal, retries and renewed consent through one relationship.
async fn removing_friend_is_scoped_mutual_and_preserves_membership_and_chats() {
    let (state, app) = setup().await;
    value(
        request(
            &app,
            "POST",
            &path(TEST_MEMBER_A_ID),
            TEST_OWNER_ID,
            json!({}),
        )
        .await,
    )
    .await;
    value(
        request(
            &app,
            "POST",
            &format!("{}/accept", path(TEST_OWNER_ID)),
            TEST_MEMBER_A_ID,
            json!({}),
        )
        .await,
    )
    .await;
    value(
        request(
            &app,
            "POST",
            "/v1/conversations/direct",
            TEST_OWNER_ID,
            json!({"friend":TEST_MEMBER_A_ID}),
        )
        .await,
    )
    .await;
    let before_chats = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM conversation_members")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    let before_rooms = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM hangout_members")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    let endpoint = format!("/v1/friends/{TEST_MEMBER_A_ID}");
    assert_eq!(
        request(&app, "DELETE", &endpoint, "not-authenticated", json!({}))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    value(request(&app, "DELETE", &endpoint, TEST_MEMBER_B_ID, json!({})).await).await;
    assert_eq!(
        relation(&list(&app, TEST_OWNER_ID).await, TEST_MEMBER_A_ID),
        "friend"
    );
    for _ in 0..2 {
        value(request(&app, "DELETE", &endpoint, TEST_OWNER_ID, json!({})).await).await;
    }
    assert_eq!(
        relation(&list(&app, TEST_OWNER_ID).await, TEST_MEMBER_A_ID),
        "none"
    );
    assert_eq!(
        relation(&list(&app, TEST_MEMBER_A_ID).await, TEST_OWNER_ID),
        "none"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM conversation_members")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        before_chats
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM hangout_members")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        before_rooms
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT server_member FROM users WHERE id=?")
            .bind(TEST_MEMBER_A_ID)
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        1
    );
    // A fresh request is allowed, but becoming friends still requires consent.
    let requested = value(
        request(
            &app,
            "POST",
            &path(TEST_MEMBER_A_ID),
            TEST_OWNER_ID,
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(relation(&requested, TEST_MEMBER_A_ID), "outgoing");
    value(request(&app, "DELETE", &endpoint, TEST_OWNER_ID, json!({})).await).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM friend_requests")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("{}/accept", path(TEST_OWNER_ID)),
            TEST_MEMBER_A_ID,
            json!({})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &app,
            "DELETE",
            &format!("/v1/friends/{TEST_OWNER_ID}"),
            TEST_OWNER_ID,
            json!({})
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&state.pool)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn removing_nonmember_friend_remains_retryable_after_directory_disappearance() {
    let (state, app) = setup().await;
    value(
        request(
            &app,
            "POST",
            &path(TEST_MEMBER_A_ID),
            TEST_OWNER_ID,
            json!({}),
        )
        .await,
    )
    .await;
    value(
        request(
            &app,
            "POST",
            &format!("{}/accept", path(TEST_OWNER_ID)),
            TEST_MEMBER_A_ID,
            json!({}),
        )
        .await,
    )
    .await;
    sqlx::query("UPDATE users SET server_member=0 WHERE id=?")
        .bind(TEST_MEMBER_A_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        relation(&list(&app, TEST_OWNER_ID).await, TEST_MEMBER_A_ID),
        "friend"
    );
    for _ in 0..2 {
        let directory = value(
            request(
                &app,
                "DELETE",
                &format!("/v1/friends/{TEST_MEMBER_A_ID}"),
                TEST_OWNER_ID,
                json!({}),
            )
            .await,
        )
        .await;
        assert!(
            !directory["people"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["id"] == TEST_MEMBER_A_ID)
        );
    }
    value(
        request(
            &app,
            "DELETE",
            &format!("/v1/friends/{}", Uuid::new_v4()),
            TEST_OWNER_ID,
            json!({}),
        )
        .await,
    )
    .await;
}
