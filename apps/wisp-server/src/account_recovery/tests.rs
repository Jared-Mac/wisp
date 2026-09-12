use super::*;
use crate::{tests::test_config, text_tests::request};

async fn fixture() -> (AppState, HeaderMap, DeviceSessionRequest) {
    let mut config = test_config();
    config.allow_dev_sessions = false;
    let mut state = AppState::new(config).await.unwrap();
    state.recovery = Arc::new(Recovery {
        mailer: Mailer::Capture(Mutex::default()),
        ..Recovery::default()
    });
    let credential = bootstrap_device(
        State(state.clone()),
        Json(BootstrapDeviceRequest {
            bootstrap_token: "test-bootstrap-token".into(),
            username: "owner".into(),
            display_name: "Owner".into(),
            password: "old password for test".into(),
            device_name: "Fixture".into(),
            protocol_version: PROTOCOL_VERSION,
        }),
    )
    .await
    .unwrap()
    .0;
    let credentials = DeviceSessionRequest {
        device_id: credential.device_id,
        device_token: credential.device_token,
        protocol_version: PROTOCOL_VERSION,
    };
    let session = device_session(State(state.clone()), Json(credentials.clone()))
        .await
        .unwrap()
        .0;
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        format!("Bearer {}", session.token).parse().unwrap(),
    );
    (state, headers, credentials)
}

