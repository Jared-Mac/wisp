use super::{
    ApiError, AppState, Checkpoint, Deserialize, HeaderMap, Json, Path, PublicIdentity, Row, Scope,
    State, Utc, Uuid, Value, account_state, authenticate_headers, authentication,
    current_precondition, invalid, json, parse_account_state, service, token_hash, unavailable,
};
use axum::extract::rejection::JsonRejection;
use wisp_crypto::account_vault::{
    envelope::{Envelope, KeyEnvelope, MAX_ENVELOPE_WIRE, SignedManifest},
    operation::{Change, DeviceBinding, Operation},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoreRequest {
    operation: Operation,
    signature: String,
    vault: Envelope,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RewrapRequest {
    grant: String,
    operation: Operation,
    signature: String,
    wrapper: KeyEnvelope,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProofRequest {
    after: Checkpoint,
    through: Checkpoint,
}

fn changed() -> ApiError {
    ApiError::conflict(
        "account_state_changed",
        "Account state changed; refresh before continuing",
    )
}

pub(crate) async fn read(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    let (origin, network) = service(&state).await?;
    // One SQL snapshot prevents mixing metadata with a concurrently replaced vault.
    let row = sqlx::query("SELECT u.username,i.public_identity,c.generation,v.wrapper_generation,v.revision,v.digest,v.wrapper,v.envelope FROM users u JOIN chat_identities i ON i.user_id=u.id JOIN secure_credentials c ON c.user_id=u.id JOIN account_vaults v ON v.user_id=u.id WHERE u.id=?")
        .bind(user.to_string()).fetch_optional(&state.pool).await.map_err(ApiError::internal)?.ok_or_else(unavailable)?;
    let account = parse_account_state(
        Scope {
            origin,
            network,
            account: user,
        },
        &row,
    )?;
    let wrapper: KeyEnvelope =
        serde_json::from_str(&row.get::<String, _>("wrapper")).map_err(|_| unavailable())?;
    let encoded: String = row.get("envelope");
    if encoded.len() > MAX_ENVELOPE_WIRE {
        return Err(unavailable());
    }
    let envelope: Envelope = serde_json::from_str(&encoded).map_err(|_| unavailable())?;
    wrapper
        .validate(
            &account.scope,
            account
                .expected
                .wrapper_generation
                .ok_or_else(unavailable)?,
        )
        .map_err(|_| unavailable())?;
    envelope
        .verify(
            &account.scope,
            account.identity.as_ref().ok_or_else(unavailable)?,
        )
        .map_err(|_| unavailable())?;
    if Some(envelope.manifest.checkpoint().map_err(|_| unavailable())?) != account.expected.vault {
        return Err(unavailable());
    }
    Ok(Json(
        json!({"state":account.response(),"wrapper":wrapper,"vault":envelope}),
    ))
}

pub(crate) async fn proof(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<ProofRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(request) = request.map_err(|_| invalid())?;
    request.after.validate().map_err(|_| invalid())?;
    request.through.validate().map_err(|_| invalid())?;
    if request.after.revision > request.through.revision {
        return Err(invalid());
    }
    let user = authenticate_headers(&state, &headers).await?;
    let account = account_state(&state, user).await?;
    let own = account.identity.as_ref().ok_or_else(unavailable)?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    for checkpoint in [&request.after, &request.through] {
        let exists:bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_vault_manifests WHERE user_id=? AND revision=? AND digest=?)")
            .bind(user.to_string()).bind(i64::try_from(checkpoint.revision).map_err(|_|invalid())?).bind(&checkpoint.sha256).fetch_one(&mut *tx).await.map_err(ApiError::internal)?;
        if !exists {
            return Err(ApiError::conflict(
                "vault_history_conflict",
                "Backup history does not match the saved checkpoint",
            ));
        }
    }
    let rows = sqlx::query("SELECT revision,digest,manifest FROM account_vault_manifests WHERE user_id=? AND revision>? AND revision<=? ORDER BY revision LIMIT 128")
        .bind(user.to_string()).bind(i64::try_from(request.after.revision).map_err(|_|invalid())?).bind(i64::try_from(request.through.revision).map_err(|_|invalid())?).fetch_all(&mut *tx).await.map_err(ApiError::internal)?;
    let mut next = request.after.clone();
    let mut manifests = Vec::new();
    let mut bytes = 1024_usize;
    for row in rows {
        let encoded: String = row.get("manifest");
        if encoded.len() > 16384 {
            return Err(unavailable());
        }
        if bytes + encoded.len() > 512 * 1024 {
            break;
        }
        let manifest: SignedManifest = serde_json::from_str(&encoded).map_err(|_| unavailable())?;
        let checkpoint = manifest
            .advance(&account.scope, own, Some(&next))
            .map_err(|_| unavailable())?;
        if checkpoint.sha256 != row.get::<String, _>("digest")
            || i64::try_from(checkpoint.revision).map_err(|_| unavailable())?
                != row.get::<i64, _>("revision")
        {
            return Err(unavailable());
        }
        bytes += encoded.len();
        next = checkpoint;
        manifests.push(manifest);
    }
    if next != request.through && manifests.is_empty() {
        return Err(unavailable());
    }
    Ok(Json(
        json!({"manifests":manifests,"complete":next==request.through,"next":next}),
    ))
}

pub(super) async fn own_identity(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user: Uuid,
) -> Result<PublicIdentity, ApiError> {
    let encoded: String =
        sqlx::query_scalar("SELECT public_identity FROM chat_identities WHERE user_id=?")
            .bind(user.to_string())
            .fetch_optional(&mut **tx)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(unavailable)?;
    if encoded.len() > 1024 {
        return Err(unavailable());
    }
    let identity: PublicIdentity = serde_json::from_str(&encoded).map_err(|_| unavailable())?;
    identity.validate().map_err(|_| unavailable())?;
    Ok(identity)
}

pub(super) fn bind_operation(
    operation: &Operation,
    scope: &Scope,
    session: authentication::Session,
) -> Result<String, ApiError> {
    if operation.scope != *scope
        || scope.account != session.user
        || operation.device != (DeviceBinding::Existing { id: session.device })
    {
        return Err(ApiError::forbidden(
            "Operation belongs to another account or device",
        ));
    }
    operation.digest().map_err(|_| invalid())
}

pub(super) async fn existing_receipt(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user: Uuid,
    id: Uuid,
    kind: &str,
    effect: &str,
) -> Result<Option<Value>, ApiError> {
    let row = sqlx::query(
        "SELECT user_id,kind,effect_digest,result FROM secure_operation_receipts WHERE id=?",
    )
    .bind(id.to_string())
    .fetch_optional(&mut **tx)
    .await
    .map_err(ApiError::internal)?;
    if let Some(row) = row {
        if row.get::<String, _>("user_id") != user.to_string()
            || row.get::<String, _>("kind") != kind
            || row.get::<String, _>("effect_digest") != effect
        {
            return Err(ApiError::conflict(
                "operation_conflict",
                "Operation differs from its committed effect",
            ));
        }
        return Ok(Some(
            serde_json::from_str(&row.get::<String, _>("result")).map_err(|_| unavailable())?,
        ));
    }
    Ok(None)
}

pub(super) async fn store_receipt(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    operation: &Operation,
    kind: &str,
    result: &Value,
) -> Result<(), ApiError> {
    sqlx::query("INSERT INTO secure_operation_receipts(id,user_id,device_id,kind,effect_digest,result,committed_at) VALUES(?,?,?,?,?,?,?)")
        .bind(operation.id.to_string()).bind(operation.scope.account.to_string()).bind(operation.device.id().to_string()).bind(kind).bind(operation.digest().map_err(|_|invalid())?).bind(serde_json::to_string(result).map_err(|_|unavailable())?).bind(Utc::now().timestamp()).execute(&mut **tx).await.map_err(ApiError::internal)?;
    Ok(())
}

pub(super) async fn consume_grant(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    operation: &Operation,
    grant: &str,
) -> Result<(), ApiError> {
    if grant.len() != 43 {
        return Err(invalid());
    }
    let consumed = sqlx::query("UPDATE secure_reauth_grants SET consumed_at=? WHERE token_hash=? AND user_id=? AND device_id=? AND generation=? AND effect_digest=? AND expires_at>? AND consumed_at IS NULL")
        .bind(Utc::now().timestamp()).bind(token_hash(grant)).bind(operation.scope.account.to_string()).bind(operation.device.id().to_string()).bind(operation.expected.credential_generation.ok_or_else(invalid)?.to_string()).bind(operation.digest().map_err(|_|invalid())?).bind(Utc::now().timestamp()).execute(&mut **tx).await.map_err(ApiError::internal)?;
    if consumed.rows_affected() != 1 {
        return Err(ApiError::unauthorized(
            "Reauthentication expired or does not match this operation",
        ));
    }
    Ok(())
}

pub(crate) async fn store(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<StoreRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(request) = request.map_err(|_| invalid())?;
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
    let effect = bind_operation(&request.operation, &scope, session)?;
    let identity = own_identity(&mut tx, session.user).await?;
    request
        .operation
        .verify(&identity, &request.signature)
        .map_err(|_| invalid())?;
    request
        .vault
        .verify(&scope, &identity)
        .map_err(|_| invalid())?;
    let next = request
        .vault
        .manifest
        .advance(&scope, &identity, request.operation.expected.vault.as_ref())
        .map_err(|_| invalid())?;
    if request.operation.change
        != (Change::StoreVault {
            vault: next.clone(),
        })
    {
        return Err(invalid());
    }
    if let Some(receipt) = existing_receipt(
        &mut tx,
        session.user,
        request.operation.id,
        "vault",
        &effect,
    )
    .await?
    {
        return Ok(Json(receipt));
    }
    if current_precondition(&mut tx, session.user).await? != request.operation.expected {
        return Err(changed());
    }
    let envelope = serde_json::to_string(&request.vault).map_err(|_| invalid())?;
    if envelope.len() > MAX_ENVELOPE_WIRE {
        return Err(invalid());
    }
    let now = Utc::now().timestamp();
    sqlx::query("INSERT INTO account_vault_manifests(user_id,revision,digest,manifest,created_at) VALUES(?,?,?,?,?)")
        .bind(session.user.to_string()).bind(i64::try_from(next.revision).map_err(|_|invalid())?).bind(&next.sha256).bind(serde_json::to_string(&request.vault.manifest).map_err(|_|invalid())?).bind(now).execute(&mut *tx).await.map_err(ApiError::internal)?;
    sqlx::query(
        "UPDATE account_vaults SET revision=?,digest=?,envelope=?,updated_at=? WHERE user_id=?",
    )
    .bind(i64::try_from(next.revision).map_err(|_| invalid())?)
    .bind(&next.sha256)
    .bind(envelope)
    .bind(now)
    .bind(session.user.to_string())
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;
    let result = json!({"committed":true,"operation_id":request.operation.id,"operation_sha256":effect,"scope":scope,"credential_generation":request.operation.expected.credential_generation,"vault":next});
    store_receipt(&mut tx, &request.operation, "vault", &result).await?;
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(result))
}

pub(crate) async fn rewrap(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<RewrapRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(request) = request.map_err(|_| invalid())?;
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
    let effect = bind_operation(&request.operation, &scope, session)?;
    let identity = own_identity(&mut tx, session.user).await?;
    request
        .operation
        .verify(&identity, &request.signature)
        .map_err(|_| invalid())?;
    let generation = request
        .operation
        .expected
        .credential_generation
        .ok_or_else(invalid)?;
    request
        .wrapper
        .validate(&scope, generation)
        .map_err(|_| invalid())?;
    if request.operation.change
        != (Change::Rewrap {
            wrapper_sha256: request.wrapper.digest().map_err(|_| invalid())?,
        })
    {
        return Err(invalid());
    }
    if let Some(receipt) = existing_receipt(
        &mut tx,
        session.user,
        request.operation.id,
        "rewrap",
        &effect,
    )
    .await?
    {
        return Ok(Json(receipt));
    }
    if current_precondition(&mut tx, session.user).await? != request.operation.expected {
        return Err(changed());
    }
    consume_grant(&mut tx, &request.operation, &request.grant).await?;
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
    let result = json!({"committed":true,"operation_id":request.operation.id,"operation_sha256":effect,"scope":scope,"credential_generation":generation,"wrapper_generation":generation,"vault":request.operation.expected.vault});
    store_receipt(&mut tx, &request.operation, "rewrap", &result).await?;
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(result))
}

pub(crate) async fn receipt(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    let (origin, network) = service(&state).await?;
    let row = sqlx::query("SELECT device_id,kind,effect_digest,result,committed_at FROM secure_operation_receipts WHERE id=? AND user_id=?").bind(id.to_string()).bind(user.to_string()).fetch_optional(&state.pool).await.map_err(ApiError::internal)?.ok_or_else(||ApiError::not_found("Operation receipt is unavailable"))?;
    let result: Value =
        serde_json::from_str(&row.get::<String, _>("result")).map_err(|_| unavailable())?;
    Ok(Json(
        json!({"operation_id":id,"operation_sha256":row.get::<String,_>("effect_digest"),"scope":Scope {origin,network,account:user},"device_id":row.get::<Option<String>,_>("device_id"),"kind":row.get::<String,_>("kind"),"result":result,"committed_at":row.get::<i64,_>("committed_at")}),
    ))
}
