use super::*;
use crate::{
    tests::{chat_headers, test_config},
    text_tests::{request, value},
    *,
};

async fn publish(state: &AppState, audience: &[&str], order: u32) -> String {
    let id = Uuid::new_v4().to_string();
    let time = format!("2026-10-26T12:00:{order:02}Z");
    sqlx::query("INSERT INTO messages(id,conversation_id,sender_id,created_at,content_type,payload,encryption_version) VALUES(?,'plugin:games.valheim',?,?,'application/vnd.wisp.encrypted+json',?,1)")
        .bind(&id).bind(TEST_OWNER_ID).bind(&time).bind(json!({"recipient_ids":audience}).to_string()).execute(&state.pool).await.unwrap();
    sqlx::query("INSERT INTO file_uploads(id,owner_id,conversation_id,file_name,caption,size,last_activity_at,message_id) VALUES(?,?,'plugin:games.valheim','encrypted','','1',?,?)")
        .bind(Uuid::new_v4().to_string()).bind(TEST_OWNER_ID).bind(time).bind(&id).execute(&state.pool).await.unwrap();
    id
}
fn report_body(revision: &str, state: &str) -> Value {
    json!({"revision":revision,"status":state,"platform":"linux","checked_at":Utc::now().timestamp()})
}
fn member(roster: &Value, user: &str) -> Value {
    roster["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["user_id"] == user)
        .unwrap()
        .clone()
}
async fn device(state: &AppState) -> (Uuid, HeaderMap) {
    let mut tx = state.pool.begin().await.unwrap();
    let device = create_device(
        &mut tx,
        TEST_OWNER_ID.parse().unwrap(),
        "Private computer hostname",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let session = device_session(
        State(state.clone()),
        Json(DeviceSessionRequest {
            device_id: device.device_id,
            device_token: device.device_token,
            protocol_version: PROTOCOL_VERSION,
        }),
    )
    .await
    .unwrap()
    .0;
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        format!("Bearer {}", session.token).parse().unwrap(),
    );
    (device.device_id, headers)
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Auth, current-revision reporting and lifecycle together.
async fn member_reports_are_device_bound_current_private_and_removed_with_membership() {
    let state = AppState::new(test_config()).await.unwrap();
    sqlx::query("INSERT INTO server_identity(id,owner_user_id) VALUES(1,?)")
        .bind(TEST_OWNER_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    let app = router(state.clone());
    let path = "/v1/server/plugins/games.valheim/sync";
    assert_eq!(
        request(&app, "GET", path, TEST_OWNER_ID, json!({}))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let digest = wisp_plugins::catalog::entry("games.valheim")
        .unwrap()
        .digest;
    value(
        request(
            &app,
            "PUT",
            "/v1/server/plugins/games.valheim",
            TEST_OWNER_ID,
            json!({"enabled":true,"digest":digest}),
        )
        .await,
    )
    .await;
    value(
        request(
            &app,
            "POST",
            "/v1/server/plugins/games.valheim/workspace",
            TEST_OWNER_ID,
            json!({}),
        )
        .await,
    )
    .await;
    let empty = value(request(&app, "GET", path, TEST_MEMBER_A_ID, json!({})).await).await;
    assert_eq!(empty["has_pack"], false);
    let rev = publish(&state, &[TEST_OWNER_ID, TEST_MEMBER_A_ID], 1).await;
    let unknown = value(request(&app, "GET", path, TEST_MEMBER_A_ID, json!({})).await).await;
    assert_eq!(member(&unknown, TEST_MEMBER_A_ID)["status"], "unknown");
    assert_eq!(
        member(&unknown, TEST_MEMBER_B_ID)["status"],
        "awaiting_access"
    );
    let synced = value(
        request(
            &app,
            "PUT",
            path,
            TEST_MEMBER_A_ID,
            report_body(&rev, "up_to_date"),
        )
        .await,
    )
    .await;
    assert_eq!(member(&synced, TEST_MEMBER_A_ID)["status"], "up_to_date");
    assert_eq!(member(&synced, TEST_OWNER_ID)["status"], "unknown");
    let mut forged = report_body(&rev, "up_to_date");
    forged["user_id"] = json!(TEST_OWNER_ID);
    assert_eq!(
        request(&app, "PUT", path, TEST_MEMBER_A_ID, forged)
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        request(
            &app,
            "PUT",
            path,
            TEST_MEMBER_B_ID,
            report_body(&rev, "up_to_date")
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let mut future = report_body(&rev, "up_to_date");
    future["checked_at"] = json!(Utc::now().timestamp() + 1000);
    assert_eq!(
        request(&app, "PUT", path, TEST_MEMBER_A_ID, future)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    // Real credentials keep two installations separate without disclosing device names/IDs.
    let (first, first_headers) = device(&state).await;
    let (_, second_headers) = device(&state).await;
    for (headers, status) in [
        (first_headers.clone(), "update_available"),
        (second_headers, "up_to_date"),
    ] {
        let _ = super::report(
            State(state.clone()),
            headers,
            Path("games.valheim".into()),
            Json(serde_json::from_value(report_body(&rev, status)).unwrap()),
        )
        .await
        .unwrap();
    }
    let mixed = super::list(
        State(state.clone()),
        chat_headers(TEST_MEMBER_A_ID),
        Path("games.valheim".into()),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(member(&mixed, TEST_OWNER_ID)["status"], "mixed");
    let encoded = mixed.to_string();
    assert!(!encoded.contains("Private computer") && !encoded.contains(&first.to_string()));
    sqlx::query("UPDATE devices SET revoked_at=? WHERE id=?")
        .bind(Utc::now().to_rfc3339())
        .bind(first.to_string())
        .execute(&state.pool)
        .await
        .unwrap();
    assert!(
        super::report(
            State(state.clone()),
            first_headers,
            Path("games.valheim".into()),
            Json(serde_json::from_value(report_body(&rev, "up_to_date")).unwrap())
        )
        .await
        .is_err()
    );
    let roster = value(request(&app, "GET", path, TEST_MEMBER_A_ID, json!({})).await).await;
    assert_eq!(member(&roster, TEST_OWNER_ID)["status"], "up_to_date");
    // Replaying the same cached check never makes it fresh again.
    let old = Utc::now().timestamp() - STALE_SECONDS - 10;
    sqlx::query("UPDATE plugin_sync_reports SET checked_at=? WHERE user_id=?")
        .bind(old)
        .bind(TEST_MEMBER_A_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    let mut cached = report_body(&rev, "up_to_date");
    cached["checked_at"] = json!(old);
    let expired = value(request(&app, "PUT", path, TEST_MEMBER_A_ID, cached).await).await;
    assert_eq!(member(&expired, TEST_MEMBER_A_ID)["status"], "stale");
    let new = publish(&state, &[TEST_OWNER_ID, TEST_MEMBER_A_ID], 2).await;
    let changed = value(request(&app, "GET", path, TEST_OWNER_ID, json!({})).await).await;
    assert_eq!(member(&changed, TEST_OWNER_ID)["status"], "needs_check");
    assert_eq!(
        request(
            &app,
            "PUT",
            path,
            TEST_MEMBER_A_ID,
            report_body(&rev, "up_to_date")
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let updated = value(
        request(
            &app,
            "PUT",
            path,
            TEST_MEMBER_A_ID,
            report_body(&new, "up_to_date"),
        )
        .await,
    )
    .await;
    assert_eq!(member(&updated, TEST_MEMBER_A_ID)["status"], "up_to_date");
    sqlx::query("UPDATE users SET display_name='Renamed member' WHERE id=?")
        .bind(TEST_MEMBER_A_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    let renamed = value(request(&app, "GET", path, TEST_OWNER_ID, json!({})).await).await;
    assert_eq!(
        member(&renamed, TEST_MEMBER_A_ID)["display_name"],
        "Renamed member"
    );
    sqlx::query("UPDATE users SET server_member=0 WHERE id=?")
        .bind(TEST_MEMBER_A_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        request(&app, "GET", path, TEST_MEMBER_A_ID, json!({}))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            "PUT",
            path,
            TEST_MEMBER_A_ID,
            report_body(&new, "up_to_date")
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plugin_sync_reports WHERE user_id=?")
        .bind(TEST_MEMBER_A_ID)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let left = value(request(&app, "GET", path, TEST_OWNER_ID, json!({})).await).await;
    assert!(
        !left["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["user_id"] == TEST_MEMBER_A_ID)
    );
    value(
        request(
            &app,
            "PUT",
            "/v1/server/plugins/games.valheim",
            TEST_OWNER_ID,
            json!({"enabled":false,"digest":digest}),
        )
        .await,
    )
    .await;
    assert_eq!(
        request(&app, "GET", path, TEST_OWNER_ID, json!({}))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plugin_sync_reports")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
#[test]
fn changed_revisions_and_old_checks_never_claim_current_sync() {
    assert_eq!(status("old", "new", 100, 100, "up_to_date"), "needs_check");
    assert_eq!(
        status("same", "same", 100, 100 + STALE_SECONDS + 1, "up_to_date"),
        "stale"
    );
    assert_eq!(status("same", "same", 100, 110, "modified"), "modified");
    assert_eq!(status("same", "same", 100, 110, "up_to_date"), "up_to_date");
}
