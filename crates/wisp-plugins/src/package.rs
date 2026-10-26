use crate::{MAX_JSON, MAX_MODULE, Manifest, identifier};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[must_use]
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn read_bounded(path: &Path, limit: usize) -> anyhow::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && metadata.len() <= limit as u64,
        "Invalid or oversized plugin file"
    );
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "Plugin file grew beyond its limit");
    Ok(bytes)
}
pub fn write_json(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    let parent = path.parent().context("Missing storage directory")?;
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(file.as_file_mut(), value)?;
    file.as_file_mut().write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Installed {
    pub manifest: Manifest,
    pub digest: String,
    pub enabled: bool,
    #[serde(default)]
    pub previous: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instance {
    pub generation: u64,
    #[serde(default)]
    pub state: Value,
    #[serde(default)]
    pub folder: Option<PathBuf>,
    #[serde(default)]
    pub archives: Vec<crate::files::Artifact>,
}
#[derive(Clone)]
pub struct Packages {
    pub root: PathBuf,
}
impl Packages {
    pub fn inspect(directory: &Path) -> anyhow::Result<(Manifest, Vec<u8>, String)> {
        ensure!(
            fs::symlink_metadata(directory)?.is_dir(),
            "Choose a plugin package folder"
        );
        let manifest: Manifest =
            serde_json::from_slice(&read_bounded(&directory.join("plugin.json"), 16 * 1024)?)?;
        manifest.validate()?;
        let module = read_bounded(&directory.join("plugin.wasm"), MAX_MODULE)?;
        ensure!(
            module.starts_with(b"\0asm"),
            "Package must contain a Wasm module"
        );
        let digest = hash(&[serde_json::to_vec(&manifest)?, module.clone()].concat());
        Ok((manifest, module, digest))
    }
    pub fn get(&self, id: &str) -> anyhow::Result<Installed> {
        identifier(id)?;
        let installed: Installed = serde_json::from_slice(&read_bounded(
            &self.root.join(id).join("installed.json"),
            16 * 1024,
        )?)?;
        ensure!(installed.manifest.id == id, "Package identity mismatch");
        installed.manifest.validate()?;
        Ok(installed)
    }
    pub fn module_path(&self, installed: &Installed) -> anyhow::Result<PathBuf> {
        ensure!(
            installed.digest.len() == 64 && installed.digest.bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid package digest"
        );
        Ok(self
            .root
            .join(&installed.manifest.id)
            .join("versions")
            .join(&installed.digest)
            .join("plugin.wasm"))
    }
    pub fn verify(&self, installed: &Installed) -> anyhow::Result<Vec<u8>> {
        let bytes = read_bounded(&self.module_path(installed)?, MAX_MODULE)?;
        ensure!(
            hash(&[serde_json::to_vec(&installed.manifest)?, bytes.clone()].concat())
                == installed.digest,
            "Installed plugin was modified; reinstall its reviewed package"
        );
        Ok(bytes)
    }
    pub fn list(&self) -> anyhow::Result<Vec<Installed>> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut items = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                && let Some(id) = entry.file_name().to_str()
                && let Ok(item) = self.get(id)
            {
                items.push(item);
            }
        }
        items.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
        Ok(items)
    }
    /// The caller passes the digest shown in the native installation review.
    pub fn install(&self, source: &Path, approved_digest: &str) -> anyhow::Result<Installed> {
        let (manifest, module, digest) = Self::inspect(source)?;
        ensure!(digest == approved_digest, "Package changed after review");
        let base = self.root.join(&manifest.id);
        let versions = base.join("versions");
        fs::create_dir_all(&versions)?;
        let previous = self.get(&manifest.id).ok().map(|p| p.digest);
        let installed = Installed {
            manifest,
            digest: digest.clone(),
            enabled: true,
            previous,
        };
        let destination = versions.join(&digest);
        if !destination.exists() {
            let staging = tempfile::tempdir_in(&versions)?;
            fs::write(staging.path().join("plugin.wasm"), module)?;
            write_json(&staging.path().join("manifest.json"), &installed.manifest)?;
            fs::rename(staging.path(), &destination)?;
        }
        self.verify(&installed)?;
        write_json(&base.join("installed.json"), &installed)?;
        Ok(installed)
    }
    pub fn enable(&self, id: &str, enabled: bool) -> anyhow::Result<()> {
        let mut item = self.get(id)?;
        if enabled {
            self.verify(&item)?;
        }
        item.enabled = enabled;
        write_json(&self.root.join(id).join("installed.json"), &item)
    }
    pub fn uninstall(&self, id: &str) -> anyhow::Result<()> {
        self.get(id)?;
        // Shared documents and managed game folders remain owned by their users.
        fs::remove_file(self.root.join(id).join("installed.json"))?;
        Ok(())
    }
    pub fn instance_path(&self, id: &str, scope: &str) -> anyhow::Result<PathBuf> {
        identifier(id)?;
        Ok(self
            .root
            .join(id)
            .join("instances")
            .join(hash(scope.as_bytes()))
            .join("state.json"))
    }
    pub fn instance(&self, id: &str, scope: &str) -> anyhow::Result<Instance> {
        let path = self.instance_path(id, scope)?;
        if !path.exists() {
            return Ok(Instance::default());
        }
        Ok(serde_json::from_slice(&read_bounded(&path, MAX_JSON)?)?)
    }
    pub fn save(&self, id: &str, scope: &str, instance: &Instance) -> anyhow::Result<()> {
        ensure!(
            serde_json::to_vec(instance)?.len() <= MAX_JSON,
            "Plugin storage exceeds limit"
        );
        write_json(&self.instance_path(id, scope)?, instance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reviewed_digest_integrity_and_state_isolation() {
        let source = tempfile::tempdir().unwrap();
        let storage = tempfile::tempdir().unwrap();
        let packages = Packages {
            root: storage.path().into(),
        };
        let manifest = Manifest {
            api: 1,
            id: "test.plugin".into(),
            name: "Test".into(),
            version: "1.0.0".into(),
            description: "Test plugin".into(),
            permissions: vec![],
        };
        write_json(&source.path().join("plugin.json"), &manifest).unwrap();
        fs::write(source.path().join("plugin.wasm"), b"\0asm\x01\0\0\0").unwrap();
        let (_, _, digest) = Packages::inspect(source.path()).unwrap();
        assert!(packages.install(source.path(), "unreviewed").is_err());
        let item = packages.install(source.path(), &digest).unwrap();
        packages
            .save(
                "test.plugin",
                "server/account/chat-one",
                &Instance {
                    generation: 2,
                    state: serde_json::json!({"name":"pack"}),
                    folder: None,
                    archives: Vec::new(),
                },
            )
            .unwrap();
        assert_eq!(
            packages
                .instance("test.plugin", "server/account/chat-two")
                .unwrap()
                .generation,
            0
        );
        assert_eq!(
            packages
                .instance("test.plugin", "server/account/chat-one")
                .unwrap()
                .generation,
            2
        );
        packages.enable("test.plugin", false).unwrap();
        assert!(!packages.get("test.plugin").unwrap().enabled);
        fs::write(packages.module_path(&item).unwrap(), b"modified").unwrap();
        assert!(packages.enable("test.plugin", true).is_err());
        packages.uninstall("test.plugin").unwrap();
        assert!(packages.list().unwrap().is_empty());
        assert_eq!(
            packages
                .instance("test.plugin", "server/account/chat-one")
                .unwrap()
                .generation,
            2
        );
    }
}
