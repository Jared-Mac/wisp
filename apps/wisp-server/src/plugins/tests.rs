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
