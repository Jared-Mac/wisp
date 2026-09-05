use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ServerAccount {
    pub id: String,
    pub name: String,
    pub server_url: String,
    pub profile: String,
    pub device_id: uuid::Uuid,
    pub device_token: String,
    #[serde(default)]
    pub media_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AccountRegistry {
    #[serde(default = "registry_version")]
    pub version: u8,
    #[serde(default)]
    pub selected_server_id: String,
    pub servers: Vec<ServerAccount>,
}

const fn registry_version() -> u8 {
    1
}

pub(crate) fn default_path() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .map(|root| root.join("wisp/accounts.json"))
}

pub(crate) fn stable_id(server_url: &str) -> String {
    let digest = Sha256::digest(server_url.trim_end_matches('/').as_bytes());
    let mut id = String::from("server-");
    for byte in &digest[..8] {
        write!(id, "{byte:02x}").expect("writing to a string cannot fail");
    }
    id
}

impl AccountRegistry {
    pub(crate) fn load(path: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("read server account registry {}", path.display()))?;
        let mut registry: Self = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse server account registry {}", path.display()))?;
        registry.validate()?;
        if registry.selected_server_id.is_empty() {
            registry.selected_server_id = registry.servers[0].id.clone();
        }
        Ok(registry)
    }

    fn validate(&mut self) -> anyhow::Result<()> {
        if self.version != 1 {
            bail!("unsupported Wisp account registry version {}", self.version);
        }
        if self.servers.is_empty() {
            bail!("the Wisp account registry contains no servers");
        }
        let mut ids = std::collections::BTreeSet::new();
        for server in &mut self.servers {
            server.server_url = server.server_url.trim_end_matches('/').to_owned();
            let parsed =
                Url::parse(&server.server_url).context("invalid server URL in registry")?;
            if parsed.scheme() != "https"
                && !parsed
                    .host_str()
                    .is_some_and(|host| matches!(host, "localhost" | "127.0.0.1" | "::1"))
            {
                bail!("public Wisp servers in the registry must use HTTPS");
            }
            if server.id.trim().is_empty() {
                server.id = stable_id(&server.server_url);
            }
            if server.name.trim().is_empty() {
                parsed
                    .host_str()
                    .unwrap_or("Wisp server")
                    .clone_into(&mut server.name);
            }
            if !ids.insert(server.id.clone()) {
                bail!("duplicate Wisp server id {}", server.id);
            }
            if server.profile.trim().is_empty() || server.device_token.trim().is_empty() {
                bail!("server account {} is incomplete", server.name);
            }
            if server.media_key.as_ref().is_some_and(|key| key.len() < 16) {
                bail!("server account {} has an invalid media key", server.name);
            }
        }
        if !self.selected_server_id.is_empty() && !ids.contains(&self.selected_server_id) {
            bail!("selected Wisp server is not present in the registry");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_fills_local_server_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("accounts.json");
        std::fs::write(
            &path,
            r#"{"version":1,"selected_server_id":"","servers":[{"id":"","name":"","server_url":"http://127.0.0.1:8787/","profile":"Member","device_id":"00000000-0000-4000-8000-000000000001","device_token":"token"}]}"#,
        )
        .unwrap();
        let registry = AccountRegistry::load(&path).unwrap();
        assert_eq!(registry.servers[0].name, "127.0.0.1");
        assert_eq!(registry.selected_server_id, registry.servers[0].id);
        assert_eq!(registry.servers[0].server_url, "http://127.0.0.1:8787");
    }
}
