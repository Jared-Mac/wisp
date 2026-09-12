#![allow(clippy::items_after_statements)] // Keep private response types beside their validation.
//! Bounded native-only account transport. All errors are fixed local messages;
//! neither server bodies nor request credentials are included in diagnostics.
use super::store::Store;
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
use wisp_crypto::{
    PublicIdentity,
    account_vault::{
        Scope,
        auth::{self, AccountContext, LoginContext},
        bundle::MAX_PLAINTEXT,
        envelope::{Checkpoint, Envelope, KeyEnvelope, SignedManifest},
        operation::Precondition,
    },
};
use zeroize::Zeroizing;

#[derive(Debug)]
pub(crate) struct Failure {
    pub(crate) code: &'static str,
    pub(crate) status: u16,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.code {
            "unauthorized"=>"Secure sign-in could not be completed. Check your password and try again.",
            "account_state_changed"=>"Account state changed on another device. Refresh and try again.",
            "operation_conflict"=>"An interrupted account action needs to be resolved before continuing.",
            "operation_committed"=>"This account action already completed. Checking its saved result is required.",
            "operation_superseded"=>"This sign-in attempt has ended. Continue through account recovery.",
            "account_unavailable"=>"That account name is unavailable. Choose another or sign in.",
            "device_conflict"=>"This device credential is already registered. Continue the saved sign-in.",
            "vault_history_conflict"=>"The backup history does not match this device. Existing keys were preserved.",
            "invalid_token"=>"This recovery link is invalid or expired. Request a new one.",
            "rate_limited"|"recovery_rate_limited"=>"Too many attempts. Please wait before trying again.",
            "mail_unavailable"=>"Recovery email is temporarily unavailable. Try again later.",
            "invalid_email"=>"Enter a valid email address.",
            "recovery_email_unavailable"=>"That recovery email is unavailable for this account.",
            "not_found"=>"This account action is unavailable. Update Wisp or refresh before continuing.",
            "secure_account_unavailable"=>"Secure account data is unavailable. Existing keys were preserved.",
            _=>"Could not complete the account request. Existing keys and pending actions were preserved.",
        })
    }
}
impl std::error::Error for Failure {}
fn code(value: &str) -> &'static str {
    match value {
        "unauthorized" => "unauthorized",
        "account_state_changed" => "account_state_changed",
        "operation_conflict" => "operation_conflict",
        "operation_committed" => "operation_committed",
        "operation_superseded" => "operation_superseded",
        "account_unavailable" => "account_unavailable",
        "device_conflict" => "device_conflict",
        "vault_history_conflict" => "vault_history_conflict",
        "invalid_token" => "invalid_token",
        "rate_limited" => "rate_limited",
        "recovery_rate_limited" => "recovery_rate_limited",
        "mail_unavailable" => "mail_unavailable",
        "invalid_email" => "invalid_email",
        "recovery_email_unavailable" => "recovery_email_unavailable",
        "not_found" => "not_found",
        "secure_account_unavailable" => "secure_account_unavailable",
        _ => "request_failed",
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Info {
    version: u32,
    suite: String,
    origin: String,
    network: Uuid,
    max_vault_plaintext_bytes: usize,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, tag = "mode", rename_all = "snake_case")]
