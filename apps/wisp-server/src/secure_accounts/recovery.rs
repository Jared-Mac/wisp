//! Email tokens authorize credential recovery, never decryption or a device.
use super::registration::{Binding, changed};
use super::{
    ApiError, AppState, Deserialize, HeaderMap, Json, Row, Scope, Sha256, State, StatusCode, Utc,
    Uuid, Value, account_recovery, auth, authentication, current_precondition, invalid, json,
    load_native, parse_uuid, registration, service, unavailable, vault,
};
use axum::extract::rejection::JsonRejection;
use sha2::Digest;
use wisp_crypto::account_vault::operation::{
    Change, Operation, RegistrationTranscript, ResetEffect,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EmailEnrollment {
    grant: String,
    operation: Operation,
    email: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResetStart {
    operation_id: Uuid,
    token: String,
    generation: Uuid,
    request: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResetFinish {
    authorization: String,
    effect: ResetEffect,
    token: String,
    registration: RegistrationTranscript,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResetStatus {
    token: String,
    operation_id: Uuid,
    effect_sha256: String,
}

pub(crate) async fn enroll_email(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<EmailEnrollment>, JsonRejection>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let Json(request) = request.map_err(|_| invalid())?;
    if !state.recovery.available() {
        return Err(account_recovery::mail_unavailable());
    }
    state.recovery.rate(&headers, "enroll", 8, 80).await?;
    let email = account_recovery::normalize_email(&request.email)?;
    if request.operation.change
        != (Change::SetRecoveryEmail {
            email_sha256: format!("{:x}", Sha256::digest(email.as_bytes())),
        })
    {
        return Err(invalid());
    }
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
    if let Some(receipt) = vault::existing_receipt(
        &mut tx,
        session.user,
        request.operation.id,
        "recovery_email",
        &effect,
    )
    .await?
    {
        return Ok((StatusCode::ACCEPTED, Json(receipt)));
    }
    if current_precondition(&mut tx, session.user).await? != request.operation.expected {
        return Err(changed());
    }
    vault::consume_grant(&mut tx, &request.operation, &request.grant).await?;
    let generation = request
        .operation
        .expected
        .credential_generation
        .ok_or_else(invalid)?;
    let fingerprint =
        account_recovery::credential_fingerprint(None, Some(&generation.to_string()))?;
    let token = account_recovery::reserve_verification(
        &mut tx,
        &session.user.to_string(),
        &email,
        &fingerprint,
    )
    .await?;
    let result = json!({"ok":true,"operation_id":request.operation.id,"operation_sha256":effect});
    vault::store_receipt(&mut tx, &request.operation, "recovery_email", &result).await?;
    tx.commit().await.map_err(ApiError::internal)?;
    if let Some(token) = token {
        state.recovery.send(&email, "verify", &token).await?;
    }
    Ok((StatusCode::ACCEPTED, Json(result)))
}

pub(crate) async fn reset_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<ResetStart>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(request) = request.map_err(|_| invalid())?;
    state.recovery.rate(&headers, "token", 60, 600).await?;
    let token_hash = account_recovery::checked_token(&request.token)?;
    let (origin, network) = service(&state).await?;
    let native = load_native(&state.pool).await.map_err(|_| unavailable())?;
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(ApiError::internal)?;
    let row = account_recovery::live_token_in(&mut tx, &token_hash, "reset").await?;
    let user = parse_uuid(&row.get::<String, _>("user_id"))?;
    let expected = current_precondition(&mut tx, user).await?;
    if expected.credential_generation == Some(request.generation) {
        return Err(changed());
    }
    let username =
        auth::canonical_username(&row.get::<String, _>("username")).map_err(|_| unavailable())?;
    let binding = Binding {
        account: auth::AccountContext {
            origin: origin.clone(),
            network,
            username,
        },
        scope: Scope {
            origin,
            network,
            account: user,
        },
        id: request.operation_id,
        generation: request.generation,
        device: None,
        identity: None,
        expected,
        request: request.request,
        signup: None,
        legacy_fingerprint: None,
        reset_token_hash: Some(token_hash),
    };
    let result = registration::begin(&mut tx, &native, "reset", binding).await?;
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(result))
}

#[allow(clippy::too_many_lines)] // Consumed proof retry and new credential commit share one transaction.
pub(crate) async fn reset_finish(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<ResetFinish>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(request) = request.map_err(|_| invalid())?;
    state.recovery.rate(&headers, "token", 60, 600).await?;
    let effect = request.effect.digest().map_err(|_| invalid())?;
    let token_hash = account_recovery::checked_token(&request.token)?;
    if request.authorization.len() != 43 {
        return Err(invalid());
    }
    let authorization_hash = super::token_hash(&request.authorization);
    let (origin, network) = service(&state).await?;
    if request.effect.scope.origin != origin
        || request.effect.scope.network != network
        || request.registration.scope != request.effect.scope
        || request.registration.generation != request.effect.generation
        || request.registration.digest().map_err(|_| invalid())?
            != request.effect.registration_sha256
    {
        return Err(invalid());
    }
    let record =
        auth::Server::registration_record(&request.registration.upload).map_err(|_| invalid())?;
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(ApiError::internal)?;
    // Only this same consumed bearer proof can read an unauthenticated historical
    // completion. It never applies the old credential again or grants a session.
    let receipt=sqlx::query("SELECT user_id,kind,effect_digest,authorization_hash,reset_token_hash,result FROM secure_operation_receipts WHERE id=?")
        .bind(request.effect.id.to_string()).fetch_optional(&mut *tx).await.map_err(ApiError::internal)?;
    if let Some(receipt) = receipt {
        if receipt.get::<String, _>("user_id") != request.effect.scope.account.to_string()
            || receipt.get::<String, _>("kind") != "reset"
            || receipt.get::<String, _>("effect_digest") != effect
            || receipt.get::<Option<String>, _>("authorization_hash") != Some(authorization_hash)
            || receipt.get::<Option<String>, _>("reset_token_hash") != Some(token_hash)
        {
            return Err(account_recovery::invalid_token());
        }
        return Ok(Json(
            serde_json::from_str(&receipt.get::<String, _>("result")).map_err(|_| unavailable())?,
        ));
    }
    let pending =
        registration::authorized(&mut tx, request.effect.id, "reset", &request.authorization)
            .await?;
    pending.check_transcript(&request.registration)?;
    if pending.binding.scope != request.effect.scope
        || pending.binding.reset_token_hash.as_ref() != Some(&token_hash)
        || pending.binding.expected.credential_generation
            != Some(request.effect.expected_generation)
        || pending.binding.generation != request.effect.generation
    {
        return Err(invalid());
    }
    let row = account_recovery::live_token_in(&mut tx, &token_hash, "reset").await?;
    if row.get::<String, _>("user_id") != request.effect.scope.account.to_string()
        || row.get::<Option<String>, _>("secure_generation")
            != Some(request.effect.expected_generation.to_string())
    {
        return Err(changed());
    }
    let consumed=sqlx::query("UPDATE account_recovery_tokens SET consumed_at=? WHERE token_hash=? AND consumed_at IS NULL AND expires_at>?")
        .bind(Utc::now().timestamp()).bind(&token_hash).bind(Utc::now().timestamp()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    if consumed.rows_affected() != 1 {
        return Err(account_recovery::invalid_token());
    }
    registration::replace_credential(
        &mut tx,
        &request.effect.scope,
        request.effect.expected_generation,
        request.effect.generation,
        &record,
    )
    .await?;
    registration::consume(&mut tx, request.effect.id).await?;
    let result = json!({"completed":true,"scope":request.effect.scope,"credential_generation":request.effect.generation,"backup_locked":true});
    sqlx::query("INSERT INTO secure_operation_receipts(id,user_id,device_id,kind,effect_digest,result,authorization_hash,reset_token_hash,committed_at) VALUES(?,?,NULL,'reset',?,?,?,?,?)")
        .bind(request.effect.id.to_string()).bind(request.effect.scope.account.to_string()).bind(&effect).bind(serde_json::to_string(&result).map_err(|_|unavailable())?)
        .bind(authorization_hash).bind(token_hash).bind(Utc::now().timestamp()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    sqlx::query("DELETE FROM account_recovery_tokens WHERE user_id=?")
        .bind(request.effect.scope.account.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    // Intentionally leave device credentials, encrypted payload, wrapper and
    // identity untouched; only an unlocked trusted client can repair the wrapper.
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(result))
}

pub(crate) async fn reset_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<ResetStatus>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(request) = request.map_err(|_| invalid())?;
    state.recovery.rate(&headers, "token", 60, 600).await?;
    if request.operation_id.is_nil()
        || request.effect_sha256.len() != 64
        || !request
            .effect_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid());
    }
    let token_hash = account_recovery::checked_token(&request.token)?;
    let (origin, network) = service(&state).await?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    let token = account_recovery::live_token_in(&mut tx, &token_hash, "reset").await?;
    let user = parse_uuid(&token.get::<String, _>("user_id"))?;
    let generation = token
        .get::<Option<String>, _>("secure_generation")
        .ok_or_else(invalid)?;
    let scope = Scope {
        origin,
        network,
        account: user,
    };
    let row=sqlx::query("SELECT result,committed_at FROM secure_operation_receipts WHERE id=? AND user_id=? AND kind='reset' AND effect_digest=?")
        .bind(request.operation_id.to_string()).bind(user.to_string()).bind(&request.effect_sha256).fetch_optional(&mut *tx).await.map_err(ApiError::internal)?;
    let receipt = if let Some(row) = row {
        Some(
            json!({"operation_id":request.operation_id,"operation_sha256":request.effect_sha256,"scope":scope,"device_id":null,"kind":"reset","result":serde_json::from_str::<Value>(&row.get::<String,_>("result")).map_err(|_|unavailable())?,"committed_at":row.get::<i64,_>("committed_at")}),
        )
    } else {
        None
    };
    Ok(Json(
        json!({"scope":scope,"credential_generation":generation,"receipt":receipt}),
    ))
}
