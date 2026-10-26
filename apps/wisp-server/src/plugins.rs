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

// Reuse the signed encrypted-channel roster and its automatic member admissions.
// The reserved ID keeps this transport out of ordinary chat navigation.
pub(super) async fn workspace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let actor = authenticate_headers(&state, &headers).await?;
    super::account_membership::require_member(&state.pool, actor).await?;
    let entry = catalog::entry(&id).map_err(|_| ApiError::not_found("Unknown plugin"))?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    let enabled: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_plugins WHERE plugin_id=?)")
            .bind(&id)
            .fetch_one(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    if !enabled {
        return Err(ApiError::not_found("This server plugin is disabled"));
    }
    let conversation = format!("plugin:{id}");
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO conversations(id,kind,label,created_at) VALUES (?,'circle',?,?)",
    )
    .bind(&conversation)
    .bind(format!("{} · server pack", entry.manifest.name))
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;
    sqlx::query("INSERT OR IGNORE INTO server_channels(conversation_id,position,created_by,created_at,visibility) VALUES (?,0,?,?,'everyone')")
        .bind(&conversation).bind(actor.to_string()).bind(&now).execute(&mut *tx).await.map_err(ApiError::internal)?;
    sqlx::query("INSERT OR IGNORE INTO plugin_workspaces(plugin_id,conversation_id) VALUES (?,?)")
        .bind(&id)
        .bind(&conversation)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    // Only bootstrap before a signed roster exists. Later arrivals use its
    // authenticated admission path instead of silently rewriting membership.
    sqlx::query("INSERT OR IGNORE INTO conversation_members(conversation_id,user_id,joined_at,role) SELECT a.conversation_id,a.user_id,?,CASE WHEN EXISTS(SELECT 1 FROM server_identity si WHERE si.owner_user_id=a.user_id) OR EXISTS(SELECT 1 FROM server_admins sa WHERE sa.user_id=a.user_id) THEN 'admin' ELSE 'member' END FROM channel_access a JOIN chat_identities ci ON ci.user_id=a.user_id WHERE a.conversation_id=? AND NOT EXISTS(SELECT 1 FROM chat_rosters WHERE conversation_id=a.conversation_id)")
        .bind(&now).bind(&conversation).execute(&mut *tx).await.map_err(ApiError::internal)?;
    super::channel_access::queue_admissions(&mut tx).await?;
    // A new member cannot decrypt old revisions until an admin republishes for
    // the expanded roster. Never advertise a revision they cannot access.
    let latest: Option<String> = sqlx::query_scalar("SELECT m.id FROM messages m WHERE m.conversation_id=? AND m.encryption_version=1 AND EXISTS(SELECT 1 FROM file_uploads f WHERE f.message_id=m.id) AND EXISTS(SELECT 1 FROM json_each(m.payload,'$.recipient_ids') r WHERE r.value=?) ORDER BY m.created_at DESC,m.id DESC LIMIT 1")
        .bind(&conversation).bind(actor.to_string()).fetch_optional(&mut *tx).await.map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(
        json!({"conversation_id":conversation,"latest_message_id":latest,
        "can_publish":server_management::is_manager(&state.pool,actor).await?}),
    ))
}

pub(super) async fn authorize_publish(
    pool: &super::SqlitePool,
    conversation: &str,
    actor: super::UserId,
) -> Result<(), ApiError> {
    if let Some(id) = conversation.strip_prefix("plugin:") {
        let enabled: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_plugins WHERE plugin_id=?)")
                .bind(id)
                .fetch_one(pool)
                .await
                .map_err(ApiError::internal)?;
        if !enabled || !server_management::is_manager(pool, actor).await? {
            return Err(ApiError::forbidden(
                "Only server admins can publish the server pack",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
