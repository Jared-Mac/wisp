use crate::{
    ApiError, AppState, HeaderMap, Json, Path, Row, State, Utc, Value, authenticate_headers, json,
};
use serde::Deserialize;
use wisp_crypto::short_invitation::WORDS;

pub(super) async fn cleanup(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM invitation_aliases WHERE invite_id IN (SELECT id FROM server_invites WHERE expires_at<=? OR used_at IS NOT NULL OR revoked_at IS NOT NULL)")
        .bind(Utc::now().to_rfc3339()).execute(pool).await?;
    sqlx::query("DELETE FROM server_invites WHERE expires_at<=?")
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await?;
    Ok(())
}

impl AppState {
    pub async fn maintain_invitations(self) {
        let mut timer = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            timer.tick().await;
            if let Err(error) = cleanup(&self.pool).await {
                tracing::warn!(%error,"invitation cleanup failed; will retry");
            }
        }
    }
}

pub(super) async fn reserve(state: &AppState, id: uuid::Uuid) -> Result<String, ApiError> {
    cleanup(&state.pool).await.map_err(ApiError::internal)?;
    // Try the whole one-word pool in a randomized rotation first. Grow to
    // distinct hyphenated phrases only once every single word is in use.
    let seed = uuid::Uuid::new_v4().as_u128();
    for depth in 1_u32..=4 {
        let capacity = WORDS.len().pow(depth);
        let start = usize::try_from(seed % capacity as u128).expect("bounded index");
        for offset in 0..capacity.min(4096) {
            let mut index = (start + offset) % capacity;
            let mut words = Vec::new();
            for _ in 0..depth {
                words.push(WORDS[index % WORDS.len()]);
                index /= WORDS.len();
            }
            let label = words.join("-");
            let result = sqlx::query(
                "INSERT OR IGNORE INTO invitation_aliases(invite_id,label) VALUES(?,?)",
            )
            .bind(id.to_string())
            .bind(&label)
            .execute(&state.pool)
            .await
            .map_err(ApiError::internal)?;
            if result.rows_affected() == 1 {
                return Ok(label);
            }
        }
    }
    Err(ApiError::conflict(
        "invite_capacity",
        "Try creating an invitation again shortly",
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Envelope {
    lookup_id: String,
    envelope: String,
}

pub(super) async fn store(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
    Json(request): Json<Envelope>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    super::account_membership::require_member(&state.pool, user).await?;
    super::account_membership::rate(&state, format!("short-invite:{user}"), 60).await?;
    if !super::account_membership::encoded(&request.lookup_id, 43, 43)
        || !super::account_membership::encoded(&request.envelope, 40, 4096)
    {
        return Err(ApiError::bad_request(
            "invalid_envelope",
            "Invalid invitation",
        ));
    }
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    // First write serializes upload and redemption. Failed uploads roll back
    // the alias and do not consume a fingerprint.
    let changed=sqlx::query("UPDATE invitation_aliases SET lookup_id=?,envelope=? WHERE invite_id=? AND envelope IS NULL AND EXISTS(SELECT 1 FROM server_invites i WHERE i.id=invite_id AND i.created_by=? AND i.envelope IS NOT NULL AND i.used_at IS NULL AND i.revoked_at IS NULL AND i.expires_at>?)")
        .bind(&request.lookup_id).bind(&request.envelope).bind(id.to_string()).bind(user.to_string()).bind(Utc::now().to_rfc3339()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    if changed.rows_affected() != 1 {
        return Err(ApiError::conflict(
            "invite_unavailable",
            "Invitation unavailable",
        ));
    }
    let inserted =
        sqlx::query("INSERT OR IGNORE INTO invitation_alias_history(lookup_id) VALUES(?)")
            .bind(&request.lookup_id)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    if inserted.rows_affected() != 1 {
        return Err(ApiError::conflict(
            "invite_code_used",
            "Generate another invitation code",
        ));
    }
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn resolve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    super::account_membership::rate(
        &state,
        crate::login_rate_key(&headers, "resolve-short-invite"),
        30,
    )
    .await?;
    if !super::account_membership::encoded(&id, 43, 43) {
        return Err(ApiError::not_found("Invitation unavailable"));
    }
    let row=sqlx::query("SELECT a.envelope FROM invitation_aliases a JOIN server_invites i ON i.id=a.invite_id JOIN users u ON u.id=i.created_by WHERE a.lookup_id=? AND a.envelope IS NOT NULL AND u.server_member=1 AND i.used_at IS NULL AND i.revoked_at IS NULL AND i.expires_at>?")
        .bind(id).bind(Utc::now().to_rfc3339()).fetch_optional(&state.pool).await.map_err(ApiError::internal)?;
    let row = row.ok_or_else(|| {
        ApiError::not_found("This invitation has expired, was used, or was revoked")
    })?;
    Ok(Json(json!({"envelope":row.get::<String,_>("envelope")})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        StatusCode, TEST_MEMBER_A_ID, TEST_OWNER_ID,
        tests::test_config,
        text_tests::{request, value},
    };
    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Follow issuance, expiry, cleanup and attempted reuse as one lifecycle.
    async fn aliases_expire_and_never_reuse_a_code_or_active_word() {
        let state = AppState::new(test_config()).await.unwrap();
        sqlx::query(
            "INSERT OR IGNORE INTO server_identity(id,owner_user_id,name) VALUES(1,?,'Example')",
        )
        .bind(TEST_OWNER_ID)
        .execute(&state.pool)
        .await
        .unwrap();
        let app = crate::router(state.clone());
        let first =
            value(request(&app, "POST", "/v2/server-invites", TEST_OWNER_ID, json!({})).await)
                .await;
        let second = value(
            request(
                &app,
                "POST",
                "/v2/server-invites",
                TEST_OWNER_ID,
                json!({"expires_in_minutes":1440}),
            )
            .await,
        )
        .await;
        assert_ne!(first["short_label"], second["short_label"]);
        for invite in [&first, &second] {
            let expiry = invite["expires_at"]
                .as_str()
                .unwrap()
                .parse::<chrono::DateTime<Utc>>()
                .unwrap();
            let remaining = expiry - Utc::now();
            assert!(remaining.num_seconds() >= 43190 && remaining.num_seconds() <= 43200);
            sqlx::query(
                "UPDATE server_invites SET envelope='test-only-inner-ciphertext' WHERE id=?",
            )
            .bind(invite["id"].as_str().unwrap())
            .execute(&state.pool)
            .await
            .unwrap();
        }
        let lookup = "a".repeat(43);
        let envelope = "b".repeat(100);
        let endpoint = format!("/v2/server-invites/{}/short", first["id"].as_str().unwrap());
        let body = json!({"lookup_id":lookup,"envelope":envelope});
        assert_eq!(
            request(&app, "PUT", &endpoint, TEST_MEMBER_A_ID, body.clone())
                .await
                .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            request(&app, "PUT", &endpoint, TEST_OWNER_ID, body.clone())
                .await
                .status(),
            StatusCode::OK
        );
        let path = format!("/v2/short-invitations/{lookup}");
        assert_eq!(
            value(request(&app, "GET", &path, TEST_OWNER_ID, json!({})).await).await["envelope"],
            envelope
        );
        assert!(
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT used_at FROM server_invites WHERE id=?"
            )
            .bind(first["id"].as_str().unwrap())
            .fetch_one(&state.pool)
            .await
            .unwrap()
            .is_none()
        );
        sqlx::query("UPDATE server_invites SET expires_at='2000-01-01T00:00:00Z' WHERE id=?")
            .bind(first["id"].as_str().unwrap())
            .execute(&state.pool)
            .await
            .unwrap();
        assert_eq!(
            request(&app, "GET", &path, TEST_OWNER_ID, json!({}))
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
        cleanup(&state.pool).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM invitation_alias_history")
                .fetch_one(&state.pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM server_invites WHERE id=?")
                .bind(first["id"].as_str().unwrap())
                .fetch_one(&state.pool)
                .await
                .unwrap(),
            0
        );
        let replacement = format!(
            "/v2/server-invites/{}/short",
            second["id"].as_str().unwrap()
        );
        let reused = request(&app, "PUT", &replacement, TEST_OWNER_ID, body).await;
        assert_eq!(reused.status(), StatusCode::CONFLICT);
        let bytes = axum::body::to_bytes(reused.into_body(), 4096)
            .await
            .unwrap();
        let error: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(error["code"], "invite_code_used");
        assert_eq!(
            request(&app, "GET", &path, TEST_OWNER_ID, json!({}))
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
    }
}
