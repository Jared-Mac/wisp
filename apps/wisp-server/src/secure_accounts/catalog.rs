//! Account-scoped opaque catalog with signed history and atomic conditional writes.
use super::{
    ApiError, AppState, HeaderMap, Json, Row, Scope, State, Utc, Uuid, authentication, invalid,
    service, unavailable, vault,
};
use axum::extract::rejection::JsonRejection;
use sqlx::{Sqlite, Transaction};
use wisp_crypto::{
    PublicIdentity,
    account_vault::{
        catalog::{
            self, Envelope, ProofPage, ProofRequest, ReadResponse, SignedManifest, Status,
            StoreRequest,
        },
        envelope::Checkpoint,
    },
};

fn changed() -> ApiError {
    ApiError::conflict(
        "catalog_state_changed",
        "Saved servers changed on another device; refresh and merge before continuing",
    )
}
async fn context(
    service_scope: (String, Uuid),
    headers: &HeaderMap,
    tx: &mut Transaction<'_, Sqlite>,
) -> Result<(Scope, PublicIdentity), ApiError> {
    let session = authentication::session(tx, headers).await?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_vaults WHERE user_id=?)")
            .bind(session.user.to_string())
            .fetch_one(&mut **tx)
            .await
            .map_err(ApiError::internal)?;
    if !exists {
        return Err(unavailable());
    }
    let (origin, network) = service_scope;
    let own = vault::own_identity(tx, session.user).await?;
    Ok((
        Scope {
            origin,
            network,
            account: session.user,
        },
        own,
    ))
}
async fn head(
    tx: &mut Transaction<'_, Sqlite>,
    account: Uuid,
) -> Result<Option<Checkpoint>, ApiError> {
    let row = sqlx::query("SELECT revision,digest FROM server_catalogs WHERE user_id=?")
        .bind(account.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(ApiError::internal)?;
    row.map(|row| {
        let checkpoint = Checkpoint {
            revision: u64::try_from(row.get::<i64, _>("revision")).map_err(|_| unavailable())?,
            sha256: row.get("digest"),
        };
        checkpoint.validate().map_err(|_| unavailable())?;
        Ok(checkpoint)
    })
    .transpose()
}
pub(crate) async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Status>, ApiError> {
    let service_scope = service(&state).await?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    let (scope, _) = context(service_scope, &headers, &mut tx).await?;
    let catalog = head(&mut tx, scope.account).await?;
    Ok(Json(Status {
        version: catalog::VERSION,
        scope,
        catalog,
    }))
}
pub(crate) async fn read(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ReadResponse>, ApiError> {
    let service_scope = service(&state).await?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    let (scope, own) = context(service_scope, &headers, &mut tx).await?;
    let expected = head(&mut tx, scope.account).await?;
    let bytes: Option<String> =
        sqlx::query_scalar("SELECT envelope FROM server_catalogs WHERE user_id=?")
            .bind(scope.account.to_string())
            .fetch_optional(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    let catalog = bytes
        .map(|bytes| {
            if bytes.len() > catalog::MAX_ENVELOPE_WIRE {
                return Err(unavailable());
            }
            let envelope: Envelope = serde_json::from_str(&bytes).map_err(|_| unavailable())?;
            envelope.verify(&scope, &own).map_err(|_| unavailable())?;
            if expected.as_ref()
                != Some(&envelope.manifest.checkpoint().map_err(|_| unavailable())?)
            {
                return Err(unavailable());
            }
            Ok(envelope)
        })
        .transpose()?;
    Ok(Json(ReadResponse {
        version: catalog::VERSION,
        scope,
        catalog,
    }))
}
pub(crate) async fn store(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<StoreRequest>, JsonRejection>,
) -> Result<Json<Status>, ApiError> {
    let Json(request) = request.map_err(|_| invalid())?;
    let service_scope = service(&state).await?;
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(ApiError::internal)?;
    let (scope, own) = context(service_scope, &headers, &mut tx).await?;
    request
        .catalog
        .verify(&scope, &own)
        .map_err(|_| invalid())?;
    let next = request
        .catalog
        .manifest
        .checkpoint()
        .map_err(|_| invalid())?;
    let current = head(&mut tx, scope.account).await?;
    // Exact retry succeeds without another revision, including response loss.
    if current.as_ref() != Some(&next) {
        if request.catalog.manifest.header.parent != current {
            return Err(changed());
        }
        request
            .catalog
            .manifest
            .advance(&scope, &own, current.as_ref())
            .map_err(|_| invalid())?;
        let envelope = serde_json::to_string(&request.catalog).map_err(ApiError::internal)?;
        let manifest =
            serde_json::to_string(&request.catalog.manifest).map_err(ApiError::internal)?;
        if envelope.len() > catalog::MAX_ENVELOPE_WIRE || manifest.len() > 16384 {
            return Err(invalid());
        }
        let revision = i64::try_from(next.revision).map_err(|_| invalid())?;
        sqlx::query("INSERT INTO server_catalog_manifests(user_id,revision,digest,manifest,created_at) VALUES(?,?,?,?,?)")
            .bind(scope.account.to_string()).bind(revision).bind(&next.sha256).bind(manifest).bind(Utc::now().timestamp())
            .execute(&mut *tx).await.map_err(ApiError::internal)?;
        sqlx::query("INSERT INTO server_catalogs(user_id,revision,digest,envelope,updated_at) VALUES(?,?,?,?,?) ON CONFLICT(user_id) DO UPDATE SET revision=excluded.revision,digest=excluded.digest,envelope=excluded.envelope,updated_at=excluded.updated_at")
            .bind(scope.account.to_string()).bind(revision).bind(&next.sha256).bind(envelope).bind(Utc::now().timestamp())
            .execute(&mut *tx).await.map_err(ApiError::internal)?;
    }
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(Status {
        version: catalog::VERSION,
        scope,
        catalog: Some(next),
    }))
}
pub(crate) async fn proof(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<ProofRequest>, JsonRejection>,
) -> Result<Json<ProofPage>, ApiError> {
    let Json(request) = request.map_err(|_| invalid())?;
    request.after.validate().map_err(|_| invalid())?;
    request.through.validate().map_err(|_| invalid())?;
    if request.after.revision > request.through.revision {
        return Err(invalid());
    }
    let service_scope = service(&state).await?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    let (scope, own) = context(service_scope, &headers, &mut tx).await?;
    for checkpoint in [&request.after, &request.through] {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_catalog_manifests WHERE user_id=? AND revision=? AND digest=?)")
            .bind(scope.account.to_string()).bind(i64::try_from(checkpoint.revision).map_err(|_| invalid())?).bind(&checkpoint.sha256)
            .fetch_one(&mut *tx).await.map_err(ApiError::internal)?;
        if !exists {
            return Err(changed());
        }
    }
    let rows = sqlx::query("SELECT revision,digest,manifest FROM server_catalog_manifests WHERE user_id=? AND revision>? AND revision<=? ORDER BY revision LIMIT 128")
        .bind(scope.account.to_string()).bind(i64::try_from(request.after.revision).map_err(|_| invalid())?).bind(i64::try_from(request.through.revision).map_err(|_| invalid())?)
        .fetch_all(&mut *tx).await.map_err(ApiError::internal)?;
    let mut next = request.after;
    let mut manifests = Vec::new();
    let mut bytes = 0;
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
            .advance(&scope, &own, Some(&next))
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
    Ok(Json(ProofPage {
        complete: next == request.through,
        next,
        manifests,
    }))
}

#[cfg(test)]
mod tests {
    use crate::secure_accounts::authentication_tests::{fixture, signed_in};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn catalog_reads_do_not_hold_a_connection_while_waiting_for_another() {
        let fixture = fixture().await;
        let (_, session) = signed_in(&fixture).await;
        // A private in-memory fixture has exactly one SQL connection. Every
        // catalog handler must use that same transaction for its account reads.
        for path in [
            "/v3/accounts/server-catalog",
            "/v3/accounts/server-catalog/status",
        ] {
            let response = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                crate::router(fixture.state.clone()).oneshot(
                    Request::get(path)
                        .header("authorization", format!("Bearer {session}"))
                        .body(Body::empty())
                        .unwrap(),
                ),
            )
            .await
            .expect("Catalog handler must not starve the SQL pool")
            .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["cache-control"], "no-store");
        }
    }
}
