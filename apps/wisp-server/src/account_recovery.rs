//! Recovery tokens are bearer secrets: hash at rest, never log bodies or mail.
use super::*;
use argon2::password_hash::rand_core::RngCore;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

const LINK_ORIGIN: &str = "https://wisp.you";
const RESET_SECONDS: i64 = 20 * 60;
const VERIFY_SECONDS: i64 = 30 * 60;

pub(super) async fn private_responses(request: Request, next: Next) -> Response {
    let recovery = request
        .uri()
        .path()
        .starts_with("/v2/accounts/recovery-email")
        || request
            .uri()
            .path()
            .starts_with("/v2/accounts/password-reset/");
    let mut response = next.run(request).await;
    if recovery {
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

pub(super) struct Recovery {
    mailer: Mailer,
    limits: Mutex<HashMap<String, VecDeque<Instant>>>,
    work: Arc<Semaphore>,
}

enum Mailer {
    Disabled,
    Local(AsyncSmtpTransport<Tokio1Executor>),
    #[cfg(test)]
    Capture(Mutex<Vec<(String, String)>>),
}

impl Default for Recovery {
    fn default() -> Self {
        Self {
            mailer: Mailer::Disabled,
            limits: Mutex::default(),
            work: Arc::new(Semaphore::new(4)),
        }
    }
}

impl AppState {
    /// The owner's existing local MTA owns relay credentials, DKIM and retries.
    /// Never submit plaintext over a non-loopback connection.
    pub async fn enable_recovery_mail(mut self) -> anyhow::Result<Self> {
        let mailer = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("127.0.0.1")
            .timeout(Some(Duration::from_secs(10)))
            .build();
        if !mailer.test_connection().await.unwrap_or(false) {
            warn!("Recovery mail service is temporarily unavailable");
        }
        self.recovery = Arc::new(Recovery {
            mailer: Mailer::Local(mailer),
            ..Recovery::default()
        });
        Ok(self)
    }
}

impl Recovery {
    pub(super) fn available(&self) -> bool {
        !matches!(self.mailer, Mailer::Disabled)
    }

    pub(super) async fn rate(
        &self,
        headers: &HeaderMap,
        scope: &str,
        per_peer: usize,
        global: usize,
    ) -> Result<(), ApiError> {
        let now = Instant::now();
        let mut limits = self.limits.lock().await;
        limits.retain(|_, attempts| {
            attempts.retain(|t| now.duration_since(*t) < LOGIN_FAILURE_WINDOW);
            !attempts.is_empty()
        });
        // The production reverse proxy replaces X-Forwarded-For; the API binds
        // loopback. Both limits are atomic and apply equally to unknown accounts.
        let peer = login_rate_key(headers, &format!("recovery-{scope}"));
        let total = format!("global-{scope}");
        if limits.get(&peer).map_or(0, VecDeque::len) >= per_peer
            || limits.get(&total).map_or(0, VecDeque::len) >= global
        {
            return Err(rate_error());
        }
        limits.entry(peer).or_default().push_back(now);
        limits.entry(total).or_default().push_back(now);
        Ok(())
    }

    pub(super) async fn send(&self, email: &str, kind: &str, token: &str) -> Result<(), ApiError> {
        let (subject, path, minutes) = if kind == "verify" {
            (
                "Verify your Wisp recovery email",
                "verify-email",
                VERIFY_SECONDS / 60,
            )
        } else {
            (
                "Reset your Wisp password",
                "reset-password",
                RESET_SECONDS / 60,
            )
        };
        let body = format!(
            "{subject}\n\n{LINK_ORIGIN}/account/{path}#token={token}\n\nThis link expires in {minutes} minutes and can be used once. If you didn't request it, ignore this email.\n\nYour devices stay signed in. You can revoke them separately in Settings → Devices. A password reset does not replace your encryption recovery file.\n\nWisp\n"
        );
        match &self.mailer {
            Mailer::Disabled => return Err(mail_unavailable()),
            Mailer::Local(transport) => {
                let message = Message::builder()
                    .from(
                        "Wisp <support@wisp.you>"
                            .parse()
                            .map_err(|_| mail_unavailable())?,
                    )
                    .to(email.parse().map_err(|_| invalid_email())?)
                    .subject(subject)
                    .body(body)
                    .map_err(|_| mail_unavailable())?;
                // No tracing feature, credentials, body, recipient, or SMTP error
                // text in logs. No application retry after uncertain acceptance.
                transport
                    .send(message)
                    .await
                    .map_err(|_| mail_unavailable())?;
            }
            #[cfg(test)]
            Mailer::Capture(messages) => messages.lock().await.push((email.into(), body)),
        }
        Ok(())
    }
}

pub(super) fn mail_unavailable() -> ApiError {
    ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        code: "mail_unavailable",
        message: "Recovery email is temporarily unavailable. Try again later".into(),
    }
}
pub(super) fn rate_error() -> ApiError {
    ApiError {
        status: StatusCode::TOO_MANY_REQUESTS,
        code: "recovery_rate_limited",
        message: "Too many attempts. Try again later".into(),
    }
}
fn invalid_email() -> ApiError {
    ApiError::bad_request("invalid_email", "Enter a valid email address")
}
pub(super) fn invalid_token() -> ApiError {
    ApiError::bad_request(
        "invalid_token",
        "This link is invalid or expired. Request a new one",
    )
}
pub(super) fn unavailable_email() -> ApiError {
    ApiError::conflict(
        "recovery_email_unavailable",
        "That email address is unavailable for this account",
    )
}

pub(super) fn normalize_email(input: &str) -> Result<String, ApiError> {
    let email = input.trim().to_ascii_lowercase();
    if email.len() > 254
        || !email.is_ascii()
        || email.contains(['\r', '\n'])
        || email.parse::<lettre::Address>().is_err()
    {
        return Err(invalid_email());
    }
    Ok(email)
}

fn new_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub(super) fn checked_token(token: &str) -> Result<String, ApiError> {
    if token.len() != 43
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(invalid_token());
    }
    Ok(token_hash(token))
}

