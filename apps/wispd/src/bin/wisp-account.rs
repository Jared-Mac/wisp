use anyhow::{Context, bail, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use clap::Parser;
use serde::Deserialize;
use std::{
    io::{self, BufRead, Read},
    path::PathBuf,
    sync::Arc,
};
use wisp_crypto::SecretString;
use zeroize::{Zeroize, Zeroizing};

use url::Url;
use wisp_protocol::{DeviceCredential, PROTOCOL_VERSION};

// Shared native controllers also serve the daemon's settings actions; each
// executable deliberately uses a subset of the same implementation.
#[allow(dead_code)]
#[path = "../account_backup.rs"]
mod account_backup;
use account_backup::{
    api::Api,
    store::{Kind, Store},
};

#[path = "../accounts.rs"]
mod accounts;

/// Manage a Wisp account using one JSON request on stdin.
#[derive(Parser)]
struct Args {}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Bootstrap,
    Register,
    Login,
    ClassicLogin,
    SetupStatus,
    Resume,
    Recover,
    RecoverClassic,
    Cancel,
    Reset,
    ResumeReset,
    FinishInstallations,
    PreviewInvite,
    AcceptInvite,
}

#[derive(Deserialize)]
struct Request {
    action: Action,
    #[serde(default)]
    server_url: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    invite_code: String,
    #[serde(default)]
    bootstrap_token: String,
    #[serde(default)]
    device_name: String,
    #[serde(default)]
    media_key: Option<String>,
    #[serde(default)]
    reset_link: String,
}
impl Drop for Request {
    fn drop(&mut self) {
        self.password.zeroize();
        self.bootstrap_token.zeroize();
        self.invite_code.zeroize();
        self.reset_link.zeroize();
        self.media_key.zeroize();
    }
}

#[derive(Deserialize)]
struct InvitePayload {
    v: u8,
    server: String,
    token: String,
    #[serde(default)]
    media_key: Option<String>,
}

fn config_path() -> anyhow::Result<PathBuf> {
    Ok(registry_path()?.with_file_name("account.env"))
}

fn registry_path() -> anyhow::Result<PathBuf> {
    accounts::default_path().context("HOME or XDG_CONFIG_HOME is required")
}

fn save_account(
    server: &Url,
    credential: &DeviceCredential,
    media_key: Option<&str>,
) -> anyhow::Result<()> {
    account_backup::install::credential(
        &registry_path()?,
        &config_path()?,
        server.as_str(),
        credential,
        media_key,
        true,
    )?;
    let origin = wisp_crypto::invitation::origin(server.as_str())?;
    Store::at(&account_backup::default_root()?, &origin)?
        .capture_media_key(media_key.map(str::to_owned))
}

/// Caller holds the account action lease. Newly accepted media keys should be
/// uploaded before setup reports success, while a failed upload keeps login usable.
async fn sync_saved(
    store: Arc<Store>,
    origin: &str,
    credential: &DeviceCredential,
) -> anyhow::Result<()> {
    let record = store.record()?;
    if !record.secure
        || !record.key_verified
        || !record.auto_sync
        || record.dirty == record.synced_dirty
    {
        return Ok(());
    }
    let mut api = Api::connect(origin, store, None, None).await?;
    api.session(credential).await?;
    api.sync().await
}

fn saved_account(server: &str) -> anyhow::Result<Option<accounts::ServerAccount>> {
    let path = registry_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let registry = accounts::AccountRegistry::load(&path)?;
    Ok(registry
        .servers
        .into_iter()
        .find(|a| a.server_url.trim_end_matches('/') == server.trim_end_matches('/')))
}

