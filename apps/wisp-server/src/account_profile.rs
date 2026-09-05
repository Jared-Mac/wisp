use super::{
    ApiError, AppState, HeaderMap, Json, Row, State, UserId, Value, authenticate_headers,
    chat_identity, check_login_rate, clear_login_failures, hash_password, password_matches,
    record_login_failure, token_hash, validate_password,
};
use serde_json::json;
use wisp_crypto::{PublicIdentity, profile::SignedProfile};
use wisp_protocol::{AccountProfile, ChangePasswordRequest};

async fn load(state: &AppState, user: UserId) -> Result<AccountProfile, ApiError> {
    let row = sqlx::query("SELECT u.username,u.display_name,u.password_hash IS NOT NULL AS password_available,COALESCE(p.revision,0) AS revision FROM users u LEFT JOIN account_profiles p ON p.user_id=u.id WHERE u.id=?")
        .bind(user.to_string()).fetch_one(&state.pool).await.map_err(ApiError::internal)?;
    Ok(AccountProfile {
        user_id: user,
        username: row.get("username"),
        display_name: row.get("display_name"),
        revision: u64::try_from(row.get::<i64, _>("revision")).map_err(ApiError::internal)?,
        password_available: row.get("password_available"),
    })
}

pub(super) async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AccountProfile>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    Ok(Json(load(&state, user).await?))
}

pub(super) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SignedProfile>,
) -> Result<Json<AccountProfile>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    if request.profile.account != user
        || request.profile.network != chat_identity::network(&state).await?
    {
        return Err(ApiError::forbidden("Invalid profile identity"));
    }
    let key: Option<String> =
        sqlx::query_scalar("SELECT public_identity FROM chat_identities WHERE user_id=?")
            .bind(user.to_string())
            .fetch_optional(&state.pool)
            .await
            .map_err(ApiError::internal)?;
    let key: PublicIdentity = serde_json::from_str(&key.ok_or_else(|| {
        ApiError::conflict(
            "encryption_required",
            "Set up account encryption before changing your display name",
        )
    })?)
    .map_err(ApiError::internal)?;
    request
        .verify(&key)
        .map_err(|_| ApiError::bad_request("invalid_profile", "Invalid signed profile"))?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    // Serialize renames with room admission and other profile updates.
    sqlx::query("UPDATE users SET display_name=display_name WHERE id=?")
        .bind(user.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM hangout_members WHERE user_id=? AND left_at IS NULL)",
    )
    .bind(user.to_string())
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::internal)?;
    if active {
        return Err(ApiError::conflict(
            "profile_in_call",
            "Leave voice on your devices before changing your display name",
        ));
    }
    let previous: Option<i64> =
        sqlx::query_scalar("SELECT revision FROM account_profiles WHERE user_id=?")
            .bind(user.to_string())
            .fetch_optional(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    if request.profile.revision
        != u64::try_from(previous.unwrap_or(0)).map_err(ApiError::internal)? + 1
    {
        return Err(ApiError::conflict(
            "profile_changed",
            "Your profile changed on another device. Refresh and try again",
        ));
    }
    sqlx::query("UPDATE users SET display_name=? WHERE id=?")
        .bind(&request.profile.display_name)
        .bind(user.to_string())
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            if error
                .as_database_error()
                .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
            {
                ApiError::conflict("display_name_taken", "That display name is already in use")
            } else {
                ApiError::internal(error)
            }
        })?;
    sqlx::query("INSERT INTO account_profiles(user_id,revision,signed_profile) VALUES (?,?,?) ON CONFLICT(user_id) DO UPDATE SET revision=excluded.revision,signed_profile=excluded.signed_profile")
        .bind(user.to_string()).bind(i64::try_from(request.profile.revision).map_err(ApiError::internal)?)
        .bind(serde_json::to_string(&request).map_err(ApiError::internal)?).execute(&mut *tx).await.map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .emit("account_profile_changed", json!({"changed":true}))
        .await;
    Ok(Json(load(&state, user).await?))
}

