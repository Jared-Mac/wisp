use super::*;
use crate::{AuthMethod, obtain_session};

async fn client(server: &str, profile: &str) -> ServerApi {
    let client = reqwest::Client::new();
    let auth = AuthMethod::Development {
        profile: profile.into(),
    };
    let token = obtain_session(&client, server, &auth).await.unwrap();
    ServerApi {
        client,
        base_url: server.into(),
        token: Arc::new(std::sync::RwLock::new(token)),
        auth,
    }
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn two_clients_encrypt_restore_and_admit_a_friend_without_manual_verification() {
    let state = wisp_server::AppState::new(wisp_server::AppConfig {
        database_url: "sqlite::memory:".into(),
        public_url: Some("https://wisp.invalid".into()),
        livekit_url: "ws://127.0.0.1:1".into(),
        livekit_api_key: "test".into(),
        livekit_api_secret: "isolated-test-no-media".into(),
        knock_ttl: std::time::Duration::from_secs(30),
        allow_dev_sessions: true,
        bootstrap_token: None,
        require_chat_e2ee: true,
    })
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, wisp_server::router(state))
            .await
            .unwrap();
    });
    let alice = client(&server, "MemberA").await;
    let bob = client(&server, "Owner").await;
    let a = alice.snapshot().await.unwrap().self_state.user.id;
    let b = bob.snapshot().await.unwrap().self_state.user.id;
    let temp = tempfile::tempdir().unwrap();
    let av = Privacy::at(temp.path().join("alice"), &server, a);
    let bv = Privacy::at(temp.path().join("bob"), &server, b);
    let backup = temp.path().join("bob-recovery.key");
    av.enable(&alice, &temp.path().join("alice-recovery.key"), None)
        .await
        .unwrap();
    bv.enable(&bob, &backup, None).await.unwrap();
    let conversation = alice.create_direct("Owner".into()).await.unwrap();
    let (vault, roster) = av.recipients(&alice, &conversation).await.unwrap();
    let mut directory = av.directory(&alice, &vault).await.unwrap();
    directory.identities.insert(
        Uuid::new_v4(),
        wisp_crypto::Identity::generate().unwrap().public(),
    );
    assert!(
        Privacy::verify_directory(&vault, &directory).is_err(),
        "The provider cannot silently enroll a new account as an existing friend"
    );
    let request = Privacy::seal(
        &vault,
        &roster,
        Uuid::new_v4(),
        Content {
            content_type: "text/plain".into(),
            payload: json!("Private integration test"),
            attachment: None,
        },
    )
    .unwrap();
    let stored: Message = decode(
        alice
            .request(reqwest::Method::POST, "/v1/e2ee/messages")
            .json(&request)
            .send()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    assert!(
        !stored
            .payload
            .to_string()
            .contains("Private integration test")
    );
    let mut snapshot = bob.snapshot().await.unwrap();
    bv.decrypt_snapshot(&bob, &mut snapshot).await;
    assert_eq!(
        snapshot
            .messages
            .iter()
            .find(|m| m.id == stored.id)
            .unwrap()
            .payload,
        "Private integration test"
    );
    let mut injected = bob.snapshot().await.unwrap();
    injected.chat_encryption_required = false; // A server flag is not trusted to downgrade an enrolled client.
    injected.messages[0].encryption_version = 0;
    injected.messages[0].content_type = "text/plain".into();
    injected.messages[0].payload = json!("forged message pretending to be a friend");
    injected.messages[0].sender.display_name = "Forged account name".into();
    bv.decrypt_snapshot(&bob, &mut injected).await;
    assert_eq!(injected.messages[0].sender.display_name, "MemberA");
    assert_eq!(
        injected.messages[0].payload,
        "[Unencrypted message blocked — encrypted chat is required]"
    );
    let restored = Privacy::at(temp.path().join("restored"), &server, b);
    restored
        .enable(
            &bob,
            &temp.path().join("recovered-backup.key"),
            Some(&backup),
        )
        .await
        .unwrap();
    let mut snapshot = bob.snapshot().await.unwrap();
    restored.decrypt_snapshot(&bob, &mut snapshot).await;
    assert_eq!(
        snapshot
            .messages
            .iter()
            .find(|m| m.id == stored.id)
            .unwrap()
            .payload,
        "Private integration test"
    );
    let room: ConversationView = decode(
        alice
            .request(reqwest::Method::POST, "/v1/rooms")
            .json(&json!({"name":"Isolated encrypted room"}))
            .send()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    let (_, initial) = av.recipients(&alice, &room).await.unwrap();
    let old = Privacy::seal(
        &vault,
        &initial,
        Uuid::new_v4(),
        Content {
            content_type: "text/plain".into(),
            payload: json!("Before invitation"),
            attachment: None,
        },
    )
    .unwrap();
    let _: Message = decode(
        alice
            .request(reqwest::Method::POST, "/v1/e2ee/messages")
            .json(&old)
            .send()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    let mut next = initial.roster.clone();
    next.revision = 1;
    next.previous = Some(initial.hash().unwrap());
    next.members.insert(
        b,
        Member {
            identity: bv.active().unwrap().unwrap().ring.identity().public(),
            role: Role::Member,
        },
    );
    let signed = next.sign(vault.ring.identity()).unwrap();
    crate::ensure_ok(
        alice
            .request(reqwest::Method::POST, "/v1/e2ee/roster")
            .json(&signed)
            .send()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    let snapshot = bob.snapshot().await.unwrap();
    assert!(!snapshot.messages.iter().any(|m| m.id == old.id));
    let conversation = snapshot
        .conversations
        .iter()
        .find(|c| c.id == room.id)
        .unwrap();
    let (_, accepted) = bv.recipients(&bob, conversation).await.unwrap();
    assert_eq!(accepted, signed);
    let edited_old = Privacy::seal_to(
        &vault,
        &signed,
        old.id,
        Content {
            content_type: "text/plain".into(),
            payload: json!("Edited without exposing pre-invite history"),
            attachment: None,
        },
        &BTreeMap::from([(a, vault.ring.identity().public())]),
    )
    .unwrap();
    crate::ensure_ok(
        alice
            .request(
                reqwest::Method::PUT,
                &format!("/v1/e2ee/messages/{}", old.id),
            )
            .json(&edited_old)
            .send()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    let binding = MessageContext {
        network: vault.network,
        conversation: room.id.clone(),
        sender: a,
        message: old.id,
        roster: signed.hash().unwrap(),
    };
    assert!(
        binding
            .open(
                bv.active().unwrap().unwrap().ring.identity(),
                b,
                &vault.ring.identity().public(),
                &STANDARD.decode(&edited_old.ciphertext).unwrap()
            )
            .is_err(),
        "An edit must not give old history to newly admitted members"
    );
    assert!(
        bv.active()
            .unwrap()
            .unwrap()
            .ring
            .accept_rosters(vault.network, &room.id, b, &[initial])
            .is_err()
    );
    // Voice invitations carry a signed admission but do not grant membership
    // until the invited person explicitly accepts. No media engine is started.
    let voice_room: ConversationView = decode(
        alice
            .request(reqwest::Method::POST, "/v1/rooms")
            .json(&json!({"name":"Private voice invite"}))
            .send()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    let joined: Value = decode(
        alice
            .request(reqwest::Method::POST, "/v1/spots/join")
            .json(&json!({"spot_id":voice_room.spot_id}))
            .send()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    let offer = av
        .invite_member(&alice, &voice_room, b)
        .await
        .unwrap()
        .unwrap();
    let mut forged = offer.clone();
    forged["signature"] = json!("invalid");
    let rejected = alice
        .request(reqwest::Method::POST, "/v1/room-invitations")
        .json(&json!({"hangout_id":joined["hangout_id"],"user_id":b,"encrypted_membership":forged}))
        .send()
        .await
        .unwrap();
    assert!(!rejected.status().is_success());
    let invitation: Value = decode(alice.request(reqwest::Method::POST, "/v1/room-invitations")
        .json(&json!({"hangout_id":joined["hangout_id"],"user_id":b,"encrypted_membership":offer})).send().await.unwrap()).await.unwrap();
    let before = bob.snapshot().await.unwrap();
    assert!(!before.conversations.iter().any(|c| c.id == voice_room.id));
    assert!(before.self_state.hangout_id.is_none());
    let response_path = format!(
        "/v1/room-invitations/{}/respond",
        invitation["id"].as_str().unwrap()
    );
    crate::ensure_ok(
        bob.request(reqwest::Method::POST, &response_path)
            .json(&json!({"accept":true}))
            .send()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    let after = bob.snapshot().await.unwrap();
    assert!(after.self_state.hangout_id.is_some());
    let admitted = after
        .conversations
        .iter()
        .find(|c| c.id == voice_room.id)
        .unwrap();
    let (_, verified) = bv.recipients(&bob, admitted).await.unwrap();
    assert_eq!(serde_json::to_value(verified).unwrap(), offer);
    crate::ensure_ok(
        bob.request(reqwest::Method::POST, "/v1/hangouts/leave")
            .send()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    crate::ensure_ok(
        bob.request(reqwest::Method::POST, &response_path)
            .json(&json!({"accept":true}))
            .send()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    assert!(
        bob.snapshot()
            .await
            .unwrap()
            .self_state
            .hangout_id
            .is_none(),
        "An accepted-invite retry must not rejoin voice"
    );
    attachment_roundtrip(&alice, &bob, &av, &bv, conversation).await;
    task.abort();
}

#[allow(clippy::too_many_lines)]
async fn attachment_roundtrip(
    alice: &ServerApi,
    bob: &ServerApi,
    av: &Privacy,
    bv: &Privacy,
    conversation: &ConversationView,
) {
    use std::io::{Cursor, Read};
    let (sender, roster) = av.recipients(alice, conversation).await.unwrap();
    for image in [false, true] {
        let plain = if image {
            let mut output = Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(image::RgbaImage::new(4, 3))
                .write_to(&mut output, image::ImageFormat::Png)
                .unwrap();
            output.into_inner()
        } else {
            vec![42; 5_000_000]
        };
        let recipients = roster
            .roster
            .members
            .values()
            .map(|m| m.identity.clone())
            .collect::<Vec<_>>();
        let mut prepared =
            wisp_crypto::attachment::prepare(Cursor::new(&plain), &recipients, &sender.temporary)
                .unwrap();
        let id = Uuid::new_v4();
        let message = Privacy::seal(&sender, &roster, id, Content {
            content_type: if image {"image/png"} else {"application/octet-stream"}.into(),
            payload: json!({"file_name":"private-test-file.png","size":plain.len(),"caption":"private test caption"}),
            attachment: Some(prepared.manifest.clone()),
        }).unwrap();
        let begin = wisp_protocol::BeginEncryptedUpload {
            upload_id: id,
            size: prepared.manifest.ciphertext_bytes,
            plaintext_size: Some(plain.len() as u64),
            keep: false,
            message,
        };
        crate::ensure_ok(
            alice
                .request(reqwest::Method::POST, "/v1/e2ee/file-uploads")
                .json(&begin)
                .send()
                .await
                .unwrap(),
        )
        .await
        .unwrap();
        let mut ciphertext = Vec::new();
        prepared.file.read_to_end(&mut ciphertext).unwrap();
        for (index, chunk) in ciphertext
            .chunks(wisp_protocol::CHAT_FILE_CHUNK_BYTES)
            .enumerate()
        {
            crate::ensure_ok(
                alice
                    .request(
                        reqwest::Method::PUT,
                        &format!("/v1/file-uploads/{id}/chunks/{index}"),
                    )
                    .body(chunk.to_vec())
                    .send()
                    .await
                    .unwrap(),
            )
            .await
            .unwrap();
        }
        let stored: Message = decode(
            alice
                .request(
                    reqwest::Method::POST,
                    &format!("/v1/file-uploads/{id}/complete"),
                )
                .send()
                .await
                .unwrap(),
        )
        .await
        .unwrap();
        assert!(!stored.payload.to_string().contains("private-test-file"));
        assert!(!stored.payload.to_string().contains("private test caption"));
        assert_eq!(
            stored.payload["retention_size"].as_u64(),
            Some(plain.len() as u64)
        );
        let mut received = bob.snapshot().await.unwrap();
        bv.decrypt_snapshot(bob, &mut received).await;
        let content = bv.content(id).unwrap();
        assert_eq!(content.payload["caption"], "private test caption");
        let recipient = bv.active().unwrap().unwrap();
        let manifest = content.attachment.unwrap();
        let mut output = crate::privacy_transfers::receive_attachment(
            bob,
            recipient.clone(),
            id,
            manifest.clone(),
            |_, _| {},
        )
        .await
        .unwrap();
        let mut bytes = Vec::new();
        output.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, plain);
        if image {
            assert_eq!(image::load_from_memory(&bytes).unwrap().width(), 4);
        }
        let before = std::fs::read_dir(&recipient.temporary).unwrap().count();
        let mut bad = manifest;
        bad.ciphertext_sha256[0] ^= 1;
        assert!(
            crate::privacy_transfers::receive_attachment(
                bob,
                recipient.clone(),
                id,
                bad,
                |_, _| {}
            )
            .await
            .is_err()
        );
        assert_eq!(
            std::fs::read_dir(&recipient.temporary).unwrap().count(),
            before,
            "Failed download leaves no private partial files"
        );
    }
}
