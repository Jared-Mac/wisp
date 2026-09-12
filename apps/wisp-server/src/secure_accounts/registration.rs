//! Registration tickets bind an immutable public transcript. Pending records
//! contain no password, plaintext authorization, private identity or vault key.
use super::*;
use axum::extract::rejection::JsonRejection;
use wisp_crypto::account_vault::{
    device::legacy_token_hash,
    envelope::{Envelope, KeyEnvelope, MAX_ENVELOPE_WIRE},
    operation::{
        Change, DeviceBinding, Operation, RegistrationTranscript, SignupMetadata,
        SignupStartBinding, credential_identifier,
    },
};

const REGISTRATION_SECONDS: i64 = 600;
type Tx<'a> = sqlx::Transaction<'a, sqlx::Sqlite>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegisterStart {
    operation_id: Uuid,
    scope: Scope,
    generation: Uuid,
    username: String,
    display_name: String,
    device: DeviceBinding,
    device_name: String,
    request: String,
    identity: PublicIdentity,
    signature: String,
    previous_binding_sha256: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MigrateStart {
    operation_id: Uuid,
    generation: Uuid,
    request: String,
    legacy_password: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PasswordStart {
    operation_id: Uuid,
    generation: Uuid,
    request: String,
    expected: Precondition,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EnrollFinish {
    authorization: String,
    operation: Operation,
    signature: String,
    registration: RegistrationTranscript,
    wrapper: KeyEnvelope,
    vault: Envelope,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PasswordFinish {
    authorization: String,
    grant: String,
    operation: Operation,
    signature: String,
    registration: RegistrationTranscript,
    wrapper: KeyEnvelope,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Binding {
    pub(super) account: auth::AccountContext,
    pub(super) scope: Scope,
    pub(super) id: Uuid,
    pub(super) generation: Uuid,
    pub(super) device: Option<DeviceBinding>,
    pub(super) identity: Option<PublicIdentity>,
    pub(super) expected: Precondition,
    pub(super) request: String,
    pub(super) signup: Option<SignupStartBinding>,
    pub(super) legacy_fingerprint: Option<String>,
    pub(super) reset_token_hash: Option<String>,
}
impl Binding {
    pub(super) fn digest(&self, kind: &str) -> Result<String, ApiError> {
        self.account.validate().map_err(|_| invalid())?;
        self.scope.validate().map_err(|_| invalid())?;
        self.expected.validate().map_err(|_| invalid())?;
        auth::registration_request_digest(&self.request).map_err(|_| invalid())?;
        if self.id.is_nil()
            || self.generation.is_nil()
            || self.account.origin != self.scope.origin
            || self.account.network != self.scope.network
        {
            return Err(invalid());
        }
        if kind == "register" {
            let signup = self.signup.as_ref().ok_or_else(invalid)?;
            if signup.scope != self.scope
                || signup.id != self.id
                || signup.generation != self.generation
                || Some(&signup.device) != self.device.as_ref()
                || Some(&signup.identity) != self.identity.as_ref()
                || signup.signup.username != self.account.username
                || self.expected != Precondition::default()
                || signup.request_sha256
                    != auth::registration_request_digest(&self.request).map_err(|_| invalid())?
                || self.legacy_fingerprint.is_some()
                || self.reset_token_hash.is_some()
            {
                return Err(invalid());
            }
            return signup.digest().map_err(|_| invalid());
        }
        authentication::digest("wisp-registration-start-v1", &(kind, self))
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Pending {
    pub(super) binding: Binding,
    pub(super) response: String,
    pub(super) expires_at: i64,
}
impl Pending {
    pub(super) fn response(&self, authorization: &str) -> Value {
        json!({"operation_id":self.binding.id,"account":self.binding.account,"scope":self.binding.scope,
            "generation":self.binding.generation,"expected":self.binding.expected,"response":self.response,
            "authorization":authorization,"expires_at":self.expires_at})
    }
    pub(super) fn check_transcript(
        &self,
        transcript: &RegistrationTranscript,
    ) -> Result<(), ApiError> {
        transcript.digest().map_err(|_| invalid())?;
        if transcript.account != self.binding.account
            || transcript.scope != self.binding.scope
            || transcript.generation != self.binding.generation
            || transcript.request != self.binding.request
            || transcript.response != self.response
        {
            return Err(invalid());
        }
        Ok(())
    }
}
fn conflict() -> ApiError {
    ApiError::conflict(
        "operation_conflict",
        "Registration differs from its saved request",
    )
}
pub(super) fn changed() -> ApiError {
    ApiError::conflict(
        "account_state_changed",
        "Account state changed; refresh before continuing",
    )
}
fn denied() -> ApiError {
    ApiError::unauthorized("Secure registration authorization is invalid or expired")
}

/// Dedicated derived ticket key; deterministic tickets survive lost responses
/// and restarts without storing plaintext tickets. Expiry is fixed-width BE i64.
pub(super) fn ticket(native: &auth::Server, binding: &str, expiry: i64) -> String {
    let setup = native.save();
    let mut derivation = Hmac::<Sha256>::new_from_slice(&setup).expect("HMAC key length");
    derivation.update(b"wisp-registration-ticket-key-v1");
    let key = Zeroizing::new(<[u8; 32]>::from(derivation.finalize().into_bytes()));
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_slice()).expect("HMAC key length");
    mac.update(b"wisp-registration-ticket-v1\0");
    mac.update(binding.as_bytes()); // Validated fixed 64-byte lowercase hex.
    mac.update(&expiry.to_be_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

fn saved(row: &SqliteRow) -> Result<Pending, ApiError> {
    let bytes = Zeroizing::new(row.get::<Vec<u8>, _>("state"));
    if bytes.len() > 65536 {
        return Err(unavailable());
    }
    let pending: Pending = serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
    if pending.expires_at != row.get::<i64, _>("expires_at")
        || pending.binding.digest(&row.get::<String, _>("kind"))?
            != row.get::<String, _>("binding_digest")
    {
        return Err(unavailable());
    }
    Ok(pending)
}

pub(super) async fn require_unused_generation(
    tx: &mut Tx<'_>,
    generation: Uuid,
) -> Result<(), ApiError> {
    if generation.is_nil() {
        return Err(invalid());
    }
    let used: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM secure_generation_history WHERE generation=?)",
    )
    .bind(generation.to_string())
    .fetch_one(&mut **tx)
    .await
    .map_err(ApiError::internal)?;
    if used {
        return Err(changed());
    }
    Ok(())
}

pub(super) async fn begin(
    tx: &mut Tx<'_>,
    native: &auth::Server,
    kind: &str,
    binding: Binding,
) -> Result<Value, ApiError> {
    let digest = binding.digest(kind)?;
    authentication::expire_and_bound(tx).await?;
    let committed: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM secure_operation_receipts WHERE id=?)")
            .bind(binding.id.to_string())
            .fetch_one(&mut **tx)
            .await
            .map_err(ApiError::internal)?;
    if committed {
        return Err(ApiError::conflict(
            "operation_committed",
            "This operation has already completed",
        ));
    }
    require_unused_generation(tx, binding.generation).await?;
    let row = sqlx::query("SELECT kind,binding_digest,state,authorization_hash,expires_at FROM secure_auth_attempts WHERE id=?")
        .bind(binding.id.to_string()).fetch_optional(&mut **tx).await.map_err(ApiError::internal)?;
    if let Some(row) = row {
        if row.get::<String, _>("kind") != kind {
            return Err(conflict());
        }
        let old = saved(&row)?;
        let old_digest: String = row.get("binding_digest");
        if old_digest == digest {
            if old.binding != binding {
                return Err(unavailable());
            }
            let authorization = ticket(native, &digest, old.expires_at);
            if row.get::<Option<String>, _>("authorization_hash")
                != Some(token_hash(&authorization))
            {
                return Err(unavailable());
            }
            return Ok(old.response(&authorization));
        }
        let signup_replacement = match (&binding.signup, &old.binding.signup) {
            (Some(next), Some(previous)) => {
                kind == "register"
                    && next.previous_binding_sha256.as_deref() == Some(&old_digest)
                    && next.identity == previous.identity
                    && next.scope == previous.scope
                    && next.id == previous.id
                    && next.generation == previous.generation
                    && next.device == previous.device
                    && next.signup == previous.signup
            }
            _ => false,
        };
        // A fresh live email token can renew transport authorization for the
        // same saved reset effect, including when issuing it invalidated the
        // previous token before its ticket deadline. Password/vault effects do
        // not contain the renewable token hash.
        let mut renewed_reset = old.binding.clone();
        renewed_reset.reset_token_hash = binding.reset_token_hash.clone();
        renewed_reset.expected = binding.expected.clone();
        let reset_replacement = kind == "reset"
            && renewed_reset == binding
            && old.binding.expected.credential_generation == binding.expected.credential_generation;
        if !signup_replacement && !reset_replacement {
            return Err(conflict());
        }
        // This write lock serializes CAS replacement with competing finishes.
        sqlx::query("DELETE FROM secure_auth_attempts WHERE id=?")
            .bind(binding.id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(ApiError::internal)?;
    }
    if kind == "register" {
        available_signup(tx, &binding).await?;
    }
    let response = native
        .registration_response(
            &credential_identifier(&binding.scope, binding.generation).map_err(|_| invalid())?,
            &binding.request,
        )
        .map_err(|_| invalid())?;
    let mut expires_at = Utc::now().timestamp() + REGISTRATION_SECONDS;
    if let Some(reset_token) = &binding.reset_token_hash {
        let token_expiry:i64=sqlx::query_scalar("SELECT expires_at FROM account_recovery_tokens WHERE token_hash=? AND kind='reset' AND consumed_at IS NULL")
            .bind(reset_token).fetch_optional(&mut **tx).await.map_err(ApiError::internal)?.ok_or_else(denied)?;
        expires_at = expires_at.min(token_expiry);
        if expires_at <= Utc::now().timestamp() {
            return Err(denied());
        }
    }
    let pending = Pending {
        binding,
        response,
        expires_at,
    };
    let authorization = ticket(native, &digest, pending.expires_at);
    let bytes = Zeroizing::new(serde_json::to_vec(&pending).map_err(|_| unavailable())?);
    if bytes.len() > 65536 {
        return Err(invalid());
    }
    sqlx::query("INSERT INTO secure_auth_attempts(id,kind,user_id,device_id,username,binding_digest,state,authorization_hash,expires_at) VALUES(?,?,?,?,?,?,?,?,?)")
        .bind(pending.binding.id.to_string()).bind(kind).bind(pending.binding.scope.account.to_string())
        .bind(pending.binding.device.as_ref().map(|v|v.id().to_string())).bind(&pending.binding.account.username)
        .bind(&digest).bind(bytes.as_slice()).bind(token_hash(&authorization)).bind(pending.expires_at)
        .execute(&mut **tx).await.map_err(ApiError::internal)?;
    Ok(pending.response(&authorization))
}

async fn available_signup(tx: &mut Tx<'_>, binding: &Binding) -> Result<(), ApiError> {
    let signup = binding.signup.as_ref().ok_or_else(invalid)?;
    let DeviceBinding::Prospective { id, token_sha256 } = &signup.device else {
        return Err(invalid());
    };
    let token_hash = legacy_token_hash(token_sha256).map_err(|_| invalid())?;
    let used: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id=? OR username=? COLLATE NOCASE OR display_name=? COLLATE NOCASE OR public_handle=? COLLATE NOCASE) OR EXISTS(SELECT 1 FROM devices WHERE id=? OR token_hash=?) OR EXISTS(SELECT 1 FROM secure_auth_attempts WHERE kind='register' AND id<>? AND (user_id=? OR device_id=? OR username=? COLLATE NOCASE))")
        .bind(binding.scope.account.to_string()).bind(&signup.signup.username).bind(&signup.signup.display_name)
        .bind(&signup.signup.username)
        .bind(id.to_string()).bind(token_hash).bind(binding.id.to_string()).bind(binding.scope.account.to_string())
        .bind(id.to_string()).bind(&signup.signup.username).fetch_one(&mut **tx).await.map_err(ApiError::internal)?;
    if used {
        return Err(ApiError::conflict(
            "account_unavailable",
            "That account name or registration is unavailable",
        ));
    }
    Ok(())
}

pub(super) async fn authorized(
    tx: &mut Tx<'_>,
    id: Uuid,
    kind: &str,
    authorization: &str,
) -> Result<Pending, ApiError> {
    if authorization.len() != 43 {
        return Err(denied());
    }
    let row = sqlx::query("SELECT kind,binding_digest,state,expires_at FROM secure_auth_attempts WHERE id=? AND kind=? AND authorization_hash=? AND expires_at>?")
        .bind(id.to_string()).bind(kind).bind(token_hash(authorization)).bind(Utc::now().timestamp())
        .fetch_optional(&mut **tx).await.map_err(ApiError::internal)?.ok_or_else(denied)?;
    let pending = saved(&row)?;
    if pending.binding.id != id {
        return Err(unavailable());
    }
    Ok(pending)
}

pub(super) async fn consume(tx: &mut Tx<'_>, id: Uuid) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM secure_auth_attempts WHERE id=?")
        .bind(id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(ApiError::internal)?;
    Ok(())
}

pub(crate) async fn register_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<RegisterStart>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(request) = request.map_err(|_| invalid())?;
    authentication::rate(&state, &headers).await?;
    let (origin, network) = service(&state).await?;
    if request.scope.origin != origin || request.scope.network != network {
        return Err(invalid());
    }
    let signup = SignupStartBinding {
        format: 1,
        scope: request.scope.clone(),
        id: request.operation_id,
        generation: request.generation,
        signup: SignupMetadata {
            username: request.username,
            display_name: request.display_name,
            device_name: request.device_name,
        },
        device: request.device.clone(),
        identity: request.identity.clone(),
        request_sha256: auth::registration_request_digest(&request.request)
            .map_err(|_| invalid())?,
        previous_binding_sha256: request.previous_binding_sha256,
    };
    signup.verify(&request.signature).map_err(|_| invalid())?;
    let binding = Binding {
        account: auth::AccountContext {
            origin,
            network,
            username: signup.signup.username.clone(),
        },
        scope: request.scope,
        id: request.operation_id,
        generation: request.generation,
        device: Some(request.device),
        identity: Some(request.identity),
        expected: Precondition::default(),
        request: request.request,
        signup: Some(signup),
        legacy_fingerprint: None,
        reset_token_hash: None,
    };
    let native = load_native(&state.pool).await.map_err(|_| unavailable())?;
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(ApiError::internal)?;
    let response = begin(&mut tx, &native, "register", binding).await?;
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(response))
}

/// Verify legacy password off the `SQLite` write lock, then compare its complete
/// slow-verifier fingerprint again inside the commit transaction.
pub(super) async fn legacy_proof(
    state: &AppState,
    user: Uuid,
    password: String,
) -> Result<String, ApiError> {
    let _permit = state.password_work.try_acquire().map_err(|_| ApiError {
        status: StatusCode::TOO_MANY_REQUESTS,
        code: "rate_limited",
        message: "Please try again shortly".into(),
    })?;
    let password = Zeroizing::new(password);
    validate_password(&password).map_err(|_| invalid())?;
    let encoded: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=? AND NOT EXISTS(SELECT 1 FROM secure_credentials WHERE user_id=?)")
        .bind(user.to_string()).bind(user.to_string()).fetch_optional(&state.pool).await.map_err(ApiError::internal)?.flatten();
    let encoded = encoded.ok_or_else(denied)?;
    let fingerprint = authentication::digest("wisp-legacy-migration-verifier-v1", &encoded)?;
    let encoded = Zeroizing::new(encoded);
    let valid = tokio::task::spawn_blocking(move || {
        let Ok(hash) = PasswordHash::new(&encoded) else {
            return false;
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .is_ok()
    })
    .await
    .map_err(ApiError::internal)?;
    if !valid {
        return Err(denied());
    }
    Ok(fingerprint)
}
async fn check_legacy(tx: &mut Tx<'_>, binding: &Binding) -> Result<(), ApiError> {
    let row = sqlx::query("SELECT u.username,u.password_hash,c.generation FROM users u LEFT JOIN secure_credentials c ON c.user_id=u.id WHERE u.id=?")
        .bind(binding.scope.account.to_string()).fetch_optional(&mut **tx).await.map_err(ApiError::internal)?.ok_or_else(changed)?;
    let password = row
        .get::<Option<String>, _>("password_hash")
        .ok_or_else(changed)?;
    if row.get::<Option<String>, _>("generation").is_some()
        || Some(authentication::digest(
            "wisp-legacy-migration-verifier-v1",
            &password,
        )?) != binding.legacy_fingerprint
        || auth::canonical_username(&row.get::<String, _>("username")).map_err(|_| unavailable())?
            != binding.account.username
    {
        return Err(changed());
    }
    if Some(vault::own_identity(tx, binding.scope.account).await?) != binding.identity {
        return Err(changed());
    }
    Ok(())
}

pub(crate) async fn migrate_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<MigrateStart>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(request) = request.map_err(|_| invalid())?;
    authentication::rate(&state, &headers).await?;
    let user = authenticate_headers(&state, &headers).await?;
    let fingerprint = legacy_proof(&state, user, request.legacy_password).await?;
    let account = account_state(&state, user).await?;
    if account.expected != Precondition::default() || account.identity.is_none() {
        return Err(changed());
    }
    let native = load_native(&state.pool).await.map_err(|_| unavailable())?;
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(ApiError::internal)?;
    let session = authentication::session(&mut tx, &headers).await?;
    if session.user != user {
        return Err(denied());
    }
    let binding = Binding {
        account: auth::AccountContext {
            origin: account.scope.origin.clone(),
            network: account.scope.network,
            username: account.username,
        },
        scope: account.scope,
        id: request.operation_id,
        generation: request.generation,
        device: Some(DeviceBinding::Existing { id: session.device }),
        identity: account.identity,
        expected: Precondition::default(),
        request: request.request,
        signup: None,
        legacy_fingerprint: Some(fingerprint),
        reset_token_hash: None,
    };
    check_legacy(&mut tx, &binding).await?;
    let response = begin(&mut tx, &native, "migrate", binding).await?;
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(response))
}

pub(crate) async fn password_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<PasswordStart>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(request) = request.map_err(|_| invalid())?;
    authentication::rate(&state, &headers).await?;
    let user = authenticate_headers(&state, &headers).await?;
    let account = account_state(&state, user).await?;
    if account.expected != request.expected
        || request.expected.credential_generation.is_none()
        || request.expected.credential_generation == Some(request.generation)
    {
        return Err(changed());
    }
    let native = load_native(&state.pool).await.map_err(|_| unavailable())?;
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(ApiError::internal)?;
    let session = authentication::session(&mut tx, &headers).await?;
    if session.user != user || current_precondition(&mut tx, user).await? != request.expected {
        return Err(changed());
    }
    let binding = Binding {
        account: auth::AccountContext {
            origin: account.scope.origin.clone(),
            network: account.scope.network,
            username: account.username,
        },
        scope: account.scope,
        id: request.operation_id,
        generation: request.generation,
        device: Some(DeviceBinding::Existing { id: session.device }),
        identity: account.identity,
        expected: request.expected,
        request: request.request,
        signup: None,
        legacy_fingerprint: None,
        reset_token_hash: None,
    };
    let response = begin(&mut tx, &native, "password", binding).await?;
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(response))
}

pub(super) async fn insert_credential(
    tx: &mut Tx<'_>,
    scope: &Scope,
    generation: Uuid,
    record: &[u8],
) -> Result<(), ApiError> {
    require_unused_generation(tx, generation).await?;
    sqlx::query("INSERT INTO secure_credentials(user_id,generation,password_file,setup_digest,updated_at) SELECT ?,?,?,digest,? FROM secure_auth_setup WHERE id=1")
        .bind(scope.account.to_string()).bind(generation.to_string()).bind(record).bind(Utc::now().timestamp()).execute(&mut **tx).await.map_err(ApiError::internal)?;
    Ok(())
}
pub(super) async fn replace_credential(
    tx: &mut Tx<'_>,
    scope: &Scope,
    expected: Uuid,
    generation: Uuid,
    record: &[u8],
) -> Result<(), ApiError> {
    require_unused_generation(tx, generation).await?;
    let result = sqlx::query("UPDATE secure_credentials SET generation=?,password_file=?,updated_at=? WHERE user_id=? AND generation=?")
        .bind(generation.to_string()).bind(record).bind(Utc::now().timestamp()).bind(scope.account.to_string()).bind(expected.to_string()).execute(&mut **tx).await.map_err(ApiError::internal)?;
    if result.rows_affected() != 1 {
        return Err(changed());
    }
    Ok(())
}

async fn insert_vault(
    tx: &mut Tx<'_>,
    scope: &Scope,
    wrapper: &KeyEnvelope,
    envelope: &Envelope,
) -> Result<(), ApiError> {
    let checkpoint = envelope.manifest.checkpoint().map_err(|_| invalid())?;
    let encoded = serde_json::to_string(envelope).map_err(|_| invalid())?;
    if encoded.len() > MAX_ENVELOPE_WIRE {
        return Err(invalid());
    }
    let now = Utc::now().timestamp();
    sqlx::query("INSERT INTO account_vault_manifests(user_id,revision,digest,manifest,created_at) VALUES(?,1,?,?,?)")
        .bind(scope.account.to_string()).bind(&checkpoint.sha256).bind(serde_json::to_string(&envelope.manifest).map_err(|_|invalid())?).bind(now).execute(&mut **tx).await.map_err(ApiError::internal)?;
    sqlx::query("INSERT INTO account_vaults(user_id,wrapper_generation,wrapper,revision,digest,envelope,updated_at) VALUES(?,?,?,1,?,?,?)")
        .bind(scope.account.to_string()).bind(wrapper.credential_generation.to_string()).bind(serde_json::to_string(wrapper).map_err(|_|invalid())?)
        .bind(&checkpoint.sha256).bind(encoded).bind(now).execute(&mut **tx).await.map_err(ApiError::internal)?;
    Ok(())
}

#[allow(clippy::too_many_lines)] // Whole enrollment and receipt are one atomic transaction.
async fn enroll(
    state: AppState,
    headers: HeaderMap,
    request: EnrollFinish,
    kind: &str,
) -> Result<Json<Value>, ApiError> {
    authentication::rate(&state, &headers).await?;
    let effect = request.operation.digest().map_err(|_| invalid())?;
    let Change::Enroll {
        generation,
        registration_sha256,
        wrapper_sha256,
        vault: checkpoint,
        signup,
    } = &request.operation.change
    else {
        return Err(invalid());
    };
    let (origin, network) = service(&state).await?;
    if request.operation.scope.origin != origin
        || request.operation.scope.network != network
        || request.registration.scope != request.operation.scope
        || request.registration.generation != *generation
        || request.registration.digest().map_err(|_| invalid())? != *registration_sha256
        || request.wrapper.digest().map_err(|_| invalid())? != *wrapper_sha256
    {
        return Err(invalid());
    }
    request
        .wrapper
        .validate(&request.operation.scope, *generation)
        .map_err(|_| invalid())?;
    let record =
        auth::Server::registration_record(&request.registration.upload).map_err(|_| invalid())?;
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(ApiError::internal)?;
    let session = if kind == "migrate" {
        Some(authentication::session(&mut tx, &headers).await?)
    } else {
        None
    };
    if let Some(session) = session {
        vault::bind_operation(&request.operation, &request.operation.scope, session)?;
    }
    let identity = if kind == "register" {
        request.vault.manifest.header.signer.clone()
    } else {
        vault::own_identity(&mut tx, request.operation.scope.account).await?
    };
    request
        .operation
        .verify(&identity, &request.signature)
        .map_err(|_| invalid())?;
    request
        .vault
        .verify(&request.operation.scope, &identity)
        .map_err(|_| invalid())?;
    if request
        .vault
        .manifest
        .advance(&request.operation.scope, &identity, None)
        .map_err(|_| invalid())?
        != *checkpoint
    {
        return Err(invalid());
    }
    if let Some(receipt) = vault::existing_receipt(
        &mut tx,
        request.operation.scope.account,
        request.operation.id,
        kind,
        &effect,
    )
    .await?
    {
        if kind == "register" {
            let proof: Option<String> = sqlx::query_scalar(
                "SELECT authorization_hash FROM secure_operation_receipts WHERE id=?",
            )
            .bind(request.operation.id.to_string())
            .fetch_one(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
            if proof != Some(token_hash(&request.authorization))
                || vault::own_identity(&mut tx, request.operation.scope.account).await? != identity
            {
                return Err(denied());
            }
        }
        return Ok(Json(receipt));
    }
    let pending = authorized(&mut tx, request.operation.id, kind, &request.authorization).await?;
    pending.check_transcript(&request.registration)?;
    if pending.binding.scope != request.operation.scope
        || pending.binding.expected != request.operation.expected
        || pending.binding.device.as_ref() != Some(&request.operation.device)
        || pending.binding.identity.as_ref() != Some(&identity)
        || pending.binding.signup.as_ref().map(|s| &s.signup) != signup.as_ref()
    {
        return Err(invalid());
    }
    let display_name = if kind == "register" {
        available_signup(&mut tx, &pending.binding).await?;
        let signup = signup.as_ref().ok_or_else(invalid)?;
        let DeviceBinding::Prospective { id, token_sha256 } = &request.operation.device else {
            return Err(invalid());
        };
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO users(id,username,public_handle,display_name,password_hash,server_member,created_at) VALUES(?,?,?,?,NULL,0,?)")
            .bind(request.operation.scope.account.to_string()).bind(&signup.username).bind(&signup.username).bind(&signup.display_name).bind(&now).execute(&mut *tx).await.map_err(ApiError::internal)?;
        sqlx::query("INSERT INTO chat_identities(user_id,public_identity) VALUES(?,?)")
            .bind(request.operation.scope.account.to_string())
            .bind(serde_json::to_string(&identity).map_err(|_| invalid())?)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
        sqlx::query("INSERT INTO devices(id,user_id,name,token_hash,created_at,last_seen_at) VALUES(?,?,?,?,?,?)")
            .bind(id.to_string()).bind(request.operation.scope.account.to_string()).bind(&signup.device_name).bind(legacy_token_hash(token_sha256).map_err(|_|invalid())?).bind(&now).bind(&now).execute(&mut *tx).await.map_err(ApiError::internal)?;
        signup.display_name.clone()
    } else {
        check_legacy(&mut tx, &pending.binding).await?;
        sqlx::query("UPDATE users SET password_hash=NULL WHERE id=?")
            .bind(request.operation.scope.account.to_string())
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
        sqlx::query_scalar("SELECT display_name FROM users WHERE id=?")
            .bind(request.operation.scope.account.to_string())
            .fetch_one(&mut *tx)
            .await
            .map_err(ApiError::internal)?
    };
    insert_credential(&mut tx, &request.operation.scope, *generation, &record).await?;
    insert_vault(
        &mut tx,
        &request.operation.scope,
        &request.wrapper,
        &request.vault,
    )
    .await?;
    consume(&mut tx, request.operation.id).await?;
    let result = json!({"device_id":request.operation.device.id(),"user":{"id":request.operation.scope.account,"display_name":display_name},"scope":request.operation.scope,"username":pending.binding.account.username,"identity":identity,"credential_generation":generation});
    vault::store_receipt(&mut tx, &request.operation, kind, &result).await?;
    sqlx::query("UPDATE secure_operation_receipts SET authorization_hash=? WHERE id=?")
        .bind(token_hash(&request.authorization))
        .bind(request.operation.id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(result))
}

pub(crate) async fn register_finish(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<EnrollFinish>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    enroll(
        state,
        headers,
        request.map_err(|_| invalid())?.0,
        "register",
    )
    .await
}
pub(crate) async fn migrate_finish(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<EnrollFinish>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    enroll(state, headers, request.map_err(|_| invalid())?.0, "migrate").await
}

pub(crate) async fn password_finish(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<PasswordFinish>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(request) = request.map_err(|_| invalid())?;
    authentication::rate(&state, &headers).await?;
    let Change::ChangePassword {
        generation,
        registration_sha256,
        wrapper_sha256,
    } = &request.operation.change
    else {
        return Err(invalid());
    };
    if request.registration.scope != request.operation.scope
        || request.registration.generation != *generation
        || request.registration.digest().map_err(|_| invalid())? != *registration_sha256
        || request.wrapper.digest().map_err(|_| invalid())? != *wrapper_sha256
    {
        return Err(invalid());
    }
    request
        .wrapper
        .validate(&request.operation.scope, *generation)
        .map_err(|_| invalid())?;
    let record =
        auth::Server::registration_record(&request.registration.upload).map_err(|_| invalid())?;
    let (origin, network) = service(&state).await?;
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(ApiError::internal)?;
    let session = authentication::session(&mut tx, &headers).await?;
    let scope = Scope {
        origin,
        network,
        account: session.user,
    };
    let effect = vault::bind_operation(&request.operation, &scope, session)?;
    let identity = vault::own_identity(&mut tx, session.user).await?;
    request
        .operation
        .verify(&identity, &request.signature)
        .map_err(|_| invalid())?;
    if let Some(receipt) = vault::existing_receipt(
        &mut tx,
        session.user,
        request.operation.id,
        "password",
        &effect,
    )
    .await?
    {
        return Ok(Json(receipt));
    }
    let pending = authorized(
        &mut tx,
        request.operation.id,
        "password",
        &request.authorization,
    )
    .await?;
    pending.check_transcript(&request.registration)?;
    if pending.binding.device.as_ref() != Some(&request.operation.device)
        || pending.binding.identity.as_ref() != Some(&identity)
        || pending.binding.expected != request.operation.expected
        || current_precondition(&mut tx, session.user).await? != request.operation.expected
    {
        return Err(changed());
    }
    vault::consume_grant(&mut tx, &request.operation, &request.grant).await?;
    replace_credential(
        &mut tx,
        &scope,
        request
            .operation
            .expected
            .credential_generation
            .ok_or_else(invalid)?,
        *generation,
        &record,
    )
    .await?;
    sqlx::query(
        "UPDATE account_vaults SET wrapper_generation=?,wrapper=?,updated_at=? WHERE user_id=?",
    )
    .bind(generation.to_string())
    .bind(serde_json::to_string(&request.wrapper).map_err(|_| invalid())?)
    .bind(Utc::now().timestamp())
    .bind(session.user.to_string())
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;
    consume(&mut tx, request.operation.id).await?;
    let result = json!({"committed":true,"operation_id":request.operation.id,"operation_sha256":effect,"scope":scope,"credential_generation":generation,"wrapper_generation":generation,"vault":request.operation.expected.vault});
    vault::store_receipt(&mut tx, &request.operation, "password", &result).await?;
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(result))
}
