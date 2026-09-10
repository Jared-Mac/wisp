use super::*;
use crate::{
    tests::test_config,
    text_tests::{request, value},
};
use axum::body::to_bytes;
use base64::{Engine as _, engine::general_purpose::STANDARD};

#[tokio::test]
async fn server_soundboard_is_shared_scoped_and_moderated() {
    let state = AppState::new(test_config()).await.unwrap();
    sqlx::query("INSERT INTO server_identity(id,owner_user_id) VALUES (1,?)")
        .bind(TEST_OWNER_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    let app = router(state.clone());
    let wav = wisp_protocol::soundboard::encode(&vec![1234; 4800]).unwrap();
    let clip = value(
        request(
            &app,
            "POST",
            "/v1/soundboard",
            TEST_MEMBER_A_ID,
            json!({"name":"Applause","data":STANDARD.encode(&wav)}),
        )
        .await,
    )
    .await;
    assert_eq!(clip["duration_ms"], 100);
    let catalog =
        value(request(&app, "GET", "/v1/soundboard", TEST_MEMBER_B_ID, json!({})).await).await;
    assert_eq!(catalog["sounds"][0]["id"], clip["id"]);
    let path = format!("/v1/soundboard/{}", clip["id"].as_str().unwrap());
    let audio = request(&app, "GET", &path, TEST_MEMBER_B_ID, json!({})).await;
    assert_eq!(audio.status(), StatusCode::OK);
    assert_eq!(audio.headers()["content-type"], "audio/wav");
    assert_eq!(
        to_bytes(audio.into_body(), 1_000_000)
            .await
            .unwrap()
            .as_ref(),
        wav
    );
    let other = router(AppState::new(test_config()).await.unwrap());
    assert_eq!(
        request(&other, "GET", &path, TEST_MEMBER_A_ID, json!({}))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/v1/soundboard",
            TEST_MEMBER_B_ID,
            json!({"name":"applause","data":STANDARD.encode(&wav)})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(&app, "DELETE", &path, TEST_MEMBER_B_ID, json!({}))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(&app, "DELETE", &path, TEST_OWNER_ID, json!({}))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(&app, "GET", &path, TEST_MEMBER_A_ID, json!({}))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let own = value(
        request(
            &app,
            "POST",
            "/v1/soundboard",
            TEST_MEMBER_A_ID,
            json!({"name":"Mine","data":STANDARD.encode(&wav)}),
        )
        .await,
    )
    .await;
    assert_eq!(
        request(
            &app,
            "DELETE",
            &format!("/v1/soundboard/{}", own["id"].as_str().unwrap()),
            TEST_MEMBER_A_ID,
            json!({})
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn invalid_audio_and_full_boards_are_rejected() {
    let state = AppState::new(test_config()).await.unwrap();
    let app = router(state.clone());
    for (name, data) in [
        ("bad", STANDARD.encode(b"not audio")),
        (
            " ",
            STANDARD.encode(wisp_protocol::soundboard::encode(&[1; 480]).unwrap()),
        ),
    ] {
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/soundboard",
                TEST_MEMBER_A_ID,
                json!({"name":name,"data":data})
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
    }
    let wav = wisp_protocol::soundboard::encode(&[1; 480]).unwrap();
    for i in 0..64 {
        sqlx::query("INSERT INTO soundboard_sounds(id,owner_id,name,wav,duration_ms,created_at) VALUES (?,?,?,?,10,'now')")
            .bind(Uuid::new_v4().to_string()).bind(TEST_MEMBER_A_ID).bind(format!("sound {i}")).bind(&wav).execute(&state.pool).await.unwrap();
    }
    assert_eq!(
        request(
            &app,
            "POST",
            "/v1/soundboard",
            TEST_MEMBER_A_ID,
            json!({"name":"One too many","data":STANDARD.encode(&wav)})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn soundboard_requires_authentication_and_bounds_http_uploads() {
    use axum::body::Body;
    use tower::ServiceExt;
    let app = router(AppState::new(test_config()).await.unwrap());
    for (method, path) in [
        ("GET", "/v1/soundboard"),
        ("GET", "/v1/soundboard/00000000-0000-0000-0000-000000000001"),
        (
            "DELETE",
            "/v1/soundboard/00000000-0000-0000-0000-000000000001",
        ),
        ("POST", "/v1/soundboard"),
    ] {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from(r#"{"name":"No access","data":""}"#))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(request).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        request(
            &app,
            "POST",
            "/v1/soundboard",
            TEST_MEMBER_A_ID,
            json!({"name":"Too big","data":"x".repeat(1_300_001)})
        )
        .await
        .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
}