pub(crate) enum Status {
    Classic {
        scope: Scope,
        username: String,
        identity: Option<PublicIdentity>,
    },
    Secure {
        scope: Scope,
        username: String,
        identity: PublicIdentity,
        credential_generation: Uuid,
        wrapper_generation: Uuid,
        vault: Checkpoint,
    },
}
impl Status {
    pub(crate) fn scope(&self) -> &Scope {
        match self {
            Self::Classic { scope, .. } | Self::Secure { scope, .. } => scope,
        }
    }
    pub(crate) fn username(&self) -> &str {
        match self {
            Self::Classic { username, .. } | Self::Secure { username, .. } => username,
        }
    }
    pub(crate) fn identity(&self) -> Option<&PublicIdentity> {
        match self {
            Self::Classic { identity, .. } => identity.as_ref(),
            Self::Secure { identity, .. } => Some(identity),
        }
    }
    pub(crate) fn expected(&self) -> Precondition {
        match self {
            Self::Classic { .. } => Precondition::default(),
            Self::Secure {
                credential_generation,
                wrapper_generation,
                vault,
                ..
            } => Precondition {
                credential_generation: Some(*credential_generation),
                wrapper_generation: Some(*wrapper_generation),
                vault: Some(vault.clone()),
            },
        }
    }
    pub(crate) fn context(&self) -> AccountContext {
        AccountContext {
            origin: self.scope().origin.clone(),
            network: self.scope().network,
            username: self.username().to_owned(),
        }
    }
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        self.scope().validate()?;
        self.context().validate()?;
        self.expected().validate()?;
        if let Some(identity) = self.identity() {
            identity.validate()?;
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct VaultResponse {
    pub(crate) state: Status,
    pub(crate) wrapper: KeyEnvelope,
    pub(crate) vault: Envelope,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProofPage {
    pub(crate) manifests: Vec<SignedManifest>,
    pub(crate) next: Checkpoint,
    pub(crate) complete: bool,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct Started {
    pub(crate) operation_id: Uuid,
    pub(crate) account: AccountContext,
    pub(crate) scope: Scope,
    pub(crate) generation: Uuid,
    pub(crate) expected: Precondition,
    pub(crate) response: String,
    pub(crate) authorization: String,
    pub(crate) expires_at: i64,
}
impl Started {
    pub(crate) fn validate(
        &self,
        account: &AccountContext,
        scope: &Scope,
        id: Uuid,
        generation: Uuid,
        expected: &Precondition,
    ) -> anyhow::Result<()> {
        self.account.validate()?;
        self.scope.validate()?;
        self.expected.validate()?;
        ensure!(
            &self.account == account
                && &self.scope == scope
                && self.operation_id == id
                && self.generation == generation
                && &self.expected == expected
                && self.authorization.len() == 43
                && self.response.len() <= auth::MAX_HANDSHAKE_WIRE,
            "Secure registration context changed"
        );
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoginStarted {
    pub(crate) context: LoginContext,
    pub(crate) response: String,
    pub(crate) expires_at: i64,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct SignedIn {
    pub(crate) device_id: Uuid,
    pub(crate) user: wisp_protocol::UserSummary,
    pub(crate) scope: Scope,
    pub(crate) username: String,
    pub(crate) identity: PublicIdentity,
    pub(crate) credential_generation: Uuid,
}
impl SignedIn {
    pub(crate) fn validate(
        &self,
        origin: &str,
        network: Uuid,
        username: &str,
        device: Uuid,
        generation: Uuid,
    ) -> anyhow::Result<()> {
        self.scope.validate()?;
        self.identity.validate()?;
        ensure!(
            self.scope.origin == origin
                && self.scope.network == network
                && self.user.id == self.scope.account
                && self.device_id == device
                && self.username == username
                && self.credential_generation == generation,
            "Secure sign-in response changed account or device"
        );
        ensure!(
            !self.user.display_name.trim().is_empty()
                && self.user.display_name.chars().count() <= 80
                && !self.user.display_name.chars().any(char::is_control),
            "Invalid account profile"
        );
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_field_names)] // Exact protocol field names.
pub(crate) struct Grant {
    pub(crate) grant: String,
    pub(crate) operation_sha256: String,
    pub(crate) expires_at: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Receipt {
    pub(crate) operation_id: Uuid,
    pub(crate) operation_sha256: String,
    pub(crate) scope: Scope,
    pub(crate) device_id: Option<Uuid>,
    pub(crate) kind: String,
    pub(crate) result: Value,
    pub(crate) committed_at: i64,
}

pub(crate) struct Api {
    client: reqwest::Client,
    pub(crate) origin: String,
    pub(crate) network: Uuid,
    pub(crate) store: Arc<Store>,
    pub(crate) device: Option<Uuid>,
    session: Option<Zeroizing<String>>,
}
impl Api {
    pub(super) fn bearer(&self) -> anyhow::Result<Zeroizing<String>> {
        self.session
            .clone()
            .context("Sign in before activating account credentials")
    }
    pub(crate) async fn connect(
        origin: &str,
        store: Arc<Store>,
        device: Option<Uuid>,
        session: Option<String>,
    ) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()?;
        let response=client.get(format!("{origin}/v3/auth/info")).send().await.map_err(|_|anyhow::anyhow!("Could not reach secure account sign-in. Check your connection and update Wisp if needed"))?;
        let info: Info = read(response, 16384).await?;
        ensure!(
            info.version == 1
                && info.suite == auth::SUITE
                && info.origin == origin
                && !info.network.is_nil()
                && info.max_vault_plaintext_bytes == MAX_PLAINTEXT,
            "This server does not support the expected secure sign-in protocol"
        );
        store.read(|record, _| {
            ensure!(record.origin == origin, "Account service changed");
            ensure!(
                record.network.is_none_or(|network| network == info.network),
                "Account network identity changed"
            );
            if let Some(scope) = &record.scope {
                ensure!(
                    scope.network == info.network,
                    "Account network identity changed"
                );
            }
            if let Some(scope) = record.pending.as_ref().and_then(|p| p.scope.as_ref()) {
                ensure!(
                    scope.network == info.network,
                    "Pending account network identity changed"
                );
            }
            Ok(())
        })?;
        if store.record()?.network.is_none() {
            store.update(None, |record| {
                ensure!(
                    record.network.is_none_or(|network| network == info.network),
                    "Account network identity changed"
                );
                record.network = Some(info.network);
                Ok(())
            })?;
        }
        Ok(Self {
            client,
            origin: origin.to_owned(),
            network: info.network,
            store,
            device,
            session: session.map(Zeroizing::new),
        })
    }
    pub(crate) async fn session(
        &mut self,
        device: &wisp_protocol::DeviceCredential,
    ) -> anyhow::Result<()> {
        let result:wisp_protocol::DeviceSession=self.post("/v1/sessions",&serde_json::json!({"device_id":device.device_id,"device_token":device.device_token,"protocol_version":wisp_protocol::PROTOCOL_VERSION}),false,16384).await?;
        ensure!(
            (32..=512).contains(&result.token.len())
                && result.device_id == device.device_id
                && result.user.id == device.user.id
                && result.protocol_version == wisp_protocol::PROTOCOL_VERSION
                && result.expires_at.timestamp() > super::sync::now(),
            "Invalid account session"
        );
        self.device = Some(device.device_id);
        self.session = Some(Zeroizing::new(result.token));
        Ok(())
    }
    pub(crate) async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        limit: usize,
    ) -> anyhow::Result<T> {
        let session = self
            .session
            .as_ref()
            .context("Sign in before accessing this account")?;
        let response = self
            .client
            .get(format!("{}{path}", self.origin))
            .bearer_auth(session.as_str())
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Could not reach the account server"))?;
        read(response, limit).await
    }
    pub(crate) async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
        authenticated: bool,
        limit: usize,
    ) -> anyhow::Result<T> {
        let mut request = self
            .client
            .post(format!("{}{path}", self.origin))
            .json(body);
        if authenticated {
            request = request.bearer_auth(
                self.session
                    .as_ref()
                    .context("Sign in before continuing")?
                    .as_str(),
            );
        }
        let response=request.send().await.map_err(|_|anyhow::anyhow!("Could not confirm the account request. Its pending state was preserved for recovery"))?;
        read(response, limit).await
    }
    pub(crate) async fn status(&self) -> anyhow::Result<Status> {
        self.status_for_recovery(false).await
    }
    pub(crate) async fn status_for_recovery(
        &self,
        migration_recovery: bool,
    ) -> anyhow::Result<Status> {
        let state: Status = self.get("/v3/accounts/vault/status", 16384).await?;
        self.validate_status(&state, migration_recovery)?;
        if matches!(state, Status::Secure { .. }) && !self.store.record()?.secure {
            self.store.update(None, |record| {
                record.scope = Some(state.scope().clone());
                record.network = Some(state.scope().network);
                record.identity = state.identity().cloned();
                record.username = Some(state.username().to_owned());
                record.secure = true;
                record.expected = Some(state.expected());
                Ok(())
            })?;
        }
        Ok(state)
    }
    pub(crate) async fn revoke(&self, device: Uuid) -> anyhow::Result<()> {
        ensure!(
            Some(device) != self.device,
            "Cannot retire the current recovery device"
        );
        let response = self
            .client
            .delete(format!("{}/v1/devices/{device}", self.origin))
            .bearer_auth(
                self.session
                    .as_ref()
                    .context("Sign in before retiring an old device")?
                    .as_str(),
            )
            .send()
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "Could not confirm the old device was retired; its journal was preserved"
                )
            })?;
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Revoked {
            ok: bool,
        }
        match read::<Revoked>(response, 4096).await {
            Ok(result) => {
                ensure!(result.ok, "Device retirement was not confirmed");
                Ok(())
            }
            Err(error)
                if error.downcast_ref::<Failure>().is_some_and(|failure| {
                    failure.status == 404 && failure.code == "not_found"
                }) =>
            {
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
    pub(crate) fn validate_status(
        &self,
        state: &Status,
        migration_recovery: bool,
    ) -> anyhow::Result<()> {
        state.validate()?;
        ensure!(
            state.scope().origin == self.origin && state.scope().network == self.network,
            "Account service or network changed"
        );
        self.store.read(|record, _| {
            if let Some(scope) = &record.scope {
                ensure!(
                    scope == state.scope(),
                    "Server changed your account identity"
                );
            }
            if let Some(pending) = &record.pending {
                ensure!(
                    pending
                        .scope
                        .as_ref()
                        .is_none_or(|scope| scope == state.scope())
                        && pending
                            .identity
                            .as_ref()
                            .is_none_or(|identity| state.identity() == Some(identity)),
                    "Account status differs from the staged identity"
                );
            }
            if let Some(username) = &record.username {
                ensure!(username == state.username(), "Sign-in account changed");
            }
            if let Some(identity) = &record.identity {
                ensure!(
                    state.identity() == Some(identity),
                    "Account encryption identity changed"
                );
            }
            ensure!(
                !record.secure
                    || matches!(state, Status::Secure { .. })
                    || (migration_recovery
                        && record
                            .pending
                            .as_ref()
                            .is_some_and(|p| p.kind == super::store::Kind::Migration)),
                "A secure account cannot return to classic sign-in"
            );
            Ok(())
        })?;
        Ok(())
    }
}
async fn read<T: DeserializeOwned>(
    mut response: reqwest::Response,
    limit: usize,
) -> anyhow::Result<T> {
    let status = response.status();
    let limit = if status.is_success() { limit } else { 8192 };
    let mut bytes = Zeroizing::new(Vec::new());
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        anyhow::anyhow!("Could not confirm the account response. Pending state was preserved")
    })? {
        ensure!(
            bytes.len().saturating_add(chunk.len()) <= limit,
            "Account response exceeded its size limit"
        );
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        let parsed = serde_json::from_slice::<Value>(&bytes).ok();
        let code = code(
            parsed
                .as_ref()
                .and_then(|value| value["code"].as_str())
                .unwrap_or("request_failed"),
        );
        return Err(Failure {
            code,
            status: status.as_u16(),
        }
        .into());
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("The server returned an invalid account response"))
}
