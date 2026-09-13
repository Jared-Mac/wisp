//! Separate sidecar keeps the strict legacy private account record compatible.
use super::{Record, Store, check_file};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
};
use wisp_crypto::account_vault::catalog::{Envelope, MAX_ENVELOPE_WIRE};
use wisp_crypto::account_vault::{Scope, envelope::Checkpoint};
use zeroize::Zeroizing;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Progress {
    pub(crate) target: Envelope,
    pub(crate) verified: Checkpoint,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct CatalogState {
    version: u32,
    scope: Scope,
    pub(crate) head: Option<Envelope>,
    pub(crate) pending: Option<Envelope>,
    pub(crate) progress: Option<Progress>,
}
impl CatalogState {
    fn empty(scope: Scope) -> Self {
        Self {
            version: 1,
            scope,
            head: None,
            pending: None,
            progress: None,
        }
    }
    pub(crate) fn checkpoint(&self) -> anyhow::Result<Option<Checkpoint>> {
        self.head
            .as_ref()
            .map(|h| h.manifest.checkpoint())
            .transpose()
    }
    fn validate(&self, record: &Record) -> anyhow::Result<()> {
        ensure!(
            self.version == 1 && Some(&self.scope) == record.scope.as_ref() && record.key_verified,
            "Saved server catalog belongs to a different or locked account"
        );
        let own = record
            .identity
            .as_ref()
            .context("Missing account identity")?;
        let key = record.local_key()?;
        if let Some(head) = &self.head {
            head.open(&key, &self.scope, own, &head.manifest.checkpoint()?)?;
        }
        if let Some(pending) = &self.pending {
            let next = pending
                .manifest
                .advance(&self.scope, own, self.checkpoint()?.as_ref())?;
            pending.open(&key, &self.scope, own, &next)?;
        }
        if let Some(progress) = &self.progress {
            progress.target.verify(&self.scope, own)?;
            progress.verified.validate()?;
            let source = self
                .checkpoint()?
                .context("Missing catalog history source")?;
            let target = progress.target.manifest.checkpoint()?;
            ensure!(
                source.revision <= progress.verified.revision
                    && progress.verified.revision <= target.revision,
                "Invalid saved catalog history progress"
            );
            ensure!(
                source.revision != progress.verified.revision || source == progress.verified,
                "Catalog history source changed"
            );
        }
        Ok(())
    }
}
const MAX_SIDECAR: u64 = (MAX_ENVELOPE_WIRE as u64) * 3 + 16_384;
impl Store {
    fn load_catalog(&self, record: &Record) -> anyhow::Result<CatalogState> {
        let path = self.path.with_extension("catalog");
        if !path.try_exists()? {
            ensure!(
                !self
                    .path
                    .with_extension("catalog-initialized")
                    .try_exists()?,
                "Saved server history is missing; existing account data was preserved"
            );
            return Ok(CatalogState::empty(
                record.scope.clone().context("Missing account scope")?,
            ));
        }
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        check_file(&file.metadata()?)?;
        ensure!(
            file.metadata()?.len() <= MAX_SIDECAR,
            "Saved server catalog exceeds its size limit"
        );
        let mut bytes = Zeroizing::new(Vec::new());
        Read::by_ref(&mut file)
            .take(MAX_SIDECAR + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 <= MAX_SIDECAR,
            "Saved server catalog exceeds its size limit"
        );
        let state: CatalogState = serde_json::from_slice(&bytes)
            .context("Saved server catalog is damaged; account data was preserved")?;
        state.validate(record)?;
        Ok(state)
    }
    pub(crate) fn catalog(&self) -> anyhow::Result<CatalogState> {
        let record = self.record()?;
        let _lock = self.lock(false)?;
        self.load_catalog(&record)
    }
    pub(crate) fn save_catalog(
        &self,
        expected: &CatalogState,
        state: &CatalogState,
    ) -> anyhow::Result<()> {
        let record = self.record()?;
        state.validate(&record)?;
        let bytes = Zeroizing::new(serde_json::to_vec(state)?);
        ensure!(
            bytes.len() as u64 <= MAX_SIDECAR,
            "Saved server catalog exceeds its size limit"
        );
        let _lock = self.lock(true)?;
        ensure!(
            self.load_catalog(&record)? == *expected,
            "Saved server state changed; retry syncing"
        );
        if let Some(old) = expected.checkpoint()? {
            let next = state
                .checkpoint()?
                .context("Saved server history cannot be erased")?;
            ensure!(
                next.revision > old.revision || next == old,
                "Saved server history cannot rewind or fork"
            );
        }
        let parent = self
            .path
            .parent()
            .context("Missing account storage directory")?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(self.path.with_extension("catalog"))
            .context("Could not save server catalog")?;
        let marker = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(self.path.with_extension("catalog-initialized"))?;
        check_file(&marker.metadata()?)?;
        marker.sync_all()?;
        File::open(parent)?.sync_all()?;
        Ok(())
    }
}
