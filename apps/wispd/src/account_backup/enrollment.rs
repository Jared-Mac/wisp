use super::{
    api::{Api, SignedIn, Started, Status},
    store::{Kind, Pending, PrivateBytes, StagedDevice},
    sync::now,
};
use age::secrecy::ExposeSecret;
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;
use wisp_crypto::{
    Identity, SecretString,
    account_vault::{
        Scope,
        auth::{self, AccountContext},
        bundle::{Bundle, NameCheckpoint, TrustState},
        envelope::{Envelope, KeyEnvelope, VaultKey},
        operation::{
            Change, DeviceBinding, Operation, Precondition, RegistrationTranscript, SignupMetadata,
            SignupStartBinding,
        },
    },
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stage {
    body: Value,
    binding: Option<SignupStartBinding>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Finish {
    authorization: String,
    operation: Operation,
    signature: String,
    registration: RegistrationTranscript,
    wrapper: KeyEnvelope,
    vault: Envelope,
}
fn context(scope: &Scope, username: &str) -> AccountContext {
    AccountContext {
        origin: scope.origin.clone(),
        network: scope.network,
        username: username.to_owned(),
    }
}
impl Api {
    #[allow(clippy::too_many_lines)] // Keep durable transition ordering reviewable as one operation.
    pub(crate) async fn signup(
        &mut self,
        username: &str,
        display_name: &str,
        device_name: &str,
        password: SecretString,
    ) -> anyhow::Result<()> {
        let username = auth::canonical_username(username)?;
        let record = self.store.record()?;
        let (mut pending, previous) = if let Some(pending) = record.pending.clone() {
            ensure!(
                pending.kind == Kind::Signup && pending.recovery.is_empty(),
                "Resolve the pending account action first"
            );
            ensure!(
                pending.username.as_deref() == Some(username.as_str()),
                "Resume the originally staged account"
            );
            if pending.finish.is_some() {
                return self.resume_enrollment(None).await;
            }
            let prior = pending
                .start
                .as_ref()
                .map(|stage| -> anyhow::Result<Stage> {
                    Ok(serde_json::from_value(stage.value()?)?)
                })
                .transpose()?;
            (pending, prior.and_then(|stage| stage.binding))
        } else {
            ensure!(
                !record.secure && record.scope.is_none() && record.identity.is_none(),
                "This device already holds an account. Its keys were preserved"
            );
            let identity = Identity::generate()?;
            let scope = Scope {
                origin: self.origin.clone(),
                network: self.network,
                account: Uuid::new_v4(),
            };
            let mut trust = TrustState::default();
            trust.pins.insert(scope.account, identity.public());
            trust.names.insert(
                scope.account,
                NameCheckpoint {
                    display_name: display_name.to_owned(),
                    revision: 0,
                },
            );
            let bundle = Bundle::new(scope.clone(), identity.recovery_key()?, None, trust)?;
            let key = VaultKey::generate()?;
            let mut pending = Pending::new(Kind::Signup, Uuid::new_v4());
            pending.scope = Some(scope);
            pending.identity = Some(identity.public());
            pending.username = Some(username.clone());
            pending.generation = Some(Uuid::new_v4());
            pending.device = Some(StagedDevice::generate()?);
            pending.key = Some(PrivateBytes::new(key.save_private().as_slice()));
            pending.bundle = Some(PrivateBytes::new(&bundle.encode_private()?));
            pending.snapshot_dirty = Some(record.dirty);
            (pending, None)
        };
        let scope = pending.scope.clone().context("Missing staged account")?;
        let identity = pending
            .identity
            .as_ref()
            .context("Missing staged identity")?;
        let bundle = Bundle::decode_private(
            &pending
                .bundle
                .as_ref()
                .context("Missing staged backup")?
                .decode(wisp_crypto::account_vault::bundle::MAX_PLAINTEXT)?,
            &scope,
            identity,
        )?;
        let key = VaultKey::restore_private(
            &pending
                .key
                .as_ref()
                .context("Missing staged backup key")?
                .decode(32)?,
        )?;
        let generation = pending.generation.context("Missing staged generation")?;
        let (client, request) = auth::Registration::start(password)?;
        let signup = SignupMetadata {
            username: username.clone(),
            display_name: display_name.to_owned(),
            device_name: device_name.to_owned(),
        };
        if let Some(previous) = &previous {
            ensure!(
                previous.signup == signup,
                "Resume the original account and device names"
            );
        }
        let binding = SignupStartBinding {
            format: 1,
            scope: scope.clone(),
            id: pending.id,
            generation,
            signup: signup.clone(),
            device: pending
                .device
                .as_ref()
                .context("Missing staged device")?
                .native()?
                .binding(),
            identity: identity.clone(),
            request_sha256: auth::registration_request_digest(&request)?,
            previous_binding_sha256: previous.map(|binding| binding.digest()).transpose()?,
        };
        let body = json!({"operation_id":binding.id,"scope":scope,"generation":generation,"username":username,"display_name":display_name,
            "device":binding.device,"device_name":device_name,"request":request,"identity":identity,"signature":binding.sign(&bundle.identity()?)?,"previous_binding_sha256":binding.previous_binding_sha256});
        pending.start = Some(PrivateBytes::json(&Stage {
            body: body.clone(),
            binding: Some(binding),
        })?);
        self.store.update(Some(record.revision), |record| {
            record.username = Some(username.clone());
            record.pending = Some(pending.clone());
            Ok(())
        })?;
        let started: Started = self
            .post("/v3/auth/register/start", &body, false, 16384)
            .await?;
        let account = context(&scope, &username);
        started.validate(
            &account,
            &scope,
            pending.id,
            generation,
            &Precondition::default(),
        )?;
        ensure!(
            started.expires_at > now() && started.expires_at <= now() + 601,
            "Registration authorization expired"
        );
        let response = started.response.clone();
        let pin = record.pin.clone();
        let result =
            tokio::task::spawn_blocking(move || client.finish(&account, &response, pin.as_ref()))
                .await
                .context("Could not complete native registration")??;
        let transcript = RegistrationTranscript {
            account: started.account.clone(),
            scope: scope.clone(),
            generation,
            request,
            response: started.response.clone(),
            upload: result.upload,
        };
        let wrapper = KeyEnvelope::wrap(scope.clone(), generation, &result.export_key, &key)?;
        let vault = Envelope::seal(&bundle, &key, None)?;
        let operation = Operation {
            format: 1,
            scope,
            id: pending.id,
            device: pending
                .device
                .as_ref()
                .context("Missing staged device")?
                .native()?
                .binding(),
            expected: Precondition::default(),
            change: Change::Enroll {
                generation,
                registration_sha256: transcript.digest()?,
                wrapper_sha256: wrapper.digest()?,
                vault: vault.manifest.checkpoint()?,
                signup: Some(signup),
            },
        };
        let finish = Finish {
            authorization: started.authorization,
            signature: operation.sign(&bundle.identity()?)?,
            operation,
            registration: transcript,
            wrapper,
            vault,
        };
        self.stage_enrollment(&pending, &finish, result.server_pin)?;
        self.submit_enrollment(&pending, &finish).await
    }
    #[allow(clippy::too_many_lines)] // Keep durable transition ordering reviewable as one operation.
    pub(crate) async fn migrate(
        &mut self,
        legacy: SecretString,
        password: SecretString,
    ) -> anyhow::Result<()> {
        ensure!(
            legacy.expose_secret() != password.expose_secret(),
            "Choose a new password different from your original account password"
        );
        let record = self.store.record()?;
        if let Some(pending) = &record.pending {
            ensure!(
                pending.kind == Kind::Migration && pending.recovery.is_empty(),
                "Resolve the pending account action first"
            );
            if pending.finish.is_some() {
                return self.resume_enrollment(Some(legacy)).await;
            }
            ensure!(!pending.sent, "Recover the pending migration first");
        }
        let state = self.status().await?;
        ensure!(
            matches!(state, Status::Classic { .. }),
            "This account already uses secure sign-in. Unlock or sync its existing backup"
        );
        let bundle = record
            .bundle()?
            .context("Restore your existing encryption identity before enabling account backup")?;
        ensure!(
            state.scope() == &bundle.scope
                && state.identity() == Some(&bundle.identity()?.public()),
            "The server does not match this device's existing encryption identity"
        );
        let device = self.device.context("Sign in before enabling backup")?;
        let generation = Uuid::new_v4();
        let (client, request) = auth::Registration::start(password)?;
        let mut pending = Pending::new(Kind::Migration, Uuid::new_v4());
        pending.scope = Some(bundle.scope.clone());
        pending.identity = Some(bundle.identity()?.public());
        pending.username = Some(state.username().to_owned());
        pending.generation = Some(generation);
        pending.key = record.key.clone();
        pending.bundle = Some(PrivateBytes::new(&bundle.encode_private()?));
        pending.snapshot_dirty = Some(record.dirty);
        let mut body = json!({"operation_id":pending.id,"generation":generation,"request":request});
        pending.start = Some(PrivateBytes::json(&Stage {
            body: body.clone(),
            binding: None,
        })?);
        self.store.update(Some(record.revision), |record| {
            record.username = Some(state.username().to_owned());
            record.pending = Some(pending.clone());
            Ok(())
        })?;
        body["legacy_password"] = json!(legacy.expose_secret());
        let response = self
            .post::<Started>("/v3/auth/migrate/start", &body, true, 16384)
            .await;
        // Minimize immutable JSON password copies; never include them in errors/journals.
        if let Some(Value::String(password)) = body.get_mut("legacy_password") {
            use zeroize::Zeroize;
            password.zeroize();
        }
        let started = response?;
        started.validate(
            &state.context(),
            &bundle.scope,
            pending.id,
            generation,
            &Precondition::default(),
        )?;
        ensure!(
            started.expires_at > now() && started.expires_at <= now() + 601,
            "Migration authorization expired"
        );
        let account = started.account.clone();
        let response = started.response.clone();
        let pin = record.pin.clone();
        let result =
            tokio::task::spawn_blocking(move || client.finish(&account, &response, pin.as_ref()))
                .await
                .context("Could not complete native migration")??;
        let registration = RegistrationTranscript {
            account: started.account,
            scope: bundle.scope.clone(),
            generation,
            request,
            response: started.response,
            upload: result.upload,
        };
        let wrapper = KeyEnvelope::wrap(
            bundle.scope.clone(),
            generation,
            &result.export_key,
            &record.local_key()?,
        )?;
        let vault = Envelope::seal(&bundle, &record.local_key()?, None)?;
        let operation = Operation {
            format: 1,
            scope: bundle.scope.clone(),
            id: pending.id,
            device: DeviceBinding::Existing { id: device },
            expected: Precondition::default(),
            change: Change::Enroll {
                generation,
                registration_sha256: registration.digest()?,
                wrapper_sha256: wrapper.digest()?,
                vault: vault.manifest.checkpoint()?,
                signup: None,
            },
        };
        let finish = Finish {
            authorization: started.authorization,
            signature: operation.sign(&bundle.identity()?)?,
            operation,
            registration,
            wrapper,
            vault,
        };
        self.stage_enrollment(&pending, &finish, result.server_pin)?;
        self.submit_enrollment(&pending, &finish).await
    }
    fn stage_enrollment(
        &self,
        pending: &Pending,
        finish: &Finish,
        pin: auth::ServerPin,
    ) -> anyhow::Result<()> {
        self.store.update(None, |record| {
            let saved = record
                .pending
                .as_mut()
                .context("Pending enrollment changed")?;
            ensure!(
                saved.id == pending.id && saved.finish.is_none() && !saved.sent,
                "Pending enrollment changed"
            );
            saved.finish = Some(PrivateBytes::json(finish)?);
            saved.effect_digest = Some(finish.operation.digest()?);
            saved.sent = true;
            record.pin = Some(pin);
            record.secure = true;
            Ok(())
        })
    }
    async fn submit_enrollment(
        &mut self,
        pending: &Pending,
        finish: &Finish,
    ) -> anyhow::Result<()> {
        let path = if pending.kind == Kind::Signup {
            "/v3/auth/register/finish"
        } else {
            "/v3/auth/migrate/finish"
        };
        let signed: SignedIn = self
            .post(path, finish, pending.kind == Kind::Migration, 16384)
            .await?;
        let scope = pending
            .scope
            .as_ref()
            .context("Missing enrollment account")?;
        let generation = pending
            .generation
            .context("Missing enrollment generation")?;
        signed.validate(
            &self.origin,
            self.network,
            pending
                .username
                .as_deref()
                .context("Missing enrollment username")?,
            finish.operation.device.id(),
            generation,
        )?;
        ensure!(
            &signed.scope == scope && Some(&signed.identity) == pending.identity.as_ref(),
            "Enrollment response changed account identity"
        );
        let key = VaultKey::restore_private(
            &pending
                .key
                .as_ref()
                .context("Missing staged key")?
                .decode(32)?,
        )?;
        let checkpoint = finish.vault.manifest.checkpoint()?;
        let bundle = finish
            .vault
            .open(&key, scope, &signed.identity, &checkpoint)?;
        self.store.update(None, |record| {
            ensure!(
                record.pending.as_ref().is_some_and(|p| p.id == pending.id),
                "Enrollment journal changed"
            );
            // During migration the live trust record may advance. Preserve it;
            // its extra dirty generation will be uploaded in the next sync.
            if record.bundle()?.is_none() {
                record.set_bundle(&bundle)?;
            }
            record.key = Some(PrivateBytes::new(key.save_private().as_slice()));
            record.key_verified = true;
            record.checkpoint = Some(checkpoint.clone());
            record.expected = Some(Precondition {
                credential_generation: Some(generation),
                wrapper_generation: Some(generation),
                vault: Some(checkpoint.clone()),
            });
            record.synced_dirty = pending
                .snapshot_dirty
                .context("Missing enrollment snapshot generation")?;
            record.secure = true;
            record.last_sync = Some(now());
            Ok(())
        })?;
        if pending.kind == Kind::Signup {
            self.accept_login(pending.id, &signed).await?;
        }
        ensure!(
            self.restore(None).await?,
            "The committed encrypted backup needs recovery"
        );
        let saved = self
            .store
            .record()?
            .pending
            .context("Missing enrollment receipt binding")?;
        ensure!(
            self.receipt(&saved).await?.is_some(),
            "Could not confirm the committed enrollment"
        );
        self.store.update(None, |record| {
            ensure!(
                record.pending.as_ref().is_some_and(|p| p.id == pending.id),
                "Enrollment journal changed"
            );
            record.pending = None;
            if record.installation.is_some() {
                record.installation_ready = true;
            }
            Ok(())
        })?;
        Ok(())
    }
    pub(crate) async fn resume_enrollment(
        &mut self,
        legacy: Option<SecretString>,
    ) -> anyhow::Result<()> {
        let pending = self
            .store
            .record()?
            .pending
            .context("No interrupted enrollment")?;
        ensure!(
            matches!(pending.kind, Kind::Signup | Kind::Migration) && pending.recovery.is_empty(),
            "Finish recovery sign-in before resuming enrollment"
        );
        let mut finish: Finish = serde_json::from_value(
            pending
                .finish
                .as_ref()
                .context("Re-enter your passwords to prepare enrollment")?
                .value()?,
        )?;
        ensure!(
            Some(finish.operation.digest()?) == pending.effect_digest,
            "Saved enrollment effect changed"
        );
        if pending.sent {
            match self.submit_enrollment(&pending, &finish).await {
                Ok(()) => return Ok(()),
                Err(error)
                    if error
                        .downcast_ref::<super::api::Failure>()
                        .is_some_and(|f| f.code == "unauthorized") => {}
                Err(error) => return Err(error),
            }
        }
        let stage: Stage = serde_json::from_value(
            pending
                .start
                .as_ref()
                .context("Missing enrollment renewal")?
                .value()?,
        )?;
        let mut body = stage.body;
        let path = if pending.kind == Kind::Signup {
            "/v3/auth/register/start"
        } else {
            body["legacy_password"] = json!(
                legacy
                    .as_ref()
                    .context("Re-enter your original password to renew this migration")?
                    .expose_secret()
            );
            "/v3/auth/migrate/start"
        };
        let response = self
            .post::<Started>(path, &body, pending.kind == Kind::Migration, 16384)
            .await;
        if let Some(Value::String(password)) = body.get_mut("legacy_password") {
            use zeroize::Zeroize;
            password.zeroize();
        }
        let started = response?;
        started.validate(
            &finish.registration.account,
            &finish.operation.scope,
            pending.id,
            finish.registration.generation,
            &finish.operation.expected,
        )?;
        ensure!(
            started.response == finish.registration.response
                && started.expires_at > now()
                && started.expires_at <= now() + 601,
            "Enrollment renewal changed its original effect"
        );
        finish.authorization = started.authorization;
        self.store.update(None, |record| {
            let saved = record
                .pending
                .as_mut()
                .context("Enrollment journal changed")?;
            ensure!(
                saved.id == pending.id && saved.effect_digest == pending.effect_digest,
                "Enrollment journal changed"
            );
            saved.finish = Some(PrivateBytes::json(&finish)?);
            saved.sent = true;
            Ok(())
        })?;
        self.submit_enrollment(&pending, &finish).await
    }
    /// The recovery transaction already made the old Existing-device effect
    /// terminal. Retain its exact prepared credentials/key/ciphertext, changing
    /// only the operation UUID and authenticated existing device.
    pub(super) fn rebind_migration(&self, original: Uuid) -> anyhow::Result<()> {
        self.store.update(None, |record| {
            let saved = record.pending.as_ref().context("Missing migration journal")?;
            ensure!(saved.id == original && saved.kind == Kind::Migration && saved.recovery.is_empty(), "Resolve every recovery credential before rebinding migration");
            let mut finish: Finish = serde_json::from_value(saved.finish.as_ref().context("Missing prepared migration")?.value()?)?;
            let device = self.device.context("Missing recovered device")?;
            ensure!(finish.operation.device.id() != device, "Migration recovery must replace the revoked device");
            let mut next = saved.clone(); next.id = Uuid::new_v4(); next.sent = false;
            finish.operation.id = next.id; finish.operation.device = DeviceBinding::Existing { id: device };
            finish.signature = finish.operation.sign(&record.bundle()?.context("Missing original identity")?.identity()?)?;
            finish.authorization.clear();
            next.effect_digest = Some(finish.operation.digest()?);
            next.finish = Some(PrivateBytes::json(&finish)?);
            next.start = Some(PrivateBytes::json(&Stage { binding: None, body: json!({"operation_id":next.id,"generation":finish.registration.generation,"request":finish.registration.request}) })?);
            record.pending = Some(next); Ok(())
        })
    }
}