pub(super) async fn issue(
    tx: &mut sqlx::SqliteConnection,
    user: &str,
    kind: &str,
    email: &str,
    fingerprint: &str,
) -> Result<String, ApiError> {
    let token = new_token();
    let now = Utc::now().timestamp();
    sqlx::query(
        "DELETE FROM account_recovery_tokens WHERE expires_at<=? OR (user_id=? AND kind=?)",
    )
    .bind(now)
    .bind(user)
    .bind(kind)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;
    sqlx::query("INSERT INTO account_recovery_tokens(token_hash,user_id,kind,email,password_fingerprint,expires_at) VALUES (?,?,?,?,?,?)")
        .bind(token_hash(&token)).bind(user).bind(kind).bind(email).bind(fingerprint)
        .bind(now + if kind == "verify" {VERIFY_SECONDS} else {RESET_SECONDS})
        .execute(tx).await.map_err(ApiError::internal)?;
    Ok(token)
}

pub(super) async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    let row =
        sqlx::query("SELECT email,pending_email FROM account_recovery_emails WHERE user_id=?")
            .bind(user.to_string())
            .fetch_optional(&state.pool)
            .await
            .map_err(ApiError::internal)?;
    let email = row
        .as_ref()
        .and_then(|r| r.get::<Option<String>, _>("email"));
    let pending = row
        .as_ref()
        .and_then(|r| r.get::<Option<String>, _>("pending_email"));
    Ok(Json(
        json!({"verified":email.is_some(),"email":email,"pending_email":pending,"delivery_available":state.recovery.available()}),
    ))
}

#[derive(Deserialize)]
pub(super) struct Enroll {
    email: String,
    current_password: String,
}

