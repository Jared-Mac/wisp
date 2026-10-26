use crate::{
    tests::test_config,
    text_tests::{request, value},
    *,
};
use axum::body::to_bytes;

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn catalog_is_member_only_and_admin_managed_with_verified_packages() {
    let state = AppState::new(test_config()).await.unwrap();
    sqlx::query("INSERT INTO server_identity(id,owner_user_id) VALUES (1,?)")
        .bind(TEST_OWNER_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    let app = router(state.clone());
    let path = "/v1/server/plugins";
    let item = "/v1/server/plugins/games.valheim";
    let package = "/v1/server/plugins/games.valheim/package";
    let initial = value(request(&app, "GET", path, TEST_MEMBER_A_ID, json!({})).await).await;
    assert_eq!(initial["can_manage"], false);
    assert_eq!(initial["catalog"][0]["enabled"], false);
    let digest = initial["catalog"][0]["digest"].as_str().unwrap();
    assert_eq!(
        request(&app, "GET", package, TEST_MEMBER_A_ID, json!({}))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let enable = json!({"enabled":true,"digest":digest});
    assert_eq!(
        request(&app, "PUT", item, TEST_MEMBER_A_ID, enable.clone())
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            "PUT",
            item,
            TEST_OWNER_ID,
            json!({"enabled":true,"digest":"stale"})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &app,
            "PUT",
            "/v1/server/plugins/not.in.catalog",
            TEST_OWNER_ID,
            enable.clone()
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let installed = value(request(&app, "PUT", item, TEST_OWNER_ID, enable.clone()).await).await;
    assert_eq!(installed["catalog"][0]["enabled"], true);
    assert_eq!(installed["can_manage"], true);
    let response = request(&app, "GET", package, TEST_MEMBER_A_ID, json!({})).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["x-wisp-plugin-digest"], digest);
    let bytes = to_bytes(response.into_body(), wisp_plugins::MAX_MODULE)
        .await
        .unwrap();
    assert_eq!(
        bytes.as_ref(),
        wisp_plugins::catalog::module("games.valheim", digest).unwrap()
    );
    // Another server has its own installation state.
    let other = router(AppState::new(test_config()).await.unwrap());
    assert_eq!(
        request(&other, "GET", package, TEST_MEMBER_A_ID, json!({}))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    // An account without membership cannot discover or retrieve server plugins.
    sqlx::query("UPDATE users SET server_member=0 WHERE id=?")
        .bind(TEST_MEMBER_B_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    for endpoint in [path, package] {
        assert_eq!(
            request(&app, "GET", endpoint, TEST_MEMBER_B_ID, json!({}))
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    // An administrator, not just the owner, can remove a server plugin.
    sqlx::query("INSERT INTO server_admins(user_id,granted_by,granted_at) VALUES (?,?,?)")
        .bind(TEST_MEMBER_A_ID)
        .bind(TEST_OWNER_ID)
        .bind(Utc::now().to_rfc3339())
        .execute(&state.pool)
        .await
        .unwrap();
    value(
        request(
            &app,
            "PUT",
            item,
            TEST_MEMBER_A_ID,
            json!({"enabled":false,"digest":digest}),
        )
        .await,
    )
    .await;
    assert_eq!(
        request(&app, "GET", package, TEST_OWNER_ID, json!({}))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn server_workspace_is_shared_and_cannot_be_published_by_members() {
    let state = AppState::new(test_config()).await.unwrap();
    let ledger: i64 = sqlx::query_scalar("SELECT MAX(version) FROM schema_migrations")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    let applied: i64 =
        sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success=1")
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(
        ledger, applied,
        "Compatibility and SQLx schema ledgers must agree"
    );

    sqlx::query("INSERT INTO server_identity(id,owner_user_id) VALUES(1,?)")
        .bind(TEST_OWNER_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET username=lower(display_name) WHERE username IS NULL")
        .execute(&state.pool)
        .await
        .unwrap();
    let app = router(state.clone());
    let path = "/v1/server/plugins/games.valheim/workspace";
    assert_eq!(
        request(&app, "POST", path, TEST_MEMBER_A_ID, json!({}))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let digest = wisp_plugins::catalog::entry("games.valheim")
        .unwrap()
        .digest;
    request(
        &app,
        "PUT",
        "/v1/server/plugins/games.valheim",
        TEST_OWNER_ID,
        json!({"enabled":true,"digest":digest}),
    )
    .await;
    let owner = value(request(&app, "POST", path, TEST_OWNER_ID, json!({})).await).await;
    let member = value(request(&app, "POST", path, TEST_MEMBER_A_ID, json!({})).await).await;
    assert_eq!(owner["conversation_id"], member["conversation_id"]);
    assert_eq!(owner["can_publish"], true);
    assert_eq!(member["can_publish"], false);
    let id = owner["conversation_id"].as_str().unwrap();
    assert!(
        super::authorize_publish(&state.pool, id, TEST_OWNER_ID.parse().unwrap())
            .await
            .is_ok()
    );
    assert!(
        super::authorize_publish(&state.pool, id, TEST_MEMBER_A_ID.parse().unwrap())
            .await
            .is_err()
    );
    assert_eq!(
        request(
            &app,
            "DELETE",
            &format!("/v1/server/channels/{id}"),
            TEST_OWNER_ID,
            json!({})
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    sqlx::query("UPDATE users SET server_member=0 WHERE id=?")
        .bind(TEST_MEMBER_A_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        request(&app, "POST", path, TEST_MEMBER_A_ID, json!({}))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    sqlx::query("INSERT OR IGNORE INTO conversation_members(conversation_id,user_id,joined_at,role) VALUES(?,?,?,'admin')")
        .bind(id).bind(TEST_OWNER_ID).bind(Utc::now().to_rfc3339()).execute(&state.pool).await.unwrap();
    let legacy = value(request(&app, "GET", "/v1/snapshot", TEST_OWNER_ID, json!({})).await).await;
    assert!(
        !legacy["conversations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == id)
    );
    let modern = value(
        request(
            &app,
            "GET",
            "/v1/snapshot?plugin_workspaces=1",
            TEST_OWNER_ID,
            json!({}),
        )
        .await,
    )
    .await;
    assert!(
        modern["conversations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == id)
    );
    // Removing/re-enabling the plugin does not destroy its pack history.
    request(
        &app,
        "PUT",
        "/v1/server/plugins/games.valheim",
        TEST_OWNER_ID,
        json!({"enabled":false,"digest":digest}),
    )
    .await;
    assert!(
        super::authorize_publish(&state.pool, id, TEST_OWNER_ID.parse().unwrap())
            .await
            .is_err()
    );
    assert_eq!(
        request(&app, "POST", path, TEST_OWNER_ID, json!({}))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    request(
        &app,
        "PUT",
        "/v1/server/plugins/games.valheim",
        TEST_OWNER_ID,
        json!({"enabled":true,"digest":digest}),
    )
    .await;
    let restored = value(request(&app, "POST", path, TEST_OWNER_ID, json!({})).await).await;
    assert_eq!(restored["conversation_id"], owner["conversation_id"]);
}
