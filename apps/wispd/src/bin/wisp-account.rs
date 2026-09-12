use anyhow::{Context, bail};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use clap::Parser;
use serde::Deserialize;
use std::{
    fs::{self, OpenOptions},
    io::{self, BufRead, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
};
use url::Url;
use wisp_protocol::{
    BootstrapDeviceRequest, DeviceCredential, LoginRequest, PROTOCOL_VERSION,
    RegisterAccountRequest,
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
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .context("HOME or XDG_CONFIG_HOME is required")?;
    Ok(root.join("wisp/account.env"))
}

fn registry_path() -> anyhow::Result<PathBuf> {
    accounts::default_path().context("HOME or XDG_CONFIG_HOME is required")
}

fn write_private(path: &std::path::Path, contents: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().context("account config has no parent")?;
    fs::create_dir_all(parent).context("create Wisp config directory")?;
    let temporary = parent.join(format!(".account.{}.tmp", uuid::Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .context("create private account config")?;
    let result = file.write_all(contents).and_then(|()| file.sync_all());
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error).context("write private account config");
    }
    fs::rename(&temporary, path).context("install private account config")?;
    Ok(())
}

fn validate_line(name: &str, value: &str) -> anyhow::Result<()> {
    if value.contains(['\n', '\r']) {
        bail!("{name} contains an invalid line break");
    }
    Ok(())
}

fn save_account(
    server: &Url,
    credential: &DeviceCredential,
    media_key: Option<&str>,
) -> anyhow::Result<()> {
    let retained = saved_account(server.as_str())?.and_then(|account| account.media_key);
    let media_key = media_key.or(retained.as_deref());
    let path = config_path()?;
    let parent = path.parent().context("account config has no parent")?;
    fs::create_dir_all(parent).context("create Wisp config directory")?;
    let server_url = server.as_str().trim_end_matches('/');
    let profile = &credential.user.display_name;
    let device_id = credential.device_id.to_string();
    let device_token = &credential.device_token;
    for (name, value) in [
        ("server URL", server_url),
        ("display name", profile),
        ("device id", &device_id),
        ("device token", device_token),
    ] {
        validate_line(name, value)?;
    }
    if let Some(media_key) = media_key {
        validate_line("media encryption key", media_key)?;
        if media_key.len() < 16 {
            bail!("invitation contains an invalid media encryption key");
        }
    }
    let mut legacy = format!(
        "WISP_SERVER_URL={server_url}\nWISP_PROFILE={profile}\nWISP_DEVICE_ID={device_id}\nWISP_DEVICE_TOKEN={device_token}\n"
    );
    if let Some(media_key) = media_key {
        use std::fmt::Write as _;
        writeln!(legacy, "WISP_E2EE_KEY={media_key}")?;
    }
    write_private(&path, legacy.as_bytes())?;

    let registry_path = registry_path()?;
    let id = accounts::stable_id(server_url);
    let mut registry = if registry_path.exists() {
        accounts::AccountRegistry::load(&registry_path)?
    } else {
        accounts::AccountRegistry {
            version: 1,
            selected_server_id: id.clone(),
            servers: Vec::new(),
        }
    };
    let server_name = server.host_str().unwrap_or("Wisp server").to_owned();
    let account = accounts::ServerAccount {
        id: id.clone(),
        name: server_name,
        server_url: server_url.to_owned(),
        profile: profile.clone(),
        device_id: credential.device_id,
        device_token: credential.device_token.clone(),
        media_key: media_key.map(str::to_owned),
    };
    if let Some(existing) = registry.servers.iter_mut().find(|item| item.id == id) {
        *existing = account;
    } else {
        registry.servers.push(account);
    }
    registry.selected_server_id = id;
    let json = serde_json::to_vec_pretty(&registry)?;
    write_private(&registry_path, &json)?;
    Ok(())
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

async fn response_value(response: reqwest::Response) -> anyhow::Result<serde_json::Value> {
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .context("Could not read Wisp response")?;
    if bytes.len() > 32_768 {
        bail!("Wisp response is too large");
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).context("Invalid Wisp response")?;
    if !status.is_success() {
        bail!(
            "{}",
            value["message"]
                .as_str()
                .unwrap_or("Wisp could not complete this request")
        );
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
    if value.starts_with("https://") || value.starts_with("http://") {
        let link = wisp_crypto::invitation::Link::parse(value)?;
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
) -> anyhow::Result<String> {
    let response=client.post(format!("{server}/v1/sessions")).json(&serde_json::json!({"device_id":credential.device_id,"device_token":credential.device_token,"protocol_version":PROTOCOL_VERSION})).send().await?;
    let value = response_value(response).await?;
    Ok(value["token"]
        .as_str()
        .context("Invalid account session")?
        .to_owned())
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
            .bearer_auth(token)
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
    let mut input = String::new();
    io::stdin()
        .lock()
        .read_line(&mut input)
        .context("read account request")?;
    anyhow::ensure!(input.len() <= 32768, "Account request is too large");
    let mut request: Request = serde_json::from_str(&input).context("parse account request")?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let resolved = if request.invite_code.starts_with("wisp-invite:")
        || request.invite_code.starts_with("https://")
        || request.invite_code.starts_with("http://")
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
        let token_credential = DeviceCredential {
            device_id: saved.device_id,
            device_token: saved.device_token,
            user: wisp_protocol::UserSummary {
                id: uuid::Uuid::nil(),
                display_name: saved.profile,
            },
        };
        accept(&client, &invite, &token_credential).await?;
        save_account(
            &Url::parse(&invite.server)?,
            &token_credential,
            invite.media_key.as_deref(),
        )?;
        println!("{}", serde_json::json!({"ok":true,"joined":true}));
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
    let (path, body) = match request.action {
        Action::Bootstrap => (
            "/v1/devices/bootstrap",
            serde_json::to_value(BootstrapDeviceRequest {
                bootstrap_token: request.bootstrap_token,
                username: request.username,
                display_name: request.display_name,
                password: request.password,
                device_name: request.device_name,
                protocol_version: PROTOCOL_VERSION,
            })?,
        ),
        Action::Register if !legacy_code.is_empty() => (
            "/v1/accounts/register",
            serde_json::to_value(RegisterAccountRequest {
                invite_code: legacy_code,
                username: request.username,
                display_name: request.display_name,
                password: request.password,
                device_name: request.device_name,
                protocol_version: PROTOCOL_VERSION,
            })?,
        ),
        Action::Register => (
            "/v2/accounts/register",
            serde_json::json!({"username":request.username,"display_name":request.display_name,"password":request.password,"device_name":request.device_name,"protocol_version":PROTOCOL_VERSION}),
        ),
        Action::Login => (
            "/v1/accounts/login",
            serde_json::to_value(LoginRequest {
                username: request.username,
                password: request.password,
                device_name: request.device_name,
                protocol_version: PROTOCOL_VERSION,
                invite_code: (!legacy_code.is_empty()).then_some(legacy_code),
            })?,
        ),
        Action::PreviewInvite | Action::AcceptInvite => unreachable!(),
    };
    let value = response_value(client.post(server.join(path)?).json(&body).send().await?).await?;
    let credential: DeviceCredential =
        serde_json::from_value(value).context("Invalid account credential")?;
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
    println!(
        "{}",
        serde_json::json!({"ok":true,"display_name":credential.user.display_name,"joined":joined,"warning":warning})
    );
    Ok(())
}
