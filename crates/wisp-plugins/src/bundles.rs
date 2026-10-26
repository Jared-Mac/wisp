//! Private mod bytes travel in the same authenticated, encrypted attachment as
//! their document. A guest receives metadata only, never ZIP bytes or paths.
use crate::{
    Document, MAX_JSON,
    files::{self, Artifact, Source},
    package::read_bounded,
};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    io::{Cursor, Read, Write},
    path::Path,
};

pub const MAX_PRIVATE_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_BUNDLE: usize = MAX_PRIVATE_BYTES + 2 * MAX_JSON + 64 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    format: u32,
    document: Document,
    archives: Vec<Artifact>,
}

fn validate(archives: &[Artifact]) -> anyhow::Result<()> {
    files::validate_plan(archives)?;
    ensure!(
        !archives.is_empty() && archives.iter().all(|f| f.source == Source::Local),
        "Bundle must contain private archives"
    );
    ensure!(
        archives.iter().map(|f| f.size).sum::<u64>() <= MAX_PRIVATE_BYTES as u64,
        "Private archives exceed 128 MiB per shared pack"
    );
    Ok(())
}

pub fn encode(
    document: Document,
    archives: Vec<Artifact>,
    cache: &Path,
) -> anyhow::Result<Vec<u8>> {
    validate(&archives)?;
    Document::parse(&serde_json::to_vec(&document)?)?;
    let header = serde_json::to_vec(&Header {
        format: 1,
        document,
        archives: archives.clone(),
    })?;
    ensure!(header.len() <= 2 * MAX_JSON, "Bundle header exceeds limit");
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("bundle.json", options)?;
    zip.write_all(&header)?;
    let mut unpacked = 0;
    for artifact in archives {
        let bytes = read_bounded(&files::cache_path(cache, &artifact), MAX_PRIVATE_BYTES)?;
        unpacked += files::unpacked_size(&bytes)?;
        ensure!(
            unpacked <= files::MAX_TOTAL,
            "Private pack exceeds 1 GiB extracted"
        );
        ensure!(
            files::local_archive(&bytes)?.0 == artifact,
            "Private archive changed before sharing"
        );
        zip.start_file(format!("archives/{}.zip", artifact.sha256), options)?;
        zip.write_all(&bytes)?;
    }
    let bytes = zip.finish()?.into_inner();
    ensure!(bytes.len() <= MAX_BUNDLE, "Shared bundle exceeds limit");
    Ok(bytes)
}

pub type PrivateBytes = Vec<(Artifact, Vec<u8>)>;
pub fn decode(bytes: &[u8]) -> anyhow::Result<(Document, PrivateBytes)> {
    ensure!(bytes.len() <= MAX_BUNDLE, "Shared bundle exceeds limit");
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
    ensure!(zip.len() <= 129, "Too many bundle entries");
    let mut names = BTreeSet::new();
    for i in 0..zip.len() {
        let file = zip.by_index(i)?;
        ensure!(
            !file.is_symlink()
                && !file.is_dir()
                && file
                    .unix_mode()
                    .is_none_or(|m| matches!(m & 0o170_000, 0 | 0o100_000)),
            "Bundle must contain regular files"
        );
        ensure!(
            names.insert(file.name().to_owned()),
            "Duplicate bundle entry"
        );
    }
    let header = read_entry(&mut zip, "bundle.json", 2 * MAX_JSON)?;
    let header: Header = serde_json::from_slice(&header).context("Invalid private pack bundle")?;
    ensure!(header.format == 1, "Unsupported private pack bundle format");
    Document::parse(&serde_json::to_vec(&header.document)?)?;
    validate(&header.archives)?;
    ensure!(
        names.len() == header.archives.len() + 1,
        "Unexpected or missing bundle files"
    );
    let mut output = Vec::new();
    let mut unpacked = 0;
    for artifact in header.archives {
        let name = format!("archives/{}.zip", artifact.sha256);
        ensure!(names.remove(&name), "Missing private archive");
        let bytes = read_entry(&mut zip, &name, usize::try_from(artifact.size)?)?;
        unpacked += files::unpacked_size(&bytes)?;
        ensure!(
            unpacked <= files::MAX_TOTAL,
            "Private pack exceeds 1 GiB extracted"
        );
        ensure!(
            files::local_archive(&bytes)?.0 == artifact,
            "Private archive checksum or identity mismatch"
        );
        output.push((artifact, bytes));
    }
    ensure!(
        names == BTreeSet::from(["bundle.json".to_string()]),
        "Unexpected bundle entry"
    );
    Ok((header.document, output))
}