async fn mailed_token(state: &AppState) -> String {
    let Mailer::Capture(messages) = &state.recovery.mailer else {
        panic!("test transport required")
    };
    let messages = messages.lock().await;
    let (_, body) = messages.last().expect("one email");
    body.split("#token=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .into()
}

async fn enroll_fixture(state: &AppState, headers: &HeaderMap) -> String {
    let _ = enroll(
        State(state.clone()),
        headers.clone(),
        Json(Enroll {
            email: "owner@example.org".into(),
            current_password: "old password for test".into(),
        }),
    )
    .await
    .unwrap();
    mailed_token(state).await
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn verification_is_required_and_reset_preserves_devices_and_identity() {
    let (state, headers, credentials) = fixture().await;
    let user = authenticate_headers(&state, &headers).await.unwrap();
    let before = sqlx::query("SELECT id,username,display_name FROM users WHERE id=?")
        .bind(user.to_string())
        .fetch_one(&state.pool)
        .await
        .unwrap();
    let token = enroll_fixture(&state, &headers).await;
    let view = status(State(state.clone()), headers.clone())
        .await
        .unwrap()
        .0;
    assert_eq!(view["verified"], false);
    assert_eq!(view["email"], Value::Null);
    reset_mail(&state, "owner").await.unwrap();
    assert_eq!(
        mailed_token(&state).await,
        token,
        "pending email must not recover"
    );
    assert!(
        inspect(
            State(state.clone()),
            HeaderMap::new(),
            Json(Token {
                token: token.clone()
            })
        )
        .await
        .is_err(),
        "verify token is not a reset token"
    );
    let _ = verify(
        State(state.clone()),
        HeaderMap::new(),
        Json(Token {
            token: token.clone(),
        }),
    )
    .await
    .unwrap();
    assert!(
        verify(
            State(state.clone()),
            HeaderMap::new(),
            Json(Token { token })
        )
        .await
        .is_err(),
        "verification is single use"
    );
    assert_eq!(
        status(State(state.clone()), headers.clone())
            .await
            .unwrap()
            .0["verified"],
        true
    );
    reset_mail(&state, "owner").await.unwrap();
    let token = mailed_token(&state).await;
    let stored: String = sqlx::query_scalar("SELECT token_hash FROM account_recovery_tokens")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_ne!(stored, token);
    assert_eq!(stored, token_hash(&token));
    assert!(
        inspect(
            State(state.clone()),
            HeaderMap::new(),
            Json(Token {
                token: token.clone()
            })
        )
        .await
        .is_ok()
    );
    let a = complete(
        State(state.clone()),
        HeaderMap::new(),
        Json(Complete {
            token: token.clone(),
            new_password: "new password for test".into(),
        }),
    );
    let b = complete(
        State(state.clone()),
        HeaderMap::new(),
        Json(Complete {
            token: token.clone(),
            new_password: "new password for test".into(),
        }),
    );
    let (a, b) = tokio::join!(a, b);
    assert_ne!(
        a.is_ok(),
        b.is_ok(),
        "concurrent consumption succeeds exactly once"
    );
    assert!(
        inspect(
            State(state.clone()),
            HeaderMap::new(),
            Json(Token { token })
        )
        .await
        .is_err()
    );
    let encoded: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=?")
        .bind(user.to_string())
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert!(
        password_matches("new password for test".into(), encoded.clone())
            .await
            .unwrap()
    );
    assert!(
        !password_matches("old password for test".into(), encoded)
            .await
            .unwrap()
    );
    assert_eq!(authenticate_headers(&state, &headers).await.unwrap(), user);
    assert!(
        device_session(State(state.clone()), Json(credentials))
            .await
            .is_ok()
    );
    let after = sqlx::query("SELECT id,username,display_name FROM users WHERE id=?")
        .bind(user.to_string())
        .fetch_one(&state.pool)
        .await
        .unwrap();
    for key in ["id", "username", "display_name"] {
        assert_eq!(before.get::<String, _>(key), after.get::<String, _>(key));
    }
}

#[tokio::test]
async fn enrollment_requires_password_and_tokens_expire_or_follow_password_changes() {
    let (state, headers, _) = fixture().await;
    let wrong = enroll(
        State(state.clone()),
        headers.clone(),
        Json(Enroll {
            email: "owner@example.org".into(),
            current_password: "wrong".into(),
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(wrong.code, "current_password_incorrect");
    assert!(
        enroll(
            State(state.clone()),
            HeaderMap::new(),
            Json(Enroll {
                email: "owner@example.org".into(),
                current_password: "old password for test".into()
            })
        )
        .await
        .is_err()
    );
    let token = enroll_fixture(&state, &headers).await;
    sqlx::query("UPDATE account_recovery_tokens SET expires_at=0")
        .execute(&state.pool)
        .await
        .unwrap();
    assert!(
        verify(
            State(state.clone()),
            HeaderMap::new(),
            Json(Token { token })
        )
        .await
        .is_err()
    );
    sqlx::query("UPDATE account_recovery_emails SET verification_sent_at=0")
        .execute(&state.pool)
        .await
        .unwrap();
    let token = enroll_fixture(&state, &headers).await;
    let _ = crate::account_profile::change_password(
        State(state.clone()),
        headers.clone(),
        Json(wisp_protocol::ChangePasswordRequest {
            current_password: "old password for test".into(),
            new_password: "different password for test".into(),
        }),
    )
    .await
    .unwrap();
    assert!(
        verify(
            State(state.clone()),
            HeaderMap::new(),
            Json(Token { token })
        )
        .await
        .is_err()
    );
    assert!(
        !status(State(state), headers).await.unwrap().0["verified"]
            .as_bool()
            .unwrap()
    );
}

#[tokio::test]
async fn public_requests_do_not_enumerate_and_throttle_unknown_accounts() {
    let (state, _, _) = fixture().await;
    let app = router(state.clone());
    // Exercise the actual membership middleware without authentication.
    for name in ["owner", "missing", "nobody@example.org"] {
        let response = request(
            &app,
            "POST",
            "/v2/accounts/password-reset/request",
            "",
            json!({"identifier":name}),
        )
        .await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            json!({"ok":true})
        );
    }
    for _ in 0..5 {
        state
            .recovery
            .rate(&HeaderMap::new(), "request", 8, 120)
            .await
            .unwrap();
    }
    assert_eq!(
        state
            .recovery
            .rate(&HeaderMap::new(), "request", 8, 120)
            .await
            .unwrap_err()
            .status,
        StatusCode::TOO_MANY_REQUESTS
    );
    let disabled = AppState::new(test_config()).await.unwrap();
    assert_eq!(
        request_reset(
            State(disabled),
            HeaderMap::new(),
            Json(ResetRequest {
                identifier: "missing".into()
            })
        )
        .await
        .unwrap_err()
        .code,
        "mail_unavailable"
    );
}

#[test]
fn addresses_and_tokens_reject_header_injection_and_unbounded_input() {
    assert_eq!(
        normalize_email(" Person@Example.org ").unwrap(),
        "person@example.org"
    );
    for email in [
        "x@example.org\r\nBcc: x@example.org",
        "Name <x@example.org>",
        "x@example.org,y@example.org",
        "",
    ] {
        assert!(normalize_email(email).is_err());
    }
    assert!(checked_token("anything").is_err());
    assert!(checked_token(&new_token()).is_ok());
}

#[tokio::test]
async fn local_smtp_uses_aligned_sender_and_does_not_retry_uncertain_delivery() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (read, mut write) = stream.into_split();
        let mut read = BufReader::new(read);
        write
            .write_all(b"220 test.example ESMTP\r\n")
            .await
            .unwrap();
        let mut line = String::new();
        let mut data = false;
        let mut body = String::new();
        let mut envelope = Vec::new();
        loop {
            line.clear();
            if read.read_line(&mut line).await.unwrap() == 0 {
                break;
            }
            if data {
                if line == ".\r\n" {
                    break;
                } // Drop after DATA: acceptance is uncertain.
                body.push_str(&line);
            } else if line.starts_with("EHLO") {
                write.write_all(b"250 test.example\r\n").await.unwrap();
            } else if line.starts_with("MAIL FROM:") || line.starts_with("RCPT TO:") {
                envelope.push(line.trim().to_string());
                write.write_all(b"250 OK\r\n").await.unwrap();
            } else if line == "DATA\r\n" {
                data = true;
                write.write_all(b"354 End with dot\r\n").await.unwrap();
            } else {
                panic!("unexpected SMTP command");
            }
        }
        drop(write);
        drop(read);
        assert!(
            tokio::time::timeout(Duration::from_millis(80), listener.accept())
                .await
                .is_err(),
            "no automatic duplicate submission"
        );
        (body, envelope)
    });
    let recovery = Recovery {
        mailer: Mailer::Local(
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("127.0.0.1")
                .port(port)
                .timeout(Some(Duration::from_secs(1)))
                .build(),
        ),
        ..Recovery::default()
    };
    let error = recovery
        .send("test@example.org", "verify", &new_token())
        .await
        .unwrap_err();
    assert_eq!(error.code, "mail_unavailable");
    let (body, envelope) = server.await.unwrap();
    assert!(envelope.iter().any(|s| s == "MAIL FROM:<support@wisp.you>"));
    assert!(envelope.iter().any(|s| s == "RCPT TO:<test@example.org>"));
    assert!(body.contains("From: Wisp <support@wisp.you>"));
    assert!(body.contains("Subject: Verify your Wisp recovery email"));
    assert!(!error.message.contains("test@example.org"));
}
