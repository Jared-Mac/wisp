//! Isolated clients generate keys; the server sees only public routing and age
//! ciphertext. No real accounts, sockets, media or hosted service are used.
use super::*;
use crate::text_tests::{request, value};
use axum::body::{Body, to_bytes};
use tests::{chat_headers, test_config};
use tower::ServiceExt;
use wisp_crypto::Identity;

async fn setup() -> (AppState, Router, String, Identity, Identity, String) {
    let mut config = test_config();
    config.require_chat_e2ee = true;
    let state = AppState::new(config).await.unwrap();
    let conversation = find_or_create_direct(
        &state.pool,
        TEST_OWNER_ID.parse().unwrap(),
        TEST_MEMBER_A_ID.parse().unwrap(),
    )
    .await
    .unwrap();
    let alice = Identity::generate().unwrap();
    let bob = Identity::generate().unwrap();
    let roster = wisp_crypto::roster::Roster {
        network: chat_identity::network(&state).await.unwrap(),
        conversation: conversation.clone(),
        revision: 0,
        previous: None,
        actor: TEST_OWNER_ID.parse().unwrap(),
        members: std::collections::BTreeMap::from([
            (
                TEST_OWNER_ID.parse().unwrap(),
                wisp_crypto::roster::Member {
                    identity: alice.public(),
                    role: wisp_crypto::roster::Role::Member,
                },
            ),
            (
                TEST_MEMBER_A_ID.parse().unwrap(),
                wisp_crypto::roster::Member {
                    identity: bob.public(),
                    role: wisp_crypto::roster::Role::Member,
                },
            ),
        ]),
    }
    .sign(&alice)
    .unwrap();
    sqlx::query("INSERT INTO chat_rosters(conversation_id,revision,signed_roster) VALUES (?,0,?)")
        .bind(&conversation)
        .bind(serde_json::to_string(&roster).unwrap())
        .execute(&state.pool)
        .await
        .unwrap();
    (
        state.clone(),
        router(state),
        conversation,
        alice,
        bob,
        roster.hash().unwrap(),
    )
}

fn sealed(
    sender: &Identity,
    recipient: &Identity,
    conversation: &str,
    text: &str,
    roster_hash: &str,
) -> wisp_protocol::EncryptedMessageRequest {
    let id = Uuid::new_v4();
    let context = format!("test-network/{conversation}/{TEST_OWNER_ID}/{id}/message");
    wisp_protocol::EncryptedMessageRequest {
        roster_hash: roster_hash.into(),
        id,
        conversation_id: conversation.into(),
        ciphertext: base64::engine::general_purpose::STANDARD.encode(
            sender
                .seal(
                    &context,
                    text.as_bytes(),
                    &[sender.public(), recipient.public()],
                )
                .unwrap(),
        ),
    }
}

