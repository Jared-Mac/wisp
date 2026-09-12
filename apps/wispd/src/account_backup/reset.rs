//! Password recovery operates in a separate journal, never the selected account.
use super::{
    api::{Api, Receipt, Started},
    store::{Kind, Pending, PrivateBytes},
    sync::now,
};
use anyhow::{Context, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;
use wisp_crypto::{
    SecretString,
    account_vault::{
        Scope, auth,
        operation::{RegistrationTranscript, ResetEffect},
    },
};
use zeroize::Zeroizing;

/// Only explicit paste from the user is accepted. OS deep links contain no token.
pub(crate) fn pasted_token(input: &str) -> anyhow::Result<Zeroizing<String>> {
    let input = input.trim();
    ensure!(input.len() <= 2048, "Invalid recovery link");
    let token = if input.starts_with("https://") {
        let url = url::Url::parse(input).context("Invalid recovery link")?;
        ensure!(
            url.host_str() == Some("wisp.you")
                && url.path() == "/account/reset-password"
                && url.username().is_empty()
                && url.password().is_none()
                && url.port_or_known_default() == Some(443)
                && url.query().is_none(),
            "Paste the reset link from your Wisp email"
        );
        let pairs: Vec<_> = url::form_urlencoded::parse(
            url.fragment().context("Missing recovery code")?.as_bytes(),
        )
        .collect();
        ensure!(
            pairs.len() == 1 && pairs[0].0 == "token",
            "Invalid recovery link"
        );
        pairs[0].1.to_string()
    } else {
        input.to_owned()
    };
    let token = Zeroizing::new(token);
    ensure!(token.len() == 43, "Invalid recovery code");
    let bytes = Zeroizing::new(
        URL_SAFE_NO_PAD
            .decode(token.as_bytes())
            .map_err(|_| anyhow::anyhow!("Invalid recovery code"))?,
    );
    ensure!(
        bytes.len() == 32 && URL_SAFE_NO_PAD.encode(&*bytes) == *token,
        "Invalid recovery code"
    );
    Ok(token)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Finish {
    authorization: String,
    effect: ResetEffect,
    token: String,
    registration: RegistrationTranscript,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Done {
    completed: bool,
    scope: Scope,
    credential_generation: Uuid,
    backup_locked: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResetStatus {
    scope: Scope,
    credential_generation: Uuid,
    receipt: Option<Receipt>,
}
impl Api {
    fn reset_only(&self) -> anyhow::Result<()> {
        self.store.read(|record, _| {
            ensure!(
                record.scope.is_none()
                    && record.installation.is_none()
                    && !record.secure
                    && record
                        .pending
                        .as_ref()
                        .is_none_or(|pending| pending.kind == Kind::Reset),
                "Password recovery must use its separate account journal"
            );
            Ok(())
        })
    }
    pub(crate) async fn reset_password(
        &self,
        link: &str,
        password: SecretString,
    ) -> anyhow::Result<()> {
        self.reset_only()?;
        let token = pasted_token(link)?;
        let record = self.store.record()?;
        if record
            .pending
            .as_ref()
            .is_some_and(|pending| pending.finish.is_some())
        {
            return self.resume_reset(Some(link)).await;
        }
        let (client, request) = auth::Registration::start(password)?;
        let mut pending = Pending::new(Kind::Reset, Uuid::new_v4());
        let generation = Uuid::new_v4();
        pending.generation = Some(generation);
        let body = json!({"operation_id":pending.id,"generation":generation,"request":request,"token":token.as_str()});
        pending.start = Some(PrivateBytes::json(&body)?);
        self.store.update(Some(record.revision), |record| {
            record.pending = Some(pending.clone());
            Ok(())
        })?;
        let started: Started = self
            .post("/v3/auth/reset/start", &body, false, 16384)
            .await?;
        started.account.validate()?;
        started.scope.validate()?;
        started.expected.validate()?;
        ensure!(
            started.account.origin == self.origin
                && started.account.network == self.network
                && started.scope.origin == self.origin
                && started.scope.network == self.network
                && started.operation_id == pending.id
                && started.generation == generation
                && started
                    .expected
                    .credential_generation
                    .is_some_and(|old| old != generation)
                && started.expires_at > now()
                && started.expires_at <= now() + 601,
            "Password recovery context changed or expired"
        );
        let account = started.account.clone();
        let response = started.response.clone();
        let pin = record.pin;
        let result =
            tokio::task::spawn_blocking(move || client.finish(&account, &response, pin.as_ref()))
                .await
                .context("Could not prepare native password recovery")??;
        let registration = RegistrationTranscript {
            account: started.account.clone(),
            scope: started.scope.clone(),
            generation,
            request,
            response: started.response,
            upload: result.upload,
        };
        let effect = ResetEffect {
            format: 1,
            scope: started.scope,
            id: pending.id,
            expected_generation: started
                .expected
                .credential_generation
                .context("Missing original credential generation")?,
            generation,
            registration_sha256: registration.digest()?,
        };
        let finish = Finish {
            authorization: started.authorization,
            effect,
            token: token.to_string(),
            registration,
        };
        self.store.update(None, |record| {
            let saved = record
                .pending
                .as_mut()
                .context("Pending recovery changed")?;
            ensure!(
                saved.id == pending.id && !saved.sent && saved.finish.is_none(),
                "Pending recovery changed"
            );
            saved.scope = Some(finish.effect.scope.clone());
            saved.username = Some(started.account.username);
            saved.effect_digest = Some(finish.effect.digest()?);
            saved.finish = Some(PrivateBytes::json(&finish)?);
            saved.sent = true;
            record.pin = Some(result.server_pin);
            Ok(())
        })?;
        self.submit_reset(&finish).await
    }
    async fn submit_reset(&self, finish: &Finish) -> anyhow::Result<()> {
        let done: Done = self
            .post("/v3/auth/reset/finish", finish, false, 8192)
            .await?;
        self.validate_reset_done(finish, &done)?;
        self.clear_reset(finish)
    }
    fn validate_reset_done(&self, finish: &Finish, done: &Done) -> anyhow::Result<()> {
        ensure!(
            done.completed
                && done.backup_locked
                && done.scope == finish.effect.scope
                && done.credential_generation == finish.effect.generation,
            "Password recovery result changed"
        );
        Ok(())
    }
    fn clear_reset(&self, finish: &Finish) -> anyhow::Result<()> {
        self.store.update(None, |record| {
            ensure!(
                record
                    .pending
                    .as_ref()
                    .is_some_and(|pending| pending.id == finish.effect.id
                        && pending.effect_digest.as_deref()
                            == finish.effect.digest().ok().as_deref()),
                "Password recovery journal changed"
            );
            record.pending = None;
            Ok(())
        })
    }
    pub(crate) async fn resume_reset(&self, new_link: Option<&str>) -> anyhow::Result<()> {
        self.reset_only()?;
        let pending = self
            .store
            .record()?
            .pending
            .context("No pending password recovery")?;
        let mut finish: Finish = serde_json::from_value(
            pending
                .finish
                .as_ref()
                .context("Re-enter your new password to restart recovery")?
                .value()?,
        )?;
        ensure!(
            Some(finish.effect.digest()?) == pending.effect_digest
                && Some(&finish.effect.scope) == pending.scope.as_ref(),
            "Saved recovery effect changed"
        );
        match self.submit_reset(&finish).await {
            Ok(()) => return Ok(()),
            Err(error)
                if error
                    .downcast_ref::<super::api::Failure>()
                    .is_some_and(|f| {
                        matches!(
                            f.code,
                            "invalid_token" | "unauthorized" | "account_state_changed"
                        )
                    }) =>
            {
                ()
            }
            Err(error) => return Err(error),
        }
        let token = pasted_token(
            new_link.context("Request a new recovery email, then paste its link to resume")?,
        )?;
        let status:ResetStatus=self.post("/v3/auth/reset/status",&json!({"token":token.as_str(),"operation_id":pending.id,"effect_sha256":finish.effect.digest()?}),false,32768).await?;
        ensure!(
            status.scope == finish.effect.scope && !status.credential_generation.is_nil(),
            "Recovery link belongs to another account"
        );
        if let Some(receipt) = status.receipt {
            ensure!(
                receipt.operation_id == pending.id
                    && receipt.operation_sha256 == finish.effect.digest()?
                    && receipt.scope == finish.effect.scope
                    && receipt.kind == "reset"
                    && receipt.device_id.is_none()
                    && receipt.committed_at > 0,
                "Recovery receipt changed"
            );
            let done: Done = serde_json::from_value(receipt.result)?;
            self.validate_reset_done(&finish, &done)?;
            return self.clear_reset(&finish);
        }
        if status.credential_generation != finish.effect.expected_generation {
            self.clear_reset(&finish)?;
            anyhow::bail!(
                "A newer password change superseded this recovery. Use a fresh recovery link to set another password"
            );
        }
        let mut body: Value = pending
            .start
            .as_ref()
            .context("Missing saved recovery request")?
            .value()?;
        body["token"] = json!(token.as_str());
        let started: Started = self
            .post("/v3/auth/reset/start", &body, false, 16384)
            .await?;
        started.account.validate()?;
        started.scope.validate()?;
        started.expected.validate()?;
        ensure!(
            started.scope == finish.effect.scope
                && started.account == finish.registration.account
                && started.operation_id == pending.id
                && started.generation == finish.effect.generation
                && started.expected.credential_generation
                    == Some(finish.effect.expected_generation)
                && started.response == finish.registration.response
                && started.expires_at > now()
                && started.expires_at <= now() + 601,
            "Password recovery renewal changed its prepared credentials"
        );
        finish.authorization = started.authorization;
        finish.token = token.to_string();
        self.store.update(None, |record| {
            let saved = record
                .pending
                .as_mut()
                .context("Recovery journal changed")?;
            ensure!(
                saved.id == pending.id && saved.effect_digest == pending.effect_digest,
                "Recovery journal changed"
            );
            saved.start = Some(PrivateBytes::json(&body)?);
            saved.finish = Some(PrivateBytes::json(&finish)?);
            Ok(())
        })?;
        self.submit_reset(&finish).await
    }
}
