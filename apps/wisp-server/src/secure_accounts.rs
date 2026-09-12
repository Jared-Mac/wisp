//! Native-only secure account authentication and encrypted backup storage.
//! No handler receives an account identity secret, vault key or export key.
use super::*;
use sha2::Digest;
use wisp_crypto::{
    PublicIdentity,
    account_vault::{Scope, auth, envelope::Checkpoint, operation::Precondition},
};
use zeroize::Zeroizing;
pub(super) mod authentication;
#[cfg(test)]
mod authentication_tests;
pub(super) mod vault;
#[cfg(test)]
mod vault_tests;

fn invalid() -> ApiError {
    ApiError::bad_request("invalid_secure_request", "Invalid secure account request")
}

fn unavailable() -> ApiError {
    ApiError::conflict(
        "secure_account_unavailable",
        "Secure account data is unavailable; existing keys were preserved",
    )
}

/// Bootstrap once in the private database. A missing/corrupted setup alongside
/// existing credentials is fatal; generating another key would strand accounts.
pub(super) async fn initialize(pool: &SqlitePool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM secure_auth_setup)")
        .fetch_one(&mut *tx)
        .await?;
    if !exists {
        let credentials: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM secure_credentials)")
                .fetch_one(&mut *tx)
                .await?;
        anyhow::ensure!(
            !credentials,
            "Secure authentication setup is missing; restore its matching backup"
        );
        let material = auth::Server::new().save();
        let digest = format!("{:x}", Sha256::digest(material.as_slice()));
        sqlx::query("INSERT INTO secure_auth_setup(id,version,material,digest) VALUES(1,1,?,?)")
            .bind(material.as_slice())
            .bind(digest)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    load_native(pool).await?;
    Ok(())
}

async fn load_native(pool: &SqlitePool) -> anyhow::Result<auth::Server> {
    let row = sqlx::query("SELECT version,material,digest FROM secure_auth_setup WHERE id=1")
        .fetch_one(pool)
        .await?;
    let material = Zeroizing::new(row.get::<Vec<u8>, _>("material"));
    let digest: String = row.get("digest");
    anyhow::ensure!(
        row.get::<i64, _>("version") == 1
            && material.len() <= 4096
            && format!("{:x}", Sha256::digest(material.as_slice())) == digest,
        "Secure authentication setup failed integrity validation"
    );
    let mismatch: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM secure_credentials WHERE setup_digest<>?)")
            .bind(digest)
            .fetch_one(pool)
            .await?;
    anyhow::ensure!(
        !mismatch,
        "Secure credentials require a different authentication setup"
    );
    auth::Server::restore(&material)
}

async fn service(state: &AppState) -> Result<(String, Uuid), ApiError> {
    let configured = state.config.public_url.as_deref().ok_or_else(unavailable)?;
    let url = reqwest::Url::parse(configured).map_err(|_| unavailable())?;
    if url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(unavailable());
    }
    let origin = url.origin().ascii_serialization();
    let network = chat_identity::network(state).await?;
    auth::AccountContext {
        origin: origin.clone(),
        network,
        username: "service".into(),
    }
    .validate()
    .map_err(|_| unavailable())?;
    Ok((origin, network))
}

pub(super) async fn info(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    // Verify availability without exposing any authentication setup material.
    load_native(&state.pool).await.map_err(|_| unavailable())?;
    let (origin, network) = service(&state).await?;
    Ok(Json(
        json!({"version":1,"suite":auth::SUITE,"origin":origin,"network":network,"max_vault_plaintext_bytes":wisp_crypto::account_vault::bundle::MAX_PLAINTEXT}),
    ))
}

pub(super) async fn private_responses(request: Request, next: Next) -> Response {
    let secure = request.uri().path().starts_with("/v3/");
    let mut response = next.run(request).await;
    if secure {
        response
            .headers_mut()
            .insert("cache-control", "no-store".parse().expect("static header"));
        response.headers_mut().insert(
            "referrer-policy",
            "no-referrer".parse().expect("static header"),
        );
    }
    response
}

struct AccountState {
    scope: Scope,
    username: String,
    identity: Option<PublicIdentity>,
    expected: Precondition,
}

impl AccountState {
    fn response(&self) -> Value {
        if let Some(generation) = self.expected.credential_generation {
            json!({"mode":"secure","scope":self.scope,"username":self.username,"identity":self.identity,
                "credential_generation":generation,"wrapper_generation":self.expected.wrapper_generation,"vault":self.expected.vault})
        } else {
            json!({"mode":"classic","scope":self.scope,"username":self.username,"identity":self.identity})
        }
    }
}

async fn account_state(state: &AppState, user: Uuid) -> Result<AccountState, ApiError> {
    let (origin, network) = service(state).await?;
    let row = sqlx::query("SELECT u.username,i.public_identity,c.generation,v.wrapper_generation,v.revision,v.digest FROM users u LEFT JOIN chat_identities i ON i.user_id=u.id LEFT JOIN secure_credentials c ON c.user_id=u.id LEFT JOIN account_vaults v ON v.user_id=u.id WHERE u.id=?")
        .bind(user.to_string()).fetch_optional(&state.pool).await.map_err(ApiError::internal)?.ok_or_else(unavailable)?;
    parse_account_state(
        Scope {
            origin,
            network,
            account: user,
        },
        &row,
    )
}

