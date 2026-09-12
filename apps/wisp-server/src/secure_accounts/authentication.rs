use super::*;
use argon2::password_hash::rand_core::RngCore;
use axum::extract::rejection::JsonRejection;
use wisp_crypto::account_vault::{
    device::legacy_token_hash,
    operation::{DeviceBinding, Operation, credential_identifier},
};
use zeroize::Zeroize;

const ATTEMPT_SECONDS: i64 = 180;
const MAX_PENDING: i64 = 1024;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoginStart {
    attempt: Uuid,
    username: String,
    device: DeviceBinding,
    device_name: String,
    request: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProofFinish {
    attempt: Uuid,
    finalization: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UnlockStart {
    attempt: Uuid,
    request: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReauthStart {
    attempt: Uuid,
    request: String,
    operation: Operation,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    context: auth::LoginContext,
    native: Vec<u8>,
    response: String,
    expires_at: i64,
    user: Option<Uuid>,
    device_name: String,
    operation: Option<Operation>,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.native.zeroize();
    }
}
impl Pending {
    fn response(&self) -> Value {
        json!({"context":self.context,"response":self.response,"expires_at":self.expires_at})
    }
}

#[derive(Clone, Copy)]
pub(super) struct Session {
    pub user: Uuid,
    pub device: Uuid,
}

pub(super) async fn session(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    headers: &HeaderMap,
) -> Result<Session, ApiError> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(denied)?;
    let row = sqlx::query("SELECT d.user_id,d.id FROM sessions s JOIN devices d ON d.id=s.device_id WHERE s.token_hash=? AND s.expires_at>? AND s.revoked_at IS NULL AND d.revoked_at IS NULL")
        .bind(token_hash(token)).bind(Utc::now().to_rfc3339()).fetch_optional(&mut **tx).await.map_err(ApiError::internal)?.ok_or_else(denied)?;
    Ok(Session {
        user: parse_uuid(&row.get::<String, _>("user_id"))?,
        device: parse_uuid(&row.get::<String, _>("id"))?,
    })
}

fn denied() -> ApiError {
    ApiError::unauthorized("Secure authentication failed")
}

pub(super) fn digest<T: Serialize>(domain: &str, value: &T) -> Result<String, ApiError> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(domain, value)).map_err(|_| invalid())?)
    ))
}

pub(super) fn fresh_token() -> String {
    let mut bytes = Zeroizing::new([0_u8; 32]);
    OsRng.fill_bytes(bytes.as_mut());
    URL_SAFE_NO_PAD.encode(bytes.as_slice())
}

pub(super) async fn rate(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    account_membership::rate(state, "secure-auth-global".into(), 1200).await?;
    account_membership::rate(state, login_rate_key(headers, "secure-auth-peer"), 120).await
}

pub(super) async fn expire_and_bound(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM secure_auth_attempts WHERE expires_at<=?")
        .bind(Utc::now().timestamp())
        .execute(&mut **tx)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query("DELETE FROM secure_reauth_grants WHERE expires_at<=?")
        .bind(Utc::now().timestamp())
        .execute(&mut **tx)
        .await
        .map_err(ApiError::internal)?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM secure_auth_attempts")
        .fetch_one(&mut **tx)
        .await
        .map_err(ApiError::internal)?;
    if count >= MAX_PENDING {
        return Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "rate_limited",
            message: "Please try again shortly".into(),
        });
    }
    Ok(())
}

fn saved(bytes: Vec<u8>) -> Result<Pending, ApiError> {
    let bytes = Zeroizing::new(bytes);
    if bytes.len() > 65_536 {
        return Err(unavailable());
    }
    serde_json::from_slice(&bytes).map_err(|_| unavailable())
}

