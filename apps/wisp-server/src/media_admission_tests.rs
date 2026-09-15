use super::*;
use crate::{TEST_OWNER_ID, tests::test_config};

fn signed(state: &AppState, claims: &serde_json::Value) -> String {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
    let input = format!("{header}.{}", URL_SAFE_NO_PAD.encode(claims.to_string()));
    let mut mac =
        Hmac::<Sha256>::new_from_slice(state.config.livekit_api_secret.as_bytes()).unwrap();
    mac.update(input.as_bytes());
    format!(
        "{input}.{}",
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    )
}

#[tokio::test]
async fn backend_control_grants_and_optional_nbf_keep_signature_and_expiry_checks() {
    let state = AppState::new(test_config()).await.unwrap();
    let claims = serde_json::json!({"iss":state.config.livekit_api_key,"exp":chrono::Utc::now().timestamp()+60,"video":{"roomList":true}});
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        format!("Bearer {}", signed(&state, &claims))
            .parse()
            .unwrap(),
    );
    assert_eq!(
        authorize(State(state.clone()), headers.clone())
            .await
            .unwrap(),
        StatusCode::NO_CONTENT
    );
    for patch in [
        serde_json::json!({"iss":"wrong-issuer"}),
        serde_json::json!({"exp":1}),
        serde_json::json!({"nbf":chrono::Utc::now().timestamp()+60}),
        serde_json::json!({"video":{"roomJoin":true}}),
    ] {
        let mut invalid = claims.clone();
        invalid
            .as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        headers.insert(
            "authorization",
            format!("Bearer {}", signed(&state, &invalid))
                .parse()
                .unwrap(),
        );
        assert_eq!(
            authorize(State(state.clone()), headers.clone())
                .await
                .unwrap_err()
                .status,
            StatusCode::UNAUTHORIZED
        );
    }
}

struct ProxyProcess(std::process::Child, std::path::PathBuf);
impl Drop for ProxyProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
        let _ = std::fs::remove_dir_all(&self.1);
    }
}

