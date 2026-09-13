//! Native discovery sync. Never installs or sends target-service credentials.
use super::{
    api::{Api, Failure},
    store::catalog::{CatalogState, Progress},
};
use anyhow::{Context, ensure};
use std::time::{Duration, Instant};
use wisp_crypto::account_vault::catalog::{
    self, Catalog, Entry, Envelope, ProofPage, ProofRequest, ReadResponse, Status, StoreRequest,
};
const PATH: &str = "/v3/accounts/server-catalog";

impl Api {
    /// Caller holds `Store::action`. A missing endpoint on an older server is a
    /// capability miss; it never erases a retained checkpoint or local servers.
    pub(crate) async fn sync_catalog(&self, entries: &[Entry]) -> anyhow::Result<Option<Catalog>> {
        // Catalog proof and upload futures are large. Keep their storage off
        // the shared command future, including commands that only join voice.
        Box::pin(self.sync_catalog_inner(entries)).await
    }

    #[allow(clippy::too_many_lines)] // Keep durable retry, catch-up and merge ordering together.
    async fn sync_catalog_inner(&self, entries: &[Entry]) -> anyhow::Result<Option<Catalog>> {
        let record = self.store.record()?;
        ensure!(
            record.secure && record.key_verified && record.pending.is_none(),
            "Finish account sign-in before syncing saved servers"
        );
        let scope = record.scope.context("Missing account scope")?;
        let bundle = self
            .store
            .record()?
            .bundle()?
            .context("Missing account identity")?;
        let identity = bundle.identity()?;
        let own = identity.public();
        let key = self.store.record()?.local_key()?;
        let mut saved = self.store.catalog()?;
        for _ in 0..3 {
            // Retry the exact signed ciphertext after an uncertain response.
            if let Some(pending) = &saved.pending {
                let result: anyhow::Result<Status> = self
                    .post(
                        PATH,
                        &StoreRequest {
                            catalog: pending.clone(),
                        },
                        true,
                        16384,
                    )
                    .await;
                match result {
                    Ok(status) => {
                        ensure!(
                            status.version == catalog::VERSION
                                && status.scope == scope
                                && status.catalog == Some(pending.manifest.checkpoint()?),
                            "Published catalog confirmation changed"
                        );
                        let mut next = saved.clone();
                        next.head = next.pending.take();
                        next.progress = None;
                        self.store.save_catalog(&saved, &next)?;
                        saved = next;
                    }
                    Err(error)
                        if error.downcast_ref::<Failure>().is_some_and(|f| {
                            f.status == 409 && f.code == "catalog_state_changed"
                        }) =>
                    {
                        // Preserve the intended entries in the pending envelope
                        // until an authenticated current head can be merged.
                    }
                    Err(error) => return Err(error),
                }
            }
            let status: Status = match self.get(&format!("{PATH}/status"), 16384).await {
                Ok(status) => status,
                Err(error)
                    if error
                        .downcast_ref::<Failure>()
                        .is_some_and(|f| f.status == 404) =>
                {
                    return Ok(None);
                }
                Err(error) => return Err(error),
            };
            ensure!(
                status.version == catalog::VERSION && status.scope == scope,
                "Catalog service or account changed"
            );
            if let Some(checkpoint) = &status.catalog {
                checkpoint.validate()?;
            }
            if saved.checkpoint()?.is_some() {
                ensure!(
                    status.catalog.is_some(),
                    "Server catalog history disappeared"
                );
            }
            if status.catalog != saved.checkpoint()? || saved.progress.is_some() {
                self.refresh_catalog(&mut saved).await?;
            }
            let mut merged = if let Some(head) = &saved.head {
                head.open(&key, &scope, &own, &head.manifest.checkpoint()?)?
            } else {
                Catalog::empty(scope.clone())
            };
            if let Some(pending) = &saved.pending {
                let unpublished =
                    pending.open(&key, &scope, &own, &pending.manifest.checkpoint()?)?;
                merged.merge(&unpublished.entries)?;
            }
            merged.merge(entries)?;
            let unchanged = saved
                .head
                .as_ref()
                .map(|head| head.open(&key, &scope, &own, &head.manifest.checkpoint()?))
                .transpose()?
                .is_some_and(|head| head == merged);
            if unchanged || (saved.head.is_none() && merged.entries.is_empty()) {
                if saved.pending.is_some() {
                    let mut next = saved.clone();
                    next.pending = None;
                    self.store.save_catalog(&saved, &next)?;
                }
                return Ok(Some(merged));
            }
            let mut next = saved.clone();
            next.pending = Some(Envelope::seal(
                &merged,
                &key,
                &identity,
                saved.checkpoint()?,
            )?);
            self.store.save_catalog(&saved, &next)?;
            saved = next;
        }
        anyhow::bail!("Saved servers are changing on another device; syncing will retry")
    }