async fn existing(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    attempt: Uuid,
    kind: &str,
    binding: &str,
) -> Result<Option<Pending>, ApiError> {
    let row = sqlx::query("SELECT kind,binding_digest,state FROM secure_auth_attempts WHERE id=?")
        .bind(attempt.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(ApiError::internal)?;
    if let Some(row) = row {
        if row.get::<String, _>("kind") != kind || row.get::<String, _>("binding_digest") != binding
        {
            return Err(ApiError::conflict(
                "operation_conflict",
                "Authentication attempt differs from its saved request",
            ));
        }
        return Ok(Some(saved(row.get("state"))?));
    }
    let committed: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM secure_operation_receipts WHERE id=?)")
            .bind(attempt.to_string())
            .fetch_one(&mut **tx)
            .await
            .map_err(ApiError::internal)?;
    if committed {
        return Err(ApiError::conflict(
            "operation_committed",
            "This operation has already completed",
        ));
    }
    Ok(None)
}

async fn save(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    kind: &str,
    binding: &str,
    pending: &Pending,
) -> Result<(), ApiError> {
    let bytes = Zeroizing::new(serde_json::to_vec(pending).map_err(|_| unavailable())?);
    sqlx::query("INSERT INTO secure_auth_attempts(id,kind,user_id,device_id,username,binding_digest,state,expires_at) VALUES(?,?,?,?,?,?,?,?)")
        .bind(pending.context.attempt.to_string()).bind(kind).bind(pending.user.map(|id|id.to_string())).bind(match pending.context.intent {auth::Intent::SignIn {device,..}|auth::Intent::Unlock {device}|auth::Intent::Reauthenticate {device,..} => device.to_string()})
        .bind(&pending.context.account.username).bind(binding).bind(bytes.as_slice()).bind(pending.expires_at).execute(&mut **tx).await.map_err(ApiError::internal)?;
    Ok(())
}

fn dummy_id(setup: &auth::Server, domain: &str, username: &str) -> Uuid {
    let key = setup.save();
    let mut mac =
        Hmac::<Sha256>::new_from_slice(key.as_slice()).expect("HMAC accepts this key length");
    mac.update(domain.as_bytes());
    mac.update(&[0]);
    mac.update(username.as_bytes());
    let digest = mac.finalize().into_bytes();
    let mut bytes: [u8; 16] = digest[..16].try_into().expect("SHA-256 length");
    bytes[6] = (bytes[6] & 15) | 64;
    bytes[8] = (bytes[8] & 63) | 128;
    Uuid::from_bytes(bytes)
}

pub(crate) async fn login_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<LoginStart>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(mut request) = request.map_err(|_| invalid())?;
    rate(&state, &headers).await?;
    request.username = auth::canonical_username(&request.username).map_err(|_| invalid())?;
    validate_device_name(&request.device_name)?;
    if request.attempt.is_nil() || request.request.len() > auth::MAX_HANDSHAKE_WIRE {
        return Err(invalid());
    }
    let DeviceBinding::Prospective { id, token_sha256 } = &request.device else {
        return Err(invalid());
    };
    if id.is_nil() {
        return Err(invalid());
    }
    legacy_token_hash(token_sha256).map_err(|_| invalid())?;
    let (origin, network) = service(&state).await?;
    let native = load_native(&state.pool).await.map_err(|_| unavailable())?;
    let binding = digest("wisp-login-start-v1", &request)?;
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(ApiError::internal)?;
    expire_and_bound(&mut tx).await?;
    if let Some(existing) = existing(&mut tx, request.attempt, "login", &binding).await? {
        return Ok(Json(existing.response()));
    }
    let row = sqlx::query("SELECT u.id,c.generation,c.password_file FROM users u JOIN secure_credentials c ON c.user_id=u.id WHERE u.username=? COLLATE NOCASE")
        .bind(&request.username).fetch_optional(&mut *tx).await.map_err(ApiError::internal)?;
    let (user, generation, record) = if let Some(row) = row {
        (
            Some(parse_uuid(&row.get::<String, _>("id"))?),
            parse_uuid(&row.get::<String, _>("generation"))?,
            Some(Zeroizing::new(row.get::<Vec<u8>, _>("password_file"))),
        )
    } else {
        (
            None,
            dummy_id(&native, "generation", &request.username),
            None,
        )
    };
    let scope = Scope {
        origin: origin.clone(),
        network,
        account: user.unwrap_or_else(|| dummy_id(&native, "account", &request.username)),
    };
    let context = auth::LoginContext {
        account: auth::AccountContext {
            origin,
            network,
            username: request.username,
        },
        attempt: request.attempt,
        credential_generation: generation,
        intent: auth::Intent::SignIn {
            device: *id,
            token_sha256: token_sha256.clone(),
        },
    };
    let identifier = credential_identifier(&scope, generation).map_err(|_| unavailable())?;
    let (attempt, response) = native
        .login(
            context.clone(),
            &identifier,
            record.as_deref().map(Vec::as_slice),
            &request.request,
        )
        .map_err(|_| invalid())?;
    let mut native = attempt.save().map_err(|_| unavailable())?;
    let pending = Pending {
        context,
        native: std::mem::take(&mut *native),
        response,
        expires_at: Utc::now().timestamp() + ATTEMPT_SECONDS,
        user,
        device_name: request.device_name,
        operation: None,
    };
    save(&mut tx, "login", &binding, &pending).await?;
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(pending.response()))
}

