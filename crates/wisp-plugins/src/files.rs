//! Host-owned installation of verified Thunderstore archives into new profiles.
use crate::package::{hash, read_bounded, write_json};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

pub const MAX_ARCHIVE: usize = 256 * 1024 * 1024;
pub const MAX_TOTAL: u64 = 1024 * 1024 * 1024;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    #[default]
    Thunderstore,
    Local,
}
impl Source {
    #[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip predicate takes a reference.
    fn is_thunderstore(&self) -> bool {
        *self == Self::Thunderstore
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub package: String,
    pub sha256: String,
    pub size: u64,
    #[serde(default, skip_serializing_if = "Source::is_thunderstore")]
    pub source: Source,
}
/// Keep private bytes out of the public download cache, even when a guest
/// supplies the same hash with a forged Thunderstore source.
#[must_use]
pub fn cache_path(cache: &Path, artifact: &Artifact) -> PathBuf {
    if artifact.source == Source::Local {
        cache.join("private").join(&artifact.sha256)
    } else {
        cache.join(&artifact.sha256)
    }
}
pub fn package_parts(id: &str) -> anyhow::Result<(&str, &str, &str)> {
    let parts: Vec<_> = id.split('-').collect();
    ensure!(
        parts.len() == 3,
        "Use Team-Package-1.2.3 dependency strings"
    );
    for part in &parts[..2] {
        ensure!(
            !part.is_empty()
                && part.len() <= 128
                && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
            "Invalid Thunderstore package name"
        );
    }
    let version = semver::Version::parse(parts[2])?;
    ensure!(
        version.pre.is_empty() && version.build.is_empty(),
        "Thunderstore versions must be Major.Minor.Patch"
    );
    Ok((parts[0], parts[1], parts[2]))
}
pub fn validate_plan(files: &[Artifact]) -> anyhow::Result<()> {
    ensure!(files.len() <= 128, "At most 128 packages per profile");
    let mut names = BTreeSet::new();
    let mut total = 0u64;
    for file in files {
        let (owner, name, _) = package_parts(&file.package)?;
        ensure!(
            file.source != Source::Local || owner == "Local",
            "Invalid private package namespace"
        );
        ensure!(
            names.insert(format!("{owner}-{name}").to_lowercase()),
            "Conflicting package versions"
        );
        ensure!(
            file.sha256.len() == 64
                && file
                    .sha256
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
            "Invalid SHA-256"
        );
        ensure!(
            file.size > 0 && file.size <= MAX_ARCHIVE as u64,
            "Package exceeds download limit"
        );
        total = total
            .checked_add(file.size)
            .context("Download size overflow")?;
    }
    ensure!(total <= MAX_TOTAL, "Profile downloads exceed 1 GiB");
    Ok(())
}
pub fn plan_digest(files: &[Artifact]) -> anyhow::Result<String> {
    validate_plan(files)?;
    Ok(hash(&serde_json::to_vec(files)?))
}
fn safe_relative(value: &str) -> anyhow::Result<()> {
    ensure!(
        value.len() <= 240
            && !value.is_empty()
            && !value.starts_with('/')
            && !value.contains('\\')
            && !value.contains(':')
            && !value.chars().any(char::is_control)
            && value
                .split('/')
                .all(|s| !s.is_empty() && s != "." && s != ".." && !s.ends_with(['.', ' '])),
        "Unsafe archive path"
    );
    Ok(())
}
#[derive(Debug, Deserialize)]
pub struct ArchiveManifest {
    pub name: String,
    pub version_number: String,
    pub dependencies: Vec<String>,
}
pub fn archive_manifest(bytes: &[u8]) -> anyhow::Result<ArchiveManifest> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
    ensure!(zip.len() <= 10_000, "Too many archive entries");
    let mut file = zip.by_name("manifest.json")?;
    ensure!(file.size() <= 16 * 1024, "Oversized package manifest");
    let mut data = Vec::new();
    file.by_ref().take(16 * 1024 + 1).read_to_end(&mut data)?;
    ensure!(data.len() <= 16 * 1024, "Oversized package manifest");
    let manifest: ArchiveManifest =
        serde_json::from_slice(data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&data))
            .context("Invalid Thunderstore package manifest")?;
    ensure!(manifest.dependencies.len() <= 128, "Too many dependencies");
    for dependency in &manifest.dependencies {
        package_parts(dependency)?;
    }
    Ok(manifest)
}
fn destination(package: &str, path: &str) -> anyhow::Result<Option<String>> {
    let normalized = path.replace('\\', "/");
    let path = normalized.as_str();
    safe_relative(path)?;
    let (owner, name, _) = package_parts(package)?;
    if [
        "manifest.json",
        "readme.md",
        "changelog.md",
        "icon.png",
        "license",
        "license.txt",
    ]
    .contains(&path.to_lowercase().as_str())
    {
        return Ok(None);
    }
    if owner == "denikson" && name == "BepInExPack_Valheim" {
        return path
            .strip_prefix("BepInExPack_Valheim/")
            .map_or(Ok(None), |p| {
                safe_relative(p)?;
                Ok(Some(p.to_owned()))
            });
    }
    let path = path.strip_prefix("BepInEx/").unwrap_or(path);
    for directory in ["plugins/", "patchers/"] {
        if let Some(relative) = path.strip_prefix(directory) {
            return Ok(Some(format!(
                "BepInEx/{directory}{owner}-{name}/{relative}"
            )));
        }
    }
    if path.starts_with("config/") {
        return Ok(Some(format!("BepInEx/{path}")));
    }
    ensure!(
        !path.starts_with("core/") && !path.starts_with("doorstop_libs/"),
        "Only the Valheim BepInEx package may supply loader files"
    );
    Ok(Some(format!("BepInEx/plugins/{owner}-{name}/{path}")))
}

