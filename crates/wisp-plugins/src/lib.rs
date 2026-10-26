//! Portable plugin contracts. Guest code has no WASI or host imports. The
//! desktop owns every privileged operation and presents its own action UI.
pub mod bundles;
pub mod files;
pub mod package;
pub mod runtime;

use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub const API_VERSION: u32 = 1;
pub const MAX_JSON: usize = 256 * 1024;
pub const MAX_MODULE: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Documents,
    ManagedFiles,
    Thunderstore,
    LocalArchives,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub api: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub permissions: Vec<Permission>,
}
impl Manifest {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(self.api == API_VERSION, "Unsupported plugin API version");
        identifier(&self.id)?;
        ensure!(self.id.contains('.'), "Plugin IDs must include a namespace");
        text(&self.name, 80)?;
        text(&self.description, 500)?;
        semver::Version::parse(&self.version).context("Invalid plugin version")?;
        ensure!(self.permissions.len() <= 4, "Duplicate plugin permissions");
        for (i, permission) in self.permissions.iter().enumerate() {
            ensure!(
                !self.permissions[..i].contains(permission),
                "Duplicate plugin permissions"
            );
        }
        Ok(())
    }
}
pub fn identifier(value: &str) -> anyhow::Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 100
            && value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
            && value
                .bytes()
                .next()
                .is_some_and(|b| b.is_ascii_alphanumeric())
            && !value.ends_with('.')
            && !value.contains(".."),
        "Invalid plugin identifier"
    );
    Ok(())
}
pub fn text(value: &str, max: usize) -> anyhow::Result<()> {
    ensure!(
        !value.trim().is_empty()
            && value.len() <= max
            && !value.chars().any(|c| c.is_control() && c != '\n'),
        "Invalid or oversized plugin text"
    );
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub multiline: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub id: String,
    pub label: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default)]
    pub actions: Vec<Action>,
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
    #[serde(default)]
    pub document: Option<Value>,
    #[serde(default)]
    pub files: Vec<files::Artifact>,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub archives: Vec<files::Artifact>,
}
impl View {
    pub fn validate(&self, manifest: &Manifest) -> anyhow::Result<()> {
        text(&self.title, 120)?;
        ensure!(
            self.description.len() <= 4000
                && self.fields.len() <= 24
                && self.actions.len() <= 16
                && self.rows.len() <= 1000,
            "Plugin view exceeds limits"
        );
        let mut ids = BTreeSet::new();
        for field in &self.fields {
            identifier(&field.id)?;
            text(&field.label, 100)?;
            ensure!(
                field.value.len() <= 128 * 1024 && ids.insert(&field.id),
                "Invalid or duplicate plugin field"
            );
        }
        ids.clear();
        for action in &self.actions {
            identifier(&action.id)?;
            text(&action.label, 80)?;
            ensure!(ids.insert(&action.id), "Duplicate plugin action");
        }
        for row in &self.rows {
            ensure!(
                row.len() <= 8 && row.iter().all(|v| v.len() <= 2000),
                "Plugin table exceeds limits"
            );
        }
        ensure!(
            self.document.is_none() || manifest.permissions.contains(&Permission::Documents),
            "Plugin has no document permission"
        );
        ensure!(
            self.files.is_empty() || manifest.permissions.contains(&Permission::ManagedFiles),
            "Profile installation requires managed-file permission"
        );
        for file in self.files.iter().chain(&self.archives) {
            let permission = if file.source == files::Source::Local {
                Permission::LocalArchives
            } else {
                Permission::Thunderstore
            };
            ensure!(
                manifest.permissions.contains(&permission),
                "Plugin lacks permission for this archive source"
            );
        }
        ensure!(
            self.archives
                .iter()
                .all(|f| f.source == files::Source::Local),
            "Private archives must have a local source"
        );
        files::validate_plan(&self.archives)?;
        ensure!(
            self.packages.len() <= 128
                && (self.packages.is_empty()
                    || manifest.permissions.contains(&Permission::Thunderstore)),
            "Plugin has no Thunderstore permission or exceeds package limit"
        );
        for package in &self.packages {
            files::package_parts(package)?;
        }
        files::validate_plan(&self.files)?;
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub api: u32,
    pub event: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub fields: Value,
    #[serde(default)]
    pub state: Value,
    #[serde(default)]
    pub document: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub state: Value,
    pub view: View,
}

/// A shared document is transported as an ordinary encrypted attachment. Its
/// author/audience come from the authenticated chat message, never this JSON.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub format: u32,
    pub plugin: String,
    pub plugin_version: String,
    pub title: String,
    pub data: Value,
}
impl Document {
    pub fn parse(bytes: &[u8]) -> anyhow::Result<Self> {
        ensure!(
            bytes.len() <= MAX_JSON,
            "Shared plugin document is too large"
        );
        let value: Self = serde_json::from_slice(bytes)?;
        ensure!(value.format == 1, "Unsupported shared document format");
        identifier(&value.plugin)?;
        text(&value.title, 120)?;
        semver::Version::parse(&value.plugin_version)?;
        Ok(value)
    }
}