async fn start_existing(
    state: AppState,
    headers: HeaderMap,
    attempt: Uuid,
    request: String,
    operation: Option<Operation>,
) -> Result<Json<Value>, ApiError> {
    rate(&state, &headers).await?;
    if attempt.is_nil() || request.len() > auth::MAX_HANDSHAKE_WIRE {
        return Err(invalid());
    }
    let (origin, network) = service(&state).await?;
    let native = load_native(&state.pool).await.map_err(|_| unavailable())?;
    let kind = if operation.is_some() {
        "reauth"
    } else {
        "unlock"
    };
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(ApiError::internal)?;
    let session = session(&mut tx, &headers).await?;
    let binding = digest(
        "wisp-existing-device-proof-v1",
        &(
            kind,
            attempt,
            &request,
            &operation,
            session.user,
            session.device,
        ),
    )?;
    expire_and_bound(&mut tx).await?;
    if let Some(existing) = existing(&mut tx, attempt, kind, &binding).await? {
        return Ok(Json(existing.response()));
    }
    let row = sqlx::query("SELECT u.username,c.generation,c.password_file,v.wrapper_generation,v.revision,v.digest FROM secure_credentials c JOIN users u ON u.id=c.user_id LEFT JOIN account_vaults v ON v.user_id=c.user_id WHERE c.user_id=?")
        .bind(session.user.to_string()).fetch_optional(&mut *tx).await.map_err(ApiError::internal)?.ok_or_else(denied)?;
    let username =
        auth::canonical_username(&row.get::<String, _>("username")).map_err(|_| unavailable())?;
    let generation = parse_uuid(&row.get::<String, _>("generation"))?;
    let scope = Scope {
        origin: origin.clone(),
        network,
        account: session.user,
    };
    let intent = if let Some(operation) = &operation {
        operation.validate().map_err(|_| invalid())?;
        let expected = current_precondition(&mut tx, session.user).await?;
        if operation.scope != scope
            || operation.device != (DeviceBinding::Existing { id: session.device })
            || operation.expected != expected
        {
            return Err(ApiError::conflict(
                "account_state_changed",
                "Account state changed; refresh before continuing",
            ));
        }
        auth::Intent::Reauthenticate {
            device: session.device,
            operation_sha256: operation.digest().map_err(|_| invalid())?,
        }
    } else {
        auth::Intent::Unlock {
            device: session.device,
        }
    };
    let context = auth::LoginContext {
        account: auth::AccountContext {
            origin,
            network,
            username,
        },
        attempt,
        credential_generation: generation,
        intent,
    };
    let record = Zeroizing::new(row.get::<Vec<u8>, _>("password_file"));
    let (attempt, response) = native
        .login(
            context.clone(),
            &credential_identifier(&scope, generation).map_err(|_| unavailable())?,
            Some(&record),
            &request,
        )
        .map_err(|_| invalid())?;
    let mut native = attempt.save().map_err(|_| unavailable())?;
    let pending = Pending {
        context,
        native: std::mem::take(&mut *native),
        response,
        expires_at: Utc::now().timestamp() + ATTEMPT_SECONDS,
        user: Some(session.user),
        device_name: String::new(),
        operation,
    };
    save(&mut tx, kind, &binding, &pending).await?;
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(pending.response()))
}