/// Inspect a private mod ZIP without extracting or executing its contents.
pub fn local_archive(bytes: &[u8]) -> anyhow::Result<(Artifact, Vec<String>)> {
    ensure!(
        bytes.len() <= crate::bundles::MAX_PRIVATE_BYTES,
        "Private ZIP exceeds 128 MiB"
    );
    let manifest = archive_manifest(bytes).context(
        "Choose a mod ZIP containing manifest.json with name, version_number, and dependencies",
    )?;
    let artifact = Artifact {
        package: format!("Local-{}-{}", manifest.name, manifest.version_number),
        sha256: hash(bytes),
        size: bytes.len() as u64,
        source: Source::Local,
    };
    validate_plan(std::slice::from_ref(&artifact))?;
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
    let mut names = BTreeSet::new();
    let mut total = 0_u64;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        ensure!(
            !file.is_symlink()
                && file
                    .unix_mode()
                    .is_none_or(|m| matches!(m & 0o170_000, 0 | 0o100_000 | 0o040_000)),
            "Archive links and special files are not supported"
        );
        if file.is_dir() || file.name().ends_with('\\') {
            safe_relative(file.name().replace('\\', "/").trim_end_matches('/'))?;
            continue;
        }
        if let Some(path) = destination(&artifact.package, file.name())? {
            ensure!(
                names.insert(path.to_lowercase()),
                "Conflicting archive paths"
            );
        }
        total = total
            .checked_add(file.size())
            .context("Archive size overflow")?;
        ensure!(
            file.size() <= MAX_ARCHIVE as u64 && total <= MAX_TOTAL,
            "Extracted archive exceeds limits"
        );
        let size = file.size();
        ensure!(
            std::io::copy(&mut file.by_ref().take(size + 1), &mut std::io::sink())? == size,
            "Archive size mismatch"
        );
    }
    ensure!(!names.is_empty(), "ZIP contains no installable mod files");
    Ok((artifact, manifest.dependencies))
}

pub fn unpacked_size(bytes: &[u8]) -> anyhow::Result<u64> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
    ensure!(zip.len() <= 10_000, "Too many archive entries");
    let mut total = 0_u64;
    for i in 0..zip.len() {
        total = total
            .checked_add(zip.by_index(i)?.size())
            .context("Archive size overflow")?;
        ensure!(total <= MAX_TOTAL, "Extracted archive exceeds 1 GiB");
    }
    Ok(total)
}