pub(super) async fn enroll(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Enroll>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let user = authenticate_headers(&state, &headers).await?.to_string();
    if !state.recovery.available() {
        return Err(mail_unavailable());
    }
    state.recovery.rate(&headers, "enroll", 8, 80).await?;
    let email = normalize_email(&request.email)?;
    let permit = state
        .password_work
        .acquire()
        .await
        .map_err(ApiError::internal)?;
    let password: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=?")
        .bind(&user)
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    let password = password.ok_or_else(|| {
        ApiError::conflict(
            "password_unavailable",
            "This account does not use password sign-in",
        )
    })?;
    if request.current_password.len() > 1024
        || !password_matches(request.current_password, password.clone()).await?
    {
        return Err(ApiError::bad_request(
            "current_password_incorrect",
            "Current password is incorrect",
        ));
    }
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(ApiError::internal)?;
    let unchanged:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id=? AND password_hash=? AND NOT EXISTS(SELECT 1 FROM secure_credentials WHERE user_id=?))")
        .bind(&user).bind(&password).bind(&user).fetch_one(&mut *tx).await.map_err(ApiError::internal)?;
    if !unchanged {
        return Err(ApiError::conflict(
            "account_state_changed",
            "Account state changed; refresh before continuing",
        ));
    }
    let token = reserve_verification(&mut tx, &user, &email, &token_hash(&password)).await?;
    tx.commit().await.map_err(ApiError::internal)?;
    drop(permit);
    if let Some(token) = token {
        state.recovery.send(&email, "verify", &token).await?;
    }
    Ok(accepted())
}

pub(super) async fn reserve_verification(
    connection: &mut sqlx::SqliteConnection,
    user: &str,
    email: &str,
    fingerprint: &str,
) -> Result<Option<String>, ApiError> {
    sqlx::query("INSERT OR IGNORE INTO account_recovery_emails(user_id) VALUES (?)")
        .bind(user)
        .execute(&mut *connection)
        .await
        .map_err(ApiError::internal)?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM account_recovery_emails WHERE email=? AND user_id<>?)",
    )
    .bind(email)
    .bind(user)
    .fetch_one(&mut *connection)
    .await
    .map_err(ApiError::internal)?;
    if exists {
        return Err(unavailable_email());
    }
    let row = sqlx::query(
        "SELECT email,verification_sent_at FROM account_recovery_emails WHERE user_id=?",
    )
    .bind(user)
    .fetch_one(&mut *connection)
    .await
    .map_err(ApiError::internal)?;
    if row.get::<Option<String>, _>("email").as_deref() == Some(email) {
        return Ok(None);
    }
    let now = Utc::now().timestamp();
    if now - row.get::<i64, _>("verification_sent_at") < 60 {
        return Err(rate_error());
    }
    sqlx::query(
        "UPDATE account_recovery_emails SET pending_email=?,verification_sent_at=? WHERE user_id=?",
    )
    .bind(email)
    .bind(now)
    .bind(user)
    .execute(&mut *connection)
    .await
    .map_err(ApiError::internal)?;
    issue(connection, user, "verify", email, fingerprint)
        .await
        .map(Some)
}

fn accepted() -> (StatusCode, Json<Value>) {
    (StatusCode::ACCEPTED, Json(json!({"ok":true})))
}

#[derive(Deserialize)]
pub(super) struct Token {
    token: String,
}
#[derive(Deserialize)]
pub(super) struct ResetRequest {
    identifier: String,
}
#[derive(Deserialize)]
pub(super) struct Complete {
    token: String,
    new_password: String,
}

pub(super) fn credential_fingerprint(
    password: Option<&str>,
    generation: Option<&str>,
) -> Result<String, ApiError> {
    if let Some(generation) = generation {
        if password.is_some() || Uuid::parse_str(generation).is_err() {
            return Err(invalid_token());
        }
        return Ok(token_hash(&format!("wisp-secure-recovery-v1:{generation}")));
    }
    password.map(token_hash).ok_or_else(invalid_token)
}

