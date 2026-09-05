use anyhow::{Context, bail};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
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

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Bootstrap,
    Register,
    Login,
}

#[derive(Deserialize)]
struct Request {
    action: Action,
    server_url: String,
    username: String,
    #[serde(default)]
    display_name: String,
    password: String,
    #[serde(default)]
    invite_code: String,
    #[serde(default)]
    bootstrap_token: String,
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut input = String::new();
    io::stdin()
        .lock()
        .read_line(&mut input)
        .context("read account request")?;
    let mut request: Request = serde_json::from_str(&input).context("parse account request")?;
    if matches!(request.action, Action::Register | Action::Login)
        && let Some(encoded) = request.invite_code.strip_prefix("wisp-invite:")
    {
        let decoded = URL_SAFE_NO_PAD
            .decode(encoded)
            .context("invalid Wisp invitation")?;
        let invite: InvitePayload =
            serde_json::from_slice(&decoded).context("invalid Wisp invitation")?;
        if invite.v != 1 {
            bail!("this invitation requires a newer Wisp version");
        }
        request.server_url = invite.server;
        request.invite_code = invite.token;
        request.media_key = invite.media_key;
    }
    let mut server = Url::parse(request.server_url.trim()).context("invalid server URL")?;
    if server.scheme() != "https"
        && !server
            .host_str()
            .is_some_and(|host| matches!(host, "localhost" | "127.0.0.1" | "::1"))
    {
        bail!("public Wisp servers must use https");
    }
    server.set_path("");
    server.set_query(None);
    server.set_fragment(None);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
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
        Action::Register => (
            "/v1/accounts/register",
            serde_json::to_value(RegisterAccountRequest {
                invite_code: request.invite_code,
                username: request.username,
                display_name: request.display_name,
                password: request.password,
                device_name: request.device_name,
                protocol_version: PROTOCOL_VERSION,
            })?,
        ),
        Action::Login => (
            "/v1/accounts/login",
            serde_json::to_value(LoginRequest {
                username: request.username,
                password: request.password,
                device_name: request.device_name,
                protocol_version: PROTOCOL_VERSION,
                invite_code: (!request.invite_code.is_empty()).then_some(request.invite_code),
            })?,
        ),
    };
    let endpoint = server.join(path)?;
    let response = client.post(endpoint).json(&body).send().await?;
    if !response.status().is_success() {
        let status = response.status();
        let detail = response
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| format!("server returned {status}"));
        bail!("{detail}");
    }
    let credential: DeviceCredential = response.json().await?;
    save_account(&server, &credential, request.media_key.as_deref())?;
    println!(
        "{}",
        serde_json::json!({"ok":true,"display_name":credential.user.display_name})
    );
    Ok(())
}