// Exercises the shipped Caddy directive against the real Wisp auth route, with
// a harmless fake signaling upstream. Requires a separately installed Caddy.
#[tokio::test]
#[ignore = "WISP_TEST_CADDY=/path/to/caddy cargo test -p wisp-server media_proxy -- --ignored"]
#[allow(clippy::too_many_lines)]
async fn media_proxy_blocks_cached_tokens_after_moderation() {
    use axum::{Router, routing::any};
    use serde_json::json;
    let state = AppState::new(test_config()).await.unwrap();
    let actor = TEST_OWNER_ID;
    let target = crate::TEST_MEMBER_A_ID;
    sqlx::query("INSERT INTO server_identity(id,owner_user_id) VALUES(1,?)")
        .bind(actor)
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO hangouts(id,livekit_room,created_at) VALUES('proxy-test','proxy-room',?)",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&state.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO hangout_members(hangout_id,user_id,joined_at) VALUES('proxy-test',?,?)",
    )
    .bind(target)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&state.pool)
    .await
    .unwrap();
    let user = crate::find_user(&state.pool, target).await.unwrap();
    let token = crate::issue_livekit_token(
        &state.config,
        &user,
        "proxy-room",
        &wisp_protocol::VoiceModeration::default(),
    )
    .unwrap();
    let api_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api_addr = api_listener.local_addr().unwrap();
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream.local_addr().unwrap();
    let app = crate::router(state.clone());
    let serve_app = app.clone();
    let api_task = tokio::spawn(async move {
        axum::serve(api_listener, serve_app).await.unwrap();
    });
    let media_task = tokio::spawn(async move {
        axum::serve(
            upstream,
            Router::new().fallback(any(|| async { StatusCode::ACCEPTED })),
        )
        .await
        .unwrap();
    });
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let directory = std::env::temp_dir().join(format!("wisp-media-proxy-{}", Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let template = include_str!("../../../infra/private-host/Caddyfile.example");
    let signaling = template
        .split("REPLACE_WITH_PUBLIC_HOSTNAME:8443")
        .nth(1)
        .unwrap()
        .replace("127.0.0.1:8787", &api_addr.to_string())
        .replace("127.0.0.1:7880", &upstream_addr.to_string());
    std::fs::write(
        directory.join("Caddyfile"),
        format!("{{\n admin off\n auto_https off\n}}\nhttp://127.0.0.1:{port}{signaling}"),
    )
    .unwrap();
    let caddy = std::env::var("WISP_TEST_CADDY").expect("Set WISP_TEST_CADDY");
    let _proxy = ProxyProcess(
        std::process::Command::new(caddy)
            .args(["run", "--adapter", "caddyfile", "--config"])
            .arg(directory.join("Caddyfile"))
            .env("XDG_DATA_HOME", &directory)
            .env("XDG_CONFIG_HOME", &directory)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap(),
        directory,
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    let url = format!("http://127.0.0.1:{port}");
    for _ in 0..100 {
        if client.get(&url).send().await.is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let paths = [
        "/rtc",
        "/rtc/validate",
        "/%72tc",
        "/twirp/livekit.RoomService/ListRooms",
    ];
    for path in paths {
        assert_eq!(
            client
                .get(format!("{url}{path}"))
                .query(&[("access_token", &token)])
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::ACCEPTED,
            "valid media token reaches upstream: {path}"
        );
        assert_eq!(
            client
                .get(format!("{url}{path}"))
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::UNAUTHORIZED
        );
    }
    // Untrusted custom headers cannot replace the query token through Caddy.
    assert_eq!(
        client
            .get(format!("{url}/rtc"))
            .header("X-Wisp-Media-Token", &token)
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    let response = crate::text_tests::request(
        &app,
        "POST",
        "/v1/server/members/moderate",
        actor,
        json!({"user_id":target,"action":"ban"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    for path in paths {
        assert_eq!(
            client
                .get(format!("{url}{path}"))
                .query(&[("access_token", &token)])
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::FORBIDDEN,
            "cached token denied: {path}"
        );
    }
    let backend = signed(
        &state,
        &json!({"iss":state.config.livekit_api_key,"exp":chrono::Utc::now().timestamp()+60,"video":{"roomAdmin":true,"room":"proxy-room"}}),
    );
    assert_eq!(
        client
            .post(format!("{url}/twirp/livekit.RoomService/RemoveParticipant"))
            .bearer_auth(backend)
            .json(&json!({"room":"proxy-room","identity":target}))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::ACCEPTED
    );
    api_task.abort();
    media_task.abort();
}

#[tokio::test]
async fn cached_media_tokens_require_current_server_and_room_membership() {
    let state = AppState::new(test_config()).await.unwrap();
    let id = Uuid::parse_str(TEST_OWNER_ID).unwrap();
    let user = crate::find_user(&state.pool, TEST_OWNER_ID).await.unwrap();
    let room = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO hangouts(id,livekit_room,created_at) VALUES(?,'admission-room',?)")
        .bind(&room)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO hangout_members(hangout_id,user_id,joined_at) VALUES(?,?,?)")
        .bind(&room)
        .bind(TEST_OWNER_ID)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&state.pool)
        .await
        .unwrap();
    let token = crate::issue_livekit_token(
        &state.config,
        &user,
        "admission-room",
        &wisp_protocol::VoiceModeration::default(),
    )
    .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("x-wisp-media-token", token.parse().unwrap());
    assert_eq!(
        authorize(State(state.clone()), headers.clone())
            .await
            .unwrap(),
        StatusCode::NO_CONTENT
    );
    sqlx::query("UPDATE users SET server_member=0 WHERE id=?")
        .bind(TEST_OWNER_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        authorize(State(state.clone()), headers.clone())
            .await
            .unwrap_err()
            .status,
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE users SET server_member=1 WHERE id=?")
        .bind(TEST_OWNER_ID)
        .execute(&state.pool)
        .await
        .unwrap();
    let mut tx = state.pool.begin().await.unwrap();
    crate::leave_active_in_transaction(&mut tx, id)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        authorize(State(state.clone()), headers.clone())
            .await
            .unwrap_err()
            .status,
        StatusCode::FORBIDDEN
    );
    headers.insert(
        "x-wisp-media-token",
        "forged.payload.signature".parse().unwrap(),
    );
    assert_eq!(
        authorize(State(state.clone()), headers)
            .await
            .unwrap_err()
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        authorize(State(state), HeaderMap::new())
            .await
            .unwrap_err()
            .status,
        StatusCode::UNAUTHORIZED
    );
}