async fn live_token(state: &AppState, digest: &str, kind: &str) -> Result<SqliteRow, ApiError> {
    let mut connection = state.pool.acquire().await.map_err(ApiError::internal)?;
    live_token_in(&mut connection, digest, kind).await
}
pub(super) async fn live_token_in(
    connection: &mut sqlx::SqliteConnection,
    digest: &str,
    kind: &str,
) -> Result<SqliteRow, ApiError> {
    let row=sqlx::query("SELECT t.user_id,t.email,t.password_fingerprint,t.expires_at,u.password_hash,u.username,c.generation AS secure_generation,r.email AS verified_email,r.pending_email FROM account_recovery_tokens t JOIN users u ON u.id=t.user_id JOIN account_recovery_emails r ON r.user_id=t.user_id LEFT JOIN secure_credentials c ON c.user_id=t.user_id WHERE t.token_hash=? AND t.kind=? AND t.consumed_at IS NULL AND t.expires_at>?")
        .bind(digest).bind(kind).bind(Utc::now().timestamp()).fetch_optional(connection).await.map_err(ApiError::internal)?.ok_or_else(invalid_token)?;
    check_live(&row, kind)?;
    Ok(row)
}
fn check_live(row: &SqliteRow, kind: &str) -> Result<(), ApiError> {
    let current: Option<String> = row.get("password_hash");
    let generation: Option<String> = row.get("secure_generation");
    let email: Option<String> = row.get(if kind == "verify" {
        "pending_email"
    } else {
        "verified_email"
    });
    if credential_fingerprint(current.as_deref(), generation.as_deref())?
        != row.get::<String, _>("password_fingerprint")
        || email.as_deref() != Some(&row.get::<String, _>("email"))
    {
        return Err(invalid_token());
    }
    Ok(())
}
fn require_classic_reset(row: &SqliteRow) -> Result<(), ApiError> {
    if row.get::<Option<String>, _>("secure_generation").is_some() {
        return Err(ApiError::conflict(
            "secure_reset_required",
            "Open Wisp to reset this account's secure password",
        ));
    }
    Ok(())
}

pub(super) async fn verify(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Token>,
) -> Result<Json<Value>, ApiError> {
    state.recovery.rate(&headers, "verify", 30, 300).await?;
    let digest = checked_token(&request.token)?;
    consume(&state, &digest, "verify", None).await?;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn request_reset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ResetRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if !state.recovery.available() {
        return Err(mail_unavailable());
    }
    state.recovery.rate(&headers, "request", 8, 120).await?;
    let permit = state
        .recovery
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| rate_error())?;
    let identifier = request.identifier.trim().to_ascii_lowercase();
    // Never wait for account lookup/SMTP before responding, including unknown or
    // unverified accounts. Capacity/throttling applies before account lookup.
    tokio::spawn(async move {
        let _permit = permit;
        if identifier.len() <= 254 && reset_mail(&state, &identifier).await.is_err() {
            warn!("Recovery email could not be queued; no automatic retry");
        }
    });
    Ok(accepted())
}

async fn reset_mail(state: &AppState, identifier: &str) -> Result<(), ApiError> {
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    // Acquire SQLite's write lock before reading cooldown or issuing a link.
    sqlx::query("UPDATE account_recovery_emails SET reset_sent_at=reset_sent_at WHERE email=? OR user_id=(SELECT id FROM users WHERE username=? COLLATE NOCASE)")
        .bind(identifier).bind(identifier).execute(&mut *tx).await.map_err(ApiError::internal)?;
    let row = sqlx::query("SELECT r.user_id,r.email,r.reset_sent_at,u.password_hash,c.generation AS secure_generation FROM account_recovery_emails r JOIN users u ON u.id=r.user_id LEFT JOIN secure_credentials c ON c.user_id=u.id WHERE r.email IS NOT NULL AND (u.password_hash IS NOT NULL OR c.generation IS NOT NULL) AND (r.email=? OR u.username=? COLLATE NOCASE)")
        .bind(identifier).bind(identifier).fetch_optional(&mut *tx).await.map_err(ApiError::internal)?;
    let Some(row) = row else { return Ok(()) };
    let now = Utc::now().timestamp();
    if now - row.get::<i64, _>("reset_sent_at") < 60 {
        return Ok(());
    }
    let user: String = row.get("user_id");
    let email: String = row.get("email");
    sqlx::query("UPDATE account_recovery_emails SET reset_sent_at=? WHERE user_id=?")
        .bind(now)
        .bind(&user)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    let token = issue(
        &mut tx,
        &user,
        "reset",
        &email,
        &credential_fingerprint(
            row.get::<Option<String>, _>("password_hash").as_deref(),
            row.get::<Option<String>, _>("secure_generation").as_deref(),
        )?,
    )
    .await?;
    tx.commit().await.map_err(ApiError::internal)?;
    state.recovery.send(&email, "reset", &token).await
}