fn parse_account_state(scope: Scope, row: &SqliteRow) -> Result<AccountState, ApiError> {
    let username = row
        .get::<Option<String>, _>("username")
        .ok_or_else(invalid)?;
    let username = auth::canonical_username(&username).map_err(|_| unavailable())?;
    let identity: Option<PublicIdentity> = row
        .get::<Option<String>, _>("public_identity")
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .map_err(|_| unavailable())?;
    if let Some(identity) = &identity {
        identity.validate().map_err(|_| unavailable())?;
    }
    let expected = if let Some(generation) = row.get::<Option<String>, _>("generation") {
        if identity.is_none() {
            return Err(unavailable());
        }
        Precondition {
            credential_generation: Some(generation.parse().map_err(|_| unavailable())?),
            wrapper_generation: Some(
                row.get::<Option<String>, _>("wrapper_generation")
                    .ok_or_else(unavailable)?
                    .parse()
                    .map_err(|_| unavailable())?,
            ),
            vault: Some(Checkpoint {
                revision: u64::try_from(
                    row.get::<Option<i64>, _>("revision")
                        .ok_or_else(unavailable)?,
                )
                .map_err(|_| unavailable())?,
                sha256: row
                    .get::<Option<String>, _>("digest")
                    .ok_or_else(unavailable)?,
            }),
        }
    } else {
        Precondition::default()
    };
    expected.validate().map_err(|_| unavailable())?;
    Ok(AccountState {
        scope,
        username,
        identity,
        expected,
    })
}

async fn current_precondition(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user: Uuid,
) -> Result<Precondition, ApiError> {
    let row = sqlx::query("SELECT c.generation,v.wrapper_generation,v.revision,v.digest FROM secure_credentials c LEFT JOIN account_vaults v ON v.user_id=c.user_id WHERE c.user_id=?")
        .bind(user.to_string()).fetch_optional(&mut **tx).await.map_err(ApiError::internal)?.ok_or_else(unavailable)?;
    let expected = Precondition {
        credential_generation: Some(parse_uuid(&row.get::<String, _>("generation"))?),
        wrapper_generation: Some(parse_uuid(
            &row.get::<Option<String>, _>("wrapper_generation")
                .ok_or_else(unavailable)?,
        )?),
        vault: Some(Checkpoint {
            revision: u64::try_from(
                row.get::<Option<i64>, _>("revision")
                    .ok_or_else(unavailable)?,
            )
            .map_err(|_| unavailable())?,
            sha256: row
                .get::<Option<String>, _>("digest")
                .ok_or_else(unavailable)?,
        }),
    };
    expected.validate().map_err(|_| unavailable())?;
    Ok(expected)
}

pub(super) async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    Ok(Json(account_state(&state, user).await?.response()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn setup_survives_reload_and_corruption_never_generates_replacement() {
        let state = AppState::new(super::super::tests::test_config())
            .await
            .unwrap();
        let before = load_native(&state.pool).await.unwrap().save();
        initialize(&state.pool).await.unwrap();
        let after = load_native(&state.pool).await.unwrap().save();
        let same = before.as_slice() == after.as_slice();
        assert!(same, "Server setup changed on reload");
        sqlx::query("UPDATE secure_auth_setup SET material=? WHERE id=1")
            .bind(vec![0_u8; 32])
            .execute(&state.pool)
            .await
            .unwrap();
        assert!(initialize(&state.pool).await.is_err());
        let retained: Vec<u8> =
            sqlx::query_scalar("SELECT material FROM secure_auth_setup WHERE id=1")
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(retained, vec![0_u8; 32]);
    }

    #[tokio::test]
    async fn secure_schema_blocks_legacy_verifiers_and_preserves_failed_migration() {
        let state = AppState::new(super::super::tests::test_config())
            .await
            .unwrap();
        let user = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,display_name,username,password_hash) VALUES(?,?,?,?)")
            .bind(user.to_string())
            .bind("Synthetic vault fixture")
            .bind("vaultfixture")
            .bind("synthetic-old-verifier")
            .execute(&state.pool)
            .await
            .unwrap();
        let identity = wisp_crypto::Identity::generate().unwrap();
        sqlx::query("INSERT INTO chat_identities(user_id,public_identity) VALUES(?,?)")
            .bind(user.to_string())
            .bind(serde_json::to_string(&identity.public()).unwrap())
            .execute(&state.pool)
            .await
            .unwrap();
        let setup: String = sqlx::query_scalar("SELECT digest FROM secure_auth_setup WHERE id=1")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        let mut tx = state.pool.begin().await.unwrap();
        sqlx::query("UPDATE users SET password_hash=NULL WHERE id=?")
            .bind(user.to_string())
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("INSERT INTO secure_credentials(user_id,generation,password_file,setup_digest,updated_at) VALUES(?,?,?,?,0)").bind(user.to_string()).bind(Uuid::new_v4().to_string()).bind(vec![1_u8]).bind(&setup).execute(&mut *tx).await.unwrap();
        assert!(
            sqlx::query("UPDATE users SET password_hash='forbidden' WHERE id=?")
                .bind(user.to_string())
                .execute(&mut *tx)
                .await
                .is_err()
        );
        tx.rollback().await.unwrap();
        let preserved: Option<String> =
            sqlx::query_scalar("SELECT password_hash FROM users WHERE id=?")
                .bind(user.to_string())
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(preserved.as_deref(), Some("synthetic-old-verifier"));
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM secure_credentials WHERE user_id=?")
                .bind(user.to_string())
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(count, 0);
    }
}
