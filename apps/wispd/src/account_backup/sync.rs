#![allow(clippy::items_after_statements)] // Keep private response types beside their validation.
//! Signed history catch-up and detached, conflict-preserving trust merges.
use super::{
    api::{Api, ProofPage, Receipt, VaultResponse},
    store::{Kind, Pending, PrivateBytes},
};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;
use wisp_crypto::{
    account_vault::{
        auth::ExportKey,
        bundle::Bundle,
        envelope::{Checkpoint, Envelope, MAX_ENVELOPE_WIRE},
        operation::{Change, DeviceBinding, Operation},
    },
    roster::SignedRoster,
};

pub(super) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .try_into()
        .unwrap_or(i64::MAX)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catchup {
    source: Checkpoint,
    verified: Checkpoint,
    target: VaultResponse,
}
impl Api {
    fn validate_vault(&self, remote: &VaultResponse) -> anyhow::Result<Checkpoint> {
        self.validate_status(&remote.state, false)?;
        let expected = remote.state.expected();
        let own = remote
            .state
            .identity()
            .context("Missing secure account identity")?;
        remote.wrapper.validate(
            remote.state.scope(),
            expected
                .wrapper_generation
                .context("Missing secure wrapper")?,
        )?;
        remote.vault.verify(remote.state.scope(), own)?;
        let checkpoint = remote.vault.manifest.checkpoint()?;
        ensure!(
            expected.vault.as_ref() == Some(&checkpoint),
            "Backup metadata changed"
        );
        Ok(checkpoint)
    }
    /// Persist every verified page before yielding. A restart resumes the exact
    /// target; it cannot skip a retained checkpoint to shorten a large history.
    #[allow(clippy::too_many_lines)] // Keep durable transition ordering reviewable as one operation.
    async fn verified_vault(&self) -> anyhow::Result<VaultResponse> {
        let record = self.store.record()?;
        let mut catchup = if let Some(saved) = &record.proof {
            let saved: Catchup = serde_json::from_value(saved.value()?)?;
            let target = self.validate_vault(&saved.target)?;
            ensure!(
                record.checkpoint.as_ref() == Some(&saved.source)
                    && saved.source.revision <= saved.verified.revision
                    && saved.verified.revision <= target.revision,
                "Saved backup history progress changed"
            );
            saved
        } else {
            let remote: VaultResponse = self
                .get("/v3/accounts/vault", MAX_ENVELOPE_WIRE + 16384)
                .await?;
            let target = self.validate_vault(&remote)?;
            let Some(source) = record.checkpoint else {
                return Ok(remote);
            };
            ensure!(
                target.revision >= source.revision,
                "Backup history rolled back"
            );
            if source == target {
                return Ok(remote);
            }
            let catchup = Catchup {
                source: source.clone(),
                verified: source,
                target: remote,
            };
            self.store.update(Some(record.revision), |record| {
                record.proof = Some(PrivateBytes::json(&catchup)?);
                Ok(())
            })?;
            catchup
        };
        let through = self.validate_vault(&catchup.target)?;
        let started = Instant::now();
        for _ in 0..32 {
            if catchup.verified == through {
                return Ok(catchup.target);
            }
            ensure!(
                started.elapsed() < Duration::from_secs(30),
                "Backup history is still catching up. Continue syncing to resume its saved progress"
            );
            let page: ProofPage = self
                .post(
                    "/v3/accounts/vault/proof",
                    &json!({"after":catchup.verified,"through":through}),
                    true,
                    512 * 1024,
                )
                .await?;
            ensure!(
                !page.manifests.is_empty() && page.manifests.len() <= 128,
                "Backup proof did not make bounded progress"
            );
            let before = catchup.verified.clone();
            let mut verified = before.clone();
            for manifest in &page.manifests {
                verified = manifest.advance(
                    catchup.target.state.scope(),
                    catchup
                        .target
                        .state
                        .identity()
                        .context("Missing account identity")?,
                    Some(&verified),
                )?;
                ensure!(
                    verified.revision <= through.revision,
                    "Backup proof passed its target"
                );
            }
            ensure!(
                page.next == verified && page.complete == (verified == through),
                "Backup proof result changed"
            );
            catchup.verified = verified;
            self.store.update(None, |record| {
                let saved: Catchup = serde_json::from_value(
                    record
                        .proof
                        .as_ref()
                        .context("Backup proof was replaced")?
                        .value()?,
                )?;
                ensure!(
                    saved.verified == before
                        && saved.source == catchup.source
                        && saved.target.vault.manifest.checkpoint()? == through,
                    "Another backup check is in progress"
                );
                record.proof = Some(PrivateBytes::json(&catchup)?);
                Ok(())
            })?;
        }
        ensure!(
            catchup.verified == through,
            "Backup history is still catching up. Continue syncing to resume its saved progress"
        );
        Ok(catchup.target)
    }
    async fn room_proofs(
        &self,
        local: &Bundle,
        remote: &Bundle,
    ) -> anyhow::Result<BTreeMap<String, Vec<SignedRoster>>> {
        let different: Vec<_> = local
            .trust
            .room_heads
            .iter()
            .filter_map(|(id, a)| {
                remote
                    .trust
                    .room_heads
                    .get(id)
                    .filter(|b| a != *b)
                    .map(|b| (id, a, b))
            })
            .collect();
        if different.is_empty() {
            return Ok(BTreeMap::new());
        }
        #[derive(Deserialize)]
        struct Directory {
            network: Uuid,
            rosters: BTreeMap<String, Vec<SignedRoster>>,
        }
        let directory: Directory = self.get("/v1/e2ee/state", 16 * 1024 * 1024).await?;
        ensure!(
            directory.network == local.scope.network,
            "Room proof network changed"
        );
        let mut proofs = BTreeMap::new();
        for (id, a, b) in different {
            let (older, newer) = if a.roster.revision < b.roster.revision {
                (a, b)
            } else {
                (b, a)
            };
            ensure!(
                older.roster.revision < newer.roster.revision,
                "Conflicting room history checkpoints"
            );
            let chain = directory
                .rosters
                .get(id)
                .context("Connecting room history is unavailable. Existing trust was preserved")?;
            let start = chain
                .iter()
                .position(|head| head == older)
                .context("Saved room checkpoint is absent from its history")?;
            let end = chain
                .iter()
                .position(|head| head == newer)
                .context("Remote room checkpoint is absent from its history")?;
            ensure!(
                end > start && end - start < 4096,
                "Invalid connecting room history"
            );
            proofs.insert(id.clone(), chain[start..=end].to_vec());
        }
        Ok(proofs)
    }
    /// Returns false only for a freshly authenticated device whose old wrapper
    /// cannot be opened after an email reset. It never generates replacement keys.
    pub(crate) async fn restore(&self, export: Option<(&ExportKey, Uuid)>) -> anyhow::Result<bool> {
        let remote = self.verified_vault().await?;
        let checkpoint = self.validate_vault(&remote)?;
        let expected = remote.state.expected();
        let record = self.store.record()?;
        let key = if let Some((export, generation)) =
            export.filter(|(_, generation)| Some(*generation) == expected.wrapper_generation)
        {
            remote
                .wrapper
                .unwrap(remote.state.scope(), generation, export)?
        } else if record.key_verified {
            record.local_key()?
        } else {
            self.store.update(None, |record| {
                ensure!(
                    record
                        .scope
                        .as_ref()
                        .is_none_or(|scope| scope == remote.state.scope()),
                    "Account changed"
                );
                record.scope = Some(remote.state.scope().clone());
                record.network = Some(remote.state.scope().network);
                record.username = Some(remote.state.username().to_owned());
                record.identity = remote.state.identity().cloned();
                record.expected = Some(expected.clone());
                record.secure = true;
                Ok(())
            })?;
            return Ok(false);
        };
        if record.key_verified {
            ensure!(
                record.local_key()?.save_private().as_slice() == key.save_private().as_slice(),
                "Account backup key changed"
            );
        }
        let bundle = remote.vault.open(
            &key,
            remote.state.scope(),
            remote
                .state
                .identity()
                .context("Missing encryption identity")?,
            &checkpoint,
        )?;
        // Fetch room proof material and merge outside the persistence lock. A
        // concurrent trust mutation invalidates the candidate instead of losing it.
        for _ in 0..3 {
            let record = self.store.record()?;
            let merged = if let Some(local) = record.bundle()? {
                let proofs = self.room_proofs(&local, &bundle).await?;
                local.merge(&bundle, &proofs)?
            } else {
                Bundle::decode_private(
                    &bundle.encode_private()?,
                    &bundle.scope,
                    &bundle.identity()?.public(),
                )?
            };
            let differs =
                merged.encode_private()?.as_slice() != bundle.encode_private()?.as_slice();
            if self.store.record()?.revision != record.revision {
                continue;
            }
            self.store.update(Some(record.revision), |record| {
                record.key = Some(PrivateBytes::new(key.save_private().as_slice()));
                record.key_verified = true;
                record.set_bundle(&merged)?;
                record.secure = true;
                record.username = Some(remote.state.username().to_owned());
                record.checkpoint = Some(checkpoint.clone());
                record.expected = Some(expected.clone());
                record.proof = None;
                if differs {
                    record.dirty = record
                        .dirty
                        .checked_add(1)
                        .context("Backup revision exhausted")?;
                } else {
                    record.synced_dirty = record.dirty;
                }
                record.last_sync = Some(now());
                Ok(())
            })?;
            return Ok(true);
        }
        anyhow::bail!(
            "Local trust changed while restoring. Continue syncing to merge its latest state"
        )
    }
    pub(crate) async fn receipt(&self, pending: &Pending) -> anyhow::Result<Option<Receipt>> {
        let response = self
            .get::<Receipt>(&format!("/v3/accounts/operations/{}", pending.id), 32768)
            .await;
        let receipt = match response {
            Err(error)
                if error
                    .downcast_ref::<super::api::Failure>()
                    .is_some_and(|f| f.code == "not_found" && f.status == 404) =>
            {
                return Ok(None);
            }
            other => other?,
        };
        ensure!(
            receipt.operation_id == pending.id
                && Some(&receipt.operation_sha256) == pending.effect_digest.as_ref()
                && Some(&receipt.scope) == pending.scope.as_ref(),
            "Account receipt does not match the saved action"
        );
        let kind = match pending.kind {
            Kind::Signup => "register",
            Kind::Migration => "migrate",
            Kind::Password => "password",
            Kind::Rewrap => "rewrap",
            Kind::Email => "recovery_email",
            Kind::Sync => "vault",
            Kind::Reset => "reset",
            Kind::Login => "login",
            Kind::ClassicRecovery => "migration_recovery",
        };
        ensure!(
            receipt.kind == kind && receipt.committed_at > 0,
            "Account receipt purpose changed"
        );
        if pending.kind == Kind::ClassicRecovery {
            ensure!(
                receipt.device_id == pending.device.as_ref().map(|device| device.id),
                "Recovery receipt device changed"
            );
        } else if pending.kind != Kind::Reset && pending.kind != Kind::Login {
            let body = pending
                .finish
                .as_ref()
                .context("Missing saved account action")?
                .value()?;
            let operation: Operation = serde_json::from_value(body["operation"].clone())?;
            ensure!(
                receipt.device_id == Some(operation.device.id()),
                "Account receipt device changed"
            );
        }
        Ok(Some(receipt))
    }
    #[allow(clippy::too_many_lines)] // Keep durable transition ordering reviewable as one operation.
    pub(crate) async fn sync(&self) -> anyhow::Result<()> {
        if let Some(pending) = self.store.record()?.pending {
            ensure!(
                pending.kind == Kind::Sync,
                "Finish the pending account action before syncing"
            );
            if self.receipt(&pending).await?.is_none() {
                let body = pending
                    .finish
                    .as_ref()
                    .context("Missing pending backup write")?
                    .value()?;
                let operation: Operation = serde_json::from_value(body["operation"].clone())?;
                let state = self.status().await?;
                if state.expected() == operation.expected {
                    let _: Value = self.post("/v3/accounts/vault", &body, true, 32768).await?;
                    ensure!(
                        self.receipt(&pending).await?.is_some(),
                        "Could not confirm backup write"
                    );
                } else {
                    // Generations never repeat and revisions never decrease;
                    // this exact precondition cannot become true again. Check
                    // the receipt after observing that terminal state change.
                    let _ = self.receipt(&pending).await?;
                }
            }
            self.store.update(None, |record| {
                ensure!(
                    record.pending.as_ref().is_some_and(|p| p.id == pending.id),
                    "Pending backup changed"
                );
                record.pending = None;
                Ok(())
            })?;
        }
        let current = self.status().await?;
        let saved = self.store.record()?;
        if saved.key_verified
            && saved.proof.is_none()
            && saved.checkpoint == current.expected().vault
        {
            // A small status check is enough when the authenticated payload
            // checkpoint is unchanged. Avoid downloading the whole backup on
            // every background tick or before publishing local-only changes.
            if saved.expected.as_ref() != Some(&current.expected()) {
                self.store.update(None, |record| {
                    record.expected = Some(current.expected());
                    Ok(())
                })?;
            }
        } else {
            ensure!(
                self.restore(None).await?,
                "Unlock your encrypted backup before syncing"
            );
        }
        let record = self.store.record()?;
        if record.synced_dirty == record.dirty {
            return Ok(());
        }
        let bundle = record
            .bundle()?
            .context("Unlock your encrypted backup before syncing")?;
        let vault = Envelope::seal(&bundle, &record.local_key()?, record.checkpoint.clone())?;
        let operation = Operation {
            format: 1,
            scope: bundle.scope.clone(),
            id: Uuid::new_v4(),
            device: DeviceBinding::Existing {
                id: self.device.context("Missing signed-in device")?,
            },
            expected: record
                .expected
                .clone()
                .context("Refresh account status before syncing")?,
            change: Change::StoreVault {
                vault: vault.manifest.checkpoint()?,
            },
        };
        let body = json!({"signature":operation.sign(&bundle.identity()?)?,"operation":operation,"vault":vault});
        let mut pending = Pending::new(Kind::Sync, operation.id);
        pending.scope = Some(bundle.scope.clone());
        pending.identity = Some(bundle.identity()?.public());
        pending.effect_digest = Some(operation.digest()?);
        pending.finish = Some(PrivateBytes::json(&body)?);
        pending.snapshot_dirty = Some(record.dirty);
        pending.sent = true;
        self.store.update(Some(record.revision), |record| {
            ensure!(
                record.pending.is_none(),
                "Another account action is in progress"
            );
            record.pending = Some(pending.clone());
            Ok(())
        })?;
        let _: Value = self.post("/v3/accounts/vault", &body, true, 32768).await?;
        ensure!(
            self.receipt(&pending).await?.is_some(),
            "Could not confirm backup write"
        );
        self.store.update(None, |record| {
            ensure!(
                record.pending.as_ref().is_some_and(|p| p.id == pending.id),
                "Pending backup changed"
            );
            record.pending = None;
            record.checkpoint = Some(vault.manifest.checkpoint()?);
            record
                .expected
                .as_mut()
                .context("Missing account state")?
                .vault = record.checkpoint.clone();
            record.synced_dirty = pending
                .snapshot_dirty
                .context("Missing backup change counter")?;
            record.last_sync = Some(now());
            Ok(())
        })?;
        Ok(())
    }
}
