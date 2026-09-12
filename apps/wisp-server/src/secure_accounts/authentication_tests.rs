use super::*;
use age::secrecy::ExposeSecret;
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use tower::ServiceExt;
use wisp_crypto::account_vault::{
    bundle::{Bundle, TrustState},
    device::ProspectiveDevice,
    envelope::{Envelope, KeyEnvelope, VaultKey},
    operation::{Change, DeviceBinding, Operation, credential_identifier},
};
use wisp_crypto::{Identity, SecretString};

const PASSWORD: &str = "synthetic native authentication password";

pub(super) struct Fixture {
    pub(super) state: AppState,
    pub(super) scope: Scope,
    pub(super) pin: auth::ServerPin,
    pub(super) bundle: Bundle,
    pub(super) key: VaultKey,
}

pub(super) async fn fixture() -> Fixture {
    let mut config = super::super::tests::test_config();
    config.allow_dev_sessions = false;
    let state = AppState::new(config).await.unwrap();
    let (origin, network) = service(&state).await.unwrap();
    let scope = Scope {
        origin,
        network,
        account: Uuid::new_v4(),
    };
    let identity = Identity::generate().unwrap();
    let generation = Uuid::new_v4();
    let context = auth::AccountContext {
        origin: scope.origin.clone(),
        network,
        username: "nativefixture".into(),
    };
    let server = load_native(&state.pool).await.unwrap();
    let (registration, request) =
        auth::Registration::start(SecretString::from(PASSWORD.to_owned())).unwrap();
    let response = server
        .registration_response(
            &credential_identifier(&scope, generation).unwrap(),
            &request,
        )
        .unwrap();
    let registered = registration.finish(&context, &response, None).unwrap();
    let record = auth::Server::registration_record(&registered.upload).unwrap();
    let mut trust = TrustState::default();
    trust.pins.insert(scope.account, identity.public());
    let bundle = Bundle::new(scope.clone(), identity.recovery_key().unwrap(), None, trust).unwrap();
    let key = VaultKey::generate().unwrap();
    let wrapper =
        KeyEnvelope::wrap(scope.clone(), generation, &registered.export_key, &key).unwrap();
    let envelope = Envelope::seal(&bundle, &key, None).unwrap();
    let checkpoint = envelope.manifest.checkpoint().unwrap();
    let setup: String = sqlx::query_scalar("SELECT digest FROM secure_auth_setup WHERE id=1")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    let mut tx = state.pool.begin().await.unwrap();
    sqlx::query("INSERT INTO users(id,display_name,username,password_hash,server_member) VALUES(?,'Synthetic user','NativeFixture',NULL,0)").bind(scope.account.to_string()).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO chat_identities(user_id,public_identity) VALUES(?,?)")
        .bind(scope.account.to_string())
        .bind(serde_json::to_string(&identity.public()).unwrap())
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO secure_credentials(user_id,generation,password_file,setup_digest,updated_at) VALUES(?,?,?,?,0)").bind(scope.account.to_string()).bind(generation.to_string()).bind(record.as_slice()).bind(setup).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO account_vault_manifests(user_id,revision,digest,manifest,created_at) VALUES(?,1,?,?,0)").bind(scope.account.to_string()).bind(&checkpoint.sha256).bind(serde_json::to_string(&envelope.manifest).unwrap()).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO account_vaults(user_id,wrapper_generation,wrapper,revision,digest,envelope,updated_at) VALUES(?,?,?,1,?,?,0)").bind(scope.account.to_string()).bind(generation.to_string()).bind(serde_json::to_string(&wrapper).unwrap()).bind(&checkpoint.sha256).bind(serde_json::to_string(&envelope).unwrap()).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    Fixture {
        state,
        scope,
        pin: registered.server_pin,
        bundle,
        key,
    }
}

