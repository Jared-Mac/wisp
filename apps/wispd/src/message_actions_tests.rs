use super::*;
use crate::{AuthMethod, obtain_session};
use std::{path::Path, sync::atomic::AtomicBool};
use tokio::sync::RwLock;
use wisp_protocol::ServerView;

async fn server(directory: &Path) -> (String, tokio::task::JoinHandle<()>) {
    let state = wisp_server::AppState::new(wisp_server::AppConfig {
        database_url: format!("sqlite:{}", directory.join("server.sqlite3").display()),
        invite_url: None,
        public_url: None,
        livekit_url: "ws://127.0.0.1:1".into(),
        livekit_api_key: "test".into(),
        livekit_api_secret: "isolated-no-media".into(),
        knock_ttl: Duration::from_secs(30),
        allow_dev_sessions: true,
        bootstrap_token: None,
        require_chat_e2ee: true,
    })
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, wisp_server::router(state))
            .await
            .unwrap();
    });
    (url, task)
}
async fn client(server: &str, profile: &str, directory: &Path) -> Session<'static> {
    let client = reqwest::Client::new();
    let auth = AuthMethod::Development {
        profile: profile.into(),
    };
    let token = obtain_session(&client, server, &auth).await.unwrap();
    let api = ServerApi {
        client,
        account_registry: None,
        base_url: server.into(),
        token: Arc::new(std::sync::RwLock::new(token)),
        auth: Arc::new(std::sync::RwLock::new(auth)),
    };
    let snapshot = api.snapshot().await.unwrap();
    let privacy = Privacy::at(directory.into(), server, snapshot.self_state.user.id);
    privacy.initialize(&api).await.unwrap();
    Session::Linked(Arc::new(LinkedServer {
        view: ServerView {
            id: server.into(),
            name: "Isolated".into(),
            url: server.into(),
            connected: true,
        },
        api,
        privacy,
        state: RwLock::new(snapshot),
        connected: AtomicBool::new(true),
        media_key: None,
    }))
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn replies_and_cross_server_forwards_keep_content_private_and_reencrypt_files() {
    let source_dir = tempfile::tempdir().unwrap();
    let dest_dir = tempfile::tempdir().unwrap();
    let (source_url, source_task) = server(source_dir.path()).await;
    let (dest_url, dest_task) = server(dest_dir.path()).await;
    let source = client(&source_url, "Owner", &source_dir.path().join("owner")).await;
    let forwarder = client(&source_url, "MemberA", &source_dir.path().join("member")).await;
    let destination = client(&dest_url, "Owner", &dest_dir.path().join("owner")).await;
    let recipient = client(&dest_url, "MemberB", &dest_dir.path().join("member")).await;
    let chat = source.api().create_direct("MemberA".into()).await.unwrap();
    let target = destination
        .api()
        .create_direct("MemberB".into())
        .await
        .unwrap();
    let (vault, roster) = source
        .privacy()
        .recipients(source.api(), &chat)
        .await
        .unwrap();
    let original = Privacy::seal(
        &vault,
        &roster,
        Uuid::new_v4(),
        Content {
            content_type: "text/plain".into(),
            payload: json!("Private original"),
            attachment: None,
            context: None,
        },
    )
    .unwrap();
    let stored: Message = decode(
        source
            .api()
            .request(reqwest::Method::POST, "/v1/e2ee/messages")
            .json(&original)
            .send()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    let visible = forwarder.message(stored.id).await.unwrap();
    assert_eq!(visible.payload, "Private original");
    let reply_context = MessageContext {
        reply_to: Some(reference(&visible)),
        forwarded_from: None,
    };
    let reply = send_content(
        &forwarder,
        &chat.id,
        "Private reply".into(),
        reply_context.clone(),
        None,
        false,
        |_, _| {},
    )
    .await
    .unwrap();
    assert!(reply.context.is_none());
    assert!(!reply.payload.to_string().contains("Private original"));
    let decrypted = source.message(reply.id).await.unwrap();
    assert_eq!(decrypted.context, Some(reply_context.clone()));
    let sent_forward = send_content(
        &destination,
        &target.id,
        decrypted.payload.as_str().unwrap().into(),
        forwarding_context(&decrypted),
        None,
        false,
        |_, _| {},
    )
    .await
    .unwrap();
    let received = recipient.message(sent_forward.id).await.unwrap();
    assert_eq!(received.payload, "Private reply");
    let context = received.context.unwrap();
    assert!(context.reply_to.is_none());
    assert_eq!(context.forwarded_from.unwrap().sender_name, "MemberA");
    assert!(recipient.message(stored.id).await.is_err());
    // Real encrypted file uploads use a new manifest, key, and destination roster.
    let mut file = tempfile::NamedTempFile::new().unwrap();
    let bytes = b"Private attachment bytes".repeat(12000);
    std::io::Write::write_all(&mut file, &bytes).unwrap();
    let attachment = AttachmentDraft::from_temporary(&file, "notes.txt".into(), false).unwrap();
    let upload = send_content(
        &source,
        &chat.id,
        "A file".into(),
        reply_context,
        Some(attachment),
        false,
        |_, _| {},
    )
    .await
    .unwrap();
    let original_file = forwarder.message(upload.id).await.unwrap();
    let downloaded = download(&forwarder, &original_file).await.unwrap();
    assert_eq!(std::fs::read(downloaded.path()).unwrap(), bytes);
    let old_manifest = forwarder
        .privacy()
        .content(upload.id)
        .unwrap()
        .attachment
        .unwrap();
    let forwarded_file = send_content(
        &destination,
        &target.id,
        "A file".into(),
        forwarding_context(&original_file),
        Some(AttachmentDraft::from_temporary(&downloaded, "notes.txt".into(), false).unwrap()),
        false,
        |_, _| {},
    )
    .await
    .unwrap();
    let received_file = recipient.message(forwarded_file.id).await.unwrap();
    let new_manifest = recipient
        .privacy()
        .content(forwarded_file.id)
        .unwrap()
        .attachment
        .unwrap();
    assert_ne!(
        serde_json::to_value(old_manifest).unwrap(),
        serde_json::to_value(new_manifest).unwrap()
    );
    assert_eq!(
        std::fs::read(download(&recipient, &received_file).await.unwrap().path()).unwrap(),
        bytes
    );
    assert!(received_file.context.as_ref().unwrap().reply_to.is_none());
    assert_eq!(
        forwarding_context(&received_file),
        received_file.context.unwrap()
    );
    assert!(source.message(forwarded_file.id).await.is_err());
    // Pins expose only the opaque encrypted message to storage and HTTP.
    let _: Value = decode(
        destination
            .api()
            .request(
                reqwest::Method::PUT,
                &format!("/v1/messages/{}/pin", forwarded_file.id),
            )
            .json(&json!({"pinned":true}))
            .send()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    let pinned: Value = decode(
        recipient
            .api()
            .request(reqwest::Method::GET, "/v1/pins")
            .query(&[("conversation_id", &target.id)])
            .send()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    assert!(!pinned.to_string().contains("notes.txt"));
    assert!(!pinned.to_string().contains("forwarded_from"));
    let _: Value = decode(
        source
            .api()
            .request(
                reqwest::Method::DELETE,
                &format!("/v1/messages/{}", stored.id),
            )
            .send()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    assert!(forwarder.message(stored.id).await.is_err());
    assert_eq!(
        recipient.message(sent_forward.id).await.unwrap().payload,
        "Private reply"
    );
    source_task.abort();
    dest_task.abort();
}
