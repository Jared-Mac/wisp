use super::*;
use text_tests::{request, value};

async fn setup() -> (AppState, Router) {
    let state = AppState::new(tests::test_config()).await.unwrap();
    sqlx::query(
        "INSERT OR IGNORE INTO server_identity(id,owner_user_id,name) VALUES(1,?,'Test community')",
    )
    .bind(TEST_OWNER_ID)
    .execute(&state.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE users SET username=lower(display_name)")
        .execute(&state.pool)
        .await
        .unwrap();
    let identity = wisp_crypto::Identity::generate().unwrap();
    sqlx::query("INSERT INTO chat_identities(user_id,public_identity) VALUES(?,?)")
        .bind(TEST_OWNER_ID)
        .bind(serde_json::to_string(&identity.public()).unwrap())
        .execute(&state.pool)
        .await
        .unwrap();
    let app = router(state.clone());
    (state, app)
}
async fn signup(app: &Router, name: &str) -> String {
    let response=request(app,"POST","/v2/accounts/register",TEST_OWNER_ID,json!({"username":name,"display_name":name,"password":"test-only-password-123","device_name":"Test device","protocol_version":1})).await;
    assert_eq!(response.status(), StatusCode::OK);
    value(response).await["user"]["id"].as_str().unwrap().into()
}

#[tokio::test]
async fn public_signup_has_no_membership_or_room_metadata() {
    let (state, app) = setup().await;
    let channel = value(
        request(
            &app,
            "POST",
            "/v1/server/channels",
            TEST_OWNER_ID,
            json!({"name":"Members only"}),
        )
        .await,
    )
    .await;
    let user = signup(&app, "new-user").await;
    let snapshot = state.snapshot(user.parse().unwrap()).await.unwrap();
    assert!(!snapshot.server_member);
    assert_eq!(snapshot.server_name, "Home");
    assert!(
        snapshot.spots.is_empty()
            && snapshot.hangouts.is_empty()
            && snapshot.conversations.is_empty()
    );
    assert!(snapshot.friends.is_empty() && snapshot.voice_moderation.is_empty());
    let people = value(request(&app, "GET", "/v1/people", &user, json!({})).await).await;
    assert_eq!(people["people"].as_array().unwrap().len(), 1);
    let members = value(request(&app, "GET", "/v1/people", TEST_OWNER_ID, json!({})).await).await;
    assert!(
        !members["people"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == user)
    );
    for (method, path, body) in [
        ("GET", "/v1/server/settings", json!({})),
        ("POST", "/v1/server/channels", json!({"name":"Attempt"})),
        ("GET", "/v1/soundboard", json!({})),
        ("POST", "/v1/account-invites", json!({"kind":"friend"})),
        (
            "POST",
            "/v1/hangouts/join-friend",
            json!({"friend":TEST_OWNER_ID}),
        ),
        ("POST", "/v2/server-invites", json!({})),
    ] {
        assert_eq!(
            request(&app, method, path, &user, body).await.status(),
            StatusCode::FORBIDDEN,
            "{path}"
        );
    }
    assert!(
        load_conversation(
            &state.pool,
            user.parse().unwrap(),
            channel["id"].as_str().unwrap()
        )
        .await
        .is_err()
    );
    let directory = value(request(&app, "GET", "/v1/e2ee/state", &user, json!({})).await).await;
    assert!(
        directory["channel_recipients"]
            .as_object()
            .unwrap()
            .is_empty()
    );
    assert!(
        directory["pending_admissions"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let login=request(&app,"POST","/v1/accounts/login",&user,json!({"username":"new-user","password":"test-only-password-123","device_name":"Second device","protocol_version":1})).await;
    assert_eq!(login.status(), StatusCode::OK);
    assert!(
        !state
            .snapshot(user.parse().unwrap())
            .await
            .unwrap()
            .server_member
    );
}

#[tokio::test]
async fn invitation_acceptance_and_last_server_leave_preserve_account() {
    let (state, app) = setup().await;
    let user = signup(&app, "recipient").await;
    let other = signup(&app, "another").await;
    let invite = value(
        request(
            &app,
            "POST",
            "/v2/server-invites",
            TEST_OWNER_ID,
            json!({"expires_in_minutes":30}),
        )
        .await,
    )
    .await;
    let code = invite["code"].as_str().unwrap();
    for _ in 0..2 {
        assert_eq!(
            request(&app, "POST", "/v2/server/join", &user, json!({"code":code}))
                .await
                .status(),
            StatusCode::OK
        );
    }
    assert!(
        state
            .snapshot(user.parse().unwrap())
            .await
            .unwrap()
            .server_member
    );
    assert!(
        !state
            .snapshot(user.parse().unwrap())
            .await
            .unwrap()
            .friends
            .iter()
            .any(|f| f.user.id.to_string() == TEST_OWNER_ID)
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/v2/server/join",
            &other,
            json!({"code":code})
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(&app, "POST", "/v2/server/leave", &user, json!({}))
            .await
            .status(),
        StatusCode::OK
    );
    assert!(
        !state
            .snapshot(user.parse().unwrap())
            .await
            .unwrap()
            .server_member
    );
    assert_eq!(
        request(&app, "POST", "/v2/server/join", &user, json!({"code":code}))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(&app, "POST", "/v2/server/leave", TEST_OWNER_ID, json!({}))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(request(&app,"POST","/v1/accounts/login",&user,json!({"username":"recipient","password":"test-only-password-123","device_name":"Still here","protocol_version":1})).await.status(),StatusCode::OK);
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Exercise consent, messaging, and blocking as one account lifecycle.
async fn handles_friend_requests_and_dms_work_without_a_server() {
    let (state, app) = setup().await;
    let alice = signup(&app, "example-alice").await;
    let bob = signup(&app, "example-bob").await;
    let found = value(
        request(
            &app,
            "POST",
            "/v2/people/lookup",
            &alice,
            json!({"handle":"@EXAMPLE-BOB"}),
        )
        .await,
    )
    .await;
    assert_eq!(found["person"]["id"], bob);
    assert!(found["person"].get("email").is_none());
    let legacy = value(
        request(
            &app,
            "POST",
            "/v2/people/lookup",
            &alice,
            json!({"handle":"owner"}),
        )
        .await,
    )
    .await;
    assert!(legacy["person"].is_null());
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/v1/friend-requests/{bob}"),
            &alice,
            json!({})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert!(
        state
            .snapshot(alice.parse().unwrap())
            .await
            .unwrap()
            .friends
            .is_empty()
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/v1/friend-requests/{alice}/accept"),
            &bob,
            json!({})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/v1/conversations/direct",
            &alice,
            json!({"friend":bob})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert!(
        !state
            .snapshot(alice.parse().unwrap())
            .await
            .unwrap()
            .server_member
    );
    let chat = find_or_create_direct(&state.pool, alice.parse().unwrap(), bob.parse().unwrap())
        .await
        .unwrap();
    let message = json!({"conversation_id":chat,"content_type":"text/plain","payload":"A message before blocking","encryption_version":0});
    assert_eq!(
        request(&app, "POST", "/v1/messages", &alice, message.clone())
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(
            &app,
            "PUT",
            &format!("/v2/people/{alice}/block"),
            &bob,
            json!({})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(&app, "POST", "/v1/messages", &alice, message.clone())
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(&app, "POST", "/v1/messages", &bob, message)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("/v1/messages?conversation_id={chat}"),
            &alice,
            json!({})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/v1/friend-requests/{bob}"),
            &alice,
            json!({})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert!(
        value(
            request(
                &app,
                "POST",
                "/v2/people/lookup",
                &alice,
                json!({"handle":"example-bob"})
            )
            .await
        )
        .await["person"]
            .is_null()
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Preview, revocation and rejection share the same one-use invitation.
async fn encrypted_invites_are_revocable_and_preview_does_not_redeem() {
    let (state, app) = setup().await;
    let invite =
        value(request(&app, "POST", "/v2/server-invites", TEST_OWNER_ID, json!({})).await).await;
    let id = invite["id"].as_str().unwrap();
    let link = wisp_crypto::invitation::Link::create("https://wisp.invalid").unwrap();
    let capsule = wisp_crypto::invitation::Invitation {
        v: 2,
        server: "https://wisp.invalid".into(),
        id: id.parse().unwrap(),
        code: invite["code"].as_str().unwrap().into(),
        server_name: "Test community".into(),
        inviter: UserSummary {
            id: TEST_OWNER_ID.parse().unwrap(),
            display_name: "Example".into(),
        },
        expires_at: invite["expires_at"].as_str().unwrap().parse().unwrap(),
        media_key: Some("test-only-media-key".into()),
    };
    let envelope = link.seal(&capsule).unwrap();
    assert_eq!(
        request(
            &app,
            "PUT",
            &format!("/v2/server-invites/{id}/envelope"),
            TEST_OWNER_ID,
            json!({"lookup_id":link.lookup_id(),"envelope":envelope})
        )
        .await
        .status(),
        StatusCode::OK
    );
    for _ in 0..2 {
        let resolved = value(
            request(
                &app,
                "GET",
                &format!("/v2/invitations/{}", link.lookup_id()),
                TEST_OWNER_ID,
                json!({}),
            )
            .await,
        )
        .await;
        assert_eq!(
            link.open(resolved["envelope"].as_str().unwrap())
                .unwrap()
                .id,
            capsule.id
        );
    }
    let redeemed_at: Option<String> =
        sqlx::query_scalar("SELECT used_at FROM server_invites WHERE id=?")
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert!(redeemed_at.is_none());
    assert_eq!(
        request(
            &app,
            "DELETE",
            &format!("/v2/server-invites/{id}"),
            TEST_MEMBER_A_ID,
            json!({})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &app,
            "DELETE",
            &format!("/v2/server-invites/{id}"),
            TEST_OWNER_ID,
            json!({})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("/v2/invitations/{}", link.lookup_id()),
            TEST_OWNER_ID,
            json!({})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let user = signup(&app, "late-user").await;
    assert_eq!(
        request(
            &app,
            "POST",
            "/v2/server/join",
            &user,
            json!({"code":capsule.code})
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn invitation_expiry_and_concurrent_redemption_are_enforced() {
    let (state, app) = setup().await;
    let first = signup(&app, "first-recipient").await;
    let second = signup(&app, "second-recipient").await;
    let expired =
        value(request(&app, "POST", "/v2/server-invites", TEST_OWNER_ID, json!({})).await).await;
    sqlx::query("UPDATE server_invites SET expires_at='2000-01-01T00:00:00Z' WHERE id=?")
        .bind(expired["id"].as_str().unwrap())
        .execute(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        request(
            &app,
            "POST",
            "/v2/server/join",
            &first,
            json!({"code":expired["code"]})
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let active =
        value(request(&app, "POST", "/v2/server-invites", TEST_OWNER_ID, json!({})).await).await;
    let (left, right) = tokio::join!(
        request(
            &app,
            "POST",
            "/v2/server/join",
            &first,
            json!({"code":active["code"]})
        ),
        request(
            &app,
            "POST",
            "/v2/server/join",
            &second,
            json!({"code":active["code"]})
        )
    );
    let mut statuses = vec![left.status().as_u16(), right.status().as_u16()];
    statuses.sort_unstable();
    assert_eq!(statuses, vec![200, 400]);
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id IN (?,?) AND server_member=1")
            .bind(first)
            .bind(second)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    for _ in 0..2 {
        account_membership::rate(&state, "test-bound".into(), 2)
            .await
            .unwrap();
    }
    assert_eq!(
        account_membership::rate(&state, "test-bound".into(), 2)
            .await
            .unwrap_err()
            .status,
        StatusCode::TOO_MANY_REQUESTS
    );
}