/// Guests may reference private bytes only after a native user action admitted
/// those exact bytes into this account/conversation's host-owned registry.
pub fn authorize_private(files: &[Artifact], admitted: &[Artifact]) -> anyhow::Result<()> {
    validate_plan(files)?;
    for file in files.iter().filter(|f| f.source == Source::Local) {
        ensure!(
            admitted.contains(file),
            "Private archive is not available in this draft; add its ZIP or import the complete shared pack"
        );
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
struct Receipt {
    digest: String,
    files: BTreeMap<String, String>,
}
pub fn verify_profile(directory: &Path, digest: &str) -> anyhow::Result<()> {
    ensure!(
        fs::symlink_metadata(directory)?.is_dir(),
        "Profile is not a regular directory"
    );
    let receipt: Receipt = serde_json::from_slice(&read_bounded(
        &directory.join(".wisp-profile.json"),
        4 * 1024 * 1024,
    )?)?;
    ensure!(
        receipt.digest == digest,
        "Profile belongs to another pack version"
    );
    let mut pending = vec![directory.to_path_buf()];
    let mut count = 0;
    while let Some(parent) = pending.pop() {
        for entry in fs::read_dir(parent)? {
            let entry = entry?;
            count += 1;
            ensure!(count <= 20_000, "Profile contains too many entries");
            let kind = entry.file_type()?;
            ensure!(!kind.is_symlink(), "Profile contains a symbolic link");
            if kind.is_dir() {
                pending.push(entry.path());
                continue;
            }
            let path = entry.path();
            let relative = path
                .strip_prefix(directory)?
                .components()
                .map(|part| part.as_os_str().to_str().context("Invalid profile path"))
                .collect::<anyhow::Result<Vec<_>>>()?
                .join("/");
            ensure!(
                kind.is_file()
                    && (relative == ".wisp-profile.json" || receipt.files.contains_key(&relative)),
                "Profile contains added files; choose a new folder to preserve local changes"
            );
        }
    }
    for (path, expected) in receipt.files {
        safe_relative(&path)?;
        ensure!(
            hash(&read_bounded(&directory.join(path), MAX_ARCHIVE)?) == expected,
            "Installed profile has local modifications; select a new folder to preserve them"
        );
    }
    Ok(())
}
/// Compare a managed installation with the published plan. A different receipt
/// means an update is available; matching receipts still require file hashing.
#[must_use]
pub fn profile_state(directory: &Path, digest: &str) -> &'static str {
    match fs::symlink_metadata(directory) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return "missing",
        Err(_) => return "unknown",
        Ok(meta) if !meta.is_dir() => return "modified",
        Ok(_) => {}
    }
    let receipt = read_bounded(&directory.join(".wisp-profile.json"), 4 * 1024 * 1024)
        .and_then(|bytes| Ok(serde_json::from_slice::<Receipt>(&bytes)?));
    match receipt {
        Ok(receipt) if receipt.digest != digest => "update_available",
        Ok(_) if verify_profile(directory, digest).is_ok() => "up_to_date",
        _ => "modified",
    }
}
/// Sync never overwrites a locally modified profile. Keep it for rollback and
/// create/reuse a verified sibling if the canonical destination was changed.
pub fn sync_profile(parent: &Path, files: &[Artifact], cache: &Path) -> anyhow::Result<PathBuf> {
    install(parent, files, cache, true)
}
/// All archives must already be downloaded into the content-addressed cache.
/// Existing game directories are never modified, and no downloaded code runs.
pub fn install_profile(parent: &Path, files: &[Artifact], cache: &Path) -> anyhow::Result<PathBuf> {
    install(parent, files, cache, false)
}
fn install(
    parent: &Path,
    files: &[Artifact],
    cache: &Path,
    repair: bool,
) -> anyhow::Result<PathBuf> {
    ensure!(!files.is_empty(), "Resolve a modpack first");
    let digest = plan_digest(files)?;
    ensure!(
        parent.is_absolute() && fs::symlink_metadata(parent)?.is_dir(),
        "Choose an existing profile parent folder"
    );
    let parent = fs::canonicalize(parent)?;
    let base = format!("wisp-profile-{}", &digest[..16]);
    let mut destination_dir = parent.join(&base);
    let mut suffix = 0;
    while fs::symlink_metadata(&destination_dir).is_ok() {
        if verify_profile(&destination_dir, &digest).is_ok() {
            return Ok(destination_dir);
        }
        ensure!(
            repair,
            "Installed profile has local changes; choose Sync to preserve them and create a fresh profile"
        );
        suffix += 1;
        ensure!(
            suffix <= 10_000,
            "Too many saved profiles; choose another folder"
        );
        destination_dir = parent.join(format!("{base}-sync-{suffix}"));
    }
    let staging = tempfile::tempdir_in(&parent)?;
    let mut receipt = Receipt {
        digest,
        files: BTreeMap::new(),
    };
    let mut names = BTreeSet::new();
    let mut total = 0u64;
    let mut entries = 0;
    for artifact in files {
        let bytes = read_bounded(&cache_path(cache, artifact), MAX_ARCHIVE)?;
        ensure!(
            bytes.len() as u64 == artifact.size && hash(&bytes) == artifact.sha256,
            "Package checksum mismatch"
        );
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
        entries += zip.len();
        ensure!(entries <= 10_000, "Too many files in profile");
        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            ensure!(
                !file.is_symlink()
                    && file.unix_mode().is_none_or(|m| m & 0o170_000 == 0
                        || m & 0o170_000 == 0o100_000
                        || m & 0o170_000 == 0o040_000),
                "Archive links and special files are not supported"
            );
            if file.is_dir() || file.name().ends_with('\\') {
                safe_relative(file.name().replace('\\', "/").trim_end_matches('/'))?;
                continue;
            }
            let Some(relative) = destination(&artifact.package, file.name())? else {
                continue;
            };
            ensure!(
                relative != ".wisp-profile.json",
                "Archive uses a reserved profile path"
            );
            ensure!(
                names.insert(relative.to_lowercase()),
                "Two packages write the same profile file"
            );
            ensure!(
                file.size() <= MAX_ARCHIVE as u64,
                "Extracted file exceeds limit"
            );
            total = total
                .checked_add(file.size())
                .context("Archive size overflow")?;
            ensure!(total <= MAX_TOTAL, "Extracted profile exceeds 1 GiB");
            let path = staging.path().join(&relative);
            fs::create_dir_all(path.parent().context("Missing parent")?)?;
            let mut data = Vec::new();
            file.by_ref()
                .take(MAX_ARCHIVE as u64 + 1)
                .read_to_end(&mut data)?;
            ensure!(data.len() as u64 == file.size(), "Archive size mismatch");
            fs::write(path, &data)?;
            receipt.files.insert(relative, hash(&data));
        }
    }
    write_json(&staging.path().join(".wisp-profile.json"), &receipt)?;
    fs::rename(staging.path(), &destination_dir)?;
    Ok(destination_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn existing_public_locks_keep_their_serialized_bytes_and_digest() {
        let legacy = format!(
            r#"[{{"package":"Team-Mod-1.0.0","sha256":"{}","size":1}}]"#,
            "a".repeat(64)
        );
        let files: Vec<Artifact> = serde_json::from_str(&legacy).unwrap();
        assert_eq!(files[0].source, Source::Thunderstore);
        assert_eq!(serde_json::to_string(&files).unwrap(), legacy);
        assert_eq!(plan_digest(&files).unwrap(), hash(legacy.as_bytes()));
    }
    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (path, data) in entries {
            zip.start_file(*path, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }
    #[test]
    fn rejects_paths_and_conflicting_versions() {
        for path in ["../escape", "/absolute", "a\\..\\b", "a/../b", "a:b"] {
            assert!(destination("Team-Mod-1.0.0", path).is_err());
        }
        let a = Artifact {
            package: "Team-Mod-1.0.0".into(),
            sha256: "a".repeat(64),
            size: 1,
            source: Source::Thunderstore,
        };
        let mut b = a.clone();
        b.package = "Team-Mod-2.0.0".into();
        assert!(validate_plan(&[a, b]).is_err());
    }
    #[test]
    fn supports_windows_archives_and_utf8_bom() {
        let bytes = archive(&[(
            "manifest.json",
            b"\xef\xbb\xbf{\"name\":\"Test\",\"version_number\":\"1.0.0\",\"dependencies\":[]}",
        )]);
        assert_eq!(archive_manifest(&bytes).unwrap().name, "Test");
        assert_eq!(
            destination("Team-Mod-1.0.0", "plugins\\Test.dll")
                .unwrap()
                .unwrap(),
            "BepInEx/plugins/Team-Mod/Test.dll"
        );
        assert!(destination("Team-Mod-1.0.0", "plugins\\..\\escape.dll").is_err());
    }
    #[test]
    fn installs_atomically_preserves_old_and_detects_changes() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let bytes = archive(&[("mod.dll", b"test")]);
        let a = Artifact {
            package: "Team-Mod-1.0.0".into(),
            sha256: hash(&bytes),
            size: bytes.len() as u64,
            source: Source::Thunderstore,
        };
        fs::write(cache.path().join(&a.sha256), bytes).unwrap();
        let path = install_profile(root.path(), std::slice::from_ref(&a), cache.path()).unwrap();
        assert_eq!(
            read_bounded(&path.join("BepInEx/plugins/Team-Mod/mod.dll"), 20).unwrap(),
            b"test"
        );
        assert_eq!(
            path,
            install_profile(root.path(), std::slice::from_ref(&a), cache.path()).unwrap()
        );
        fs::write(path.join("BepInEx/plugins/Team-Mod/mod.dll"), "changed").unwrap();
        assert!(install_profile(root.path(), &[a], cache.path()).is_err());
        assert_eq!(
            fs::read_to_string(path.join("BepInEx/plugins/Team-Mod/mod.dll")).unwrap(),
            "changed"
        );
    }
    #[test]
    fn traversal_leaves_no_profile() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let bytes = archive(&[("../escape", b"test")]);
        let a = Artifact {
            package: "Team-Mod-1.0.0".into(),
            sha256: hash(&bytes),
            size: bytes.len() as u64,
            source: Source::Thunderstore,
        };
        fs::write(cache.path().join(&a.sha256), bytes).unwrap();
        assert!(install_profile(root.path(), &[a], cache.path()).is_err());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
    #[test]
    fn sync_checks_bytes_and_preserves_modified_and_previous_profiles() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let bytes = archive(&[("mod.dll", b"version one")]);
        let mut artifact = Artifact {
            package: "Team-Mod-1.0.0".into(),
            sha256: hash(&bytes),
            size: bytes.len() as u64,
            source: Source::Thunderstore,
        };
        fs::write(cache.path().join(&artifact.sha256), bytes).unwrap();
        let plan = [artifact.clone()];
        let digest = plan_digest(&plan).unwrap();
        let old = sync_profile(root.path(), &plan, cache.path()).unwrap();
        assert_eq!(profile_state(&old, &digest), "up_to_date");
        let mod_file = "BepInEx/plugins/Team-Mod/mod.dll";
        fs::write(old.join(mod_file), b"user edit").unwrap();
        assert_eq!(profile_state(&old, &digest), "modified");
        let fresh = sync_profile(root.path(), &plan, cache.path()).unwrap();
        assert_ne!(old, fresh);
        assert_eq!(profile_state(&fresh, &digest), "up_to_date");
        assert_eq!(fs::read(old.join(mod_file)).unwrap(), b"user edit");
        assert_eq!(
            sync_profile(root.path(), &plan, cache.path()).unwrap(),
            fresh
        );
        fs::write(fresh.join("extra.dll"), b"extra").unwrap();
        assert_eq!(profile_state(&fresh, &digest), "modified");
        fs::remove_file(fresh.join("extra.dll")).unwrap();
        fs::remove_file(fresh.join(mod_file)).unwrap();
        assert_eq!(profile_state(&fresh, &digest), "modified");
        let bytes = archive(&[("mod.dll", b"version two")]);
        artifact.package = "Team-Mod-2.0.0".into();
        artifact.sha256 = hash(&bytes);
        artifact.size = bytes.len() as u64;
        fs::write(cache.path().join(&artifact.sha256), bytes).unwrap();
        let digest = plan_digest(std::slice::from_ref(&artifact)).unwrap();
        assert_eq!(profile_state(&old, &digest), "update_available");
        let new = sync_profile(root.path(), &[artifact], cache.path()).unwrap();
        assert_eq!(profile_state(&new, &digest), "up_to_date");
        fs::remove_dir_all(&new).unwrap();
        assert_eq!(profile_state(&new, &digest), "missing");
        assert_eq!(fs::read(old.join(mod_file)).unwrap(), b"user edit");
    }
}

