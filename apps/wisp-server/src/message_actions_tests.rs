use super::*;
use crate::{
    tests::test_config,
    text_tests::{request, value},
};

async fn send(app: &Router, conversation: &str, user: &str, context: Value) -> Value {
    value(request(app,"POST","/v1/messages",user,json!({"conversation_id":conversation,"content_type":"text/plain","payload":"A message","encryption_version":0,"context":context})).await).await
}

#[tokio::test]
async fn reply_context_survives_history_and_edit_but_cannot_reference_another_chat() {
    let state = AppState::new(test_config()).await.unwrap();
    let chat = find_or_create_direct(
        &state.pool,
        TEST_OWNER_ID.parse().unwrap(),
        TEST_MEMBER_A_ID.parse().unwrap(),
    )
    .await
    .unwrap();
    let other = find_or_create_direct(
        &state.pool,
        TEST_OWNER_ID.parse().unwrap(),
        TEST_MEMBER_B_ID.parse().unwrap(),
    )
    .await
    .unwrap();
    let app = router(state.clone());
    let original = send(&app, &chat, TEST_OWNER_ID, Value::Null).await;
    let context = json!({"reply_to":{"message_id":original["id"],"sender_name":"Owner","preview":"A message"}});
    let reply = send(&app, &chat, TEST_MEMBER_A_ID, context.clone()).await;
    assert_eq!(reply["context"], context);
    let path = format!("/v1/messages/{}", reply["id"].as_str().unwrap());
    value(
        request(
            &app,
            "PATCH",
            &path,
            TEST_MEMBER_A_ID,
            json!({"text":"Edited reply"}),
        )
        .await,
    )
    .await;
    let read = value(request(&app, "GET", &path, TEST_OWNER_ID, json!({})).await).await;
    assert_eq!(read["payload"], "Edited reply");
    assert_eq!(read["context"], context);
    assert!(
        load_recent_messages(&state.pool, TEST_OWNER_ID.parse().unwrap())
            .await
            .unwrap()
            .iter()
            .any(|m| serde_json::to_value(&m.context).unwrap() == context)
    );
    assert_eq!(request(&app,"POST","/v1/messages",TEST_OWNER_ID,json!({"conversation_id":other,"content_type":"text/plain","payload":"No leak","encryption_version":0,"context":context})).await.status(),StatusCode::BAD_REQUEST);
    clear_history_for(&state.pool, TEST_MEMBER_A_ID.parse().unwrap(), &chat)
        .await
        .unwrap();
    assert_eq!(
        request(&app, "GET", &path, TEST_MEMBER_A_ID, json!({}))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(request(&app,"POST","/v1/messages",TEST_MEMBER_A_ID,json!({"conversation_id":chat,"content_type":"text/plain","payload":"No leak","encryption_version":0,"context":context})).await.status(),StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn dm_pins_are_shared_durable_visibility_checked_and_removed_with_the_message() {
    let state = AppState::new(test_config()).await.unwrap();
    let chat = find_or_create_direct(
        &state.pool,
        TEST_OWNER_ID.parse().unwrap(),
        TEST_MEMBER_A_ID.parse().unwrap(),
    )
    .await
    .unwrap();
    let app = router(state.clone());
    let message = send(&app, &chat, TEST_OWNER_ID, Value::Null).await;
    let path = format!("/v1/messages/{}", message["id"].as_str().unwrap());
    let pin = format!("{path}/pin");
    let list = format!("/v1/pins?conversation_id={chat}");
    for user in [TEST_OWNER_ID, TEST_MEMBER_A_ID] {
        value(request(&app, "PUT", &pin, user, json!({"pinned":true})).await).await;
        let pins = value(request(&app, "GET", &list, user, json!({})).await).await;
        assert_eq!(pins["can_manage"], true);
        assert_eq!(pins["messages"].as_array().unwrap().len(), 1);
    }
    assert_eq!(
        request(&app, "GET", &path, TEST_MEMBER_C_ID, json!({}))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(&app, "PUT", &pin, TEST_MEMBER_C_ID, json!({"pinned":false}))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(&app, "GET", &list, TEST_MEMBER_C_ID, json!({}))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    // The pin remains even when its message is older than the snapshot's recent window.
    sqlx::query("UPDATE messages SET created_at='2020-01-01T00:00:00+00:00' WHERE id=?")
        .bind(message["id"].as_str().unwrap())
        .execute(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        value(request(&app, "GET", &list, TEST_MEMBER_A_ID, json!({})).await).await["messages"][0]
            ["id"],
        message["id"]
    );
    clear_history_for(&state.pool, TEST_MEMBER_A_ID.parse().unwrap(), &chat)
        .await
        .unwrap();
    assert_eq!(
        value(request(&app, "GET", &list, TEST_MEMBER_A_ID, json!({})).await).await["messages"],
        json!([])
    );
    assert_eq!(
        value(request(&app, "GET", &list, TEST_OWNER_ID, json!({})).await).await["messages"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    value(request(&app, "DELETE", &path, TEST_OWNER_ID, json!({})).await).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM message_pins")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Exercise both room and channel roles through their HTTP endpoints.
async fn only_server_admins_manage_room_and_channel_pins() {
    let state = AppState::new(test_config()).await.unwrap();
    sqlx::query("INSERT INTO server_identity(id,owner_user_id) VALUES(1,?)")
        .bind(TEST_OWNER_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    for id in [TEST_OWNER_ID, TEST_MEMBER_A_ID, TEST_MEMBER_B_ID] {
        sqlx::query("UPDATE users SET username=?,password_hash='test' WHERE id=?")
            .bind(id)
            .bind(id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO chat_identities(user_id,public_identity) VALUES (?,?)")
            .bind(id)
            .bind(format!("test-identity-{id}"))
            .execute(&state.pool)
            .await
            .unwrap();
    }
    let app = router(state.clone());
    let channel = value(
        request(
            &app,
            "POST",
            "/v1/server/channels",
            TEST_OWNER_ID,
            json!({"name":"Notes","member_ids":[TEST_MEMBER_A_ID,TEST_MEMBER_B_ID]}),
        )
        .await,
    )
    .await;
    let room = format!("spot:{TEST_ROOM_ID}");
    // Load the normal seeded room memberships without joining voice.
    state
        .snapshot(TEST_OWNER_ID.parse().unwrap())
        .await
        .unwrap();
    for chat in [room, channel["id"].as_str().unwrap().to_owned()] {
        let message = send(&app, &chat, TEST_OWNER_ID, Value::Null).await;
        let path = format!("/v1/messages/{}/pin", message["id"].as_str().unwrap());
        assert_eq!(
            request(&app, "PUT", &path, TEST_MEMBER_B_ID, json!({"pinned":true}))
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
        value(request(&app, "PUT", &path, TEST_OWNER_ID, json!({"pinned":true})).await).await;
        let pins = value(
            request(
                &app,
                "GET",
                &format!("/v1/pins?conversation_id={chat}"),
                TEST_MEMBER_B_ID,
                json!({}),
            )
            .await,
        )
        .await;
        assert_eq!(pins["can_manage"], false);
        assert_eq!(pins["messages"].as_array().unwrap().len(), 1);
        assert_eq!(
            request(
                &app,
                "PUT",
                &path,
                TEST_MEMBER_B_ID,
                json!({"pinned":false})
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        value(
            request(
                &app,
                "POST",
                "/v1/server/admins",
                TEST_OWNER_ID,
                json!({"user_id":TEST_MEMBER_A_ID,"admin":true}),
            )
            .await,
        )
        .await;
        value(
            request(
                &app,
                "PUT",
                &path,
                TEST_MEMBER_A_ID,
                json!({"pinned":false}),
            )
            .await,
        )
        .await;
        assert_eq!(
            value(
                request(
                    &app,
                    "GET",
                    &format!("/v1/pins?conversation_id={chat}"),
                    TEST_MEMBER_B_ID,
                    json!({})
                )
                .await
            )
            .await["messages"],
            json!([])
        );
    }
}
