//! Recovery retains every possibly activated credential until a bound receipt
//! and server-side retirement prove it cannot become active later.
use super::{
    api::{Api, Status},
    store::{Kind, Pending, PrivateBytes, StagedDevice},
};
use age::secrecy::ExposeSecret;
use anyhow::{Context, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;
use wisp_crypto::{
    SecretString,
    account_vault::{
        Scope,
        operation::{MigrationRecoveryBinding, Operation},
    },
};
use wisp_protocol::{DeviceCredential, UserSummary};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_field_names)] // Exact protocol field names.
struct Recovered {
    recovered: bool,
    recovery_id: Uuid,
    scope: Scope,
    operation_id: Uuid,
    operation_sha256: String,
    terminated_device_id: Uuid,
    device_id: Uuid,
    user: UserSummary,
    username: String,
    identity: wisp_crypto::PublicIdentity,
}
impl Api {
    pub(crate) async fn recover_secure(
        &mut self,
        username: &str,
        password: SecretString,
        device_name: &str,
    ) -> anyhow::Result<bool> {
        let unlocked = self.login(username, password, device_name).await?;
        let Some(root) = self.store.record()?.pending else {
            return Ok(unlocked);
        };
        ensure!(
            !matches!(root.kind, Kind::Login | Kind::Reset | Kind::ClassicRecovery),
            "Invalid pending recovery action"
        );
        let state = self.status().await?;
        ensure!(
            matches!(state, Status::Secure { .. }) && Some(state.scope()) == root.scope.as_ref(),
            "Recovery returned another account"
        );
        // Classic recovery candidates cannot activate after the irreversible
        // secure-mode transition. Any existing activation is retired explicitly.
        for candidate in &root.recovery {
            ensure!(
                candidate.kind == Kind::ClassicRecovery,
                "Resolve pending sign-ins first"
            );
            let _ = self.receipt(candidate).await?;
            self.revoke(
                candidate
                    .device
                    .as_ref()
                    .context("Missing recovery device")?
                    .id,
            )
            .await?;
            let _ = self.receipt(candidate).await?;
        }
        if root.finish.is_none() && !root.sent {
            // No finalization was ever available for dispatch. Fresh native
            // sign-in has already resolved every older staged login.
            self.clear_recovered_root(&root)?;
            return Ok(unlocked);
        }
        let operation: Operation = serde_json::from_value(
            root.finish
                .as_ref()
                .context("Missing original account action")?
                .value()?["operation"]
                .clone(),
        )?;
        ensure!(
            Some(operation.digest()?) == root.effect_digest
                && Some(&operation.scope) == root.scope.as_ref(),
            "Original account effect changed"
        );
        if root.kind == Kind::Signup {
            ensure!(
                self.receipt(&root).await?.is_some(),
                "The original signup receipt is unavailable; its staged identity was preserved"
            );
        }
        self.revoke(operation.device.id()).await?;
        // Read after revocation: Existing-device handlers authenticate inside the
        // same serialized write transaction, so an absent effect is now terminal.
        let _ = self.receipt(&root).await?;
        self.clear_recovered_root(&root)?;
        Ok(unlocked)
    }
    fn clear_recovered_root(&self, root: &Pending) -> anyhow::Result<()> {
        self.store.update(None, |record| {
            let current = record
                .pending
                .as_ref()
                .context("Recovery journal changed")?;
            ensure!(
                current.id == root.id
                    && current
                        .recovery
                        .iter()
                        .map(|p| p.id)
                        .eq(root.recovery.iter().map(|p| p.id)),
                "Recovery journal changed"
            );
            record.pending = None;
            record.installation_ready = true;
            Ok(())
        })
    }
    async fn classic_receipt(
        &self,
        candidate: &Pending,
        legacy: Option<&SecretString>,
    ) -> anyhow::Result<Recovered> {
        let mut body = candidate
            .finish
            .as_ref()
            .context("Missing signed recovery effect")?
            .value()?;
        body["legacy_password"] = json!(legacy.map_or("", ExposeSecret::expose_secret));
        let result = self
            .post::<Recovered>("/v3/auth/migrate/recover", &body, false, 16384)
            .await;
        if let Some(Value::String(password)) = body.get_mut("legacy_password") {
            use zeroize::Zeroize;
            password.zeroize();
        }
        let result = result?;
        let binding: MigrationRecoveryBinding = serde_json::from_value(body["recovery"].clone())?;
        ensure!(
            result.recovered
                && result.recovery_id == candidate.id
                && result.scope == binding.operation.scope
                && result.operation_id == binding.operation.id
                && result.operation_sha256 == binding.operation.digest()?
                && result.terminated_device_id == binding.operation.device.id()
                && result.device_id == binding.device.id()
                && result.user.id == binding.operation.scope.account
                && Some(&result.identity) == candidate.identity.as_ref()
                && candidate.username.as_deref() == Some(result.username.as_str()),
            "Classic recovery receipt changed account or device"
        );
        Ok(result)
    }
    async fn use_classic_candidate(
        &mut self,
        candidate: &Pending,
        result: Recovered,
    ) -> anyhow::Result<()> {
        let device = candidate
            .device
            .as_ref()
            .context("Missing recovery credential")?
            .native()?;
        let credential = DeviceCredential {
            device_id: device.id(),
            device_token: device.token().expose_secret().to_owned(),
            user: result.user,
        };
        self.store.update(None, |record| {
            record.installation = Some(PrivateBytes::json(&credential)?);
            record.installation_pending = true;
            record.installation_ready = false;
            Ok(())
        })?;
        self.session(&credential).await
    }
    #[allow(clippy::too_many_lines)] // Keep durable transition ordering reviewable as one operation.
    pub(crate) async fn recover_classic_migration(
        &mut self,
        legacy: SecretString,
        device_name: &str,
    ) -> anyhow::Result<()> {
        let root = self
            .store
            .record()?
            .pending
            .context("No interrupted classic migration")?;
        ensure!(
            root.kind == Kind::Migration && root.finish.is_some(),
            "Only the original classic migration can use this recovery flow"
        );
        let operation: Operation = serde_json::from_value(
            root.finish
                .as_ref()
                .context("Missing migration effect")?
                .value()?["operation"]
                .clone(),
        )?;
        ensure!(
            Some(operation.digest()?) == root.effect_digest,
            "Original migration effect changed"
        );
        let mut active = None;
        if let Some(candidate) = root
            .recovery
            .last()
            .filter(|candidate| candidate.kind == Kind::ClassicRecovery)
        {
            match self.classic_receipt(candidate, None).await {
                Ok(result) => match self.use_classic_candidate(candidate, result).await {
                    Ok(()) => active = Some(candidate.id),
                    Err(error)
                        if error
                            .downcast_ref::<super::api::Failure>()
                            .is_some_and(|failure| matches!(failure.status, 401 | 404)) => {}
                    Err(error) => return Err(error),
                },
                Err(error)
                    if error
                        .downcast_ref::<super::api::Failure>()
                        .is_some_and(|failure| {
                            matches!(failure.code, "unauthorized" | "account_state_changed")
                        }) => {}
                Err(error) => return Err(error),
            }
        }
        if active.is_none() {
            let record = self.store.record()?;
            let device = StagedDevice::generate()?;
            let binding = MigrationRecoveryBinding {
                format: 1,
                id: Uuid::new_v4(),
                operation: operation.clone(),
                device: device.native()?.binding(),
                device_name: device_name.to_owned(),
            };
            let identity = record
                .bundle()?
                .context("Original encryption identity is required")?
                .identity()?;
            let body = json!({"recovery":binding,"signature":binding.sign(&identity)?});
            let mut candidate = Pending::new(Kind::ClassicRecovery, binding.id);
            candidate.scope = Some(operation.scope.clone());
            candidate.username = root.username.clone();
            candidate.identity = Some(identity.public());
            candidate.device = Some(device);
            candidate.effect_digest = Some(binding.digest()?);
            candidate.finish = Some(PrivateBytes::json(&body)?);
            candidate.sent = true;
            self.store.update(Some(record.revision), |record| {
                let saved = record
                    .pending
                    .as_mut()
                    .context("Missing migration journal")?;
                ensure!(saved.id == root.id, "Migration journal changed");
                saved.recovery.push(candidate.clone());
                Ok(())
            })?;
            let result = self.classic_receipt(&candidate, Some(&legacy)).await?;
            self.use_classic_candidate(&candidate, result).await?;
            active = Some(candidate.id);
        }
        let active = active.context("No confirmed recovery device")?;
        let state: Status = self.get("/v3/accounts/vault/status", 16384).await?;
        self.validate_status(&state, true)?;
        ensure!(
            matches!(state, Status::Classic { .. }),
            "The account migrated on another device. Recover using its secure password"
        );
        let root = self
            .store
            .record()?
            .pending
            .context("Migration journal changed")?;
        for candidate in &root.recovery {
            if candidate.id == active {
                continue;
            }
            if candidate.kind == Kind::Login && !candidate.sent && candidate.finish.is_none() {
                continue;
            }
            ensure!(
                candidate.kind == Kind::ClassicRecovery,
                "Resolve interrupted secure sign-in first"
            );
            // A missing public receipt is not noncommit: retry with original
            // password if necessary to obtain a deterministic terminal result,
            // then explicitly revoke that exact activated credential.
            match self.classic_receipt(candidate, None).await {
                Ok(_) => (),
                Err(error)
                    if error
                        .downcast_ref::<super::api::Failure>()
                        .is_some_and(|failure| failure.code == "unauthorized") =>
                {
                    self.classic_receipt(candidate, Some(&legacy)).await?;
                }
                Err(error) => return Err(error),
            }
            self.revoke(
                candidate
                    .device
                    .as_ref()
                    .context("Missing prior recovery device")?
                    .id,
            )
            .await?;
        }
        self.store.update(None, |record| {
            let saved = record
                .pending
                .as_mut()
                .context("Migration journal changed")?;
            ensure!(
                saved.id == root.id
                    && saved
                        .recovery
                        .iter()
                        .map(|p| p.id)
                        .eq(root.recovery.iter().map(|p| p.id)),
                "Another recovery action started"
            );
            saved.recovery.clear();
            Ok(())
        })?;
        self.rebind_migration(root.id)?;
        self.resume_enrollment(Some(legacy)).await
    }
}