/// Thunderstore declarations are minimum requirements; the roots selected by
/// the user are exact pins. The resulting shared plan always pins exact bytes.
pub fn package_key(package: &str) -> anyhow::Result<String> {
    let (owner, name, _) = package_parts(package)?;
    Ok(format!("{owner}-{name}").to_lowercase())
}
pub fn satisfies(selected: &str, requirement: &str) -> anyhow::Result<bool> {
    let (_, _, selected_version) = package_parts(selected)?;
    let (_, _, required_version) = package_parts(requirement)?;
    Ok(package_key(selected)? == package_key(requirement)?
        && semver::Version::parse(selected_version)? >= semver::Version::parse(required_version)?)
}
pub struct Selection {
    pins: BTreeMap<String, String>,
    pub chosen: BTreeMap<String, String>,
}
impl Selection {
    pub fn new(roots: &[String]) -> anyhow::Result<Self> {
        let mut pins = BTreeMap::new();
        for root in roots {
            if let Some(previous) = pins.insert(package_key(root)?, root.clone()) {
                ensure!(previous == *root, "Conflicting explicit root pins");
            }
        }
        Ok(Self {
            pins,
            chosen: BTreeMap::new(),
        })
    }
    /// Returns a concrete package to download only when the selection changes.
    pub fn require(&mut self, requirement: &str) -> anyhow::Result<Option<String>> {
        let key = package_key(requirement)?;
        let candidate = if let Some(pin) = self.pins.get(&key) {
            ensure!(
                satisfies(pin, requirement)?,
                "Pinned {pin} is older than dependency requirement {requirement}"
            );
            pin.clone()
        } else {
            requirement.to_owned()
        };
        if let Some(previous) = self.chosen.get(&key)
            && satisfies(previous, &candidate)?
        {
            return Ok(None);
        }
        self.chosen.insert(key, candidate.clone());
        ensure!(
            self.chosen.len() <= 128,
            "Dependency graph exceeds 128 packages"
        );
        Ok(Some(candidate))
    }
}
#[cfg(test)]
mod dependency_tests {
    use super::*;
    #[test]
    fn root_pins_satisfy_older_minima_and_reject_newer() {
        let mut s = Selection::new(&["denikson-BepInExPack_Valheim-5.4.2351".into()]).unwrap();
        assert_eq!(
            s.require("denikson-BepInExPack_Valheim-5.4.2202")
                .unwrap()
                .unwrap(),
            "denikson-BepInExPack_Valheim-5.4.2351"
        );
        assert!(s.require("denikson-BepInExPack_Valheim-5.4.2352").is_err());
    }
    #[test]
    fn shared_dependencies_upgrade_once_to_the_highest_requirement() {
        let mut s = Selection::new(&[]).unwrap();
        assert!(s.require("Team-Library-1.0.0").unwrap().is_some());
        assert!(s.require("Team-Library-1.2.0").unwrap().is_some());
        assert!(s.require("Team-Library-1.1.0").unwrap().is_none());
        assert_eq!(s.chosen.len(), 1);
    }
}
