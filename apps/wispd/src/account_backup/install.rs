//! Recoverable installation of committed credentials. The account journal is
//! marked installed only after both config files have been durably replaced.
use super::{api::Api, store::Store};
use crate::accounts::{self, AccountRegistry, ServerAccount};
use age::secrecy::ExposeSecret;
use anyhow::{Context, ensure};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::Path,
};
use wisp_protocol::DeviceCredential;
use zeroize::Zeroizing;

#[allow(clippy::verbose_bit_mask)]
fn read(path: &Path, limit: usize) -> anyhow::Result<Option<Zeroizing<Vec<u8>>>> {
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file()
            && metadata.mode() & 0o077 == 0
            && metadata.nlink() == 1
            && metadata.len() <= limit as u64,
        "Account configuration must be a private bounded regular file"
    );
    let mut bytes = Zeroizing::new(Vec::new());
    Read::by_ref(&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "Account configuration is too large");
    Ok(Some(bytes))
}
fn write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .context("Missing account configuration folder")?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path)
        .context("Could not install account configuration; its recovery journal was preserved")?;
    File::open(parent)?.sync_all()?;
    Ok(())
}
struct ConfigLease(File);
impl Drop for ConfigLease {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}
#[allow(clippy::verbose_bit_mask)]
fn lock(path: &Path) -> anyhow::Result<ConfigLease> {
    let parent = path
        .parent()
        .context("Missing account configuration folder")?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)?;
    ensure!(
        fs::symlink_metadata(parent)?.is_dir(),
        "Account configuration folder must not be a link"
    );
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(parent.join(".accounts.lock"))?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.mode() & 0o077 == 0 && metadata.nlink() == 1,
        "Invalid account configuration lock"
    );
    file.lock()?;
    Ok(ConfigLease(file))
}
/// Also used by the explicit classic flow, so concurrent installations preserve
/// every server entry. Callers must never log `credential` or these file bytes.
#[allow(clippy::too_many_lines)] // Keep durable transition ordering reviewable as one operation.
pub(crate) fn credential(
    registry_path: &Path,
    env_path: &Path,
    origin: &str,
    credential: &DeviceCredential,
    media: Option<&str>,
    select: bool,
) -> anyhow::Result<()> {
    let _lock = lock(registry_path)?;
    ensure!(
        registry_path.parent() == env_path.parent(),
        "Account configuration files must share a folder"
    );
    let origin = wisp_crypto::invitation::origin(origin)?;
    let previous = read(registry_path, 4 * 1024 * 1024)?;
    let mut registry = if let Some(bytes) = &previous {
        let registry: AccountRegistry = serde_json::from_slice(bytes)
            .context("Could not read saved accounts; existing credentials were preserved")?;
        ensure!(
            registry.version == 1 && registry.servers.len() <= 4096,
            "Invalid saved accounts"
        );
        registry
    } else {
        AccountRegistry {
            version: 1,
            selected_server_id: String::new(),
            servers: Vec::new(),
        }
    };
    let index = registry
        .servers
        .iter()
        .position(|account| account.server_url.trim_end_matches('/') == origin);
    let existing = index.map(|index| &registry.servers[index]);
    let id = existing.map_or_else(
        || accounts::stable_id(&origin),
        |account| account.id.clone(),
    );
    let media = media
        .map(str::to_owned)
        .or_else(|| existing.and_then(|account| account.media_key.clone()));
    let account = ServerAccount {
        id: id.clone(),
        name: existing.map_or_else(|| "Wisp".to_owned(), |account| account.name.clone()),
        server_url: origin.clone(),
        profile: credential.user.display_name.clone(),
        device_id: credential.device_id,
        device_token: credential.device_token.clone(),
        media_key: media.clone(),
    };
    for value in [&origin, &account.profile, &account.device_token]
        .into_iter()
        .chain(media.as_ref())
    {
        ensure!(
            !value.chars().any(char::is_control),
            "Account configuration contains an invalid line break"
        );
    }
    ensure!(
        !credential.device_id.is_nil()
            && !credential.user.id.is_nil()
            && !account.device_token.is_empty(),
        "Invalid committed account credential"
    );
    if let Some(index) = index {
        registry.servers[index] = account;
    } else {
        registry.servers.push(account);
    }
    if select || registry.selected_server_id.is_empty() {
        registry.selected_server_id = id;
    }
    let old_env = read(env_path, 1024 * 1024)?;
    let was_primary = old_env
        .as_ref()
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .is_some_and(|text| {
            text.lines().any(|line| {
                line.strip_prefix("WISP_SERVER_URL=")
                    .is_some_and(|value| value.trim_end_matches('/') == origin)
            })
        });
    let update_env = select || old_env.is_none() || was_primary;
    ensure!(
        registry.servers.len() <= 4096,
        "Too many saved account services"
    );
    registry.validate()?;
    let json = Zeroizing::new(serde_json::to_vec_pretty(&registry)?);
    if previous.as_ref().map(|bytes| bytes.as_slice()) != Some(json.as_slice()) {
        if let Some(previous) = previous {
            write(&registry_path.with_extension("json.previous"), &previous)?;
        }
        write(registry_path, &json)?;
    }
    if update_env {
        use std::fmt::Write as _;
        let mut env = Zeroizing::new(format!(
            "WISP_SERVER_URL={origin}\nWISP_PROFILE={}\nWISP_DEVICE_ID={}\nWISP_DEVICE_TOKEN={}\n",
            credential.user.display_name, credential.device_id, credential.device_token
        ));
        if let Some(media) = &media {
            writeln!(&mut *env, "WISP_E2EE_KEY={media}")?;
        }
        if old_env.as_ref().map(|bytes| bytes.as_slice()) != Some(env.as_bytes()) {
            if let Some(previous) = old_env {
                write(&env_path.with_extension("env.previous"), &previous)?;
            }
            write(env_path, env.as_bytes())?;
        }
    }
    Ok(())
}
/// The caller holds this account's action lease; config writes do not block
/// ordinary encrypted-chat trust updates.
pub(crate) fn committed(store: &Store, registry: &Path, env: &Path) -> anyhow::Result<bool> {
    let record = store.record()?;
    if !record.installation_pending {
        return Ok(false);
    }
    ensure!(
        record.installation_ready,
        "Finish restoring or recovering this account before installing its credentials"
    );
    let saved: DeviceCredential = serde_json::from_value(
        record
            .installation
            .as_ref()
            .context("Missing committed installation")?
            .value()?,
    )?;
    let bundle = record.bundle()?;
    credential(
        registry,
        env,
        &record.origin,
        &saved,
        bundle
            .as_ref()
            .and_then(|bundle| bundle.media_key())
            .map(ExposeSecret::expose_secret),
        record.installation_select,
    )?;
    store.update(None, |record| {
        let current: DeviceCredential = serde_json::from_value(
            record
                .installation
                .as_ref()
                .context("Account installation changed")?
                .value()?,
        )?;
        ensure!(
            record.installation_ready
                && current.device_id == saved.device_id
                && current.user.id == saved.user.id
                && current.device_token == saved.device_token,
            "Account installation changed while saving"
        );
        record.installation_pending = false;
        Ok(())
    })?;
    Ok(true)
}
impl Api {
    pub(crate) fn install_default(&self) -> anyhow::Result<bool> {
        ensure!(
            !cfg!(test),
            "Unit tests must install into explicit fixture paths"
        );
        let path = accounts::default_path()
            .context("A private account configuration directory is required")?;
        committed(&self.store, &path, &path.with_file_name("account.env"))
    }
    pub(crate) fn session_bearer(&self) -> anyhow::Result<Zeroizing<String>> {
        self.bearer()
    }
}

pub(crate) fn finish_installations(
    root: &Path,
    registry: &Path,
    env: &Path,
) -> anyhow::Result<usize> {
    let mut installed = 0;
    for origin in Store::installed_origins(root)? {
        let store = Store::at(root, &origin)?;
        let Ok(_action) = store.action() else {
            continue;
        };
        if committed(&store, registry, env)? {
            installed += 1;
        }
    }
    Ok(installed)
}