async fn response_value(mut response: reqwest::Response) -> anyhow::Result<serde_json::Value> {
    let status = response.status();
    let mut bytes = Zeroizing::new(Vec::new());
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| anyhow::anyhow!("Could not read the account response"))?
    {
        ensure!(
            bytes.len().saturating_add(chunk.len()) <= 32768,
            "Wisp response is too large"
        );
        bytes.extend_from_slice(&chunk);
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| anyhow::anyhow!("Invalid account response"))?;
    if !status.is_success() {
        // Response bodies may echo credentials. Only fixed messages cross the UI boundary.
        bail!(match value["code"].as_str().unwrap_or("") {
            "secure_authentication_required" | "secure_account_required" =>
                "Use secure sign-in for this account.",
            "invalid_credentials" | "login_failed" | "unauthorized" =>
                "Sign-in failed. Check your username and password.",
            "username_taken" => "That username is unavailable.",
            "invalid_password" => "Use a password of at least 12 characters.",
            "invite_expired" | "invalid_invite" =>
                "This invitation is unavailable. Ask for a new link.",
            "login_rate_limited" | "rate_limited" => "Too many attempts. Wait before trying again.",
            _ => "Wisp could not complete this request. Your saved account was preserved.",
        });
    }
    Ok(value)
}

async fn default_server(client: &reqwest::Client) -> anyhow::Result<String> {
    if let Ok(value) = std::env::var("WISP_ACCOUNT_SERVER") {
        return wisp_crypto::invitation::origin(&value);
    }
    let response = client
        .get("https://wisp.you/.well-known/wisp-account.json")
        .send()
        .await
        .context("Could not reach Wisp account setup. Try again shortly.")?;
    let value = response_value(response).await?;
    wisp_crypto::invitation::origin(
        value["server"]
            .as_str()
            .context("Wisp account service is unavailable")?,
    )
}

enum Resolved {
    Modern(wisp_crypto::invitation::Invitation),
    Legacy(InvitePayload),
}
impl Resolved {
    fn server(&self) -> &str {
        match self {
            Self::Modern(i) => &i.server,
            Self::Legacy(i) => &i.server,
        }
    }
    fn media_key(&self) -> Option<&str> {
        match self {
            Self::Modern(i) => i.media_key.as_deref(),
            Self::Legacy(i) => i.media_key.as_deref(),
        }
    }
}

async fn resolve(client: &reqwest::Client, value: &str) -> anyhow::Result<Resolved> {
    if value.len() > 16384 {
        bail!("Invitation is too large");
    }
    let unwrapped;
    let value = if let Some(encoded) = value.strip_prefix("wisp-invite:v2.") {
        unwrapped = String::from_utf8(
            URL_SAFE_NO_PAD
                .decode(encoded)
                .context("Invalid Wisp invitation")?,
        )
        .context("Invalid Wisp invitation")?;
        unwrapped.as_str()
    } else {
        value
    };
    let expanded = if value.starts_with("wisp.you/") {
        format!("https://{value}")
    } else if wisp_crypto::short_invitation::valid_code(value) {
        format!("https://wisp.you/{value}")
    } else {
        value.to_owned()
    };
    let mut value = expanded;
    if value.starts_with("https://") || value.starts_with("http://") {
        if let Ok(short) = wisp_crypto::short_invitation::ShortLink::parse(&value) {
            let response = client
                .get(format!(
                    "{}/v2/short-invitations/{}",
                    short.origin,
                    short.lookup_id()
                ))
                .send()
                .await
                .context("Could not open invitation")?;
            let response = response_value(response).await?;
            value = short.open(
                response["envelope"]
                    .as_str()
                    .context("Invalid encrypted invitation")?,
            )?;
        }
        let link = wisp_crypto::invitation::Link::parse(&value)?;
        let response = client
            .get(format!(
                "{}/v2/invitations/{}",
                link.origin,
                link.lookup_id()
            ))
            .send()
            .await
            .context("Could not open invitation")?;
        let value = response_value(response).await?;
        let invite = link.open(
            value["envelope"]
                .as_str()
                .context("Invalid encrypted invitation")?,
        )?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        anyhow::ensure!(
            invite.expires_at.timestamp() > i64::try_from(now)?,
            "This invitation has expired. Ask for a new one."
        );
        return Ok(Resolved::Modern(invite));
    }
    let encoded = value
        .strip_prefix("wisp-invite:")
        .context("Paste a Wisp invitation link")?;
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded)
        .context("Invalid Wisp invitation")?;
    let mut invite: InvitePayload =
        serde_json::from_slice(&decoded).context("Invalid Wisp invitation")?;
    anyhow::ensure!(
        invite.v == 1 && !invite.token.is_empty() && invite.token.len() <= 256,
        "This invitation requires a newer Wisp version"
    );
    let origin = wisp_crypto::invitation::origin(&invite.server)?;
    anyhow::ensure!(
        origin == invite.server.trim_end_matches('/'),
        "Invalid invitation issuer"
    );
    anyhow::ensure!(
        invite
            .media_key
            .as_ref()
            .is_none_or(|key| (16..=1024).contains(&key.len()) && !key.contains(['\n', '\r'])),
        "Invalid invitation media key"
    );
    invite.server = origin;
    Ok(Resolved::Legacy(invite))
}

