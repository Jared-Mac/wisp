#![allow(clippy::manual_assert_eq)] // Never print private records or credentials on failure.
use std::process::{Command, Stdio};

#[test]
fn help_succeeds_without_reading_stdin_or_writing_account_files() {
    let config = tempfile::tempdir().expect("temporary account config");
    for flag in ["--help", "-h"] {
        let result = Command::new(env!("CARGO_BIN_EXE_wisp-account"))
            .arg(flag)
            .env("XDG_CONFIG_HOME", config.path())
            .stdin(Stdio::null())
            .output()
            .expect("start account helper");
        assert!(result.status.success());
        assert!(String::from_utf8_lossy(&result.stdout).contains("JSON request on stdin"));
        assert!(result.stderr.is_empty());
        assert_eq!(
            config
                .path()
                .read_dir()
                .expect("read config directory")
                .count(),
            0
        );
    }
}

/// Real child-process/native/server integration; every config and database is
/// isolated, and no password, device credential, or private record is logged.
#[tokio::test]
#[allow(clippy::too_many_lines)] // One synthetic end-to-end recovery scenario.
async fn secure_cli_restores_same_identity_and_reconciles_interrupted_installation() {
    use base64::Engine as _;
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::{
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    use tokio::io::AsyncWriteExt;
    const PASSWORD: &str = "synthetic cli only secret password";
    const MEDIA: &str = "synthetic cli private media key";
    async fn call(config: &Path, origin: &str, request: &Value) -> std::process::Output {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_wisp-account"))
            .env_clear()
            .env("HOME", config)
            .env("XDG_CONFIG_HOME", config)
            .env("WISP_ACCOUNT_SERVER", origin)
            .env("WISP_PRIVACY_DIR", config.join("wisp/privacy"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("start synthetic account helper");
        let mut input = serde_json::to_vec(request).unwrap();
        input.push(b'\n');
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(&input).await.unwrap();
        drop(stdin);
        tokio::time::timeout(std::time::Duration::from_secs(45), child.wait_with_output())
            .await
            .unwrap()
            .unwrap()
    }
    fn private_record(config: &Path, origin: &str) -> (std::path::PathBuf, Value) {
        let path = config
            .join("wisp/privacy/account-backup")
            .join(format!("{:x}.json", Sha256::digest(origin.as_bytes())));
        let value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        (path, value)
    }
    let folder = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let database = format!("sqlite:{}", folder.path().join("server.sqlite3").display());
    let state = wisp_server::AppState::new(wisp_server::AppConfig {
        database_url: database.clone(),
        public_url: Some(origin.clone()),
        invite_url: None,
        livekit_url: "ws://127.0.0.1:1".into(),
        livekit_api_key: "synthetic".into(),
        livekit_api_secret: "synthetic-no-media".into(),
        knock_ttl: std::time::Duration::from_secs(30),
        allow_dev_sessions: false,
        bootstrap_token: None,
        require_chat_e2ee: true,
    })
    .await
    .unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let forbidden = Arc::new(AtomicUsize::new(0));
    let monitor = (requests.clone(), forbidden.clone());
    let router = wisp_server::router(state).layer(axum::middleware::from_fn_with_state(
        monitor,
        |axum::extract::State((requests, forbidden)): axum::extract::State<(
            Arc<AtomicUsize>,
            Arc<AtomicUsize>,
        )>,
         request: axum::extract::Request,
         next: axum::middleware::Next| async move {
            requests.fetch_add(1, Ordering::Relaxed);
            let (parts, body) = request.into_parts();
            let bytes = axum::body::to_bytes(body, 16 * 1024 * 1024).await.unwrap();
            if bytes
                .windows(PASSWORD.len())
                .any(|v| v == PASSWORD.as_bytes())
                || bytes.windows(MEDIA.len()).any(|v| v == MEDIA.as_bytes())
                || parts.uri.path() == "/v1/accounts/login"
            {
                forbidden.fetch_add(1, Ordering::Relaxed);
            }
            next.run(axum::extract::Request::from_parts(
                parts,
                axum::body::Body::from(bytes),
            ))
            .await
        },
    ));
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let first = folder.path().join("first");
    let second = folder.path().join("second");
    let created=call(&first,&origin,&json!({"action":"register","username":"CliUser","display_name":"Synthetic user","password":PASSWORD,"device_name":"Synthetic desktop","media_key":MEDIA})).await;
    assert!(
        created.status.success(),
        "synthetic secure registration failed"
    );
    let logged_in=call(&second,&origin,&json!({"action":"login","username":"CLIUSER","password":PASSWORD,"device_name":"Synthetic remote desktop"})).await;
    assert!(
        logged_in.status.success(),
        "synthetic secure remote login failed"
    );
    let (_, original) = private_record(&first, &origin);
    let (path, mut restored) = private_record(&second, &origin);
    let bundle: Value = serde_json::from_slice(
        &base64::engine::general_purpose::STANDARD
            .decode(restored["bundle"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(bundle["media_key"] == MEDIA);
    assert!(original["identity"] == restored["identity"]);
    assert!(original["key"] == restored["key"] && restored["key_verified"] == true);
    let count = requests.load(Ordering::Relaxed);
    let classic = call(
        &second,
        &origin,
        &json!({"action":"classic_login","username":"cliuser","password":PASSWORD}),
    )
    .await;
    assert!(!classic.status.success());
    assert!(
        requests.load(Ordering::Relaxed) == count,
        "secure latch must reject classic flow before HTTP"
    );
    // Both native actions have completed, but one config replacement is lost.
    restored["installation_pending"] = json!(true);
    std::fs::write(&path, serde_json::to_vec(&restored).unwrap()).unwrap();
    std::fs::remove_file(second.join("wisp/account.env")).unwrap();
    let finish = call(&second, &origin, &json!({"action":"finish_installations"})).await;
    assert!(finish.status.success());
    let result: Value = serde_json::from_slice(&finish.stdout).unwrap();
    assert!(result["installed"] == 1);
    assert!(second.join("wisp/account.env").exists());
    let again = call(&second, &origin, &json!({"action":"finish_installations"})).await;
    assert!(again.status.success());
    let result: Value = serde_json::from_slice(&again.stdout).unwrap();
    assert!(result["installed"] == 0);
    assert!(
        requests.load(Ordering::Relaxed) == count,
        "installation recovery must be offline"
    );
    assert!(
        forbidden.load(Ordering::Relaxed) == 0,
        "native flows must not send raw passwords or use classic login"
    );
    let pool = sqlx::SqlitePool::connect(&database).await.unwrap();
    let devices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM devices WHERE revoked_at IS NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(devices == 2);
    pool.close().await;
    server.abort();
}
