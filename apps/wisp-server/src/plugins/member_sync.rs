//! Advisory reports from authenticated clients, not proof of a running game's mods.
use crate::{ApiError, AppState, HeaderMap, Json, Path, Row, State, Utc, Value};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

const STALE_SECONDS: i64 = 24 * 60 * 60;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Report {
    revision: String,
    status: String,
    platform: String,
    checked_at: i64,
}

async fn enabled(state: &AppState, id: &str) -> Result<(), ApiError> {
    let found: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_plugins WHERE plugin_id=?)")
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .map_err(ApiError::internal)?;
    if !found {
        return Err(ApiError::not_found("This server plugin is disabled"));
    }
    Ok(())
}

async fn latest(
    db: &mut sqlx::SqliteConnection,
    id: &str,
) -> Result<Option<(String, String)>, ApiError> {
    sqlx::query_as("SELECT m.id,m.payload FROM plugin_workspaces w JOIN messages m ON m.conversation_id=w.conversation_id WHERE w.plugin_id=? AND m.encryption_version=1 AND EXISTS(SELECT 1 FROM file_uploads f WHERE f.message_id=m.id) ORDER BY m.created_at DESC,m.id DESC LIMIT 1")
        .bind(id).fetch_optional(db).await.map_err(ApiError::internal)
}

fn recipients(payload: &str) -> Vec<String> {
    serde_json::from_str::<Value>(payload)
        .ok()
        .and_then(|v| v["recipient_ids"].as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

fn status<'a>(revision: &str, current: &str, checked: i64, now: i64, reported: &'a str) -> &'a str {
    if revision != current {
        "needs_check"
    } else if now.saturating_sub(checked) > STALE_SECONDS {
        "stale"
    } else {
        reported
    }
}

pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let actor = crate::authenticate_headers(&state, &headers).await?;
    crate::account_membership::require_member(&state.pool, actor).await?;
    enabled(&state, &id).await?;
    let mut db = state.pool.acquire().await.map_err(ApiError::internal)?;
    let published = latest(&mut db, &id).await?;
    let audience = published
        .as_ref()
        .map_or_else(Vec::new, |(_, payload)| recipients(payload));
    let members: Vec<(String,String)> = sqlx::query_as("SELECT id,display_name FROM users WHERE server_member=1 AND NOT EXISTS(SELECT 1 FROM server_bans WHERE user_id=users.id) ORDER BY display_name COLLATE NOCASE,id")
        .fetch_all(&mut *db).await.map_err(ApiError::internal)?;
    // Device names may contain private hostnames. Expose only a coarse platform,
    // and omit revoked credentials; the development alias is test-only.
    let rows = sqlx::query("SELECT * FROM (SELECT r.*,row_number() OVER(PARTITION BY r.user_id ORDER BY r.checked_at DESC,r.device_id) AS position FROM plugin_sync_reports r JOIN users u ON u.id=r.user_id LEFT JOIN devices d ON d.id=r.device_id AND d.user_id=r.user_id WHERE r.plugin_id=? AND u.server_member=1 AND ((d.id IS NOT NULL AND d.revoked_at IS NULL) OR (? AND r.device_id=r.user_id))) WHERE position<=8 ORDER BY user_id,position")
        .bind(&id).bind(state.config.allow_dev_sessions).fetch_all(&mut *db).await.map_err(ApiError::internal)?;
    let now = Utc::now().timestamp();
    let mut devices: HashMap<String, Vec<Value>> = HashMap::new();
    for row in rows {
        let revision: String = row.get("revision");
        let reported: String = row.get("status");
        let checked: i64 = row.get("checked_at");
        let current = published.as_ref().map_or("", |(id, _)| id.as_str());
        devices.entry(row.get("user_id")).or_default().push(json!({
            "platform":row.get::<String,_>("platform"),"status":status(&revision,current,checked,now,&reported),"checked_at":checked
        }));
    }
    let members: Vec<_> = members
        .into_iter()
        .map(|(user, name)| {
            let reports = devices.remove(&user).unwrap_or_default();
            let first = reports
                .first()
                .and_then(|r| r["status"].as_str())
                .unwrap_or("unknown");
            let summary = if published.is_none() {
                "no_pack"
            } else if !audience.contains(&user) {
                "awaiting_access"
            } else if reports.iter().all(|r| r["status"] == first) {
                first
            } else {
                "mixed"
            };
            json!({"user_id":user,"display_name":name,"status":summary,"devices":reports})
        })
        .collect();
    Ok(Json(
        json!({"members":members,"has_pack":published.is_some(),"server_time":now,"stale_after_seconds":STALE_SECONDS}),
    ))
}

pub(crate) async fn report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(report): Json<Report>,
) -> Result<Json<Value>, ApiError> {
    // No caller-supplied user/device ID: a member can report only their own PC.
    let (actor, device) = crate::device_activity::identity(&state, &headers).await?;
    crate::account_membership::require_member(&state.pool, actor).await?;
    enabled(&state, &id).await?;
    let now = Utc::now().timestamp();
    if ![
        "up_to_date",
        "update_available",
        "modified",
        "missing",
        "not_installed",
        "unknown",
    ]
    .contains(&report.status.as_str())
        || !["linux", "windows", "macos", "android", "other"].contains(&report.platform.as_str())
        || report.checked_at <= 0
        || report.checked_at > now + 300
    {
        return Err(ApiError::bad_request(
            "invalid_sync_report",
            "Invalid installation report or check time",
        ));
    }
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    let published = latest(&mut tx, &id)
        .await?
        .ok_or_else(|| ApiError::conflict("pack_changed", "No server pack is published"))?;
    if published.0 != report.revision {
        return Err(ApiError::conflict(
            "pack_changed",
            "The server pack changed; check it again",
        ));
    }
    if !recipients(&published.1).contains(&actor.to_string()) {
        return Err(ApiError::forbidden(
            "Ask an admin to republish the pack for your membership",
        ));
    }
    sqlx::query("INSERT INTO plugin_sync_reports(plugin_id,user_id,device_id,revision,status,platform,checked_at,reported_at) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(plugin_id,user_id,device_id) DO UPDATE SET revision=excluded.revision,status=excluded.status,platform=excluded.platform,checked_at=excluded.checked_at,reported_at=excluded.reported_at WHERE excluded.checked_at>=plugin_sync_reports.checked_at")
        .bind(&id).bind(actor.to_string()).bind(device.to_string()).bind(&report.revision).bind(&report.status).bind(&report.platform).bind(report.checked_at.min(now)).bind(now)
        .execute(&mut *tx).await.map_err(ApiError::internal)?;
    // Bound storage per member/plugin even across repeated device registrations.
    sqlx::query("DELETE FROM plugin_sync_reports WHERE plugin_id=? AND user_id=? AND device_id NOT IN (SELECT device_id FROM plugin_sync_reports WHERE plugin_id=? AND user_id=? ORDER BY checked_at DESC,device_id LIMIT 8)")
        .bind(&id).bind(actor.to_string()).bind(&id).bind(actor.to_string()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    list(State(state), headers, Path(id)).await
}

#[cfg(test)]
mod tests;
