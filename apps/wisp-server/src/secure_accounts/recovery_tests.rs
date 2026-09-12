use super::authentication_tests::{Fixture, existing_proof, fixture, post, signed_in};
use super::*;
use age::secrecy::ExposeSecret;
use wisp_crypto::SecretString;
use wisp_crypto::account_vault::{
    device::ProspectiveDevice,
    envelope::{Envelope, KeyEnvelope},
    operation::{Change, DeviceBinding, Operation, RegistrationTranscript, ResetEffect},
};
const NEW_PASSWORD: &str = "synthetic replacement secure password";

async fn token(fixture: &Fixture) -> String {
    sqlx::query("UPDATE account_recovery_emails SET reset_sent_at=0 WHERE user_id=?")
        .bind(fixture.scope.account.to_string())
        .execute(&fixture.state.pool)
        .await
        .unwrap();
    let before = account_recovery::test_mail_count(&fixture.state).await;
    assert_eq!(
        post(
            &fixture.state,
            "/v2/accounts/password-reset/request",
            json!({"identifier":"nativefixture"}),
            None
        )
        .await
        .0,
        StatusCode::ACCEPTED
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        while account_recovery::test_mail_count(&fixture.state).await == before {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    account_recovery::last_test_token(&fixture.state).await
}
async fn verified_fixture() -> Fixture {
    let mut fixture = fixture().await;
    fixture.state.recovery = account_recovery::captured_mail();
    sqlx::query(
        "INSERT INTO account_recovery_emails(user_id,email) VALUES(?,'synthetic@example.invalid')",
    )
    .bind(fixture.scope.account.to_string())
    .execute(&fixture.state.pool)
    .await
    .unwrap();
    fixture
}

async fn reset_body(fixture: &Fixture, token: &str) -> Value {
    let (client, request) =
        auth::Registration::start(SecretString::from(NEW_PASSWORD.to_owned())).unwrap();
    let operation_id = Uuid::new_v4();
    let generation = Uuid::new_v4();
    let (status,started)=post(&fixture.state,"/v3/auth/reset/start",json!({"operation_id":operation_id,"generation":generation,"token":token,"request":request}),None).await;
    assert_eq!(status, StatusCode::OK, "{started}");
    let account: auth::AccountContext = serde_json::from_value(started["account"].clone()).unwrap();
    let registered = client
        .finish(
            &account,
            started["response"].as_str().unwrap(),
            Some(&fixture.pin),
        )
        .unwrap();
    let registration = RegistrationTranscript {
        account,
        scope: fixture.scope.clone(),
        generation,
        request,
        response: started["response"].as_str().unwrap().into(),
        upload: registered.upload,
    };
    let effect = ResetEffect {
        format: 1,
        scope: fixture.scope.clone(),
        id: operation_id,
        expected_generation: serde_json::from_value(
            started["expected"]["credential_generation"].clone(),
        )
        .unwrap(),
        generation,
        registration_sha256: registration.digest().unwrap(),
    };
    json!({"authorization":started["authorization"],"effect":effect,"token":token,"registration":registration})
}

async fn new_password_unlock(fixture: &Fixture, session: &str) -> auth::LoginResult {
    let attempt = Uuid::new_v4();
    let (client, request) =
        auth::Login::start(SecretString::from(NEW_PASSWORD.to_owned())).unwrap();
    let (status, started) = post(
        &fixture.state,
        "/v3/auth/unlock/start",
        json!({"attempt":attempt,"request":request}),
        Some(session),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{started}");
    let context: auth::LoginContext = serde_json::from_value(started["context"].clone()).unwrap();
    let result = client
        .finish(
            &context,
            started["response"].as_str().unwrap(),
            Some(&fixture.pin),
        )
        .unwrap();
    let (status, finished) = post(
        &fixture.state,
        "/v3/auth/unlock/finish",
        json!({"attempt":attempt,"finalization":result.finalization}),
        Some(session),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{finished}");
    assert_eq!(finished["confirmed"], true);
    assert!(finished.get("grant").is_none());
    result
}

#[tokio::test]
async fn secure_email_enrollment_uses_exact_normalized_target_and_verifies_without_classic_password()
 {
    let mut fixture = fixture().await;
    fixture.state.recovery = account_recovery::captured_mail();
    let (device, session) = signed_in(&fixture).await;
    let state = account_state(&fixture.state, fixture.scope.account)
        .await
        .unwrap();
    let email = "synthetic@example.invalid";
    let operation = Operation {
        format: 1,
        scope: fixture.scope.clone(),
        id: Uuid::new_v4(),
        device: DeviceBinding::Existing { id: device.id() },
        expected: state.expected,
        change: Change::SetRecoveryEmail {
            email_sha256: format!("{:x}", Sha256::digest(email.as_bytes())),
        },
    };
    let (attempt, proof) = existing_proof(&fixture, &session, Some(&operation)).await;
    let (_, granted) = post(
        &fixture.state,
        "/v3/auth/reauth/finish",
        json!({"attempt":attempt,"finalization":proof.finalization}),
        Some(&session),
    )
    .await;
    let mut body =
        json!({"operation":operation,"grant":granted["grant"],"email":"changed@example.invalid"});
    assert_eq!(
        post(
            &fixture.state,
            "/v3/accounts/recovery-email",
            body.clone(),
            Some(&session)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    body["email"] = json!("  SYNTHETIC@example.invalid  ");
    let (status, done) = post(
        &fixture.state,
        "/v3/accounts/recovery-email",
        body.clone(),
        Some(&session),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{done}");
    assert_eq!(
        post(
            &fixture.state,
            "/v3/accounts/recovery-email",
            body,
            Some(&session)
        )
        .await
        .1,
        done
    );
    assert_eq!(account_recovery::test_mail_count(&fixture.state).await, 1);
    let token = account_recovery::last_test_token(&fixture.state).await;
    assert_eq!(
        post(
            &fixture.state,
            "/v2/accounts/recovery-email/verify",
            json!({"token":token}),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        post(
            &fixture.state,
            "/v2/accounts/recovery-email",
            json!({"email":email,"current_password":NEW_PASSWORD}),
            Some(&session)
        )
        .await
        .1["code"],
        "password_unavailable"
    );
    let row = sqlx::query("SELECT email,pending_email FROM account_recovery_emails")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("email"), email);
    assert!(row.get::<Option<String>, _>("pending_email").is_none());
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Whole recovery lifecycle must preserve the original encryption identity.
async fn reset_preserves_locked_backup_devices_and_identity_then_trusted_rewrap_restores_it() {
    let fixture = verified_fixture().await;
    let (device, session) = signed_in(&fixture).await;
    let before = sqlx::query("SELECT wrapper,envelope FROM account_vaults")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    let token = token(&fixture).await;
    let (_, inspection) = post(
        &fixture.state,
        "/v2/accounts/password-reset/inspect",
        json!({"token":token}),
        None,
    )
    .await;
    assert_eq!(inspection["secure_reset_required"], true);
    assert_eq!(
        post(
            &fixture.state,
            "/v2/accounts/password-reset/complete",
            json!({"token":token,"new_password":"synthetic unsafe classic route"}),
            None
        )
        .await
        .1["code"],
        "secure_reset_required"
    );
    let body = reset_body(&fixture, &token).await;
    let (status, done) = post(&fixture.state, "/v3/auth/reset/finish", body.clone(), None).await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["backup_locked"], true);
    assert_eq!(
        post(&fixture.state, "/v3/auth/reset/finish", body.clone(), None)
            .await
            .1,
        done
    );
    let after = sqlx::query("SELECT wrapper,envelope FROM account_vaults")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(
        before.get::<String, _>("wrapper"),
        after.get::<String, _>("wrapper")
    );
    assert_eq!(
        before.get::<String, _>("envelope"),
        after.get::<String, _>("envelope")
    );
    let account = account_state(&fixture.state, fixture.scope.account)
        .await
        .unwrap();
    assert_ne!(
        account.expected.credential_generation,
        account.expected.wrapper_generation
    );
    assert_eq!(
        account.identity.as_ref(),
        Some(&fixture.bundle.identity().unwrap().public())
    );
    let old_login=post(&fixture.state,"/v1/accounts/login",json!({"username":"nativefixture","password":NEW_PASSWORD,"device_name":"Forbidden classic","protocol_version":PROTOCOL_VERSION}),None).await;
    assert_eq!(old_login.0, StatusCode::UNAUTHORIZED);
    let new_device = ProspectiveDevice::generate().unwrap();
    let attempt = Uuid::new_v4();
    let (client, request) =
        auth::Login::start(SecretString::from(NEW_PASSWORD.to_owned())).unwrap();
    let (status,started)=post(&fixture.state,"/v3/auth/login/start",json!({"attempt":attempt,"username":"nativefixture","device":new_device.binding(),"device_name":"Restoring remote device","request":request}),None).await;
    assert_eq!(status, StatusCode::OK);
    let context: auth::LoginContext = serde_json::from_value(started["context"].clone()).unwrap();
    let logged = client
        .finish(
            &context,
            started["response"].as_str().unwrap(),
            Some(&fixture.pin),
        )
        .unwrap();
    assert_eq!(
        post(
            &fixture.state,
            "/v3/auth/login/finish",
            json!({"attempt":attempt,"finalization":logged.finalization}),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let old_wrapper: KeyEnvelope =
        serde_json::from_str(&after.get::<String, _>("wrapper")).unwrap();
    assert!(
        old_wrapper
            .unwrap(
                &fixture.scope,
                context.credential_generation,
                &logged.export_key
            )
            .is_err()
    );
    // Original trusted device remains authenticated and has its retained key.
    assert_eq!(post(&fixture.state,"/v1/sessions",json!({"device_id":device.id(),"device_token":device.token().expose_secret(),"protocol_version":PROTOCOL_VERSION}),None).await.0,StatusCode::OK);
    let unlock = new_password_unlock(&fixture, &session).await;
    let wrapper = KeyEnvelope::wrap(
        fixture.scope.clone(),
        context.credential_generation,
        &unlock.export_key,
        &fixture.key,
    )
    .unwrap();
    let operation = Operation {
        format: 1,
        scope: fixture.scope.clone(),
        id: Uuid::new_v4(),
        device: DeviceBinding::Existing { id: device.id() },
        expected: account.expected,
        change: Change::Rewrap {
            wrapper_sha256: wrapper.digest().unwrap(),
        },
    };
    let proof_attempt = Uuid::new_v4();
    let (proof_client, proof_request) =
        auth::Login::start(SecretString::from(NEW_PASSWORD.to_owned())).unwrap();
    let (_, reauth) = post(
        &fixture.state,
        "/v3/auth/reauth/start",
        json!({"attempt":proof_attempt,"request":proof_request,"operation":operation}),
        Some(&session),
    )
    .await;
    let proof_context: auth::LoginContext =
        serde_json::from_value(reauth["context"].clone()).unwrap();
    let proof = proof_client
        .finish(
            &proof_context,
            reauth["response"].as_str().unwrap(),
            Some(&fixture.pin),
        )
        .unwrap();
    let (_, granted) = post(
        &fixture.state,
        "/v3/auth/reauth/finish",
        json!({"attempt":proof_attempt,"finalization":proof.finalization}),
        Some(&session),
    )
    .await;
    assert_eq!(post(&fixture.state,"/v3/accounts/vault/rewrap",json!({"operation":operation,"signature":operation.sign(&fixture.bundle.identity().unwrap()).unwrap(),"grant":granted["grant"],"wrapper":wrapper}),Some(&session)).await.0,StatusCode::OK);
    let key = wrapper
        .unwrap(
            &fixture.scope,
            context.credential_generation,
            &logged.export_key,
        )
        .unwrap();
    let envelope: Envelope = serde_json::from_str(&after.get::<String, _>("envelope")).unwrap();
    let restored = envelope
        .open(
            &key,
            &fixture.scope,
            &fixture.bundle.identity().unwrap().public(),
            &envelope.manifest.checkpoint().unwrap(),
        )
        .unwrap();
    assert_eq!(
        restored.encode_private().unwrap().as_slice(),
        fixture.bundle.encode_private().unwrap().as_slice()
    );
    // A late reset retry reports its historical outcome and never restores the
    // old locked wrapper or changes any newer account state.
    assert_eq!(
        post(&fixture.state, "/v3/auth/reset/finish", body, None)
            .await
            .1,
        done
    );
    let account = account_state(&fixture.state, fixture.scope.account)
        .await
        .unwrap();
    assert_eq!(
        account.expected.credential_generation,
        account.expected.wrapper_generation
    );
}

#[tokio::test]
async fn new_email_authorization_renews_same_reset_effect_and_receipts_require_scoped_proof() {
    let fixture = verified_fixture().await;
    let old_token = token(&fixture).await;
    let body = reset_body(&fixture, &old_token).await;
    let effect: ResetEffect = serde_json::from_value(body["effect"].clone()).unwrap();
    let next_token = token(&fixture).await;
    assert_ne!(old_token, next_token);
    let (status,renewed)=post(&fixture.state,"/v3/auth/reset/start",json!({"operation_id":effect.id,"generation":effect.generation,"token":next_token,"request":body["registration"]["request"]}),None).await;
    assert_eq!(status, StatusCode::OK, "{renewed}");
    assert_eq!(renewed["response"], body["registration"]["response"]);
    assert_ne!(renewed["authorization"], body["authorization"]);
    assert_ne!(
        post(&fixture.state, "/v3/auth/reset/finish", body.clone(), None)
            .await
            .0,
        StatusCode::OK
    );
    let mut retry = body.clone();
    retry["token"] = json!(next_token);
    retry["authorization"] = renewed["authorization"].clone();
    assert_eq!(
        post(&fixture.state, "/v3/auth/reset/finish", retry.clone(), None)
            .await
            .0,
        StatusCode::OK
    );
    let mut forged = retry.clone();
    forged["authorization"] = json!("x".repeat(43));
    assert_eq!(
        post(&fixture.state, "/v3/auth/reset/finish", forged, None)
            .await
            .1["code"],
        "invalid_token"
    );
    assert_ne!(post(&fixture.state,"/v3/auth/reset/status",json!({"token":next_token,"operation_id":effect.id,"effect_sha256":effect.digest().unwrap()}),None).await.0,StatusCode::OK);
    let fresh_token = token(&fixture).await;
    let (status,receipt)=post(&fixture.state,"/v3/auth/reset/status",json!({"token":fresh_token,"operation_id":effect.id,"effect_sha256":effect.digest().unwrap()}),None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(receipt["scope"], json!(fixture.scope));
    assert_eq!(receipt["receipt"]["result"]["completed"], true);
    assert_eq!(
        receipt["credential_generation"],
        effect.generation.to_string()
    );
    let (_, missing) = post(
        &fixture.state,
        "/v3/auth/reset/status",
        json!({"token":fresh_token,"operation_id":Uuid::new_v4(),"effect_sha256":"c".repeat(64)}),
        None,
    )
    .await;
    assert!(missing["receipt"].is_null());
    assert_eq!(
        missing["credential_generation"],
        effect.generation.to_string()
    );
    let stored: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=?")
        .bind(fixture.scope.account.to_string())
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert!(stored.is_none());
}

#[tokio::test]
async fn concurrent_resets_consume_one_token_and_consumed_proofs_cannot_change_credentials() {
    let fixture = verified_fixture().await;
    let token = token(&fixture).await;
    let left = reset_body(&fixture, &token).await;
    let right = reset_body(&fixture, &token).await;
    let (a, b) = tokio::join!(
        post(&fixture.state, "/v3/auth/reset/finish", left, None),
        post(&fixture.state, "/v3/auth/reset/finish", right, None)
    );
    assert_eq!(
        usize::from(a.0 == StatusCode::OK) + usize::from(b.0 == StatusCode::OK),
        1
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM secure_generation_history")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
    let old = account_state(&fixture.state, fixture.scope.account)
        .await
        .unwrap()
        .expected;
    // A consumed token cannot authorize any new registration effect.
    assert_eq!(post(&fixture.state,"/v3/auth/reset/start",json!({"operation_id":Uuid::new_v4(),"generation":Uuid::new_v4(),"token":token,"request":"invalid"}),None).await.1["code"],"invalid_token");
    assert_eq!(
        account_state(&fixture.state, fixture.scope.account)
            .await
            .unwrap()
            .expected,
        old
    );
}