pub(super) async fn inspect(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Token>,
) -> Result<Json<Value>, ApiError> {
    state.recovery.rate(&headers, "token", 60, 600).await?;
    let row = live_token(&state, &checked_token(&request.token)?, "reset").await?;
    let expires =
        chrono::DateTime::from_timestamp(row.get("expires_at"), 0).ok_or_else(invalid_token)?;
    Ok(Json(
        json!({"valid":true,"expires_at":expires.to_rfc3339(),"secure_reset_required":row.get::<Option<String>,_>("secure_generation").is_some()}),
    ))
}

pub(super) async fn complete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Complete>,
) -> Result<Json<Value>, ApiError> {
    state.recovery.rate(&headers, "token", 60, 600).await?;
    let digest = checked_token(&request.token)?;
    require_classic_reset(&live_token(&state, &digest, "reset").await?)?;
    validate_password(&request.new_password)?;
    let _permit = state
        .password_work
        .acquire()
        .await
        .map_err(ApiError::internal)?;
    let password = hash_password(request.new_password).await?;
    consume(&state, &digest, "reset", Some(password)).await?;
    Ok(Json(json!({"ok":true})))
}

async fn consume(
    state: &AppState,
    digest: &str,
    kind: &str,
    password: Option<String>,
) -> Result<(), ApiError> {
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    let changed = sqlx::query("UPDATE account_recovery_tokens SET consumed_at=? WHERE token_hash=? AND kind=? AND consumed_at IS NULL AND expires_at>?")
        .bind(Utc::now().timestamp()).bind(digest).bind(kind).bind(Utc::now().timestamp())
        .execute(&mut *tx).await.map_err(ApiError::internal)?.rows_affected();
    if changed != 1 {
        return Err(invalid_token());
    }
    let row = sqlx::query("SELECT t.user_id,t.email,t.password_fingerprint,u.password_hash,c.generation AS secure_generation,r.email AS verified_email,r.pending_email FROM account_recovery_tokens t JOIN users u ON u.id=t.user_id JOIN account_recovery_emails r ON r.user_id=t.user_id LEFT JOIN secure_credentials c ON c.user_id=t.user_id WHERE t.token_hash=?")
        .bind(digest).fetch_one(&mut *tx).await.map_err(ApiError::internal)?;
    check_live(&row, kind)?;
    let user: String = row.get("user_id");
    if kind == "verify" {
        sqlx::query(
            "UPDATE account_recovery_emails SET email=?,pending_email=NULL WHERE user_id=?",
        )
        .bind(row.get::<String, _>("email"))
        .bind(&user)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            if e.as_database_error()
                .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
            {
                unavailable_email()
            } else {
                ApiError::internal(e)
            }
        })?;
    } else {
        require_classic_reset(&row)?;
        sqlx::query("UPDATE users SET password_hash=? WHERE id=?")
            .bind(password.ok_or_else(invalid_token)?)
            .bind(&user)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    }
    // Neither path changes UUIDs, chat identities, memberships, or device tokens.
    sqlx::query("DELETE FROM account_recovery_tokens WHERE user_id=?")
        .bind(user)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) fn captured_mail() -> Arc<Recovery> {
    Arc::new(Recovery {
        mailer: Mailer::Capture(Mutex::default()),
        ..Recovery::default()
    })
}
#[cfg(test)]
pub(crate) async fn last_test_token(state: &AppState) -> String {
    let Mailer::Capture(messages) = &state.recovery.mailer else {
        panic!("Synthetic capture required")
    };
    let messages = messages.lock().await;
    messages
        .last()
        .unwrap()
        .1
        .split("#token=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}
#[cfg(test)]
pub(crate) async fn test_mail_count(state: &AppState) -> usize {
    let Mailer::Capture(messages) = &state.recovery.mailer else {
        panic!("Synthetic capture required")
    };
    messages.lock().await.len()
}