#[tokio::test]
async fn encrypted_message_round_trip_retry_edit_and_authorization() {
    let (state, app, conversation, alice, bob, hash) = setup().await;
    let original = sealed(
        &alice,
        &bob,
        &conversation,
        "Not readable by the server",
        &hash,
    );
    let body = serde_json::to_value(&original).unwrap();
    let first = value(
        request(
            &app,
            "POST",
            "/v1/e2ee/messages",
            TEST_OWNER_ID,
            body.clone(),
        )
        .await,
    )
    .await;
    let retry = value(
        request(
            &app,
            "POST",
            "/v1/e2ee/messages",
            TEST_OWNER_ID,
            body.clone(),
        )
        .await,
    )
    .await;
    assert_eq!(first, retry);
    assert_eq!(
        request(
            &app,
            "POST",
            "/v1/e2ee/messages",
            TEST_MEMBER_C_ID,
            body.clone()
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(&app, "POST", "/v1/e2ee/messages", TEST_MEMBER_A_ID, body)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let context = format!(
        "test-network/{conversation}/{TEST_OWNER_ID}/{}/message",
        original.id
    );
    let cipher = base64::engine::general_purpose::STANDARD
        .decode(first["payload"]["ciphertext"].as_str().unwrap())
        .unwrap();
    let recovered = Identity::recover(&bob.recovery_key().unwrap()).unwrap();
    assert_eq!(
        &*recovered.open(&context, &cipher, &alice.public()).unwrap(),
        b"Not readable by the server"
    );
    let path = format!("/v1/e2ee/messages/{}", original.id);
    let mut edit = original.clone();
    edit.ciphertext = base64::engine::general_purpose::STANDARD.encode(
        alice
            .seal(&context, b"Private edit", &[alice.public(), bob.public()])
            .unwrap(),
    );
    assert_eq!(
        request(&app, "PUT", &path, TEST_MEMBER_A_ID, json!(edit))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(&app, "PUT", &path, TEST_OWNER_ID, json!(edit))
            .await
            .status(),
        StatusCode::OK
    );
    let rows = load_recent_messages(&state.pool, TEST_MEMBER_A_ID.parse().unwrap())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].payload["ciphertext"], edit.ciphertext);
    assert!(rows[0].edited_at.is_some());
    let stored: String = sqlx::query_scalar("SELECT payload FROM messages")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert!(!stored.contains("Private edit"));
    assert!(!stored.contains("Not readable"));
    privacy::ensure_ciphertext_storage(&state.pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn plaintext_routes_and_fake_encryption_versions_fail_closed() {
    let (state, app, conversation, ..) = setup().await;
    for (path, body) in [
        (
            "/v1/messages",
            json!({"conversation_id":conversation,"content_type":"text/plain","payload":"secret","encryption_version":0}),
        ),
        (
            "/v1/messages/image",
            json!({"conversation_id":conversation,"caption":"secret","png_base64":""}),
        ),
        (
            "/v1/messages/file",
            json!({"conversation_id":conversation,"caption":"secret","file_name":"secret.txt","data_base64":""}),
        ),
        (
            "/v1/file-uploads",
            json!({"id":Uuid::new_v4(),"conversation_id":conversation,"file_name":"secret.txt","size":0,"caption":"secret"}),
        ),
    ] {
        assert_eq!(
            request(&app, "POST", path, TEST_OWNER_ID, body)
                .await
                .status(),
            StatusCode::FORBIDDEN,
            "{path}"
        );
    }
    assert_eq!(request(&app, "POST", "/v1/e2ee/messages", TEST_OWNER_ID, json!({"id":Uuid::new_v4(),"conversation_id":conversation,"ciphertext":"not an envelope"})).await.status(), StatusCode::BAD_REQUEST);
    assert!(
        validate_message(&SendMessageRequest {
            conversation_id: conversation.clone(),
            content_type: "text/plain".into(),
            payload: json!("plaintext"),
            encryption_version: 1
        })
        .is_err()
    );
    // Strict readiness catches old plaintext without deleting or rewriting it.
    sqlx::query("INSERT INTO messages(id,conversation_id,sender_id,created_at,content_type,payload,encryption_version) VALUES (?,?,?,?, 'text/plain', '\"old history\"',0)")
        .bind(Uuid::new_v4().to_string()).bind(conversation).bind(TEST_OWNER_ID).bind(Utc::now().to_rfc3339()).execute(&state.pool).await.unwrap();
    assert!(
        privacy::ensure_ciphertext_storage(&state.pool)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM messages")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn encrypted_attachment_chunks_keep_and_expiry_work_without_filenames() {
    let (state, app, conversation, alice, bob, hash) = setup().await;
    let mut bytes = Vec::new();
    wisp_crypto::encrypt_stream(
        &b"private attachment bytes"[..],
        &mut bytes,
        &[alice.public(), bob.public()],
    )
    .unwrap();
    let message = sealed(
        &alice,
        &bob,
        &conversation,
        "private-filename.png and caption",
        &hash,
    );
    let upload = Uuid::new_v4();
    let begin = json!({"upload_id":upload,"size":bytes.len(),"keep":false,"message":message});
    assert_eq!(
        request(
            &app,
            "POST",
            "/v1/e2ee/file-uploads",
            TEST_OWNER_ID,
            begin.clone()
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(&app, "POST", "/v1/e2ee/file-uploads", TEST_OWNER_ID, begin)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(request(&app,"POST","/v1/file-uploads",TEST_OWNER_ID,json!({"id":upload,"conversation_id":conversation,"size":bytes.len(),"file_name":"Encrypted attachment","caption":"plaintext"})).await.status(),StatusCode::FORBIDDEN);
    let path = format!("/v1/file-uploads/{upload}/chunks/0");
    for _ in 0..2 {
        let mut req = Request::builder()
            .method("PUT")
            .uri(&path)
            .body(Body::from(bytes.clone()))
            .unwrap();
        *req.headers_mut() = chat_headers(TEST_OWNER_ID);
        assert_eq!(
            app.clone().oneshot(req).await.unwrap().status(),
            StatusCode::OK
        );
    }
    let sent = value(
        request(
            &app,
            "POST",
            &format!("/v1/file-uploads/{upload}/complete"),
            TEST_OWNER_ID,
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(sent["id"], message.id.to_string());
    assert!(!sent.to_string().contains("private-filename"));
    let stored_name: String = sqlx::query_scalar("SELECT file_name FROM file_uploads")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(stored_name, "Encrypted attachment");
    let response = get_chat_file(
        State(state.clone()),
        chat_headers(TEST_MEMBER_A_ID),
        Path(message.id),
    )
    .await
    .unwrap();
    let downloaded = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let mut plaintext = Vec::new();
    bob.decrypt_stream(downloaded.as_ref(), &mut plaintext)
        .unwrap();
    assert_eq!(plaintext, b"private attachment bytes");
    let retention = format!("/v1/messages/{}/retention", message.id);
    assert_eq!(
        request(
            &app,
            "PATCH",
            &retention,
            TEST_MEMBER_A_ID,
            json!({"keep":true})
        )
        .await
        .status(),
        StatusCode::OK
    );
    sqlx::query("UPDATE messages SET payload=json_set(payload,'$.keep',json('false'),'$.expires_at',?) WHERE id=?").bind((Utc::now()-ChronoDuration::hours(1)).to_rfc3339()).bind(message.id.to_string()).execute(&state.pool).await.unwrap();
    assert_eq!(
        attachments::cleanup(&state.pool, Utc::now()).await.unwrap(),
        1
    );
    assert!(
        get_chat_file(
            State(state.clone()),
            chat_headers(TEST_MEMBER_A_ID),
            Path(message.id)
        )
        .await
        .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM file_chunks")
            .fetch_one(&state.pool)
            .await
            .unwrap(),
        0
    );
}
