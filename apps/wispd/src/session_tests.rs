use super::*;

async fn isolated_server() -> (String, tokio::task::JoinHandle<()>) {
    isolated_server_with_media("ws://127.0.0.1:1".into()).await
}

async fn isolated_server_with_media(livekit_url: String) -> (String, tokio::task::JoinHandle<()>) {
    let state = wisp_server::AppState::new(wisp_server::AppConfig {
        database_url: "sqlite::memory:".into(),
        public_url: None,
        livekit_url,
        livekit_api_key: "isolated".into(),
        livekit_api_secret: "isolated-no-media".into(),
        knock_ttl: Duration::from_secs(30),
        allow_dev_sessions: true,
        bootstrap_token: None,
        require_chat_e2ee: false,
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

pub(crate) fn resident_bytes() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    status
        .lines()
        .find(|line| line.starts_with("VmRSS:"))
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse::<u64>()
        .unwrap()
        * 1024
}

#[tokio::test]
#[ignore = "requires WISP_TEST_LIVEKIT_BINARY; isolated relay only, no capture or publication"]
#[allow(clippy::too_many_lines)]
async fn failed_rtc_join_releases_resources_without_growth() {
    use livekit::{Room, RoomOptions, webrtc::peer_connection_factory::IceTransportsType};
    let binary = std::env::var_os("WISP_TEST_LIVEKIT_BINARY").expect("local test relay required");
    let temp = tempfile::tempdir().unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let udp_port = std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let config = temp.path().join("relay.yaml");
    std::fs::write(&config, format!("port: {port}\nbind_addresses: [127.0.0.1]\nrtc:\n  udp_port: {udp_port}\n  use_external_ip: false\nkeys:\n  isolated: isolated-no-media\nlogging:\n  level: warn\n")).unwrap();
    let mut relay = tokio::process::Command::new(binary)
        .args([
            "--config",
            config.to_str().unwrap(),
            "--node-ip",
            "127.0.0.1",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let client = reqwest::Client::new();
    let relay_url = format!("http://127.0.0.1:{port}");
    let mut ready = false;
    for _ in 0..100 {
        if client.get(&relay_url).send().await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(ready, "isolated relay did not start");
    let (url, server) = isolated_server_with_media(format!("ws://127.0.0.1:{port}")).await;
    let (api, _) = ServerApi::connect_with_auth(
        url,
        AuthMethod::Development {
            profile: "Owner".into(),
        },
    )
    .await
    .unwrap();
    let room: wisp_protocol::ConversationView = decode(
        api.request(reqwest::Method::POST, "/v1/rooms")
            .json(&json!({"name":"ICE failure fixture"}))
            .send()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    api.join_spot(room.spot_id.unwrap()).await.unwrap();
    let credentials = api.livekit_token().await.unwrap();
    // Wisp keeps one platform runtime alive across joins. Match that lifetime
    // instead of repeatedly rebuilding native codec factories in this fixture.
    // Creating the runtime does not start capture or publish an audio track.
    let _platform = livekit::prelude::PlatformAudio::new().unwrap();
    let mut samples = Vec::new();
    for _ in 0..2 {
        let mut options = RoomOptions::default();
        options.join_retries = 0;
        options.connect_timeout = Duration::from_secs(3);
        options.auto_subscribe = false;
        options.encryption = Some(livekit::E2eeOptions {
            encryption_type: livekit::e2ee::EncryptionType::Gcm,
            key_provider: livekit::e2ee::key_provider::KeyProvider::with_shared_key(
                livekit::e2ee::key_provider::KeyProviderOptions::default(),
                b"isolated-media-fixture-key-32bytes".to_vec(),
            ),
        });
        // No TURN server is configured: force ICE failure without touching the
        // user's network configuration or capturing/publishing any media.
        options.rtc_config.ice_transport_type = IceTransportsType::Relay;
        let result = tokio::time::timeout(
            Duration::from_secs(35),
            Room::connect(&credentials.url, &credentials.token, options),
        )
        .await;
        assert!(
            result.is_ok(),
            "failed join did not finish cleanup promptly"
        );
        let error = result
            .unwrap()
            .expect_err("relay-only ICE must fail without TURN");
        assert!(
            error.to_string().contains("wait_pc_connection"),
            "fixture must reach the ICE timeout, not fail authentication"
        );
        tokio::time::sleep(Duration::from_secs(2)).await;
        let memory = resident_bytes();
        println!(
            "Failed join {}: RSS {} MiB",
            samples.len() + 1,
            memory / (1024 * 1024)
        );
        samples.push(memory);
    }
    let before = resident_bytes();
    for _ in 0..5 {
        tokio::time::sleep(Duration::from_secs(2)).await;
        println!("Idle RSS {} MiB", resident_bytes() / (1024 * 1024));
    }
    assert!(
        resident_bytes() < before + 32 * 1024 * 1024,
        "memory grew after failed sessions ended"
    );
    assert!(
        samples[1] < samples[0] + 64 * 1024 * 1024,
        "failed joins retained excessive memory"
    );
    api.leave().await.unwrap();
    server.abort();
    relay.kill().await.unwrap();
    println!(
        "Isolated failed ICE joins cleaned up; memory remained stable without media publication"
    );
}

#[tokio::test]
async fn startup_clears_stale_room_before_returning_a_snapshot() {
    let (url, server) = isolated_server().await;
    let auth = || AuthMethod::Development {
        profile: "Owner".into(),
    };
    let (api, _) = ServerApi::connect_with_auth(url.clone(), auth())
        .await
        .unwrap();
    let room: wisp_protocol::ConversationView = decode(
        api.request(reqwest::Method::POST, "/v1/rooms")
            .json(&json!({"name":"Restart fixture"}))
            .send()
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    api.join_spot(room.spot_id.unwrap()).await.unwrap();
    assert!(
        api.snapshot()
            .await
            .unwrap()
            .self_state
            .hangout_id
            .is_some()
    );
    // Simulate a fresh daemon authenticating after its predecessor vanished.
    let (_, restarted) = ServerApi::connect_with_auth(url.clone(), auth())
        .await
        .unwrap();
    assert!(restarted.self_state.hangout_id.is_none());
    assert!(
        api.snapshot()
            .await
            .unwrap()
            .self_state
            .hangout_id
            .is_none()
    );
    assert!(!restarted.self_state.media.microphone_published);
    assert!(!restarted.self_state.media.camera.active);
    assert!(!restarted.self_state.media.screen_share.active);
    // A second fresh login with no stale room is harmless.
    let (_, again) = ServerApi::connect_with_auth(url, auth()).await.unwrap();
    assert!(again.self_state.hangout_id.is_none());
    server.abort();
}

#[tokio::test]
async fn failed_media_is_latched_until_explicit_join_or_leave() {
    let (url, server) = isolated_server().await;
    let (api, mut snapshot) = ServerApi::connect_with_auth(
        url.clone(),
        AuthMethod::Development {
            profile: "Owner".into(),
        },
    )
    .await
    .unwrap();
    let room = uuid::Uuid::new_v4();
    snapshot.self_state.hangout_id = Some(room);
    // Fail before requesting a media token or opening any capture device.
    snapshot.chat_encryption_required = true;
    let view = ServerView {
        id: "fixture".into(),
        name: "Fixture".into(),
        url,
        connected: true,
    };
    let (media, _events) = MediaManager::new(false, None);
    let daemon = Daemon::new(
        "Owner".into(),
        view.clone(),
        vec![view],
        api,
        snapshot,
        None,
        media,
        true,
        Duration::from_secs(30),
        ShortcutManager::from_environment(),
    );
    assert!(daemon.reconcile_media().await.is_err());
    assert_eq!(*daemon.failed_media_room.lock().await, Some(room));
    for _ in 0..1000 {
        // Even a coordination reconnect cannot silently re-attempt media.
        daemon
            .set_connection(ConnectionState::Reconnecting, None)
            .await;
        assert!(daemon.reconcile_media().await.is_ok());
        assert_eq!(
            daemon.state.read().await.self_state.connection,
            ConnectionState::Failed
        );
    }
    daemon.set_connection(ConnectionState::Joining, None).await;
    assert!(
        daemon.reconcile_media().await.is_err(),
        "explicit join permits exactly another attempt"
    );
    daemon.state.write().await.self_state.hangout_id = None;
    daemon.reconcile_media().await.unwrap();
    assert!(daemon.failed_media_room.lock().await.is_none());
    assert_eq!(
        daemon.state.read().await.self_state.connection,
        ConnectionState::Available
    );
    assert!(
        !daemon
            .state
            .read()
            .await
            .self_state
            .media
            .microphone_published
    );
    server.abort();
}
