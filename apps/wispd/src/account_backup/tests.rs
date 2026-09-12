#![allow(clippy::manual_assert_eq)] // Never print private records or credentials on failure.
use super::{
    api::Api,
    store::{Kind, Store},
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use wisp_crypto::{Identity, SecretString, keyring::TrustStore};
const PASSWORD: &str = "synthetic private account password";
struct Fixture {
    folder: tempfile::TempDir,
    origin: String,
    pool: sqlx::SqlitePool,
    faults: Arc<Mutex<Option<String>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Fixture {
    async fn new() -> Self {
        let folder = tempfile::tempdir().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let database_url = format!("sqlite:{}", folder.path().join("server.sqlite3").display());
        let state = wisp_server::AppState::new(wisp_server::AppConfig {
            database_url: database_url.clone(),
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
        let faults = Arc::new(Mutex::new(None));
        let router = wisp_server::router(state).layer(axum::middleware::from_fn_with_state(
            faults.clone(),
            lose_response,
        ));
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let pool = sqlx::SqlitePool::connect(&database_url).await.unwrap();
        Self {
            folder,
            origin,
            pool,
            faults,
            task,
        }
    }
    async fn client(&self, name: &str) -> Api {
        let store = Arc::new(Store::at(&self.folder.path().join(name), &self.origin).unwrap());
        Api::connect(&self.origin, store, None, None).await.unwrap()
    }
    fn lose(&self, path: &str) {
        *self.faults.lock().unwrap() = Some(path.to_owned());
    }
    fn fail_before(&self, path: &str) {
        self.lose(&format!("before:{path}"));
    }
    async fn revoke(&self, device: Uuid) {
        sqlx::query("UPDATE devices SET revoked_at='2099-01-01T00:00:00Z' WHERE id=?")
            .bind(device.to_string())
            .execute(&self.pool)
            .await
            .unwrap();
    }
}
async fn lose_response(
    axum::extract::State(faults): axum::extract::State<Arc<Mutex<Option<String>>>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let (lose, before) = {
        let mut faults = faults.lock().unwrap();
        if faults.as_deref() == Some(request.uri().path()) {
            *faults = None;
            (true, false)
        } else if faults
            .as_deref()
            .and_then(|path| path.strip_prefix("before:"))
            == Some(request.uri().path())
        {
            *faults = None;
            (false, true)
        } else {
            (false, false)
        }
    };
    if before {
        return (
            axum::http::StatusCode::BAD_GATEWAY,
            axum::Json(json!({"code":"synthetic_rejected_request"})),
        )
            .into_response();
    }
    let response = next.run(request).await;
    if lose && response.status().is_success() {
        (
            axum::http::StatusCode::BAD_GATEWAY,
            axum::Json(json!({"code":"synthetic_lost_response"})),
        )
            .into_response()
    } else {
        response
    }
}
fn password() -> SecretString {
    SecretString::from(PASSWORD.to_owned())
}
#[tokio::test]
async fn desktop_signup_remote_login_and_two_device_trust_sync_preserve_the_same_key() {
    let fixture = Fixture::new().await;
    let mut first = fixture.client("first").await;
    first
        .signup(
            "Synthetic",
            "Synthetic user",
            "Synthetic desktop",
            password(),
        )
        .await
        .unwrap();
    let original = first.store.record().unwrap();
    assert!(original.pending.is_none() && original.key_verified);
    let mut second = fixture.client("second").await;
    assert!(
        second
            .login("SYNTHETIC", password(), "Synthetic second device")
            .await
            .unwrap()
    );
    assert!(
        second
            .store
            .record()
            .unwrap()
            .local_key()
            .unwrap()
            .save_private()
            .as_slice()
            == original.local_key().unwrap().save_private().as_slice()
    );
    assert!(second.store.record().unwrap().identity == original.identity);
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let a_key = Identity::generate().unwrap().public();
    let b_key = Identity::generate().unwrap().public();
    TrustStore::update(&*first.store, &mut |trust| {
        trust.pins.insert(a, a_key.clone());
        Ok(())
    })
    .unwrap();
    TrustStore::update(&*second.store, &mut |trust| {
        trust.pins.insert(b, b_key.clone());
        Ok(())
    })
    .unwrap();
    first.sync().await.unwrap();
    second.sync().await.unwrap();
    first.sync().await.unwrap();
    let trust = first.store.snapshot().unwrap();
    assert!(trust.pins[&a] == a_key && trust.pins[&b] == b_key);
    assert!(first.store.record().unwrap().synced_dirty == first.store.record().unwrap().dirty);
}
#[tokio::test]
async fn lost_signup_finish_retries_exact_effect_without_password_or_replacement_identity() {
    let fixture = Fixture::new().await;
    let mut first = fixture.client("first").await;
    fixture.lose("/v3/auth/register/finish");
    assert!(
        first
            .signup(
                "synthetic",
                "Synthetic user",
                "Synthetic desktop",
                password()
            )
            .await
            .is_err()
    );
    let pending = first.store.record().unwrap().pending.unwrap();
    assert!(pending.sent && pending.kind == Kind::Signup);
    let key = pending.key.unwrap().decode(32).unwrap();
    let identity = pending.identity;
    // A fresh process can complete the exact signed body; native registration
    // state and the password are intentionally absent from durable storage.
    let mut reopened = fixture.client("first").await;
    reopened.resume_enrollment(None).await.unwrap();
    let record = reopened.store.record().unwrap();
    assert!(record.pending.is_none() && record.identity == identity);
    assert!(record.local_key().unwrap().save_private().as_slice() == key.as_slice());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM devices")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}
#[tokio::test]
async fn repeated_unknown_revoked_logins_are_all_terminated_before_journal_clear() {
    let fixture = Fixture::new().await;
    let mut first = fixture.client("first").await;
    first
        .signup(
            "synthetic",
            "Synthetic user",
            "Synthetic desktop",
            password(),
        )
        .await
        .unwrap();
    let mut other = fixture.client("other").await;
    let mut old = Vec::new();
    for _ in 0..2 {
        fixture.lose("/v3/auth/login/finish");
        assert!(
            other
                .login("synthetic", password(), "Synthetic recovery")
                .await
                .is_err()
        );
        let pending = other.store.record().unwrap().pending.unwrap();
        let leaf = pending.recovery.last().unwrap_or(&pending);
        let device = leaf.device.as_ref().unwrap().id;
        old.push((leaf.id, device));
        fixture.revoke(device).await;
    }
    assert!(
        other
            .login("synthetic", password(), "Synthetic final recovery")
            .await
            .unwrap()
    );
    assert!(other.store.record().unwrap().pending.is_none());
    for (attempt, device) in old {
        let terminal:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM secure_login_terminations WHERE attempt=? AND device_id=?)").bind(attempt.to_string()).bind(device.to_string()).fetch_one(&fixture.pool).await.unwrap();
        assert!(terminal);
    }
}
#[tokio::test]
async fn account_action_lease_keeps_trust_updates_available() {
    let fixture = Fixture::new().await;
    let mut first = fixture.client("first").await;
    first
        .signup(
            "synthetic",
            "Synthetic user",
            "Synthetic desktop",
            password(),
        )
        .await
        .unwrap();
    let lease = first.store.action().unwrap();
    assert!(first.store.action().is_err());
    let friend = Uuid::new_v4();
    let pin = Identity::generate().unwrap().public();
    TrustStore::update(&*first.store, &mut |trust| {
        trust.pins.insert(friend, pin.clone());
        Ok(())
    })
    .unwrap();
    drop(lease);
    assert!(first.store.action().is_ok());
}

#[tokio::test]
async fn password_change_and_lost_completion_preserve_payload_key_and_new_sign_in() {
    let fixture = Fixture::new().await;
    let mut first = fixture.client("first").await;
    first
        .signup(
            "synthetic",
            "Synthetic user",
            "Synthetic desktop",
            password(),
        )
        .await
        .unwrap();
    let before = first.store.record().unwrap();
    fixture.lose("/v3/auth/password/finish");
    let new = "synthetic replacement account password";
    assert!(
        first
            .password_change(password(), SecretString::from(new.to_owned()))
            .await
            .is_err()
    );
    assert!(
        first
            .store
            .record()
            .unwrap()
            .pending
            .as_ref()
            .is_some_and(|p| p.kind == Kind::Password && p.sent)
    );
    // Receipt reconciliation does not ask again for the old or new password.
    first.resume_mutation(None).await.unwrap();
    assert!(first.store.record().unwrap().pending.is_none());
    assert!(
        first
            .store
            .record()
            .unwrap()
            .local_key()
            .unwrap()
            .save_private()
            .as_slice()
            == before.local_key().unwrap().save_private().as_slice()
    );
    assert!(first.store.record().unwrap().checkpoint == before.checkpoint);
    let mut other = fixture.client("other").await;
    assert!(
        other
            .login(
                "synthetic",
                SecretString::from(new.to_owned()),
                "Synthetic other"
            )
            .await
            .unwrap()
    );
    assert!(other.store.record().unwrap().identity == before.identity);
}
#[tokio::test]
async fn classic_migration_preserves_media_and_concurrent_trust_updates_after_lost_finish() {
    let fixture = Fixture::new().await;
    let (mut first, _credential, key) = classic_client(&fixture, "first").await;
    let legacy = "synthetic original classic password";
    fixture.lose("/v3/auth/migrate/finish");
    assert!(
        first
            .migrate(SecretString::from(legacy.to_owned()), password())
            .await
            .is_err()
    );
    let friend = Uuid::new_v4();
    let pin = Identity::generate().unwrap().public();
    TrustStore::update(&*first.store, &mut |trust| {
        trust.pins.insert(friend, pin.clone());
        Ok(())
    })
    .unwrap();
    first.resume_enrollment(None).await.unwrap();
    first.sync().await.unwrap();
    let mut other = fixture.client("other").await;
    assert!(
        other
            .login("classicuser", password(), "Synthetic restored")
            .await
            .unwrap()
    );
    assert!(other.store.snapshot().unwrap().pins[&friend] == pin);
    assert!(
        other
            .store
            .record()
            .unwrap()
            .bundle()
            .unwrap()
            .unwrap()
            .media_key()
            .is_some()
    );
    assert!(
        other
            .store
            .record()
            .unwrap()
            .local_key()
            .unwrap()
            .save_private()
            .as_slice()
            == key.save_private().as_slice()
    );
}

#[tokio::test]
async fn separate_reset_journal_leaves_selected_identity_untouched_then_trusted_rewrap_restores_remote_device()
 {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use sha2::{Digest, Sha256};
    let fixture = Fixture::new().await;
    let mut trusted = fixture.client("trusted").await;
    trusted
        .signup(
            "synthetic",
            "Synthetic user",
            "Synthetic trusted",
            password(),
        )
        .await
        .unwrap();
    let initial = trusted.store.record().unwrap();
    let before = std::fs::read(trusted.store.private_path()).unwrap();
    let scope = initial.scope.clone().unwrap();
    let generation = initial
        .expected
        .as_ref()
        .unwrap()
        .credential_generation
        .unwrap();
    // Only this fixture's disposable DB is touched. No mailer or real account is used.
    let token = URL_SAFE_NO_PAD.encode([19_u8; 32]);
    let hash = |text: &str| URL_SAFE_NO_PAD.encode(Sha256::digest(text.as_bytes()));
    sqlx::query(
        "INSERT INTO account_recovery_emails(user_id,email) VALUES(?,'synthetic@example.invalid')",
    )
    .bind(scope.account.to_string())
    .execute(&fixture.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO account_recovery_tokens(token_hash,user_id,kind,email,password_fingerprint,expires_at) VALUES(?,?,'reset','synthetic@example.invalid',?,?)")
        .bind(hash(&token)).bind(scope.account.to_string()).bind(hash(&format!("wisp-secure-recovery-v1:{generation}"))).bind(super::sync::now()+1200).execute(&fixture.pool).await.unwrap();
    let reset = fixture.client("separate-reset").await;
    let new = "synthetic post-reset account password";
    fixture.lose("/v3/auth/reset/finish");
    assert!(
        reset
            .reset_password(
                &format!("https://wisp.you/account/reset-password#token={token}"),
                SecretString::from(new.to_owned())
            )
            .await
            .is_err()
    );
    assert!(
        reset
            .store
            .record()
            .unwrap()
            .pending
            .as_ref()
            .is_some_and(|pending| pending.kind == Kind::Reset && pending.sent)
    );
    reset.resume_reset(None).await.unwrap();
    assert!(std::fs::read(trusted.store.private_path()).unwrap() == before);
    let mut remote = fixture.client("remote").await;
    assert!(
        !remote
            .login(
                "synthetic",
                SecretString::from(new.to_owned()),
                "Synthetic remote"
            )
            .await
            .unwrap()
    );
    assert!(remote.store.record().unwrap().bundle().unwrap().is_none());
    assert!(remote.store.record().unwrap().key.is_none());
    trusted
        .rewrap(SecretString::from(new.to_owned()))
        .await
        .unwrap();
    let state = remote.status().await.unwrap();
    let export = remote
        .unlock(SecretString::from(new.to_owned()), &state)
        .await
        .unwrap();
    assert!(
        remote
            .restore(Some((
                &export,
                state.expected().credential_generation.unwrap()
            )))
            .await
            .unwrap()
    );
    assert!(
        remote
            .store
            .record()
            .unwrap()
            .local_key()
            .unwrap()
            .save_private()
            .as_slice()
            == initial.local_key().unwrap().save_private().as_slice()
    );
    assert!(remote.store.record().unwrap().identity == initial.identity);
}

async fn classic_client(
    fixture: &Fixture,
    name: &str,
) -> (
    Api,
    wisp_protocol::DeviceCredential,
    wisp_crypto::account_vault::envelope::VaultKey,
) {
    use super::store::PrivateBytes;
    use wisp_crypto::account_vault::{
        Scope,
        bundle::{Bundle, TrustState},
        envelope::VaultKey,
    };
    let mut first = fixture.client(name).await;
    let legacy = "synthetic original classic password";
    let credential:wisp_protocol::DeviceCredential=first.post("/v2/accounts/register",&json!({"username":"ClassicUser","display_name":"Synthetic classic","password":legacy,"device_name":"Synthetic classic device","protocol_version":wisp_protocol::PROTOCOL_VERSION}),false,16384).await.unwrap();
    first.session(&credential).await.unwrap();
    let identity = Identity::generate().unwrap();
    let scope = Scope {
        origin: fixture.origin.clone(),
        network: first.network,
        account: credential.user.id,
    };
    let public = identity.public();
    let signature = identity.sign_statement(
        "wisp-account-key-v1",
        &serde_json::to_vec(&(scope.network, scope.account, &public)).unwrap(),
    );
    let _: Value = first
        .post(
            "/v1/e2ee/identity",
            &json!({"identity":public,"signature":signature}),
            true,
            4096,
        )
        .await
        .unwrap();
    let mut trust = TrustState::default();
    trust.pins.insert(scope.account, identity.public());
    let bundle = Bundle::new(
        scope,
        identity.recovery_key().unwrap(),
        Some(SecretString::from(
            "synthetic saved media encryption key".to_owned(),
        )),
        trust,
    )
    .unwrap();
    let key = VaultKey::generate().unwrap();
    first
        .store
        .update(None, |record| {
            record.key = Some(PrivateBytes::new(key.save_private().as_slice()));
            record.set_bundle(&bundle)
        })
        .unwrap();
    (first, credential, key)
}

#[tokio::test]
async fn revoked_existing_device_recovery_reconciles_lost_password_change() {
    let fixture = Fixture::new().await;
    let mut first = fixture.client("first").await;
    first
        .signup("synthetic", "Synthetic user", "Synthetic first", password())
        .await
        .unwrap();
    let before = first.store.record().unwrap();
    let old = first.installation().unwrap();
    fixture.lose("/v3/auth/password/finish");
    let new = "synthetic replacement account password";
    assert!(
        first
            .password_change(password(), SecretString::from(new.to_owned()))
            .await
            .is_err()
    );
    fixture.revoke(old.device_id).await;
    assert!(
        first
            .recover_secure(
                "synthetic",
                SecretString::from(new.to_owned()),
                "Synthetic recovered"
            )
            .await
            .unwrap()
    );
    assert!(first.store.record().unwrap().pending.is_none());
    assert!(first.installation().unwrap().device_id != old.device_id);
    assert!(
        first
            .store
            .record()
            .unwrap()
            .local_key()
            .unwrap()
            .save_private()
            .as_slice()
            == before.local_key().unwrap().save_private().as_slice()
    );
}
#[tokio::test]
async fn classic_recovery_survives_two_lost_revoked_candidates_without_replacing_prepared_credentials()
 {
    let fixture = Fixture::new().await;
    let (mut first, original, key) = classic_client(&fixture, "first").await;
    let legacy = "synthetic original classic password";
    fixture.fail_before("/v3/auth/migrate/finish");
    assert!(
        first
            .migrate(SecretString::from(legacy.to_owned()), password())
            .await
            .is_err()
    );
    let initial = first.store.record().unwrap().pending.unwrap();
    let prepared = initial.finish.as_ref().unwrap().value().unwrap();
    fixture.revoke(original.device_id).await;
    let mut abandoned = Vec::new();
    for _ in 0..2 {
        fixture.lose("/v3/auth/migrate/recover");
        assert!(
            first
                .recover_classic_migration(
                    SecretString::from(legacy.to_owned()),
                    "Synthetic recovered"
                )
                .await
                .is_err()
        );
        let root = first.store.record().unwrap().pending.unwrap();
        let candidate = root.recovery.last().unwrap();
        let id = candidate.device.as_ref().unwrap().id;
        abandoned.push(id);
        fixture.revoke(id).await;
    }
    first
        .recover_classic_migration(SecretString::from(legacy.to_owned()), "Synthetic final")
        .await
        .unwrap();
    assert!(first.store.record().unwrap().pending.is_none());
    assert!(
        first
            .store
            .record()
            .unwrap()
            .expected
            .unwrap()
            .credential_generation
            == initial.generation
    );
    assert!(
        first
            .store
            .record()
            .unwrap()
            .local_key()
            .unwrap()
            .save_private()
            .as_slice()
            == key.save_private().as_slice()
    );
    let vault: super::api::VaultResponse = first
        .get("/v3/accounts/vault", 12 * 1024 * 1024)
        .await
        .unwrap();
    assert!(serde_json::to_value(vault.vault).unwrap() == prepared["vault"]);
    for id in abandoned {
        let revoked: bool =
            sqlx::query_scalar("SELECT revoked_at IS NOT NULL FROM devices WHERE id=?")
                .bind(id.to_string())
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
        assert!(revoked);
    }
    let active: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM devices WHERE revoked_at IS NULL")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(active, 1);
}
#[tokio::test]
async fn lost_signup_recovery_uses_staged_identity_then_revokes_original_device() {
    let fixture = Fixture::new().await;
    let mut first = fixture.client("first").await;
    fixture.lose("/v3/auth/register/finish");
    assert!(
        first
            .signup("synthetic", "Synthetic user", "Synthetic first", password())
            .await
            .is_err()
    );
    let pending = first.store.record().unwrap().pending.unwrap();
    let old = pending.device.as_ref().unwrap().id;
    assert!(
        first
            .recover_secure("synthetic", password(), "Synthetic recovered")
            .await
            .unwrap()
    );
    assert!(first.store.record().unwrap().pending.is_none());
    assert!(first.store.record().unwrap().identity == pending.identity);
    let revoked: bool = sqlx::query_scalar("SELECT revoked_at IS NOT NULL FROM devices WHERE id=?")
        .bind(old.to_string())
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert!(revoked);
}