async fn session(
    client: &reqwest::Client,
    server: &str,
    credential: &DeviceCredential,
) -> anyhow::Result<wisp_protocol::DeviceSession> {
    let response=client.post(format!("{server}/v1/sessions")).json(&serde_json::json!({"device_id":credential.device_id,"device_token":credential.device_token,"protocol_version":PROTOCOL_VERSION})).send().await.map_err(|_|anyhow::anyhow!("Could not reach the account server"))?;
    let value: wisp_protocol::DeviceSession =
        serde_json::from_value(response_value(response).await?)
            .map_err(|_| anyhow::anyhow!("Invalid account session"))?;
    ensure!(
        value.device_id == credential.device_id
            && !value.user.id.is_nil()
            && (credential.user.id.is_nil() || credential.user.id == value.user.id)
            && value.protocol_version == PROTOCOL_VERSION
            && (32..=512).contains(&value.token.len()),
        "Invalid account session"
    );
    Ok(value)
}

async fn accept(
    client: &reqwest::Client,
    invite: &wisp_crypto::invitation::Invitation,
    credential: &DeviceCredential,
) -> anyhow::Result<()> {
    let token = session(client, &invite.server, credential).await?;
    response_value(
        client
            .post(format!("{}/v2/server/join", invite.server))
            .bearer_auth(token.token)
            .json(&serde_json::json!({"code":invite.code}))
            .send()
            .await?,
    )
    .await?;
    Ok(())
}

