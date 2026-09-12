#![allow(clippy::items_after_statements)] // Keep private response types beside their validation.
//! Native password proofs and durable prospective-device sign-in.
use super::{
    api::{Api, Grant, LoginStarted, SignedIn, Status},
    store::{Kind, Pending, PrivateBytes, StagedDevice},
    sync::now,
};
use age::secrecy::ExposeSecret;
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;
use wisp_crypto::{
    SecretString,
    account_vault::{
        auth::{self, AccountContext, ExportKey, Intent, LoginContext},
        operation::{DeviceBinding, Operation},
    },
};
use wisp_protocol::DeviceCredential;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginStage {
    request: Value,
    context: Option<LoginContext>,
}

fn account(api: &Api, username: &str) -> anyhow::Result<AccountContext> {
    let account = AccountContext {
        origin: api.origin.clone(),
        network: api.network,
        username: auth::canonical_username(username)?,
    };
    account.validate()?;
    Ok(account)
}
impl Api {
    /// Password proofs are computed off the async runtime; no password/export
    /// key is returned through IPC or written to the recovery journal.
    pub(crate) async fn unlock(
        &self,
        password: SecretString,
        state: &Status,
    ) -> anyhow::Result<ExportKey> {
        self.validate_status(state, false)?;
        let generation = state
            .expected()
            .credential_generation
            .context("Enable encrypted account backup first")?;
        let device = self
            .device
            .context("Sign in before confirming this action")?;
        let id = Uuid::new_v4();
        let (client, request) = auth::Login::start(password)?;
        let started: LoginStarted = self
            .post(
                "/v3/auth/unlock/start",
                &json!({"attempt":id,"request":request}),
                true,
                16384,
            )
            .await?;
        let expected = LoginContext {
            account: state.context(),
            attempt: id,
            credential_generation: generation,
            intent: Intent::Unlock { device },
        };
        ensure!(
            started.context == expected
                && started.expires_at > now()
                && started.expires_at <= now() + 181,
            "Secure password proof context changed or expired"
        );
        let pin = self.store.record()?.pin;
        let result = tokio::task::spawn_blocking(move || {
            client.finish(&expected, &started.response, pin.as_ref())
        })
        .await
        .context("Could not finish native password proof")??;
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Unlocked {
            confirmed: bool,
            credential_generation: Uuid,
        }
        let done: Unlocked = self
            .post(
                "/v3/auth/unlock/finish",
                &json!({"attempt":id,"finalization":result.finalization}),
                true,
                4096,
            )
            .await?;
        ensure!(
            done.confirmed && done.credential_generation == generation,
            "Secure unlock response changed"
        );
        self.store.update(None, |record| {
            record.pin = Some(result.server_pin);
            record.secure = true;
            Ok(())
        })?;
        Ok(result.export_key)
    }
    pub(crate) async fn reauth(
        &self,
        password: SecretString,
        state: &Status,
        operation: &Operation,
    ) -> anyhow::Result<String> {
        self.validate_status(state, false)?;
        let generation = state
            .expected()
            .credential_generation
            .context("Enable encrypted backup first")?;
        let device = self
            .device
            .context("Sign in before confirming this action")?;
        ensure!(
            &operation.scope == state.scope()
                && operation.expected == state.expected()
                && operation.device == DeviceBinding::Existing { id: device },
            "Account action changed before password confirmation"
        );
        let id = Uuid::new_v4();
        let (client, request) = auth::Login::start(password)?;
        let started: LoginStarted = self
            .post(
                "/v3/auth/reauth/start",
                &json!({"attempt":id,"request":request,"operation":operation}),
                true,
                16384,
            )
            .await?;
        let expected = LoginContext {
            account: state.context(),
            attempt: id,
            credential_generation: generation,
            intent: Intent::Reauthenticate {
                device,
                operation_sha256: operation.digest()?,
            },
        };
        ensure!(
            started.context == expected
                && started.expires_at > now()
                && started.expires_at <= now() + 181,
            "Secure password proof context changed or expired"
        );
        let pin = self.store.record()?.pin;
        let result = tokio::task::spawn_blocking(move || {
            client.finish(&expected, &started.response, pin.as_ref())
        })
        .await
        .context("Could not finish native password proof")??;
        let grant: Grant = self
            .post(
                "/v3/auth/reauth/finish",
                &json!({"attempt":id,"finalization":result.finalization}),
                true,
                8192,
            )
            .await?;
        ensure!(
            grant.operation_sha256 == operation.digest()?
                && grant.grant.len() == 43
                && grant.expires_at > now()
                && grant.expires_at <= now() + 121,
            "Account authorization changed or expired"
        );
        self.store.update(None, |record| {
            record.pin = Some(result.server_pin);
            record.secure = true;
            Ok(())
        })?;
        Ok(grant.grant)
    }
    fn login_pending(&self, id: Uuid) -> anyhow::Result<Pending> {
        let root = self
            .store
            .record()?
            .pending
            .context("Missing staged sign-in")?;
        if root.id == id {
            return Ok(root);
        }
        root.recovery
            .into_iter()
            .find(|pending| pending.id == id)
            .context("Missing recovery sign-in")
    }
    fn update_login(
        &self,
        id: Uuid,
        change: impl FnOnce(&mut Pending) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.store.update(None, |record| {
            let root = record.pending.as_mut().context("Missing staged sign-in")?;
            let pending = if root.id == id {
                root
            } else {
                root.recovery
                    .iter_mut()
                    .find(|p| p.id == id)
                    .context("Recovery sign-in changed")?
            };
            change(pending)
        })
    }
    /// When a prior effect is unresolved, new sign-in is nested inside it. Every
    /// possibly activated old credential survives until terminal evidence.
    #[allow(clippy::too_many_lines)] // Keep durable transition ordering reviewable as one operation.
    pub(crate) async fn login(
        &mut self,
        username: &str,
        password: SecretString,
        device_name: &str,
    ) -> anyhow::Result<bool> {
        let context = account(self, username)?;
        ensure!(
            !device_name.trim().is_empty()
                && device_name.trim() == device_name
                && device_name.chars().count() <= 80
                && !device_name.chars().any(char::is_control),
            "Enter a valid device name"
        );
        let record = self.store.record()?;
        ensure!(
            record
                .username
                .as_ref()
                .is_none_or(|name| name == &context.username),
            "This device holds a different account. Its keys were preserved"
        );
        if let Some(pending) = &record.pending {
            ensure!(
                pending.kind != Kind::Reset,
                "Finish the separate password recovery action first"
            );
            ensure!(
                pending
                    .username
                    .as_ref()
                    .is_none_or(|name| name == &context.username),
                "Recover the originally staged account"
            );
        }
        let (client, request) = auth::Login::start(password)?;
        let device = StagedDevice::generate()?;
        let native = device.native()?;
        let binding = native.binding();
        let DeviceBinding::Prospective { token_sha256, .. } = binding else {
            unreachable!()
        };
        let mut pending = Pending::new(Kind::Login, Uuid::new_v4());
        let id = pending.id;
        let body = json!({"attempt":id,"username":context.username,"device":native.binding(),"device_name":device_name,"request":request});
        pending.username = Some(context.username.clone());
        pending.scope = record.scope.clone().or_else(|| {
            record
                .pending
                .as_ref()
                .and_then(|pending| pending.scope.clone())
        });
        pending.identity = record.identity.clone().or_else(|| {
            record
                .pending
                .as_ref()
                .and_then(|pending| pending.identity.clone())
        });
        pending.device = Some(device);
        pending.start = Some(PrivateBytes::json(&LoginStage {
            request: body.clone(),
            context: None,
        })?);
        self.store.update(Some(record.revision), |record| {
            record.username = Some(context.username.clone());
            if record.pending.as_ref().is_some_and(|root| {
                root.kind == Kind::Login
                    && !root.sent
                    && root.finish.is_none()
                    && root.recovery.is_empty()
            }) {
                record.pending = None;
            }
            if let Some(root) = record.pending.as_mut() {
                root.recovery.retain(|old| old.sent || old.finish.is_some());
                root.recovery.push(pending);
            } else {
                record.pending = Some(pending);
            }
            Ok(())
        })?;
        let started: LoginStarted = self
            .post("/v3/auth/login/start", &body, false, 16384)
            .await?;
        let expected = LoginContext {
            account: context,
            attempt: id,
            credential_generation: started.context.credential_generation,
            intent: Intent::SignIn {
                device: native.id(),
                token_sha256,
            },
        };
        ensure!(
            started.context == expected
                && started.expires_at > now()
                && started.expires_at <= now() + 181,
            "Secure sign-in context changed or expired"
        );
        let pin = record.pin;
        let result = tokio::task::spawn_blocking(move || {
            client.finish(&expected, &started.response, pin.as_ref())
        })
        .await
        .context("Could not finish native sign-in")??;
        let generation = started.context.credential_generation;
        let finish = json!({"attempt":id,"finalization":result.finalization});
        self.update_login(id, |pending| {
            pending.generation = Some(generation);
            pending.start = Some(PrivateBytes::json(&LoginStage {
                request: body,
                context: Some(started.context),
            })?);
            pending.finish = Some(PrivateBytes::json(&finish)?);
            pending.sent = true;
            Ok(())
        })?;
        self.store.update(None, |record| {
            record.pin = Some(result.server_pin);
            record.secure = true;
            Ok(())
        })?;
        let signed: SignedIn = self
            .post("/v3/auth/login/finish", &finish, false, 16384)
            .await?;
        self.accept_login(id, &signed).await?;
        let unlocked = self.restore(Some((&result.export_key, generation))).await?;
        self.end_recovered_logins(id).await?;
        Ok(unlocked)
    }
    pub(super) async fn accept_login(&mut self, id: Uuid, signed: &SignedIn) -> anyhow::Result<()> {
        let pending = self.login_pending(id)?;
        let device = pending
            .device
            .as_ref()
            .context("Missing staged device")?
            .native()?;
        signed.validate(
            &self.origin,
            self.network,
            pending
                .username
                .as_deref()
                .context("Missing sign-in account")?,
            device.id(),
            pending.generation.context("Missing sign-in generation")?,
        )?;
        ensure!(
            pending
                .scope
                .as_ref()
                .is_none_or(|scope| scope == &signed.scope)
                && pending
                    .identity
                    .as_ref()
                    .is_none_or(|identity| identity == &signed.identity),
            "Sign-in returned a different account identity"
        );
        let credential = DeviceCredential {
            device_id: device.id(),
            device_token: device.token().expose_secret().to_owned(),
            user: signed.user.clone(),
        };
        // Save before even probing the session. A lost/failed session response is
        // not proof that login did not commit.
        self.store.update(None, |record| {
            record.scope = Some(signed.scope.clone());
            record.network = Some(signed.scope.network);
            record.identity = Some(signed.identity.clone());
            record.username = Some(signed.username.clone());
            record.secure = true;
            record.installation = Some(PrivateBytes::json(&credential)?);
            record.installation_pending = true;
            record.installation_ready = false;
            Ok(())
        })?;
        self.update_login(id, |pending| {
            pending.scope = Some(signed.scope.clone());
            pending.identity = Some(signed.identity.clone());
            Ok(())
        })?;
        self.session(&credential).await
    }
    pub(crate) async fn resume_login(&mut self) -> anyhow::Result<bool> {
        let pending = self
            .store
            .record()?
            .pending
            .context("No interrupted sign-in to resume")?;
        let active = pending.recovery.last().unwrap_or(&pending);
        ensure!(
            active.kind == Kind::Login,
            "Recover the pending account action with secure sign-in"
        );
        let id = active.id;
        let body = active
            .finish
            .as_ref()
            .context("Re-enter your password to restart the interrupted sign-in")?
            .value()?;
        let signed: SignedIn = self
            .post("/v3/auth/login/finish", &body, false, 16384)
            .await?;
        self.accept_login(id, &signed).await?;
        let unlocked = self.restore(None).await?;
        self.end_recovered_logins(id).await?;
        Ok(unlocked)
    }
    async fn end_recovered_logins(&self, active: Uuid) -> anyhow::Result<()> {
        let root = self
            .store
            .record()?
            .pending
            .context("Missing recovery journal")?;
        let scope = self
            .store
            .record()?
            .scope
            .context("Missing recovered account")?;
        for old in std::iter::once(&root).chain(root.recovery.iter()) {
            if old.id == active || old.kind != Kind::Login {
                continue;
            }
            let device = old
                .device
                .as_ref()
                .context("Missing old sign-in credential")?
                .native()?;
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            #[allow(clippy::struct_field_names)] // Exact protocol field names.
            struct Terminated {
                terminated: bool,
                attempt: Uuid,
                device_id: Uuid,
                scope: wisp_crypto::account_vault::Scope,
                revoked: bool,
            }
            let result: Terminated = self
                .post(
                    "/v3/auth/login/terminate",
                    &json!({"attempt":old.id,"device":device.binding()}),
                    true,
                    8192,
                )
                .await?;
            ensure!(
                result.terminated
                    && result.attempt == old.id
                    && result.device_id == device.id()
                    && result.scope == scope,
                "Old sign-in termination was not confirmed"
            );
            let _ = result.revoked; // The bound terminated verdict, not this field, retires the attempt.
        }
        self.store.update(None, |record| {
            let current = record
                .pending
                .as_mut()
                .context("Recovery journal changed")?;
            ensure!(current.id == root.id, "Recovery journal changed");
            ensure!(
                current
                    .recovery
                    .iter()
                    .map(|pending| pending.id)
                    .eq(root.recovery.iter().map(|pending| pending.id)),
                "Another recovery sign-in started; its staged credential was preserved"
            );
            if root.kind == Kind::Login {
                record.pending = None;
                record.installation_ready = true;
            } else {
                // The selected recovery credential is durably retained separately.
                current
                    .recovery
                    .retain(|pending| pending.kind != Kind::Login);
            }
            Ok(())
        })?;
        Ok(())
    }
    pub(crate) fn installation(&self) -> anyhow::Result<DeviceCredential> {
        serde_json::from_value(
            self.store
                .record()?
                .installation
                .context("No restored account is ready to install")?
                .value()?,
        )
        .context("Invalid private account installation")
    }
    pub(crate) fn cancel_prepared(&self) -> anyhow::Result<()> {
        self.store.update(None, |record| {
            let pending = record
                .pending
                .as_ref()
                .context("No prepared account action")?;
            ensure!(
                !pending.sent && pending.recovery.is_empty(),
                "This action may have reached the server. Resume or recover it before clearing"
            );
            record.pending = None;
            if record.scope.is_none() && !record.secure && record.installation.is_none() {
                record.username = None;
            }
            Ok(())
        })
    }
}
