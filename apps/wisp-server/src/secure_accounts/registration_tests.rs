use super::authentication_tests::{existing_proof, fixture, post, signed_in};
use super::*;
use age::secrecy::ExposeSecret;
use wisp_crypto::account_vault::{
    bundle::{Bundle, TrustState},
    device::ProspectiveDevice,
    envelope::{Envelope, KeyEnvelope, VaultKey},
    operation::{
        Change, DeviceBinding, Operation, RegistrationTranscript, SignupMetadata,
        SignupStartBinding,
    },
};
use wisp_crypto::{Identity, SecretString};
const PASSWORD: &str = "synthetic enrollment password";

struct Signup {
    binding: SignupStartBinding,
    identity: Identity,
    device: ProspectiveDevice,
    key: VaultKey,
    bundle: Bundle,
    client: Option<auth::Registration>,
    request: String,
}
impl Signup {
    async fn new(state: &AppState) -> Self {
        let (origin, network) = service(state).await.unwrap();
        let scope = Scope {
            origin,
            network,
            account: Uuid::new_v4(),
        };
        let identity = Identity::generate().unwrap();
        let device = ProspectiveDevice::generate().unwrap();
        let (client, request) =
            auth::Registration::start(SecretString::from(PASSWORD.to_owned())).unwrap();
        let binding = SignupStartBinding {
            format: 1,
            scope: scope.clone(),
            id: Uuid::new_v4(),
            generation: Uuid::new_v4(),
            signup: SignupMetadata {
                username: "newsecureuser".into(),
                display_name: "New secure user".into(),
                device_name: "Synthetic signup device".into(),
            },
            device: device.binding(),
            identity: identity.public(),
            request_sha256: auth::registration_request_digest(&request).unwrap(),
            previous_binding_sha256: None,
        };
        let mut trust = TrustState::default();
        trust.pins.insert(scope.account, identity.public());
        let bundle = Bundle::new(scope, identity.recovery_key().unwrap(), None, trust).unwrap();
        Self {
            binding,
            identity,
            device,
            key: VaultKey::generate().unwrap(),
            bundle,
            client: Some(client),
            request,
        }
    }
    fn start_body(&self) -> Value {
        json!({"operation_id":self.binding.id,"scope":self.binding.scope,"generation":self.binding.generation,
            "username":self.binding.signup.username,"display_name":self.binding.signup.display_name,"device":self.binding.device,
            "device_name":self.binding.signup.device_name,"request":self.request,"identity":self.binding.identity,
            "signature":self.binding.sign(&self.identity).unwrap(),"previous_binding_sha256":self.binding.previous_binding_sha256})
    }
    fn replace(&mut self) {
        self.binding.previous_binding_sha256 = Some(self.binding.digest().unwrap());
        let (client, request) =
            auth::Registration::start(SecretString::from(PASSWORD.to_owned())).unwrap();
        self.client = Some(client);
        self.request = request;
        self.binding.request_sha256 = auth::registration_request_digest(&self.request).unwrap();
    }
    fn finish_body(&mut self, response: &Value) -> Value {
        let account: auth::AccountContext =
            serde_json::from_value(response["account"].clone()).unwrap();
        let result = self
            .client
            .take()
            .unwrap()
            .finish(&account, response["response"].as_str().unwrap(), None)
            .unwrap();
        let transcript = RegistrationTranscript {
            account,
            scope: self.binding.scope.clone(),
            generation: self.binding.generation,
            request: self.request.clone(),
            response: response["response"].as_str().unwrap().into(),
            upload: result.upload,
        };
        let wrapper = KeyEnvelope::wrap(
            self.binding.scope.clone(),
            self.binding.generation,
            &result.export_key,
            &self.key,
        )
        .unwrap();
        let vault = Envelope::seal(&self.bundle, &self.key, None).unwrap();
        let operation = Operation {
            format: 1,
            scope: self.binding.scope.clone(),
            id: self.binding.id,
            device: self.binding.device.clone(),
            expected: Precondition::default(),
            change: Change::Enroll {
                generation: self.binding.generation,
                registration_sha256: transcript.digest().unwrap(),
                wrapper_sha256: wrapper.digest().unwrap(),
                vault: vault.manifest.checkpoint().unwrap(),
                signup: Some(self.binding.signup.clone()),
            },
        };
        json!({"authorization":response["authorization"],"signature":operation.sign(&self.identity).unwrap(),"operation":operation,"registration":transcript,"wrapper":wrapper,"vault":vault})
    }
}
async fn empty() -> AppState {
    let mut config = super::super::tests::test_config();
    config.allow_dev_sessions = false;
    AppState::new(config).await.unwrap()
}
async fn session_for(state: &AppState, device: &ProspectiveDevice) -> (StatusCode, Value) {
    post(state,"/v1/sessions",json!({"device_id":device.id(),"device_token":device.token().expose_secret(),"protocol_version":PROTOCOL_VERSION}),None).await
}

