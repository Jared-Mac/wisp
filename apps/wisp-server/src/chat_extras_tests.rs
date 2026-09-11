use super::*;
use crate::{
    tests::test_config,
    text_tests::{request, value},
};

#[tokio::test]
async fn invitation_messages_reject_reactions_without_disclosing_private_messages() {
    let state = AppState::new(test_config()).await.unwrap();
    let conversation = find_or_create_direct(
        &state.pool,
        TEST_OWNER_ID.parse().unwrap(),
        TEST_MEMBER_A_ID.parse().unwrap(),
    )
    .await
    .unwrap();
    let target = Uuid::new_v4();
    sqlx::query("INSERT INTO messages(id,conversation_id,sender_id,created_at,content_type,payload) VALUES (?,?,?,?,'application/vnd.wisp.room-invitation+json','{}')")
        .bind(target.to_string()).bind(&conversation).bind(TEST_OWNER_ID)
        .bind(Utc::now().to_rfc3339()).execute(&state.pool).await.unwrap();
    let app = router(state.clone());
    let path = format!("/v1/messages/{target}/reactions");
    for (user, expected) in [
        (TEST_OWNER_ID, StatusCode::BAD_REQUEST),
        (TEST_MEMBER_A_ID, StatusCode::BAD_REQUEST),
        (TEST_MEMBER_B_ID, StatusCode::NOT_FOUND),
    ] {
        let response = request(
            &app,
            "PUT",
            &path,
            user,
            json!({"id":Uuid::new_v4(),"emoji":"🔥"}),
        )
        .await;
        assert_eq!(response.status(), expected);
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message_reactions")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // One library lifecycle includes authorization, paging and historical images.
async fn emoji_libraries_enforce_ownership_paginate_and_preserve_history_images() {
    let state = AppState::new(test_config()).await.unwrap();
    sqlx::query("INSERT INTO server_identity(id,owner_user_id) VALUES (1,?)")
        .bind(TEST_OWNER_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    let app = router(state.clone());
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbaImage::new(4, 4)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let data = base64::engine::general_purpose::STANDARD.encode(png.into_inner());
    let personal = value(
        request(
            &app,
            "POST",
            "/v1/emojis",
            TEST_MEMBER_A_ID,
            json!({"name":"my_wisp","scope":"account","data":data}),
        )
        .await,
    )
    .await;
    assert_eq!(
        request(
            &app,
            "POST",
            "/v1/emojis",
            TEST_MEMBER_A_ID,
            json!({"name":"server_wisp","scope":"server","data":data})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let shared = value(
        request(
            &app,
            "POST",
            "/v1/emojis",
            TEST_OWNER_ID,
            json!({"name":"server_wisp","scope":"server","data":data}),
        )
        .await,
    )
    .await;
    let list = value(request(&app, "GET", "/v1/emojis", TEST_MEMBER_B_ID, json!({})).await).await;
    assert_eq!(list["emojis"].as_array().unwrap().len(), 1);
    assert_eq!(list["emojis"][0]["id"], shared["id"]);
    let path = format!("/v1/emojis/{}", personal["id"].as_str().unwrap());
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
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(&app, "DELETE", &path, TEST_MEMBER_A_ID, json!({}))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(&app, "GET", &path, TEST_MEMBER_B_ID, json!({}))
            .await
            .status(),
        StatusCode::OK
    );
    for n in 0..105 {
        sqlx::query("INSERT INTO custom_emojis(id,owner_id,scope,name,png,created_at) VALUES (?,?,'account',?,X'','now')").bind(Uuid::new_v4().to_string()).bind(TEST_MEMBER_A_ID).bind(format!("emoji_{n}")).execute(&state.pool).await.unwrap();
    }
    let first = value(request(&app, "GET", "/v1/emojis", TEST_MEMBER_A_ID, json!({})).await).await;
    assert_eq!(first["emojis"].as_array().unwrap().len(), 100);
    let after = first["next"].as_str().unwrap();
    let second = value(
        request(
            &app,
            "GET",
            &format!("/v1/emojis?after={after}"),
            TEST_MEMBER_A_ID,
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(second["emojis"].as_array().unwrap().len(), 6);
    assert!(second["next"].is_null());
    assert_eq!(
        request(
            &app,
            "POST",
            "/v1/emojis",
            TEST_OWNER_ID,
            json!({"name":"bad","scope":"server","data":"PHN2Zz4="})
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn reactions_are_visible_only_with_the_message_and_only_the_sender_can_remove_them() {
    let state = AppState::new(test_config()).await.unwrap();
    let app = router(state.clone());
    let conversation = find_or_create_direct(
        &state.pool,
        TEST_OWNER_ID.parse().unwrap(),
        TEST_MEMBER_A_ID.parse().unwrap(),
    )
    .await
    .unwrap();
    let sent = value(
        request(
            &app,
            "POST",
            "/v1/messages",
            TEST_OWNER_ID,
            json!({"conversation_id":conversation,"payload":"A message"}),
        )
        .await,
    )
    .await;
    let id = sent["id"].as_str().unwrap();
    let path = format!("/v1/messages/{id}/reactions");
    let reaction = Uuid::new_v4();
    let body = json!({"id":reaction,"emoji":"🔥"});
    assert_eq!(
        request(&app, "PUT", &path, TEST_MEMBER_B_ID, body.clone())
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    for _ in 0..2 {
        assert_eq!(
            request(&app, "PUT", &path, TEST_MEMBER_A_ID, body.clone())
                .await
                .status(),
            StatusCode::OK
        );
    }
    let messages = load_recent_messages(&state.pool, TEST_OWNER_ID.parse().unwrap())
        .await
        .unwrap();
    assert_eq!(
        chat_extras::load_reactions(&state.pool, &messages)
            .await
            .unwrap()
            .len(),
        1
    );
    let remove = format!("/v1/reactions/{reaction}");
    assert_eq!(
        request(&app, "DELETE", &remove, TEST_OWNER_ID, json!({}))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        chat_extras::load_reactions(&state.pool, &messages)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        request(&app, "DELETE", &remove, TEST_MEMBER_A_ID, json!({}))
            .await
            .status(),
        StatusCode::OK
    );
    assert!(
        chat_extras::load_reactions(&state.pool, &messages)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        request(&app, "PUT", &path, TEST_MEMBER_A_ID, body)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(
            &app,
            "DELETE",
            &format!("/v1/messages/{id}"),
            TEST_OWNER_ID,
            json!({})
        )
        .await
        .status(),
        StatusCode::OK
    );
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM message_reactions")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
}