#[tokio::main]
#[allow(clippy::too_many_lines)] // One CLI action owns preview, credential preservation, and acceptance.
async fn main() -> anyhow::Result<()> {
    Args::parse();
    let mut input = Zeroizing::new(String::new());
    io::stdin()
        .lock()
        .take(32769)
        .read_line(&mut input)
        .context("read account request")?;
    anyhow::ensure!(input.len() <= 32768, "Account request is too large");
    let mut request: Request =
        serde_json::from_str(&input).map_err(|_| anyhow::anyhow!("Invalid account request"))?;
    input.zeroize();
    if matches!(request.action, Action::FinishInstallations) {
        let installed = account_backup::install::finish_installations(
            &account_backup::default_root()?,
            &registry_path()?,
            &config_path()?,
        )?;
        println!("{}", serde_json::json!({"ok":true,"installed":installed}));
        return Ok(());
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let resolved = if request.invite_code.starts_with("wisp-invite:")
        || request.invite_code.starts_with("https://")
        || request.invite_code.starts_with("http://")
        || request.invite_code.starts_with("wisp.you/")
        || wisp_crypto::short_invitation::valid_code(request.invite_code.trim())
    {
        Some(resolve(&client, request.invite_code.trim()).await?)
    } else {
        None
    };
    if matches!(request.action, Action::PreviewInvite) {
        let invite = resolved.context("Paste a Wisp invitation link")?;
        let saved = saved_account(invite.server())?;
        let value = match &invite {
            Resolved::Modern(i) => {
                serde_json::json!({"server":i.server,"server_name":i.server_name,"inviter":i.inviter.display_name,"expires_at":i.expires_at,"legacy":false,"saved_account":saved.is_some(),"account_name":saved.as_ref().map(|a|&a.profile)})
            }
            Resolved::Legacy(i) => {
                serde_json::json!({"server":i.server,"server_name":Url::parse(&i.server)?.host_str(),"legacy":true,"saved_account":false})
            }
        };
        println!("{value}");
        return Ok(());
    }
    if matches!(request.action, Action::AcceptInvite) {
        let Resolved::Modern(invite) = resolved.context("Paste a Wisp invitation link")? else {
            bail!("Sign in to accept a legacy invitation");
        };
        let saved = saved_account(&invite.server)?.context("Sign in to accept this invitation")?;
        let mut token_credential = DeviceCredential {
            device_id: saved.device_id,
            device_token: saved.device_token,
            user: wisp_protocol::UserSummary {
                id: uuid::Uuid::nil(),
                display_name: saved.profile,
            },
        };
        token_credential.user = session(&client, &invite.server, &token_credential)
            .await?
            .user;
        accept(&client, &invite, &token_credential).await?;
        save_account(
            &Url::parse(&invite.server)?,
            &token_credential,
            invite.media_key.as_deref(),
        )?;
        let store = Arc::new(Store::at(&account_backup::default_root()?, &invite.server)?);
        let warning = if let Ok(_action) = store.action() {
            sync_saved(store.clone(), &invite.server, &token_credential)
                .await
                .err()
                .map(|_| "You joined. Your encrypted backup has changes waiting to sync.")
        } else {
            Some("You joined. Your encrypted backup will sync after the current account action.")
        };
        println!(
            "{}",
            serde_json::json!({"ok":true,"joined":true,"warning":warning})
        );
        return Ok(());
    }
    if let Some(invite) = &resolved {
        request.server_url = invite.server().into();
    }
    if request.server_url.trim().is_empty() {
        request.server_url = default_server(&client).await?;
    }
    let canonical = wisp_crypto::invitation::origin(request.server_url.trim())?;
    let server = Url::parse(&canonical)?;
    let legacy_code = match &resolved {
        Some(Resolved::Legacy(i)) => i.token.clone(),
        Some(Resolved::Modern(_)) => String::new(),
        None => request.invite_code.clone(),
    };
    let root = account_backup::default_root()?;
    let reset = matches!(request.action, Action::Reset | Action::ResumeReset);
    let store_root = if reset {
        root.join("password-recovery")
    } else {
        root
    };
    let store = Arc::new(Store::at(&store_root, &canonical)?);
    let _action = store.action()?;
    if matches!(request.action, Action::SetupStatus) {
        let record = store.record()?;
        println!(
            "{}",
            serde_json::json!({"ok":true,"username":record.username,
            "secure":record.secure,"pending":record.pending.as_ref().map(|p|p.kind),
            "pending_sent":record.pending.as_ref().is_some_and(|p|p.sent),
            "can_cancel":record.pending.as_ref().is_some_and(|p|!p.sent && p.recovery.is_empty()),
            "installation_ready":record.installation_ready,"unlocked":record.key_verified})
        );
        return Ok(());
    }
    let credential = if matches!(request.action, Action::Bootstrap | Action::ClassicLogin) {
        ensure!(
            !store.record()?.secure && store.record()?.pending.is_none(),
            "Use secure sign-in or resume the pending account action. Classic sign-in is disabled for this account."
        );
        let (path, mut body) = if matches!(request.action, Action::Bootstrap) {
            (
                "/v1/devices/bootstrap",
                serde_json::json!({"bootstrap_token":request.bootstrap_token,
                "username":request.username,"display_name":request.display_name,"password":request.password,
                "device_name":request.device_name,"protocol_version":PROTOCOL_VERSION}),
            )
        } else {
            (
                "/v1/accounts/login",
                serde_json::json!({"username":request.username,"password":request.password,
                "device_name":request.device_name,"protocol_version":PROTOCOL_VERSION,
                "invite_code":(!legacy_code.is_empty()).then_some(&legacy_code)}),
            )
        };
        let sent = client.post(server.join(path)?).json(&body).send().await;
        if let Some(serde_json::Value::String(password)) = body.get_mut("password") {
            password.zeroize();
        }
        if let Some(serde_json::Value::String(token)) = body.get_mut("bootstrap_token") {
            token.zeroize();
        }
        let credential: DeviceCredential = serde_json::from_value(
            response_value(
                sent.map_err(|_| anyhow::anyhow!("Could not reach the account server"))?,
            )
            .await?,
        )
        .map_err(|_| anyhow::anyhow!("Invalid account credential"))?;
        ensure!(
            !credential.user.id.is_nil() && !credential.device_id.is_nil(),
            "Invalid account credential"
        );
        if let Some(scope) = store.record()?.scope {
            ensure!(
                scope.account == credential.user.id,
                "This device holds a different account. Its keys were preserved."
            );
        }
        credential
    } else {
        ensure!(
            legacy_code.is_empty() || reset,
            "Use a current Wisp invitation link with secure sign-in. Ask for a new invitation."
        );
        let mut api = Api::connect(&canonical, store.clone(), None, None).await?;
        if matches!(request.action, Action::Resume)
            && let Ok(credential) = api.installation()
        {
            let _ = api.session(&credential).await;
        }
        let native_password = || SecretString::from(request.password.clone());
        match request.action {
            Action::Register => {
                api.signup(
                    &request.username,
                    &request.display_name,
                    &request.device_name,
                    native_password(),
                )
                .await?;
            }
            Action::Login => {
                api.login(&request.username, native_password(), &request.device_name)
                    .await?;
            }
            Action::Recover => {
                api.recover_secure(&request.username, native_password(), &request.device_name)
                    .await?;
            }
            Action::RecoverClassic => {
                api.recover_classic_migration(native_password(), &request.device_name)
                    .await?;
            }
            Action::Cancel => {
                api.cancel_prepared()?;
                println!("{}", serde_json::json!({"ok":true,"cancelled":true}));
                return Ok(());
            }
            Action::Reset => {
                api.reset_password(&request.reset_link, native_password())
                    .await?;
                println!("{}", serde_json::json!({"ok":true,"reset":true}));
                return Ok(());
            }
            Action::ResumeReset => {
                api.resume_reset(
                    (!request.reset_link.is_empty()).then_some(request.reset_link.as_str()),
                )
                .await?;
                println!("{}", serde_json::json!({"ok":true,"reset":true}));
                return Ok(());
            }
            Action::Resume => {
                if let Some(pending) = store.record()?.pending {
                    let optional = (!request.password.is_empty()).then(native_password);
                    match pending.kind {
                        Kind::Signup | Kind::Migration => api.resume_enrollment(optional).await?,
                        Kind::Login => {
                            api.resume_login().await?;
                        }
                        Kind::Password | Kind::Rewrap | Kind::Email => {
                            api.resume_mutation(optional).await?;
                        }
                        Kind::Sync => api.sync().await?,
                        _ => bail!("Resume password recovery from its own form"),
                    }
                }
            }
            _ => unreachable!(),
        }
        store.update(None, |record| {
            record.installation_select = true;
            Ok(())
        })?;
        ensure!(
            store.record()?.installation_ready,
            "Complete account recovery before opening Wisp"
        );
        let credential = api.installation()?;
        api.session(&credential).await?;
        api.install_default()?;
        credential
    };
    // Save successful authentication even if the invite expires during signup.
    // Never force someone to create the account a second time after that race.
    save_account(&server, &credential, request.media_key.as_deref())?;
    let mut joined = false;
    let mut warning = None;
    if let Some(Resolved::Modern(invite)) = &resolved {
        match accept(&client, invite, &credential).await {
            Ok(()) => {
                save_account(&server, &credential, invite.media_key.as_deref())?;
                joined = true;
            }
            Err(_) => {
                warning = Some(
                    "Your account is saved, but the invitation could not be accepted. Open Wisp and try a new invitation.",
                );
            }
        }
    } else if let Some(invite) = &resolved {
        save_account(&server, &credential, invite.media_key())?;
        joined = true;
    }
    if sync_saved(store, &canonical, &credential).await.is_err() && warning.is_none() {
        warning = Some("Your account is saved. Its encrypted backup has changes waiting to sync.");
    }
    println!(
        "{}",
        serde_json::json!({"ok":true,"display_name":credential.user.display_name,"joined":joined,"warning":warning})
    );
    Ok(())
}