pub(crate) async fn unlock_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<UnlockStart>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(request) = request.map_err(|_| invalid())?;
    start_existing(state, headers, request.attempt, request.request, None).await
}
pub(crate) async fn reauth_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<ReauthStart>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(request) = request.map_err(|_| invalid())?;
    start_existing(
        state,
        headers,
        request.attempt,
        request.request,
        Some(request.operation),
    )
    .await
}

#[allow(clippy::too_many_lines)] // Keep proof consumption and credential/grant commit in one transaction.
async fn finish(
    state: AppState,
    headers: HeaderMap,
    request: ProofFinish,
    kind: &str,
) -> Result<Json<Value>, ApiError> {
    rate(&state, &headers).await?;
    if request.attempt.is_nil() || request.finalization.len() > auth::MAX_HANDSHAKE_WIRE {
        return Err(invalid());
    }
    let effect = digest("wisp-proof-finish-v1", &(kind, &request))?;
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(ApiError::internal)?;
    if kind == "login" {
        let row = sqlx::query(
            "SELECT kind,effect_digest,result FROM secure_operation_receipts WHERE id=?",
        )
        .bind(request.attempt.to_string())
        .fetch_optional(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
        if let Some(row) = row {
            if row.get::<String, _>("kind") != "login"
                || row.get::<String, _>("effect_digest") != effect
            {
                return Err(denied());
            }
            return Ok(Json(
                serde_json::from_str(&row.get::<String, _>("result")).map_err(|_| unavailable())?,
            ));
        }
    }
    let row = sqlx::query(
        "SELECT state FROM secure_auth_attempts WHERE id=? AND kind=? AND expires_at>?",
    )
    .bind(request.attempt.to_string())
    .bind(kind)
    .bind(Utc::now().timestamp())
    .fetch_optional(&mut *tx)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(denied)?;
    let pending = saved(row.get("state"))?;
    if pending.context.attempt != request.attempt {
        return Err(unavailable());
    }
    let device = match pending.context.intent {
        auth::Intent::SignIn { device, .. }
        | auth::Intent::Unlock { device }
        | auth::Intent::Reauthenticate { device, .. } => device,
    };
    if kind != "login" {
        let session = session(&mut tx, &headers).await?;
        if pending.user != Some(session.user) || device != session.device {
            return Err(denied());
        }
    }
    let attempt = auth::ServerAttempt::restore(&pending.context, &pending.native)
        .map_err(|_| unavailable())?;
    sqlx::query("DELETE FROM secure_auth_attempts WHERE id=?")
        .bind(request.attempt.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    if attempt.finish(&request.finalization).is_err() || pending.user.is_none() {
        tx.commit().await.map_err(ApiError::internal)?;
        return Err(denied());
    }
    let user = pending.user.expect("checked account");
    let row = sqlx::query("SELECT u.display_name,u.username,c.generation,i.public_identity FROM secure_credentials c JOIN users u ON u.id=c.user_id JOIN chat_identities i ON i.user_id=c.user_id WHERE c.user_id=?")
        .bind(user.to_string()).fetch_optional(&mut *tx).await.map_err(ApiError::internal)?.ok_or_else(denied)?;
    let generation = parse_uuid(&row.get::<String, _>("generation"))?;
    if generation != pending.context.credential_generation
        || auth::canonical_username(&row.get::<String, _>("username")).map_err(|_| unavailable())?
            != pending.context.account.username
    {
        return Err(denied());
    }
    let result = match (&pending.context.intent, kind) {
        (auth::Intent::SignIn { token_sha256, .. }, "login") => {
            let duplicate: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM devices WHERE id=? OR token_hash=?)",
            )
            .bind(device.to_string())
            .bind(legacy_token_hash(token_sha256).map_err(|_| unavailable())?)
            .fetch_one(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
            if duplicate {
                return Err(ApiError::conflict(
                    "device_conflict",
                    "This device credential already exists",
                ));
            }
            let now = Utc::now().to_rfc3339();
            sqlx::query("INSERT INTO devices(id,user_id,name,token_hash,created_at,last_seen_at) VALUES(?,?,?,?,?,?)")
                .bind(device.to_string()).bind(user.to_string()).bind(&pending.device_name).bind(legacy_token_hash(token_sha256).map_err(|_|unavailable())?).bind(&now).bind(&now).execute(&mut *tx).await.map_err(ApiError::internal)?;
            let identity: PublicIdentity =
                serde_json::from_str(&row.get::<String, _>("public_identity"))
                    .map_err(|_| unavailable())?;
            identity.validate().map_err(|_| unavailable())?;
            json!({"device_id":device,"user":{"id":user,"display_name":row.get::<String,_>("display_name")},"scope":Scope {origin:pending.context.account.origin.clone(),network:pending.context.account.network,account:user},"username":pending.context.account.username,"identity":identity,"credential_generation":generation})
        }
        (auth::Intent::Unlock { .. }, "unlock") => {
            json!({"confirmed":true,"credential_generation":generation})
        }
        (
            auth::Intent::Reauthenticate {
                operation_sha256, ..
            },
            "reauth",
        ) => {
            let operation = pending.operation.as_ref().ok_or_else(unavailable)?;
            if current_precondition(&mut tx, user).await? != operation.expected {
                return Err(ApiError::conflict(
                    "account_state_changed",
                    "Account state changed; refresh before continuing",
                ));
            }
            if operation.digest().map_err(|_| unavailable())? != *operation_sha256 {
                return Err(unavailable());
            }
            let grant = fresh_token();
            let expires = Utc::now().timestamp() + 120;
            sqlx::query("INSERT INTO secure_reauth_grants(token_hash,user_id,device_id,generation,effect_digest,expires_at) VALUES(?,?,?,?,?,?)")
                .bind(token_hash(&grant)).bind(user.to_string()).bind(device.to_string()).bind(generation.to_string()).bind(operation_sha256).bind(expires).execute(&mut *tx).await.map_err(ApiError::internal)?;
            json!({"grant":grant,"operation_sha256":operation_sha256,"expires_at":expires})
        }
        _ => return Err(denied()),
    };
    if kind == "login" {
        sqlx::query("INSERT INTO secure_operation_receipts(id,user_id,device_id,kind,effect_digest,result,committed_at) VALUES(?,?,?,'login',?,?,?)")
            .bind(request.attempt.to_string()).bind(user.to_string()).bind(device.to_string()).bind(effect).bind(serde_json::to_string(&result).map_err(|_|unavailable())?).bind(Utc::now().timestamp()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    }
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(result))
}

pub(crate) async fn login_finish(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<ProofFinish>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    finish(state, headers, request.map_err(|_| invalid())?.0, "login").await
}
pub(crate) async fn unlock_finish(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<ProofFinish>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    finish(state, headers, request.map_err(|_| invalid())?.0, "unlock").await
}
pub(crate) async fn reauth_finish(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<ProofFinish>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    finish(state, headers, request.map_err(|_| invalid())?.0, "reauth").await
}
