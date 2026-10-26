//! Server-installed tools are catalog registrations, not server-executed guest code.
use super::{
    ApiError, AppState, HeaderMap, Json, Path, State, Value, authenticate_headers,
    server_management,
};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use wisp_plugins::catalog;

pub(super) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let actor = authenticate_headers(&state, &headers).await?;
    super::account_membership::require_member(&state.pool, actor).await?;
    let enabled: Vec<(String, String)> =
        sqlx::query_as("SELECT plugin_id,digest FROM server_plugins ORDER BY plugin_id")
            .fetch_all(&state.pool)
            .await
            .map_err(ApiError::internal)?;
    let entries = catalog::entries().map_err(ApiError::internal)?.into_iter().map(|entry| {
        let installed = enabled.iter().find(|(id,_)| id == &entry.manifest.id);
        json!({"manifest":entry.manifest,"digest":entry.digest,"installed_digest":installed.map(|(_,digest)|digest),"enabled":installed.is_some()})
    }).collect::<Vec<_>>();
    Ok(Json(
        json!({"catalog":entries,"can_manage":server_management::is_manager(&state.pool,actor).await?}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Enable {
    enabled: bool,
    digest: String,
}
pub(super) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<Enable>,
) -> Result<Json<Value>, ApiError> {
    let actor = server_management::require_manager(&state, &headers).await?;
    super::account_membership::require_member(&state.pool, actor).await?;
    let entry = catalog::entry(&id)
        .map_err(|_| ApiError::not_found("Plugin is not in the server catalog"))?;
    if entry.digest != request.digest {
        return Err(ApiError::conflict(
            "plugin_changed",
            "The catalog changed. Refresh and review this plugin again.",
        ));
    }
    if request.enabled {
        sqlx::query("INSERT INTO server_plugins(plugin_id,digest,enabled_by,enabled_at) VALUES(?,?,?,?) ON CONFLICT(plugin_id) DO UPDATE SET digest=excluded.digest,enabled_by=excluded.enabled_by,enabled_at=excluded.enabled_at")
            .bind(&id).bind(&entry.digest).bind(actor.to_string()).bind(chrono::Utc::now().to_rfc3339())
            .execute(&state.pool).await.map_err(ApiError::internal)?;
    } else {
        sqlx::query("DELETE FROM server_plugins WHERE plugin_id=?")
            .bind(&id)
            .execute(&state.pool)
            .await
            .map_err(ApiError::internal)?;
    }
    state
        .emit("server_plugins_changed", json!({"changed":true}))
        .await;
    list(State(state), headers).await
}
pub(super) async fn package(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let actor = authenticate_headers(&state, &headers).await?;
    super::account_membership::require_member(&state.pool, actor).await?;
    let digest: String = sqlx::query_scalar("SELECT digest FROM server_plugins WHERE plugin_id=?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("This plugin is no longer enabled on the server"))?;
    let bytes = catalog::module(&id, &digest).map_err(|_| {
        ApiError::conflict(
            "plugin_changed",
            "An administrator must update this server plugin from the catalog",
        )
    })?;
    Ok((
        [
            ("content-type", "application/wasm"),
            ("cache-control", "private, no-store"),
            ("x-content-type-options", "nosniff"),
            ("x-wisp-plugin-digest", digest.as_str()),
        ],
        bytes.to_vec(),
    )
        .into_response())
}

#[cfg(test)]
mod tests;