pub(super) async fn post(
    state: &AppState,
    path: &str,
    body: Value,
    session: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::post(path).header("content-type", "application/json");
    if let Some(session) = session {
        request = request.header("authorization", format!("Bearer {session}"));
    }
    let response = router(state.clone())
        .oneshot(
            request
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    if path.starts_with("/v3/") {
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    let bytes = to_bytes(response.into_body(), 1_000_000).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn login_start(
    fixture: &Fixture,
    password: &str,
    username: &str,
    device: &ProspectiveDevice,
) -> (Uuid, auth::Login, Value) {
    let attempt = Uuid::new_v4();
    let (client, request) = auth::Login::start(SecretString::from(password.to_owned())).unwrap();
    let (status,response) = post(&fixture.state,"/v3/auth/login/start",json!({"attempt":attempt,"username":username,"device":device.binding(),"device_name":"Synthetic device","request":request}),None).await;
    assert_eq!(status, StatusCode::OK);
    (attempt, client, response)
}

async fn credential_session(fixture: &Fixture, device: &ProspectiveDevice) -> (StatusCode, Value) {
    post(&fixture.state,"/v1/sessions",json!({"device_id":device.id(),"device_token":device.token().expose_secret(),"protocol_version":PROTOCOL_VERSION}),None).await
}

pub(super) async fn signed_in(fixture: &Fixture) -> (ProspectiveDevice, String) {
    let device = ProspectiveDevice::generate().unwrap();
    let (attempt, client, response) =
        login_start(fixture, PASSWORD, "nativefixture", &device).await;
    let context: auth::LoginContext = serde_json::from_value(response["context"].clone()).unwrap();
    let proof = client
        .finish(
            &context,
            response["response"].as_str().unwrap(),
            Some(&fixture.pin),
        )
        .unwrap();
    assert_eq!(
        post(
            &fixture.state,
            "/v3/auth/login/finish",
            json!({"attempt":attempt,"finalization":proof.finalization}),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, response) = credential_session(fixture, &device).await;
    assert_eq!(status, StatusCode::OK);
    (device, response["token"].as_str().unwrap().to_owned())
}

pub(super) async fn existing_proof(
    fixture: &Fixture,
    session: &str,
    operation: Option<&Operation>,
) -> (Uuid, auth::LoginResult) {
    let attempt = Uuid::new_v4();
    let (client, request) = auth::Login::start(SecretString::from(PASSWORD.to_owned())).unwrap();
    let (path, body) = if let Some(operation) = operation {
        (
            "/v3/auth/reauth/start",
            json!({"attempt":attempt,"request":request,"operation":operation}),
        )
    } else {
        (
            "/v3/auth/unlock/start",
            json!({"attempt":attempt,"request":request}),
        )
    };
    let (status, response) = post(&fixture.state, path, body, Some(session)).await;
    assert_eq!(status, StatusCode::OK);
    let context: auth::LoginContext = serde_json::from_value(response["context"].clone()).unwrap();
    let proof = client
        .finish(
            &context,
            response["response"].as_str().unwrap(),
            Some(&fixture.pin),
        )
        .unwrap();
    (attempt, proof)
}

#[tokio::test]
async fn unlock_cannot_issue_grants_and_reauthentication_rechecks_exact_current_vault_state() {
    let fixture = fixture().await;
    let (device, session) = signed_in(&fixture).await;
    let (attempt, proof) = existing_proof(&fixture, &session, None).await;
    let body = json!({"attempt":attempt,"finalization":proof.finalization});
    assert_eq!(
        post(
            &fixture.state,
            "/v3/auth/reauth/finish",
            body.clone(),
            Some(&session)
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, unlocked) = post(
        &fixture.state,
        "/v3/auth/unlock/finish",
        body,
        Some(&session),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(unlocked["confirmed"], true);
    assert!(unlocked.get("grant").is_none());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM secure_reauth_grants")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);

    let mut tx = fixture.state.pool.begin().await.unwrap();
    let expected = current_precondition(&mut tx, fixture.scope.account)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let operation = Operation {
        format: 1,
        scope: fixture.scope.clone(),
        id: Uuid::new_v4(),
        device: DeviceBinding::Existing { id: device.id() },
        expected: expected.clone(),
        change: Change::SetRecoveryEmail {
            email_sha256: "a".repeat(64),
        },
    };
    let (attempt, proof) = existing_proof(&fixture, &session, Some(&operation)).await;
    let (status, granted) = post(
        &fixture.state,
        "/v3/auth/reauth/finish",
        json!({"attempt":attempt,"finalization":proof.finalization}),
        Some(&session),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(granted["operation_sha256"], operation.digest().unwrap());
    let stored: String = sqlx::query_scalar("SELECT token_hash FROM secure_reauth_grants")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    let matches_hash = stored == token_hash(granted["grant"].as_str().unwrap());
    assert!(matches_hash, "Grant hash mismatch");

    let (attempt, proof) = existing_proof(&fixture, &session, Some(&operation)).await;
    let next = Envelope::seal(&fixture.bundle, &fixture.key, expected.vault).unwrap();
    let checkpoint = next.manifest.checkpoint().unwrap();
    let mut tx = fixture.state.pool.begin().await.unwrap();
    sqlx::query("INSERT INTO account_vault_manifests(user_id,revision,digest,manifest,created_at) VALUES(?,?,?,?,0)").bind(fixture.scope.account.to_string()).bind(i64::try_from(checkpoint.revision).unwrap()).bind(&checkpoint.sha256).bind(serde_json::to_string(&next.manifest).unwrap()).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE account_vaults SET revision=?,digest=?,envelope=? WHERE user_id=?")
        .bind(i64::try_from(checkpoint.revision).unwrap())
        .bind(&checkpoint.sha256)
        .bind(serde_json::to_string(&next).unwrap())
        .bind(fixture.scope.account.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let (status, response) = post(
        &fixture.state,
        "/v3/auth/reauth/finish",
        json!({"attempt":attempt,"finalization":proof.finalization}),
        Some(&session),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(response["code"], "account_state_changed");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM secure_reauth_grants")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let devices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM devices")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(devices, 1);
}

#[tokio::test]
async fn login_activates_only_the_staged_device_once_and_receipt_never_reactivates_revoked_device()
{
    let fixture = fixture().await;
    let device = ProspectiveDevice::generate().unwrap();
    let (attempt, client, response) =
        login_start(&fixture, PASSWORD, "NATIVEFIXTURE", &device).await;
    assert_eq!(
        credential_session(&fixture, &device).await.0,
        StatusCode::UNAUTHORIZED
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM devices")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let context: auth::LoginContext = serde_json::from_value(response["context"].clone()).unwrap();
    let proof = client
        .finish(
            &context,
            response["response"].as_str().unwrap(),
            Some(&fixture.pin),
        )
        .unwrap();
    let body = json!({"attempt":attempt,"finalization":proof.finalization});
    let (status, completed) =
        post(&fixture.state, "/v3/auth/login/finish", body.clone(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(completed["user"]["id"], fixture.scope.account.to_string());
    assert_eq!(completed["device_id"], device.id().to_string());
    assert!(completed.get("device_token").is_none());
    assert_eq!(
        credential_session(&fixture, &device).await.0,
        StatusCode::OK
    );
    let (status, repeated) =
        post(&fixture.state, "/v3/auth/login/finish", body.clone(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(completed, repeated);
    sqlx::query("UPDATE devices SET revoked_at=? WHERE id=?")
        .bind(Utc::now().to_rfc3339())
        .bind(device.id().to_string())
        .execute(&fixture.state.pool)
        .await
        .unwrap();
    let (status, _) = post(&fixture.state, "/v3/auth/login/finish", body, None).await;
    assert_eq!(status, StatusCode::OK);
    let revoked: Option<String> = sqlx::query_scalar("SELECT revoked_at FROM devices WHERE id=?")
        .bind(device.id().to_string())
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert!(revoked.is_some());
    assert_eq!(
        credential_session(&fixture, &device).await.0,
        StatusCode::UNAUTHORIZED
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM devices")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn wrong_password_unknown_user_expired_or_changed_generation_never_activate() {
    let fixture = fixture().await;
    for username in ["nativefixture", "unknownfixture"] {
        let device = ProspectiveDevice::generate().unwrap();
        let (_, client, response) =
            login_start(&fixture, "incorrect synthetic password", username, &device).await;
        assert!(response.get("user").is_none());
        assert!(response.get("scope").is_none());
        let context: auth::LoginContext =
            serde_json::from_value(response["context"].clone()).unwrap();
        assert!(
            client
                .finish(&context, response["response"].as_str().unwrap(), None)
                .is_err()
        );
    }
    for expired in [true, false] {
        let device = ProspectiveDevice::generate().unwrap();
        let (attempt, client, response) =
            login_start(&fixture, PASSWORD, "nativefixture", &device).await;
        let context: auth::LoginContext =
            serde_json::from_value(response["context"].clone()).unwrap();
        let proof = client
            .finish(
                &context,
                response["response"].as_str().unwrap(),
                Some(&fixture.pin),
            )
            .unwrap();
        if expired {
            sqlx::query("UPDATE secure_auth_attempts SET expires_at=0 WHERE id=?")
                .bind(attempt.to_string())
                .execute(&fixture.state.pool)
                .await
                .unwrap();
        } else {
            sqlx::query("UPDATE secure_credentials SET generation=? WHERE user_id=?")
                .bind(Uuid::new_v4().to_string())
                .bind(fixture.scope.account.to_string())
                .execute(&fixture.state.pool)
                .await
                .unwrap();
        }
        let (status, _) = post(
            &fixture.state,
            "/v3/auth/login/finish",
            json!({"attempt":attempt,"finalization":proof.finalization}),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM devices")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
