//! Explicit original-password recovery when an uncommitted migration device is
//! revoked. A signed staged credential avoids orphan devices after a lost reply.
use super::{
    ApiError, AppState, Deserialize, HeaderMap, Json, Row, State, Utc, Value, Zeroizing, auth,
    authentication, invalid, json, registration, service, unavailable, vault,
};
use axum::extract::rejection::JsonRejection;
use wisp_crypto::account_vault::{
    device::legacy_token_hash,
    operation::{DeviceBinding, MigrationRecoveryBinding},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequestBody {
    recovery: MigrationRecoveryBinding,
    signature: String,
    legacy_password: String,
}
fn changed() -> ApiError {
    ApiError::conflict(
        "account_state_changed",
        "This migration requires secure sign-in recovery",
    )
}
async fn prior(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    recovery: &MigrationRecoveryBinding,
    digest: &str,
) -> Result<Option<Value>, ApiError> {
    vault::existing_receipt(
        tx,
        recovery.operation.scope.account,
        recovery.id,
        "migration_recovery",
        digest,
    )
    .await
}
#[allow(clippy::too_many_lines)] // Revocation, activation and receipt must share one transaction.
pub(crate) async fn recover(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<RequestBody>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(request) = request.map_err(|_| invalid())?;
    authentication::rate(&state, &headers).await?;
    let recovery = request.recovery;
    let digest = recovery.digest().map_err(|_| invalid())?;
    let (origin, network) = service(&state).await?;
    let scope = &recovery.operation.scope;
    if scope.origin != origin || scope.network != network {
        return Err(invalid());
    }
    // An exact receipt is public only to its identity-signed owner. No password
    // is needed to read an already committed result, and replay never activates.
    {
        let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
        let identity = vault::own_identity(&mut tx, scope.account).await?;
        recovery
            .verify(&identity, &request.signature)
            .map_err(|_| invalid())?;
        if let Some(receipt) = prior(&mut tx, &recovery, &digest).await? {
            return Ok(Json(receipt));
        }
    }
    let fingerprint =
        registration::legacy_proof(&state, scope.account, request.legacy_password).await?;
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(ApiError::internal)?;
    let identity = vault::own_identity(&mut tx, scope.account).await?;
    recovery
        .verify(&identity, &request.signature)
        .map_err(|_| invalid())?;
    if let Some(receipt) = prior(&mut tx, &recovery, &digest).await? {
        return Ok(Json(receipt));
    }
    let row = sqlx::query("SELECT u.username,u.display_name,u.password_hash,c.generation FROM users u LEFT JOIN secure_credentials c ON c.user_id=u.id WHERE u.id=?")
        .bind(scope.account.to_string()).fetch_one(&mut *tx).await.map_err(ApiError::internal)?;
    let password = Zeroizing::new(
        row.get::<Option<String>, _>("password_hash")
            .ok_or_else(changed)?,
    );
    if row.get::<Option<String>, _>("generation").is_some()
        || authentication::digest("wisp-legacy-migration-verifier-v1", &password.as_str())?
            != fingerprint
    {
        return Err(changed());
    }
    let original_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM devices WHERE id=? AND user_id=?)")
            .bind(recovery.operation.device.id().to_string())
            .bind(scope.account.to_string())
            .fetch_one(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    let original_committed: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM secure_operation_receipts WHERE id=?)")
            .bind(recovery.operation.id.to_string())
            .fetch_one(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    if !original_exists || original_committed {
        return Err(changed());
    }
    let DeviceBinding::Prospective { id, token_sha256 } = &recovery.device else {
        return Err(invalid());
    };
    let occupied: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM devices WHERE id=?)")
        .bind(id.to_string())
        .fetch_one(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    if occupied {
        return Err(ApiError::conflict(
            "device_conflict",
            "Staged recovery device is already registered",
        ));
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE devices SET revoked_at=COALESCE(revoked_at,?) WHERE id=? AND user_id=?")
        .bind(&now)
        .bind(recovery.operation.device.id().to_string())
        .bind(scope.account.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query("UPDATE sessions SET revoked_at=COALESCE(revoked_at,?) WHERE device_id=?")
        .bind(&now)
        .bind(recovery.operation.device.id().to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query("INSERT INTO devices(id,user_id,name,token_hash,created_at,last_seen_at) VALUES(?,?,?,?,?,?)")
        .bind(id.to_string()).bind(scope.account.to_string()).bind(&recovery.device_name).bind(legacy_token_hash(token_sha256).map_err(|_|invalid())?).bind(&now).bind(&now).execute(&mut *tx).await.map_err(ApiError::internal)?;
    let result = json!({"recovered":true,"recovery_id":recovery.id,"scope":scope,"operation_id":recovery.operation.id,
        "operation_sha256":recovery.operation.digest().map_err(|_|invalid())?,"terminated_device_id":recovery.operation.device.id(),
        "device_id":id,"user":{"id":scope.account,"display_name":row.get::<String,_>("display_name")},
        "username":auth::canonical_username(&row.get::<String,_>("username")).map_err(|_|unavailable())?,"identity":identity});
    sqlx::query("INSERT INTO secure_operation_receipts(id,user_id,device_id,kind,effect_digest,result,committed_at) VALUES(?,?,?,'migration_recovery',?,?,?)")
        .bind(recovery.id.to_string()).bind(scope.account.to_string()).bind(id.to_string()).bind(digest).bind(serde_json::to_string(&result).map_err(|_|unavailable())?).bind(Utc::now().timestamp()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .remove_device_activity(scope.account, recovery.operation.device.id())
        .await;
    Ok(Json(result))
}
