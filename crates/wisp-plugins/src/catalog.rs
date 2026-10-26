//! Reviewed repository catalog, shipped with each coordinated client/server release.
//! A server cannot introduce arbitrary modules or permissions through this catalog.
use crate::{
    Manifest,
    package::{Installed, Packages, hash},
};
use anyhow::{Context, ensure};
use serde::Serialize;

const MANIFEST: &[u8] = include_bytes!("../../../plugins/catalog/games.valheim/plugin.json");
const MODULE: &[u8] = include_bytes!("../../../plugins/catalog/games.valheim/plugin.wasm");
#[derive(Serialize)]
pub struct Entry {
    pub manifest: Manifest,
    pub digest: String,
}
pub fn entries() -> anyhow::Result<Vec<Entry>> {
    Ok(vec![entry("games.valheim")?])
}
pub fn entry(id: &str) -> anyhow::Result<Entry> {
    ensure!(
        id == "games.valheim",
        "Plugin is not in this Wisp release's catalog"
    );
    let manifest: Manifest = serde_json::from_slice(MANIFEST)?;
    manifest.validate()?;
    ensure!(
        manifest.id == id && MODULE.starts_with(b"\0asm"),
        "Invalid catalog package"
    );
    let digest = hash(&[serde_json::to_vec(&manifest)?, MODULE.to_vec()].concat());
    Ok(Entry { manifest, digest })
}
pub fn module(id: &str, digest: &str) -> anyhow::Result<&'static [u8]> {
    ensure!(
        entry(id)?.digest == digest,
        "Update Wisp to use this server's plugin version"
    );
    Ok(MODULE)
}
/// Cache a known catalog package only after an explicit Open action. No game files are installed.
pub fn prepare(
    packages: &Packages,
    id: &str,
    digest: &str,
    downloaded: &[u8],
) -> anyhow::Result<Installed> {
    let reviewed = entry(id)?;
    ensure!(
        reviewed.digest == digest,
        "Update Wisp to use this server's plugin version"
    );
    ensure!(
        downloaded == module(id, digest)?,
        "Server plugin package failed verification"
    );
    if let Ok(item) = packages.get(id)
        && item.digest == digest
        && packages.verify(&item).is_ok()
    {
        if !item.enabled {
            packages.enable(id, true)?;
        }
        return packages.get(id);
    }
    let stage = tempfile::tempdir().context("Prepare plugin cache")?;
    std::fs::write(
        stage.path().join("plugin.json"),
        serde_json::to_vec(&reviewed.manifest)?,
    )?;
    std::fs::write(stage.path().join("plugin.wasm"), downloaded)?;
    packages.install(stage.path(), digest)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_preparation_rejects_unreviewed_code_and_preserves_drafts() {
        let temp = tempfile::tempdir().unwrap();
        let packages = Packages {
            root: temp.path().join("plugins"),
        };
        let entry = entry("games.valheim").unwrap();
        assert!(module("unknown", &entry.digest).is_err());
        assert!(prepare(&packages, &entry.manifest.id, &"a".repeat(64), MODULE).is_err());
        assert!(prepare(&packages, &entry.manifest.id, &entry.digest, b"bad").is_err());
        let installed = prepare(&packages, &entry.manifest.id, &entry.digest, MODULE).unwrap();
        assert!(installed.enabled);
        let path = packages.instance_path(&entry.manifest.id, "test").unwrap();
        crate::package::write_json(&path, &serde_json::json!({"draft":"kept"})).unwrap();
        packages.enable(&entry.manifest.id, false).unwrap();
        assert!(
            prepare(&packages, &entry.manifest.id, &entry.digest, MODULE)
                .unwrap()
                .enabled
        );
        assert!(std::fs::read_to_string(path).unwrap().contains("kept"));
        let response = crate::runtime::invoke(
            MODULE,
            &entry.manifest,
            &crate::Request {
                api: 1,
                event: "open".into(),
                action: String::new(),
                fields: serde_json::Value::Null,
                state: serde_json::Value::Null,
                document: None,
            },
        )
        .unwrap();
        assert_eq!(response.view.title, "Valheim Modpacks");
    }
}