fn read_entry(
    zip: &mut zip::ZipArchive<Cursor<&[u8]>>,
    name: &str,
    limit: usize,
) -> anyhow::Result<Vec<u8>> {
    let mut file = zip.by_name(name)?;
    ensure!(file.size() <= limit as u64, "Bundle entry exceeds limit");
    let mut bytes = Vec::new();
    file.by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= limit && bytes.len() as u64 == file.size(),
        "Invalid bundle entry size"
    );
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn archive(payload_path: &str) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in [
            (
                "manifest.json",
                br#"{"name":"Foundation","version_number":"0.1.0","dependencies":[]}"#.as_slice(),
            ),
            (payload_path, b"private-mod-fixture"),
        ] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }
    #[test]
    fn private_bundle_preserves_exact_bytes_and_requires_admission() {
        let bytes = archive("plugins/Foundation.dll");
        let artifact = files::local_archive(&bytes).unwrap().0;
        let cache = tempfile::tempdir().unwrap();
        std::fs::create_dir(cache.path().join("private")).unwrap();
        std::fs::write(files::cache_path(cache.path(), &artifact), &bytes).unwrap();
        let document = Document {
            format: 1,
            plugin: "games.valheim".into(),
            plugin_version: "0.2.0".into(),
            title: "Friends".into(),
            data: serde_json::json!({"private":true}),
        };
        let encoded = encode(document, vec![artifact.clone()], cache.path()).unwrap();
        let (document, decoded) = decode(&encoded).unwrap();
        assert_eq!(document.title, "Friends");
        assert_eq!(decoded, vec![(artifact.clone(), bytes)]);
        assert!(files::authorize_private(std::slice::from_ref(&artifact), &[]).is_err());
        files::authorize_private(
            std::slice::from_ref(&artifact),
            std::slice::from_ref(&artifact),
        )
        .unwrap();
        let mut forged = artifact.clone();
        forged.sha256 = "0".repeat(64);
        assert!(files::authorize_private(&[forged], &[artifact]).is_err());
    }
    #[test]
    fn public_source_cannot_install_private_cached_bytes() {
        let bytes = archive("Foundation.dll");
        let artifact = files::local_archive(&bytes).unwrap().0;
        let cache = tempfile::tempdir().unwrap();
        std::fs::create_dir(cache.path().join("private")).unwrap();
        std::fs::write(files::cache_path(cache.path(), &artifact), bytes).unwrap();
        let profiles = tempfile::tempdir().unwrap();
        files::install_profile(
            profiles.path(),
            std::slice::from_ref(&artifact),
            cache.path(),
        )
        .unwrap();
        let mut forged = artifact.clone();
        forged.source = Source::Thunderstore;
        assert!(files::install_profile(profiles.path(), &[forged], cache.path()).is_err());
        let mut forged = artifact;
        forged.package = "denikson-BepInExPack_Valheim-5.4.2351".into();
        assert!(files::validate_plan(&[forged]).is_err());
    }
    #[test]
    fn private_zip_rejects_traversal_and_bad_payloads() {
        assert!(files::local_archive(&archive("../escape.dll")).is_err());
        assert!(files::local_archive(&archive("BepInEx/core/loader.dll")).is_err());
        assert!(decode(b"not a ZIP").is_err());
        let bytes = archive("Foundation.dll");
        let artifact = files::local_archive(&bytes).unwrap().0;
        let document = Document {
            format: 1,
            plugin: "games.valheim".into(),
            plugin_version: "0.2.0".into(),
            title: "Friends".into(),
            data: serde_json::Value::Null,
        };
        let header = serde_json::to_vec(&Header {
            format: 1,
            document,
            archives: vec![artifact.clone()],
        })
        .unwrap();
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("bundle.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&header).unwrap();
        zip.start_file(
            format!("archives/{}.zip", artifact.sha256),
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(&archive("different.dll")).unwrap();
        assert!(decode(&zip.finish().unwrap().into_inner()).is_err());
    }
}
