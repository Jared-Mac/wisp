use super::*;
use axum::body::{Body, to_bytes};
use tests::{chat_headers, test_config};
use tower::ServiceExt;

async fn setup() -> (AppState, Router, String) {
    let state = AppState::new(test_config()).await.unwrap();
    let conversation = find_or_create_direct(
        &state.pool,
        Uuid::parse_str(JARED_ID).unwrap(),
        Uuid::parse_str(TYLER_ID).unwrap(),
    )
    .await
    .unwrap();
    (state.clone(), router(state), conversation)
}

pub(super) async fn request(
    app: &Router,
    method: &str,
    path: &str,
    user: &str,
    body: Value,
) -> Response {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    *request.headers_mut() = chat_headers(user);
    request
        .headers_mut()
        .insert("content-type", "application/json".parse().unwrap());
    app.clone().oneshot(request).await.unwrap()
}

pub(super) async fn value(response: Response) -> Value {
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn large_text() -> String {
    // 1,200,000 Unicode characters / 2.8 MB: exceeds both the old character cap
    // and Axum's default 2 MiB JSON body limit. Newlines survive too.
    "é🙂\n".repeat(400_000)
}

#[tokio::test]
async fn long_text_round_trips_and_edits_through_http_without_truncation() {
    let (state, app, conversation) = setup().await;
    let text = large_text();
    let sent = value(request(&app, "POST", "/v1/messages", JARED_ID, json!({
        "conversation_id":conversation,"content_type":"text/plain","payload":text,"encryption_version":0
    })).await).await;
    assert_eq!(sent["payload"], text);
    let path = format!("/v1/messages/{}", sent["id"].as_str().unwrap());
    let edited = format!("{text}edited");
    let response = request(&app, "PATCH", &path, JARED_ID, json!({"text":edited})).await;
    assert_eq!(response.status(), StatusCode::OK);
    let messages = load_recent_messages(&state.pool, Uuid::parse_str(TYLER_ID).unwrap())
        .await
        .unwrap();
    assert_eq!(messages[0].payload, edited);
    assert!(messages[0].edited_at.is_some());
    assert_eq!(
        request(&app, "PATCH", &path, TYLER_ID, json!({"text":edited}))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(&app, "PATCH", &path, JARED_ID, json!({"text":" \n "}))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(request(&app, "POST", "/v1/messages", CHARLIE_ID, json!({
        "conversation_id":conversation,"content_type":"text/plain","payload":text,"encryption_version":0
    })).await.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn long_captions_work_for_images_legacy_files_and_chunked_uploads() {
    let (_, app, conversation) = setup().await;
    let caption = large_text();
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(1, 1)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    for (path, mut body) in [
        (
            "/v1/messages/image",
            json!({"png_base64":base64::engine::general_purpose::STANDARD.encode(png.into_inner())}),
        ),
        (
            "/v1/messages/file",
            json!({"file_name":"test.txt","data_base64":"aGk="}),
        ),
    ] {
        body["conversation_id"] = json!(conversation);
        body["caption"] = json!(caption);
        let sent = value(request(&app, "POST", path, JARED_ID, body).await).await;
        assert_eq!(sent["payload"]["caption"], caption);
        let path = format!("/v1/messages/{}", sent["id"].as_str().unwrap());
        assert_eq!(
            request(
                &app,
                "PATCH",
                &path,
                JARED_ID,
                json!({"text":format!("{caption}edited")})
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            request(&app, "PATCH", &path, JARED_ID, json!({"text":""}))
                .await
                .status(),
            StatusCode::OK
        );
    }
    let id = Uuid::new_v4();
    let body = json!({"id":id,"conversation_id":conversation,"file_name":"empty.txt","size":0,"caption":caption});
    assert_eq!(
        request(&app, "POST", "/v1/file-uploads", JARED_ID, body.clone())
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(&app, "POST", "/v1/file-uploads", JARED_ID, body)
            .await
            .status(),
        StatusCode::OK
    );
    let sent = value(
        request(
            &app,
            "POST",
            &format!("/v1/file-uploads/{id}/complete"),
            JARED_ID,
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(sent["payload"]["caption"], caption);
}

#[tokio::test]
async fn uncapped_text_routes_authenticate_before_reading_the_body() {
    let (_, app, _) = setup().await;
    for (method, path) in [
        ("POST", "/v1/messages"),
        ("POST", "/v1/messages/image"),
        ("POST", "/v1/messages/file"),
        ("POST", "/v1/file-uploads"),
        ("POST", "/v1/e2ee/messages"),
        ("POST", "/v1/e2ee/file-uploads"),
        (
            "PUT",
            "/v1/e2ee/messages/00000000-0000-0000-0000-000000000001",
        ),
        ("PATCH", "/v1/messages/00000000-0000-0000-0000-000000000001"),
    ] {
        let body = Body::from_stream(futures_util::stream::poll_fn(
            |_| -> std::task::Poll<Option<Result<axum::body::Bytes, std::io::Error>>> {
                panic!("unauthenticated request body was read")
            },
        ));
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(body)
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let response = request(
        &app,
        "POST",
        "/v1/presence",
        JARED_ID,
        json!({"presence":large_text()}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let response = request(
        &app,
        "PUT",
        "/v1/file-uploads/00000000-0000-0000-0000-000000000001/chunks/0",
        JARED_ID,
        json!("x".repeat(wisp_protocol::CHAT_FILE_CHUNK_BYTES + 1)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[test]
fn text_validation_has_no_character_cap_but_still_requires_nonempty_text() {
    for text in ["x".repeat(4_000), "x".repeat(4_001), "x".repeat(100_000)] {
        assert!(
            validate_message(&SendMessageRequest {
                conversation_id: "test".into(),
                content_type: "text/plain".into(),
                payload: json!(text),
                encryption_version: 0
            })
            .is_ok()
        );
    }
    assert!(
        validate_message(&SendMessageRequest {
            conversation_id: "test".into(),
            content_type: "text/plain".into(),
            payload: json!(" \n\t"),
            encryption_version: 0
        })
        .is_err()
    );
}
