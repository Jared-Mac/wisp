//! Thunderstore adapter. Only exact public CDN package URLs can be requested;
//! no Wisp credentials, arbitrary guest URLs, or native package code are used.
use anyhow::{Context, ensure};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::Path,
    time::Duration,
};
use wisp_plugins::{
    files::{Artifact, MAX_ARCHIVE, MAX_TOTAL, archive_manifest, package_parts},
    package::{hash, read_bounded},
};

async fn download(id: &str) -> anyhow::Result<Vec<u8>> {
    package_parts(id)?;
    let client = reqwest::Client::builder()
        .user_agent("Wisp-Modpacks/0.1")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(90))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() < 3
                && attempt.url().scheme() == "https"
                && matches!(
                    attempt.url().host_str(),
                    Some("cdn.thunderstore.io" | "gcdn.thunderstore.io")
                )
                && attempt.url().port().is_none()
            {
                attempt.follow()
            } else {
                attempt.error("Unexpected package download redirect")
            }
        }))
        .build()?;
    let mut response = client
        .get(format!(
            "https://cdn.thunderstore.io/live/repository/packages/{id}.zip"
        ))
        .send()
        .await?
        .error_for_status()?;
    ensure!(
        response
            .content_length()
            .is_none_or(|n| n <= MAX_ARCHIVE as u64),
        "Package exceeds 256 MiB"
    );
    let mut data = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            data.len() + chunk.len() <= MAX_ARCHIVE,
            "Package exceeds 256 MiB"
        );
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}
fn cache_bytes(cache: &Path, bytes: &[u8]) -> anyhow::Result<String> {
    use std::io::Write;
    std::fs::create_dir_all(cache)?;
    let digest = hash(bytes);
    let path = cache.join(&digest);
    if !path.exists() {
        let mut total = bytes.len() as u64;
        for entry in std::fs::read_dir(cache)? {
            total = total
                .checked_add(entry?.metadata()?.len())
                .context("Cache size overflow")?;
        }
        ensure!(
            total <= 4 * MAX_TOTAL,
            "Plugin download cache is full (4 GiB). Remove unused cached archives before retrying"
        );
        let mut file = tempfile::NamedTempFile::new_in(cache)?;
        file.write_all(bytes)?;
        file.as_file().sync_all()?;
        file.persist(path)?;
    }
    Ok(digest)
}
pub(super) async fn resolve(roots: &[String], cache: &Path) -> anyhow::Result<Vec<Artifact>> {
    ensure!(
        !roots.is_empty() && roots.len() <= 128,
        "Choose between 1 and 128 root packages"
    );
    let mut queue: VecDeque<_> = roots.iter().cloned().collect();
    let mut selection = wisp_plugins::files::Selection::new(roots)?;
    let mut archives = BTreeMap::new();
    let mut total = 0_u64;
    let mut downloads = 0;
    while let Some(requirement) = queue.pop_front() {
        let Some(id) = selection.require(&requirement)? else {
            continue;
        };
        downloads += 1;
        ensure!(downloads <= 256, "Too many dependency version changes");
        let (_, name, version) = package_parts(&id)?;
        let bytes = download(&id).await?;
        total += bytes.len() as u64;
        ensure!(total <= MAX_TOTAL, "Downloads exceed 1 GiB");
        let manifest = archive_manifest(&bytes)?;
        ensure!(
            manifest.name == name && manifest.version_number == version,
            "Archive manifest does not match requested version"
        );
        queue.extend(manifest.dependencies.clone());
        let artifact = Artifact {
            package: id.clone(),
            sha256: cache_bytes(cache, &bytes)?,
            size: bytes.len() as u64,
        };
        archives.insert(id, (artifact, manifest.dependencies));
    }
    // Superseded versions can have different dependencies. Keep only the
    // closure reachable from the final root selections.
    let mut reachable = BTreeSet::new();
    let mut queue: VecDeque<_> = roots.iter().cloned().collect();
    while let Some(requirement) = queue.pop_front() {
        let key = wisp_plugins::files::package_key(&requirement)?;
        if !reachable.insert(key.clone()) {
            continue;
        }
        let id = selection
            .chosen
            .get(&key)
            .context("Missing selected dependency")?;
        let (_, dependencies) = archives.get(id).context("Missing downloaded dependency")?;
        queue.extend(dependencies.iter().cloned());
    }
    let mut output: Vec<_> = selection
        .chosen
        .into_iter()
        .filter(|(key, _)| reachable.contains(key))
        .map(|(_, id)| {
            archives
                .remove(&id)
                .map(|(artifact, _)| artifact)
                .context("Missing archive")
        })
        .collect::<anyhow::Result<_>>()?;
    output.sort_by(|a, b| a.package.cmp(&b.package));
    Ok(output)
}
pub(super) async fn obtain(files: &[Artifact], cache: &Path) -> anyhow::Result<()> {
    wisp_plugins::files::validate_plan(files)?;
    for file in files {
        let path = cache.join(&file.sha256);
        if let Ok(bytes) = read_bounded(&path, MAX_ARCHIVE)
            && bytes.len() as u64 == file.size
            && hash(&bytes) == file.sha256
        {
            continue;
        }
        let bytes = download(&file.package).await?;
        ensure!(
            bytes.len() as u64 == file.size && hash(&bytes) == file.sha256,
            "Shared package checksum mismatch: {}",
            file.package
        );
        // Validate the package's own manifest, even when a peer supplied its hash.
        let (_, name, version) = package_parts(&file.package)?;
        let manifest = archive_manifest(&bytes)?;
        ensure!(
            manifest.name == name && manifest.version_number == version,
            "Package identity mismatch"
        );
        cache_bytes(cache, &bytes).context("Could not cache package")?;
    }
    // All transitive dependencies must be explicitly pinned in a shared lock.
    for file in files {
        let bytes = read_bounded(&cache.join(&file.sha256), MAX_ARCHIVE)?;
        let manifest = archive_manifest(&bytes)?;
        let (_, name, version) = package_parts(&file.package)?;
        ensure!(
            manifest.name == name && manifest.version_number == version,
            "Cached package identity mismatch"
        );
        for dependency in manifest.dependencies {
            ensure!(
                files
                    .iter()
                    .any(|f| wisp_plugins::files::satisfies(&f.package, &dependency)
                        .unwrap_or(false)),
                "Shared pack is missing dependency {dependency}"
            );
        }
    }
    Ok(())
}
