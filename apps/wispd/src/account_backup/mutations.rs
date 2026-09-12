//! Existing-device changes use exact-operation, single-use password proofs.
use super::{
    api::{Api, Started, Status},
    store::{Kind, Pending, PrivateBytes},
    sync::now,
};
use age::secrecy::ExposeSecret;
use anyhow::{Context, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use wisp_crypto::{
    SecretString,
    account_vault::{
        auth,
        envelope::KeyEnvelope,
        operation::{Change, DeviceBinding, Operation, RegistrationTranscript},
    },
};

fn copy(password: &SecretString) -> SecretString {
    SecretString::from(password.expose_secret().to_owned())
}
fn route(kind: Kind) -> anyhow::Result<&'static str> {
    match kind {
        Kind::Password => Ok("/v3/auth/password/finish"),
        Kind::Rewrap => Ok("/v3/accounts/vault/rewrap"),
        Kind::Email => Ok("/v3/accounts/recovery-email"),
        _ => anyhow::bail!("Invalid account mutation"),
    }
}
impl Api {
    async fn unlocked_state(&self, password: &SecretString) -> anyhow::Result<Status> {
        let state = self.status().await?;
        if !self.store.record()?.key_verified {
            let export = self.unlock(copy(password), &state).await?;
            ensure!(
                self.restore(Some((
                    &export,
                    state
                        .expected()
                        .credential_generation
                        .context("Enable account backup first")?
                )))
                .await?,
                "This backup still needs an existing trusted device to repair its password wrapper"
            );
        } else {
            ensure!(self.restore(None).await?, "Unlock your backup first");
        }
        self.status().await
    }
    pub(crate) async fn password_change(
        &self,
        current: SecretString,
        new: SecretString,
    ) -> anyhow::Result<()> {
        if let Some(pending) = self.store.record()?.pending {
            ensure!(
                pending.kind == Kind::Password,
                "Finish the pending account action first"
            );
            if pending.finish.is_some() {
                return self.resume_mutation(Some(current)).await;
            }
        }
        let state = self.unlocked_state(&current).await?;
        let record = self.store.record()?;
        ensure!(
            record
                .pending
                .as_ref()
                .is_none_or(|p| p.kind == Kind::Password && !p.sent),
            "Finish the pending account action first"
        );
        let (client, request) = auth::Registration::start(new)?;
        let id = Uuid::new_v4();
        let generation = Uuid::new_v4();
        let body = json!({"operation_id":id,"generation":generation,"request":request,"expected":state.expected()});
        let mut pending = Pending::new(Kind::Password, id);
        pending.scope = Some(state.scope().clone());
        pending.identity = state.identity().cloned();
        pending.username = Some(state.username().to_owned());
        pending.generation = Some(generation);
        pending.start = Some(PrivateBytes::json(&body)?);
        self.store.update(Some(record.revision), |record| {
            record.pending = Some(pending.clone());
            Ok(())
        })?;
        let started: Started = self
            .post("/v3/auth/password/start", &body, true, 16384)
            .await?;
        started.validate(
            &state.context(),
            state.scope(),
            id,
            generation,
            &state.expected(),
        )?;
        ensure!(
            started.expires_at > now() && started.expires_at <= now() + 601,
            "Password change authorization expired"
        );
        let account = started.account.clone();
        let response = started.response.clone();
        let pin = record.pin.clone();
        let result =
            tokio::task::spawn_blocking(move || client.finish(&account, &response, pin.as_ref()))
                .await
                .context("Could not finish native password registration")??;
        let registration = RegistrationTranscript {
            account: started.account,
            scope: state.scope().clone(),
            generation,
            request,
            response: started.response,
            upload: result.upload,
        };
        let wrapper = KeyEnvelope::wrap(
            state.scope().clone(),
            generation,
            &result.export_key,
            &record.local_key()?,
        )?;
        let operation = Operation {
            format: 1,
            scope: state.scope().clone(),
            id,
            device: DeviceBinding::Existing {
                id: self.device.context("Missing account device")?,
            },
            expected: state.expected(),
            change: Change::ChangePassword {
                generation,
                registration_sha256: registration.digest()?,
                wrapper_sha256: wrapper.digest()?,
            },
        };
        let identity = record
            .bundle()?
            .context("Missing local identity")?
            .identity()?;
        let finish = json!({"authorization":started.authorization,"grant":"","signature":operation.sign(&identity)?,"operation":operation,"registration":registration,"wrapper":wrapper});
        self.save_mutation(&pending, &operation, &finish)?;
        self.store.update(None, |record| {
            record.pin = Some(result.server_pin);
            Ok(())
        })?;
        self.resume_mutation(Some(current)).await
    }
    pub(crate) async fn rewrap(&self, password: SecretString) -> anyhow::Result<()> {
        if let Some(pending) = self.store.record()?.pending {
            ensure!(
                pending.kind == Kind::Rewrap,
                "Finish the pending account action first"
            );
            return self.resume_mutation(Some(password)).await;
        }
        ensure!(
            self.store.record()?.key_verified,
            "Use an existing trusted device to repair this backup"
        );
        ensure!(
            self.restore(None).await?,
            "Restore your trusted identity before repairing this backup"
        );
        let state = self.status().await?;
        let export = self.unlock(copy(&password), &state).await?;
        ensure!(
            self.status().await?.expected() == state.expected(),
            "Account changed during password confirmation. Try again"
        );
        let record = self.store.record()?;
        let wrapper = KeyEnvelope::wrap(
            state.scope().clone(),
            state
                .expected()
                .credential_generation
                .context("Missing secure credential")?,
            &export,
            &record.local_key()?,
        )?;
        let operation = Operation {
            format: 1,
            scope: state.scope().clone(),
            id: Uuid::new_v4(),
            device: DeviceBinding::Existing {
                id: self.device.context("Missing signed-in device")?,
            },
            expected: state.expected(),
            change: Change::Rewrap {
                wrapper_sha256: wrapper.digest()?,
            },
        };
        let identity = record
            .bundle()?
            .context("Missing local identity")?
            .identity()?;
        let finish = json!({"grant":"","signature":operation.sign(&identity)?,"operation":operation,"wrapper":wrapper});
        let mut pending = Pending::new(Kind::Rewrap, operation.id);
        pending.scope = Some(state.scope().clone());
        pending.identity = Some(identity.public());
        pending.username = Some(state.username().to_owned());
        self.save_mutation(&pending, &operation, &finish)?;
        self.resume_mutation(Some(password)).await
    }
    pub(crate) async fn recovery_email(
        &self,
        email: &str,
        password: SecretString,
    ) -> anyhow::Result<()> {
        if let Some(pending) = self.store.record()?.pending {
            ensure!(
                pending.kind == Kind::Email,
                "Finish the pending account action first"
            );
            return self.resume_mutation(Some(password)).await;
        }
        let state = self.status().await?;
        let email = email.trim().to_ascii_lowercase();
        ensure!(
            (3..=254).contains(&email.len())
                && email.contains('@')
                && email.is_ascii()
                && !email.chars().any(char::is_control),
            "Enter a valid recovery email address"
        );
        let identity = state
            .identity()
            .context("Enable account backup first")?
            .clone();
        let operation = Operation {
            format: 1,
            scope: state.scope().clone(),
            id: Uuid::new_v4(),
            device: DeviceBinding::Existing {
                id: self.device.context("Missing signed-in device")?,
            },
            expected: state.expected(),
            change: Change::SetRecoveryEmail {
                email_sha256: format!("{:x}", Sha256::digest(email.as_bytes())),
            },
        };
        let finish = json!({"grant":"","operation":operation,"email":email});
        let mut pending = Pending::new(Kind::Email, operation.id);
        pending.scope = Some(state.scope().clone());
        pending.identity = Some(identity);
        pending.username = Some(state.username().to_owned());
        self.save_mutation(&pending, &operation, &finish)?;
        self.resume_mutation(Some(password)).await
    }
    fn save_mutation(
        &self,
        pending: &Pending,
        operation: &Operation,
        finish: &Value,
    ) -> anyhow::Result<()> {
        self.store.update(None, |record| {
            ensure!(
                record
                    .pending
                    .as_ref()
                    .is_none_or(|saved| saved.id == pending.id
                        && !saved.sent
                        && saved.finish.is_none()),
                "Another account action is pending"
            );
            let mut pending = pending.clone();
            pending.finish = Some(PrivateBytes::json(finish)?);
            pending.effect_digest = Some(operation.digest()?);
            record.pending = Some(pending);
            Ok(())
        })
    }
    pub(crate) async fn resume_mutation(
        &self,
        password: Option<SecretString>,
    ) -> anyhow::Result<()> {
        let pending = self
            .store
            .record()?
            .pending
            .context("No pending account action")?;
        ensure!(
            pending.recovery.is_empty(),
            "Finish the recovery sign-in first"
        );
        let path = route(pending.kind)?;
        let mut body = pending
            .finish
            .as_ref()
            .context("Re-enter your passwords to prepare the account action")?
            .value()?;
        let operation: Operation = serde_json::from_value(body["operation"].clone())?;
        ensure!(
            Some(operation.digest()?) == pending.effect_digest
                && Some(&operation.scope) == pending.scope.as_ref(),
            "Saved account effect changed"
        );
        if self.receipt(&pending).await?.is_some() {
            return self.finish_mutation(&pending).await;
        }
        let state = self.status().await?;
        if state.expected() != operation.expected {
            let completed = self.receipt(&pending).await?.is_some();
            self.finish_mutation(&pending).await?;
            ensure!(
                completed,
                "Account changed before this action completed. Review its current settings and try again"
            );
            return Ok(());
        }
        ensure!(
            self.device == Some(operation.device.id()),
            "Recover and revoke the original device before resolving its pending action"
        );
        if pending.sent {
            match self.post::<Value>(path, &body, true, 32768).await {
                Ok(_) => {
                    ensure!(
                        self.receipt(&pending).await?.is_some(),
                        "Could not confirm the account action"
                    );
                    return self.finish_mutation(&pending).await;
                }
                Err(error)
                    if error
                        .downcast_ref::<super::api::Failure>()
                        .is_some_and(|f| f.code == "unauthorized") =>
                {
                    ()
                }
                Err(error) => return Err(error),
            }
        }
        if pending.kind == Kind::Password {
            let renewal = pending
                .start
                .as_ref()
                .context("Missing password-change renewal")?
                .value()?;
            let started: Started = self
                .post("/v3/auth/password/start", &renewal, true, 16384)
                .await?;
            let registration: RegistrationTranscript =
                serde_json::from_value(body["registration"].clone())?;
            started.validate(
                &registration.account,
                &registration.scope,
                pending.id,
                registration.generation,
                &operation.expected,
            )?;
            ensure!(
                started.response == registration.response
                    && started.expires_at > now()
                    && started.expires_at <= now() + 601,
                "Password renewal changed the prepared credentials"
            );
            body["authorization"] = json!(started.authorization);
        }
        let grant = self
            .reauth(
                password.context("Re-enter your current password to finish this action")?,
                &state,
                &operation,
            )
            .await?;
        body["grant"] = json!(grant);
        self.store.update(None, |record| {
            let saved = record.pending.as_mut().context("Pending action changed")?;
            ensure!(
                saved.id == pending.id && saved.effect_digest == pending.effect_digest,
                "Pending action changed"
            );
            saved.finish = Some(PrivateBytes::json(&body)?);
            saved.sent = true;
            Ok(())
        })?;
        let _: Value = self.post(path, &body, true, 32768).await?;
        ensure!(
            self.receipt(&pending).await?.is_some(),
            "Could not confirm the account action"
        );
        self.finish_mutation(&pending).await
    }
    async fn finish_mutation(&self, pending: &Pending) -> anyhow::Result<()> {
        // Fetch authoritative current state, since the receipt is historical and
        // another device may already have advanced the account again.
        if pending.kind == Kind::Email {
            let state = self.status().await?;
            self.store.update(None, |record| {
                record.expected = Some(state.expected());
                Ok(())
            })?;
        } else {
            ensure!(
                self.restore(None).await?,
                "Account changed; its encrypted backup remains locked"
            );
        }
        self.store.update(None, |record| {
            ensure!(
                record
                    .pending
                    .as_ref()
                    .is_some_and(|saved| saved.id == pending.id),
                "Pending account action changed"
            );
            record.pending = None;
            Ok(())
        })?;
        Ok(())
    }
}