pub(super) async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChangePasswordRequest>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    validate_password(&request.new_password)?;
    if request.current_password.len() > 1024 {
        return Err(ApiError::bad_request(
            "invalid_password",
            "Current password is incorrect",
        ));
    }
    let rate_key = token_hash(&format!("password-change:{user}"));
    let _permit = state
        .password_work
        .acquire()
        .await
        .map_err(ApiError::internal)?;
    check_login_rate(&state, &rate_key).await?;
    let encoded: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=?")
        .bind(user.to_string())
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    let encoded = encoded.ok_or_else(|| {
        ApiError::conflict(
            "password_unavailable",
            "This account does not use password sign-in",
        )
    })?;
    if !password_matches(request.current_password, encoded.clone()).await? {
        record_login_failure(&state, &rate_key).await;
        return Err(ApiError::bad_request(
            "current_password_incorrect",
            "Current password is incorrect",
        ));
    }
    let hashed = hash_password(request.new_password).await?;
    let changed = sqlx::query("UPDATE users SET password_hash=? WHERE id=? AND password_hash=?")
        .bind(hashed)
        .bind(user.to_string())
        .bind(encoded)
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?
        .rows_affected();
    if changed != 1 {
        return Err(ApiError::conflict(
            "password_changed",
            "Your password changed on another device. Try again",
        ));
    }
    clear_login_failures(&state, &rate_key).await;
    // Device credentials are separately revocable from Settings → Devices.
    Ok(Json(json!({"ok":true})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use crate::{
        tests::test_config,
        text_tests::{request, value},
    };
    use wisp_crypto::{Identity, profile::Profile};

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn profile_routes_reject_forgery_replay_and_in_call_rename() {
        let state = AppState::new(test_config()).await.unwrap();
        let app = router(state.clone());
        let user = Uuid::parse_str(TEST_OWNER_ID).unwrap();
        let identity = Identity::generate().unwrap();
        sqlx::query("INSERT INTO chat_identities(user_id,public_identity) VALUES (?,?)")
            .bind(TEST_OWNER_ID)
            .bind(serde_json::to_string(&identity.public()).unwrap())
            .execute(&state.pool)
            .await
            .unwrap();
        let signed = Profile {
            network: chat_identity::network(&state).await.unwrap(),
            account: user,
            revision: 1,
            display_name: "New Owner".into(),
        }
        .sign(&identity)
        .unwrap();
        let body = serde_json::to_value(&signed).unwrap();
        let before = value(
            request(
                &app,
                "GET",
                "/v1/accounts/profile",
                TEST_OWNER_ID,
                json!({}),
            )
            .await,
        )
        .await;
        assert_eq!(before["revision"], 0);
        assert!(before.get("password_hash").is_none());
        assert_eq!(
            request(
                &app,
                "PATCH",
                "/v1/accounts/profile",
                TEST_MEMBER_A_ID,
                body.clone()
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        let mut forged = body.clone();
        forged["profile"]["display_name"] = json!("Forged name");
        assert_eq!(
            request(&app, "PATCH", "/v1/accounts/profile", TEST_OWNER_ID, forged)
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
        let updated = value(
            request(
                &app,
                "PATCH",
                "/v1/accounts/profile",
                TEST_OWNER_ID,
                body.clone(),
            )
            .await,
        )
        .await;
        assert_eq!(updated["display_name"], "New Owner");
        assert_eq!(updated["user_id"], TEST_OWNER_ID);
        assert_eq!(
            request(&app, "PATCH", "/v1/accounts/profile", TEST_OWNER_ID, body)
                .await
                .status(),
            StatusCode::CONFLICT
        );
        let directory =
            value(request(&app, "GET", "/v1/e2ee/state", TEST_MEMBER_A_ID, json!({})).await).await;
        assert_eq!(
            directory["profiles"][TEST_OWNER_ID]["profile"]["display_name"],
            "New Owner"
        );
        let mut next = signed.profile.clone();
        next.revision = 2;
        next.display_name = "MemberA".into();
        assert_eq!(
            request(
                &app,
                "PATCH",
                "/v1/accounts/profile",
                TEST_OWNER_ID,
                serde_json::to_value(next.clone().sign(&identity).unwrap()).unwrap()
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );
        next.display_name = "Another name".into();
        value(
            request(
                &app,
                "POST",
                "/v1/spots/join",
                TEST_OWNER_ID,
                json!({"spot_id":TEST_ROOM_ID}),
            )
            .await,
        )
        .await;
        assert_eq!(
            request(
                &app,
                "PATCH",
                "/v1/accounts/profile",
                TEST_OWNER_ID,
                serde_json::to_value(next.sign(&identity).unwrap()).unwrap()
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn password_change_checks_current_secret_and_preserves_device_credentials() {
        let mut config = test_config();
        config.allow_dev_sessions = false;
        let state = AppState::new(config).await.unwrap();
        let credential = bootstrap_device(
            State(state.clone()),
            Json(BootstrapDeviceRequest {
                bootstrap_token: "test-bootstrap-token".into(),
                username: "owner".into(),
                display_name: "Owner".into(),
                password: "old password for test".into(),
                device_name: "Fixture".into(),
                protocol_version: PROTOCOL_VERSION,
            }),
        )
        .await
        .unwrap()
        .0;
        let session_request = DeviceSessionRequest {
            device_id: credential.device_id,
            device_token: credential.device_token,
            protocol_version: PROTOCOL_VERSION,
        };
        let session = device_session(State(state.clone()), Json(session_request.clone()))
            .await
            .unwrap()
            .0;
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            format!("Bearer {}", session.token).parse().unwrap(),
        );
        let change = |current: &str, new: &str| {
            Json(ChangePasswordRequest {
                current_password: current.into(),
                new_password: new.into(),
            })
        };
        assert!(
            change_password(
                State(state.clone()),
                HeaderMap::new(),
                change("old password for test", "new password for test")
            )
            .await
            .is_err()
        );
        assert!(
            change_password(
                State(state.clone()),
                headers.clone(),
                change("wrong", "new password for test")
            )
            .await
            .is_err()
        );
        assert!(
            change_password(
                State(state.clone()),
                headers.clone(),
                change("old password for test", "short")
            )
            .await
            .is_err()
        );
        let _ = change_password(
            State(state.clone()),
            headers.clone(),
            change("old password for test", "new password for test"),
        )
        .await
        .unwrap();
        assert!(
            device_session(State(state.clone()), Json(session_request))
                .await
                .is_ok()
        );
        assert!(authenticate_headers(&state, &headers).await.is_ok());
        let login = |password: &str| {
            Json(LoginRequest {
                username: "owner".into(),
                password: password.into(),
                device_name: "Second fixture".into(),
                protocol_version: PROTOCOL_VERSION,
                invite_code: None,
            })
        };
        assert!(
            login_account(
                State(state.clone()),
                HeaderMap::new(),
                login("old password for test")
            )
            .await
            .is_err()
        );
        assert!(
            login_account(
                State(state.clone()),
                HeaderMap::new(),
                login("new password for test")
            )
            .await
            .is_ok()
        );
        let rate_key = token_hash(&format!("password-change:{}", credential.user.id));
        for _ in 0..LOGIN_FAILURE_LIMIT {
            record_login_failure(&state, &rate_key).await;
        }
        let error = change_password(
            State(state),
            headers,
            change("new password for test", "another password for test"),
        )
        .await
        .unwrap_err();
        assert_eq!(error.status, StatusCode::TOO_MANY_REQUESTS);
    }
}