#[tokio::test]
async fn signup_start_retry_survives_reload_and_finish_is_atomic_idempotent_without_membership() {
    let state = empty().await;
    let mut signup = Signup::new(&state).await;
    let (status, started) =
        post(&state, "/v3/auth/register/start", signup.start_body(), None).await;
    assert_eq!(status, StatusCode::OK, "{started}");
    assert_eq!(
        session_for(&state, &signup.device).await.0,
        StatusCode::UNAUTHORIZED
    );
    // Reinitialize persisted server state; each route reloads it (no memory ticket cache).
    initialize(&state.pool).await.unwrap();
    let (_, retried) = post(&state, "/v3/auth/register/start", signup.start_body(), None).await;
    assert_eq!(started, retried);
    let rows = sqlx::query("SELECT state,authorization_hash FROM secure_auth_attempts")
        .fetch_all(&state.pool)
        .await
        .unwrap();
    let stored = String::from_utf8(rows[0].get("state")).unwrap();
    assert!(!stored.contains(PASSWORD));
    assert!(!stored.contains(started["authorization"].as_str().unwrap()));
    assert_eq!(
        rows[0].get::<String, _>("authorization_hash"),
        token_hash(started["authorization"].as_str().unwrap())
    );
    let body = signup.finish_body(&started);
    let mut tampered = body.clone();
    tampered["registration"]["request"] = json!("invalid");
    assert_eq!(
        post(&state, "/v3/auth/register/finish", tampered, None)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        session_for(&state, &signup.device).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, finished) = post(&state, "/v3/auth/register/finish", body.clone(), None).await;
    assert_eq!(status, StatusCode::OK, "{finished}");
    assert_eq!(
        post(&state, "/v3/auth/register/finish", body.clone(), None)
            .await
            .1,
        finished
    );
    assert_eq!(session_for(&state, &signup.device).await.0, StatusCode::OK);
    let row = sqlx::query("SELECT password_hash,server_member,public_handle FROM users WHERE id=?")
        .bind(signup.binding.scope.account.to_string())
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert!(row.get::<Option<String>, _>("password_hash").is_none());
    assert_eq!(row.get::<i64, _>("server_member"), 0);
    assert_eq!(row.get::<String, _>("public_handle"), "newsecureuser");
    sqlx::query("UPDATE devices SET revoked_at=? WHERE id=?")
        .bind(Utc::now().to_rfc3339())
        .bind(signup.device.id().to_string())
        .execute(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        post(&state, "/v3/auth/register/finish", body, None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        session_for(&state, &signup.device).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(&state, "/v3/auth/register/start", signup.start_body(), None)
            .await
            .1["code"],
        "operation_committed"
    );
}

#[tokio::test]
async fn signed_signup_replacement_cas_rejects_delayed_prior_replay_and_competitors() {
    let state = empty().await;
    let mut signup = Signup::new(&state).await;
    let original = signup.start_body();
    let mut tampered = original.clone();
    tampered["display_name"] = json!("Tampered");
    assert_eq!(
        post(&state, "/v3/auth/register/start", tampered, None)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let (_, started) = post(&state, "/v3/auth/register/start", original.clone(), None).await;
    let old_finish = signup.finish_body(&started);
    signup.replace();
    let first = signup.start_body();
    let mut competing = first.clone();
    let (_, request) = auth::Registration::start(SecretString::from(PASSWORD.to_owned())).unwrap();
    let mut competing_binding = signup.binding.clone();
    competing_binding.request_sha256 = auth::registration_request_digest(&request).unwrap();
    competing["request"] = json!(request);
    competing["signature"] = json!(competing_binding.sign(&signup.identity).unwrap());
    let (left, right) = tokio::join!(
        post(&state, "/v3/auth/register/start", first.clone(), None),
        post(&state, "/v3/auth/register/start", competing.clone(), None)
    );
    assert_eq!(
        usize::from(left.0 == StatusCode::OK) + usize::from(right.0 == StatusCode::OK),
        1
    );
    let (winner, response) = if left.0 == StatusCode::OK {
        (first, left.1)
    } else {
        (competing, right.1)
    };
    assert_eq!(
        post(&state, "/v3/auth/register/start", winner, None)
            .await
            .1,
        response
    );
    assert_eq!(
        post(&state, "/v3/auth/register/start", original, None)
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        post(&state, "/v3/auth/register/finish", old_finish, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        session_for(&state, &signup.device).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn signup_finish_and_replacement_serialize_to_one_winner() {
    for _ in 0..3 {
        let state = empty().await;
        let mut signup = Signup::new(&state).await;
        let (_, started) = post(&state, "/v3/auth/register/start", signup.start_body(), None).await;
        let body = signup.finish_body(&started);
        signup.replace();
        let (finish, replacement) = tokio::join!(
            post(&state, "/v3/auth/register/finish", body, None),
            post(&state, "/v3/auth/register/start", signup.start_body(), None)
        );
        assert_eq!(
            usize::from(finish.0 == StatusCode::OK) + usize::from(replacement.0 == StatusCode::OK),
            1
        );
        if finish.0 == StatusCode::OK {
            assert_eq!(replacement.1["code"], "operation_committed");
        } else {
            assert_eq!(finish.0, StatusCode::UNAUTHORIZED);
            assert_eq!(
                session_for(&state, &signup.device).await.0,
                StatusCode::UNAUTHORIZED
            );
        }
    }
}

#[tokio::test]
async fn expired_signup_renews_original_effect_without_regenerating_private_state() {
    let state = empty().await;
    let mut signup = Signup::new(&state).await;
    let (_, started) = post(&state, "/v3/auth/register/start", signup.start_body(), None).await;
    let body = signup.finish_body(&started);
    sqlx::query("DELETE FROM secure_auth_attempts")
        .execute(&state.pool)
        .await
        .unwrap();
    let (status, renewed) =
        post(&state, "/v3/auth/register/start", signup.start_body(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(started["response"], renewed["response"]);
    let mut retry = body.clone();
    retry["authorization"] = renewed["authorization"].clone();
    assert_eq!(body["operation"], retry["operation"]);
    assert_eq!(
        post(&state, "/v3/auth/register/finish", retry, None)
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Complete password transaction including authenticated proof.
async fn password_change_requires_exact_grant_preserves_payload_and_never_reuses_generation() {
    let fixture = fixture().await;
    let (device, session) = signed_in(&fixture).await;
    let account = account_state(&fixture.state, fixture.scope.account)
        .await
        .unwrap();
    let before: String = sqlx::query_scalar("SELECT envelope FROM account_vaults")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    let generation = Uuid::new_v4();
    let id = Uuid::new_v4();
    let (client, request) =
        auth::Registration::start(SecretString::from(PASSWORD.to_owned())).unwrap();
    let start = json!({"operation_id":id,"generation":generation,"request":request,"expected":account.expected});
    let (status, started) = post(
        &fixture.state,
        "/v3/auth/password/start",
        start.clone(),
        Some(&session),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{started}");
    assert_eq!(
        post(
            &fixture.state,
            "/v3/auth/password/start",
            start,
            Some(&session)
        )
        .await
        .1,
        started
    );
    let context: auth::AccountContext = serde_json::from_value(started["account"].clone()).unwrap();
    let registered = client
        .finish(
            &context,
            started["response"].as_str().unwrap(),
            Some(&fixture.pin),
        )
        .unwrap();
    let transcript = RegistrationTranscript {
        account: context,
        scope: fixture.scope.clone(),
        generation,
        request,
        response: started["response"].as_str().unwrap().into(),
        upload: registered.upload,
    };
    let wrapper = KeyEnvelope::wrap(
        fixture.scope.clone(),
        generation,
        &registered.export_key,
        &fixture.key,
    )
    .unwrap();
    let operation = Operation {
        format: 1,
        scope: fixture.scope.clone(),
        id,
        device: DeviceBinding::Existing { id: device.id() },
        expected: account.expected.clone(),
        change: Change::ChangePassword {
            generation,
            registration_sha256: transcript.digest().unwrap(),
            wrapper_sha256: wrapper.digest().unwrap(),
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
    let mut body = json!({"authorization":started["authorization"],"grant":"x".repeat(43),"signature":operation.sign(&fixture.bundle.identity().unwrap()).unwrap(),"operation":operation,"registration":transcript,"wrapper":wrapper});
    assert_eq!(
        post(
            &fixture.state,
            "/v3/auth/password/finish",
            body.clone(),
            Some(&session)
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    body["grant"] = granted["grant"].clone();
    let (status, done) = post(
        &fixture.state,
        "/v3/auth/password/finish",
        body.clone(),
        Some(&session),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["credential_generation"], generation.to_string());
    assert_eq!(
        post(
            &fixture.state,
            "/v3/auth/password/finish",
            body,
            Some(&session)
        )
        .await
        .1,
        done
    );
    let after: String = sqlx::query_scalar("SELECT envelope FROM account_vaults")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(before, after);
    assert!(
        sqlx::query("UPDATE secure_credentials SET generation=?")
            .bind(account.expected.credential_generation.unwrap().to_string())
            .execute(&fixture.state.pool)
            .await
            .is_err()
    );
    assert_eq!(
        current_precondition(
            &mut fixture.state.pool.begin().await.unwrap(),
            fixture.scope.account
        )
        .await
        .unwrap()
        .credential_generation,
        Some(generation)
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Delayed migration race followed by exact-effect renewal.
async fn classic_migration_requires_old_password_identity_and_verifier_unchanged_at_finish() {
    let state = empty().await;
    let mut signup = Signup::new(&state).await;
    let old = hash_password("synthetic old classic password".into())
        .await
        .unwrap();
    let account = signup.binding.scope.account;
    let device = signup.device.id();
    sqlx::query("INSERT INTO users(id,username,display_name,password_hash,server_member) VALUES(?,?,'Classic migration',?,0)")
        .bind(account.to_string()).bind(&signup.binding.signup.username).bind(old).execute(&state.pool).await.unwrap();
    sqlx::query("INSERT INTO chat_identities(user_id,public_identity) VALUES(?,?)")
        .bind(account.to_string())
        .bind(serde_json::to_string(&signup.identity.public()).unwrap())
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO devices(id,user_id,name,token_hash,created_at) VALUES(?,?,'Synthetic',?,?)",
    )
    .bind(device.to_string())
    .bind(account.to_string())
    .bind(token_hash(signup.device.token().expose_secret()))
    .bind(Utc::now().to_rfc3339())
    .execute(&state.pool)
    .await
    .unwrap();
    let (_, response) = session_for(&state, &signup.device).await;
    let session = response["token"].as_str().unwrap();
    let mut start = json!({"operation_id":signup.binding.id,"generation":signup.binding.generation,"request":signup.request,"legacy_password":"synthetic incorrect password"});
    assert_eq!(
        post(
            &state,
            "/v3/auth/migrate/start",
            start.clone(),
            Some(session)
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    start["legacy_password"] = json!("synthetic old classic password");
    let (status, started) = post(&state, "/v3/auth/migrate/start", start, Some(session)).await;
    assert_eq!(status, StatusCode::OK, "{started}");
    let mut finish = signup.finish_body(&started);
    let mut operation: Operation = serde_json::from_value(finish["operation"].clone()).unwrap();
    operation.device = DeviceBinding::Existing { id: device };
    if let Change::Enroll { signup, .. } = &mut operation.change {
        *signup = None;
    }
    finish["signature"] = json!(operation.sign(&signup.identity).unwrap());
    finish["operation"] = json!(operation);
    // An intervening classic password change invalidates the migration authorization.
    let new = hash_password("different synthetic classic password".into())
        .await
        .unwrap();
    sqlx::query("UPDATE users SET password_hash=? WHERE id=?")
        .bind(new)
        .bind(account.to_string())
        .execute(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        post(
            &state,
            "/v3/auth/migrate/finish",
            finish.clone(),
            Some(session)
        )
        .await
        .1["code"],
        "account_state_changed"
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM secure_credentials")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    // Restore the original verifier authorization via a fresh start after expiry,
    // retaining the exact same complete signed effect and cryptographic transcript.
    sqlx::query("DELETE FROM secure_auth_attempts")
        .execute(&state.pool)
        .await
        .unwrap();
    let (_,renewed)=post(&state,"/v3/auth/migrate/start",json!({"operation_id":signup.binding.id,"generation":signup.binding.generation,"request":signup.request,"legacy_password":"different synthetic classic password"}),Some(session)).await;
    assert_eq!(renewed["response"], started["response"]);
    finish["authorization"] = renewed["authorization"].clone();
    let (status, done) = post(
        &state,
        "/v3/auth/migrate/finish",
        finish.clone(),
        Some(session),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["identity"], json!(signup.identity.public()));
    assert_eq!(
        post(&state, "/v3/auth/migrate/finish", finish, Some(session))
            .await
            .1,
        done
    );
    let hash: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=?")
        .bind(account.to_string())
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert!(hash.is_none());
    assert_eq!(session_for(&state, &signup.device).await.0, StatusCode::OK);
}

#[tokio::test]
async fn termination_is_account_scoped_checks_device_hash_and_forbids_self_revocation() {
    let fixture = fixture().await;
    let (first_device, first_session) = signed_in(&fixture).await;
    let mut second = Signup::new(&fixture.state).await;
    let (_, started) = post(
        &fixture.state,
        "/v3/auth/register/start",
        second.start_body(),
        None,
    )
    .await;
    let finish = second.finish_body(&started);
    assert_eq!(
        post(&fixture.state, "/v3/auth/register/finish", finish, None)
            .await
            .0,
        StatusCode::OK
    );
    let (_, second_auth) = session_for(&fixture.state, &second.device).await;
    let second_session = second_auth["token"].as_str().unwrap();
    let id = Uuid::new_v4();
    let target = ProspectiveDevice::generate().unwrap();
    let terminate = json!({"attempt":id,"device":target.binding()});
    // Account A's missing-row tombstone cannot reserve/preempt account B's ID.
    assert_eq!(
        post(
            &fixture.state,
            "/v3/auth/login/terminate",
            terminate.clone(),
            Some(&first_session)
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, request) = auth::Login::start(SecretString::from(PASSWORD.to_owned())).unwrap();
    let body = json!({"attempt":id,"username":second.binding.signup.username,"device":target.binding(),"device_name":"Second account test","request":request});
    assert_eq!(
        post(&fixture.state, "/v3/auth/login/start", body, None)
            .await
            .0,
        StatusCode::OK
    );
    let mut changed = terminate.clone();
    changed["device"]["token_sha256"] = json!("c".repeat(64));
    assert_eq!(
        post(
            &fixture.state,
            "/v3/auth/login/terminate",
            changed,
            Some(second_session)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(
            &fixture.state,
            "/v3/auth/login/terminate",
            json!({"attempt":Uuid::new_v4(),"device":first_device.binding()}),
            Some(&first_session)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(
            &fixture.state,
            "/v3/auth/login/terminate",
            json!({"attempt":Uuid::new_v4(),"device":first_device.binding()}),
            Some(second_session)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(
            &fixture.state,
            "/v3/auth/login/terminate",
            terminate,
            Some(second_session)
        )
        .await
        .0,
        StatusCode::OK
    );
    // Neither the unrelated account nor the recovery device was revoked.
    assert_eq!(
        session_for(&fixture.state, &first_device).await.0,
        StatusCode::OK
    );
    assert_eq!(
        session_for(&fixture.state, &second.device).await.0,
        StatusCode::OK
    );
}

async fn pending_classic_migration() -> (AppState, Signup, String, Value) {
    let state = empty().await;
    let mut signup = Signup::new(&state).await;
    let old = hash_password("synthetic old classic password".into())
        .await
        .unwrap();
    let account = signup.binding.scope.account;
    sqlx::query("INSERT INTO users(id,username,display_name,password_hash,server_member) VALUES(?,?,'Classic recovery',?,0)")
        .bind(account.to_string()).bind(&signup.binding.signup.username).bind(old).execute(&state.pool).await.unwrap();
    sqlx::query("INSERT INTO chat_identities(user_id,public_identity) VALUES(?,?)")
        .bind(account.to_string())
        .bind(serde_json::to_string(&signup.identity.public()).unwrap())
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO devices(id,user_id,name,token_hash,created_at) VALUES(?,?,'Synthetic',?,?)",
    )
    .bind(signup.device.id().to_string())
    .bind(account.to_string())
    .bind(token_hash(signup.device.token().expose_secret()))
    .bind(Utc::now().to_rfc3339())
    .execute(&state.pool)
    .await
    .unwrap();
    let (_, response) = session_for(&state, &signup.device).await;
    let session = response["token"].as_str().unwrap().to_owned();
    let (status, started) = post(&state, "/v3/auth/migrate/start", json!({"operation_id":signup.binding.id,"generation":signup.binding.generation,"request":signup.request,"legacy_password":"synthetic old classic password"}),Some(&session)).await;
    assert_eq!(status, StatusCode::OK);
    let mut finish = signup.finish_body(&started);
    let mut operation: Operation = serde_json::from_value(finish["operation"].clone()).unwrap();
    operation.device = DeviceBinding::Existing {
        id: signup.device.id(),
    };
    if let Change::Enroll { signup, .. } = &mut operation.change {
        *signup = None;
    }
    finish["signature"] = json!(operation.sign(&signup.identity).unwrap());
    finish["operation"] = json!(operation);
    (state, signup, session, finish)
}
#[tokio::test]
async fn staged_classic_recovery_terminates_old_migration_and_replay_never_reactivates() {
    use wisp_crypto::account_vault::operation::MigrationRecoveryBinding;
    let (state, signup, session, finish) = pending_classic_migration().await;
    let device = ProspectiveDevice::generate().unwrap();
    let binding = MigrationRecoveryBinding {
        format: 1,
        id: Uuid::new_v4(),
        operation: serde_json::from_value(finish["operation"].clone()).unwrap(),
        device: device.binding(),
        device_name: "Synthetic recovery".into(),
    };
    let mut body = json!({"recovery":binding,"signature":binding.sign(&signup.identity).unwrap(),"legacy_password":"synthetic wrong classic password"});
    assert_eq!(
        post(&state, "/v3/auth/migrate/recover", body.clone(), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        session_for(&state, &device).await.0,
        StatusCode::UNAUTHORIZED
    );
    body["legacy_password"] = json!("synthetic old classic password");
    let (status, result) = post(&state, "/v3/auth/migrate/recover", body.clone(), None).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(
        result["operation_sha256"],
        binding.operation.digest().unwrap()
    );
    assert_eq!(
        result["terminated_device_id"],
        signup.device.id().to_string()
    );
    assert_eq!(
        post(&state, "/v3/auth/migrate/finish", finish, Some(&session))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        session_for(&state, &signup.device).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(session_for(&state, &device).await.0, StatusCode::OK);
    // Exact signed receipt remains recoverable with no old password, even if
    // the recovery device was revoked before its response reached the client.
    sqlx::query("UPDATE devices SET revoked_at=? WHERE id=?")
        .bind(Utc::now().to_rfc3339())
        .bind(device.id().to_string())
        .execute(&state.pool)
        .await
        .unwrap();
    body["legacy_password"] = json!("");
    assert_eq!(
        post(&state, "/v3/auth/migrate/recover", body.clone(), None)
            .await
            .1,
        result
    );
    assert_eq!(
        session_for(&state, &device).await.0,
        StatusCode::UNAUTHORIZED
    );
    body["recovery"]["device_name"] = json!("tampered");
    assert!(
        !post(&state, "/v3/auth/migrate/recover", body, None)
            .await
            .0
            .is_success()
    );
}
#[tokio::test]
async fn migration_and_staged_classic_recovery_have_only_one_winner() {
    use wisp_crypto::account_vault::operation::MigrationRecoveryBinding;
    let (state, signup, session, finish) = pending_classic_migration().await;
    let device = ProspectiveDevice::generate().unwrap();
    let binding = MigrationRecoveryBinding {
        format: 1,
        id: Uuid::new_v4(),
        operation: serde_json::from_value(finish["operation"].clone()).unwrap(),
        device: device.binding(),
        device_name: "Synthetic recovery".into(),
    };
    let body = json!({"recovery":binding,"signature":binding.sign(&signup.identity).unwrap(),"legacy_password":"synthetic old classic password"});
    let (migrated, recovered) = tokio::join!(
        post(&state, "/v3/auth/migrate/finish", finish, Some(&session)),
        post(&state, "/v3/auth/migrate/recover", body, None)
    );
    assert_ne!(migrated.0.is_success(), recovered.0.is_success());
    let secure: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM secure_credentials WHERE user_id=?)")
            .bind(signup.binding.scope.account.to_string())
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(secure, migrated.0.is_success());
    assert_eq!(
        session_for(&state, &device).await.0.is_success(),
        recovered.0.is_success()
    );
}
#[tokio::test]
async fn public_secure_concurrency_rejects_before_reading_unbounded_body() {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    let state = empty().await;
    let _held = state.secure_public_work.acquire_many(8).await.unwrap();
    let body = Body::from_stream(futures_util::stream::pending::<
        Result<axum::body::Bytes, std::io::Error>,
    >());
    let request = Request::builder()
        .method("POST")
        .uri("/v3/auth/register/finish")
        .header("content-type", "application/json")
        .body(body)
        .unwrap();
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        router(state.clone()).oneshot(request),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()["cache-control"], "no-store");
}