    async fn refresh_catalog(&self, saved: &mut CatalogState) -> anyhow::Result<()> {
        let record = self.store.record()?;
        let scope = record.scope.as_ref().context("Missing account scope")?;
        let own = record
            .identity
            .as_ref()
            .context("Missing account identity")?;
        let key = record.local_key()?;
        if saved.progress.is_none() {
            let response: ReadResponse = self.get(PATH, catalog::MAX_ENVELOPE_WIRE + 16384).await?;
            ensure!(
                response.version == catalog::VERSION && &response.scope == scope,
                "Catalog service or account changed"
            );
            let target = response
                .catalog
                .context("Server catalog history disappeared")?;
            target.verify(scope, own)?;
            let through = target.manifest.checkpoint()?;
            if let Some(source) = saved.checkpoint()? {
                ensure!(
                    through.revision >= source.revision
                        && (through.revision != source.revision || through == source),
                    "Server catalog history rolled back or forked"
                );
                let mut next = saved.clone();
                next.progress = Some(Progress {
                    target,
                    verified: source,
                });
                self.store.save_catalog(saved, &next)?;
                *saved = next;
            } else {
                target.open(&key, scope, own, &through)?;
                let mut next = saved.clone();
                next.head = Some(target);
                self.merge_pending_into_head(saved, &mut next)?;
                self.store.save_catalog(saved, &next)?;
                *saved = next;
                return Ok(());
            }
        }
        let started = Instant::now();
        for _ in 0..32 {
            let progress = saved
                .progress
                .as_ref()
                .context("Missing catalog history progress")?;
            let through = progress.target.manifest.checkpoint()?;
            if progress.verified == through {
                progress.target.open(&key, scope, own, &through)?;
                let mut next = saved.clone();
                next.head = Some(progress.target.clone());
                next.progress = None;
                self.merge_pending_into_head(saved, &mut next)?;
                self.store.save_catalog(saved, &next)?;
                *saved = next;
                return Ok(());
            }
            ensure!(
                started.elapsed() < Duration::from_secs(30),
                "Server history catch-up will continue on the next sync"
            );
            let page: ProofPage = self
                .post(
                    &format!("{PATH}/proof"),
                    &ProofRequest {
                        after: progress.verified.clone(),
                        through: through.clone(),
                    },
                    true,
                    512 * 1024 + 16384,
                )
                .await?;
            ensure!(
                !page.manifests.is_empty() && page.manifests.len() <= catalog::MAX_PROOF_PAGE,
                "Catalog proof did not make bounded progress"
            );
            let mut verified = progress.verified.clone();
            for manifest in &page.manifests {
                verified = manifest.advance(scope, own, Some(&verified))?;
                ensure!(
                    verified.revision <= through.revision,
                    "Catalog proof passed its target"
                );
            }
            ensure!(
                page.next == verified && page.complete == (verified == through),
                "Catalog proof result changed"
            );
            let mut next = saved.clone();
            next.progress.as_mut().context("Missing history")?.verified = verified;
            self.store.save_catalog(saved, &next)?;
            *saved = next;
        }
        anyhow::bail!("Server history catch-up will continue on the next sync")
    }
    // Preserve an interrupted intended union across a head change, re-signing
    // only after the current head has passed signed history verification.
    fn merge_pending_into_head(
        &self,
        saved: &CatalogState,
        next: &mut CatalogState,
    ) -> anyhow::Result<()> {
        if let Some(pending) = &saved.pending {
            let record = self.store.record()?;
            let scope = record.scope.as_ref().context("Missing account scope")?;
            let identity = record.bundle()?.context("Missing identity")?.identity()?;
            let key = record.local_key()?;
            let unpublished = pending.open(
                &key,
                scope,
                &identity.public(),
                &pending.manifest.checkpoint()?,
            )?;
            let head = next.head.as_ref().context("Missing catalog head")?;
            let mut merged = head.open(
                &key,
                scope,
                &identity.public(),
                &head.manifest.checkpoint()?,
            )?;
            next.pending = if merged.merge(&unpublished.entries)? {
                Some(Envelope::seal(
                    &merged,
                    &key,
                    &identity,
                    next.checkpoint()?,
                )?)
            } else {
                None
            };
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "catalog/tests.rs"]
mod tests;
