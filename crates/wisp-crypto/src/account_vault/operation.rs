//! Exact, domain-bound authorization for account mutations. Session tokens are
//! not sufficient to replace encrypted backups or secure account credentials.
use super::{Scope, auth::AccountContext, envelope::Checkpoint, is_digest};
use crate::{Identity, PublicIdentity};
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const DOMAIN: &str = "wisp-account-operation-v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeviceBinding {
    Existing { id: Uuid },
    Prospective { id: Uuid, token_sha256: String },
}
impl DeviceBinding {
    #[must_use]
    pub fn id(&self) -> Uuid {
        match self {
            Self::Existing { id } | Self::Prospective { id, .. } => *id,
        }
    }
    fn validate(&self) -> anyhow::Result<()> {
        ensure!(!self.id().is_nil(), "Invalid account operation device");
        if let Self::Prospective { token_sha256, .. } = self {
            ensure!(
                is_digest(token_sha256),
                "Invalid prospective device binding"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Precondition {
    pub credential_generation: Option<Uuid>,
    pub wrapper_generation: Option<Uuid>,
    pub vault: Option<Checkpoint>,
}
impl Precondition {
    pub fn validate(&self) -> anyhow::Result<()> {
        match (
            self.credential_generation,
            self.wrapper_generation,
            &self.vault,
        ) {
            (None, None, None) => Ok(()),
            (Some(credential), Some(wrapper), Some(vault)) => {
                ensure!(
                    !credential.is_nil() && !wrapper.is_nil(),
                    "Invalid account credential state"
                );
                vault.validate()
            }
            _ => anyhow::bail!("Incomplete account credential state"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    Enroll {
        generation: Uuid,
        registration_sha256: String,
        wrapper_sha256: String,
        vault: Checkpoint,
        /// Present for new accounts; absent when enabling an existing account.
        signup: Option<SignupMetadata>,
    },
    ChangePassword {
        generation: Uuid,
        registration_sha256: String,
        wrapper_sha256: String,
    },
    Rewrap {
        wrapper_sha256: String,
    },
    StoreVault {
        vault: Checkpoint,
    },
    SetRecoveryEmail {
        email_sha256: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub format: u32,
    pub scope: Scope,
    pub id: Uuid,
    pub device: DeviceBinding,
    pub expected: Precondition,
    pub change: Change,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SignupMetadata {
    pub username: String,
    pub display_name: String,
    pub device_name: String,
}

impl SignupMetadata {
    fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            super::auth::canonical_username(&self.username)? == self.username,
            "Invalid signup username"
        );
        for value in [&self.display_name, &self.device_name] {
            ensure!(
                value.trim() == value
                    && !value.is_empty()
                    && value.chars().count() <= 80
                    && !value.chars().any(char::is_control),
                "Invalid signup name"
            );
        }
        Ok(())
    }
}

/// Authorizes a pending signup before a password file/vault effect exists.
/// Replacement is a compare-and-swap against the entire previous binding.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SignupStartBinding {
    pub format: u32,
    pub scope: Scope,
    pub id: Uuid,
    pub generation: Uuid,
    pub signup: SignupMetadata,
    pub device: DeviceBinding,
    pub identity: PublicIdentity,
    pub request_sha256: String,
    pub previous_binding_sha256: Option<String>,
}

impl SignupStartBinding {
    pub fn validate(&self) -> anyhow::Result<()> {
        self.scope.validate()?;
        self.signup.validate()?;
        self.device.validate()?;
        ensure!(
            self.format == 1
                && !self.id.is_nil()
                && !self.generation.is_nil()
                && matches!(self.device, DeviceBinding::Prospective { .. })
                && is_digest(&self.request_sha256)
                && self
                    .previous_binding_sha256
                    .as_deref()
                    .is_none_or(is_digest)
                && self.identity.encryption.len() <= 128
                && self.identity.signing.len() <= 64,
            "Invalid signup reservation binding"
        );
        self.identity.validate()
    }

    fn statement(&self) -> anyhow::Result<Vec<u8>> {
        self.validate()?;
        Ok(serde_json::to_vec(&("wisp-signup-reservation-v1", self))?)
    }

    pub fn digest(&self) -> anyhow::Result<String> {
        Ok(format!("{:x}", Sha256::digest(self.statement()?)))
    }

    pub fn sign(&self, identity: &Identity) -> anyhow::Result<String> {
        ensure!(
            identity.public() == self.identity,
            "Signup identity differs from staged identity"
        );
        Ok(identity.sign_statement("wisp-signup-reservation-v1", &self.statement()?))
    }

    pub fn verify(&self, signature: &str) -> anyhow::Result<()> {
        ensure!(
            signature.len() <= 128,
            "Invalid signup reservation signature"
        );
        self.identity
            .verify_statement("wisp-signup-reservation-v1", &self.statement()?, signature)
    }
}
impl Operation {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.format == 1 && !self.id.is_nil(),
            "Unsupported account operation"
        );
        self.scope.validate()?;
        self.device.validate()?;
        self.expected.validate()?;
        match &self.change {
            Change::Enroll {
                generation,
                registration_sha256,
                wrapper_sha256,
                vault,
                signup,
            } => {
                ensure!(
                    self.expected == Precondition::default()
                        && !generation.is_nil()
                        && is_digest(registration_sha256)
                        && is_digest(wrapper_sha256)
                        && vault.revision == 1,
                    "Invalid account enrollment operation"
                );
                vault.validate()?;
                match (&self.device, signup) {
                    (DeviceBinding::Prospective { .. }, Some(metadata)) => metadata.validate()?,
                    (DeviceBinding::Existing { .. }, None) => (),
                    _ => anyhow::bail!("Invalid signup or migration device binding"),
                }
            }
            change => {
                ensure!(
                    self.expected.credential_generation.is_some()
                        && matches!(self.device, DeviceBinding::Existing { .. }),
                    "Account operation needs an existing secure device"
                );
                match change {
                    Change::ChangePassword {
                        generation,
                        registration_sha256,
                        wrapper_sha256,
                    } => {
                        ensure!(
                            !generation.is_nil()
                                && Some(*generation) != self.expected.credential_generation
                                && is_digest(registration_sha256)
                                && is_digest(wrapper_sha256),
                            "Invalid password change operation"
                        );
                    }
                    Change::Rewrap { wrapper_sha256 } => {
                        ensure!(is_digest(wrapper_sha256), "Invalid backup rewrap operation");
                    }
                    Change::SetRecoveryEmail { email_sha256 } => {
                        ensure!(is_digest(email_sha256), "Invalid recovery email operation");
                    }
                    Change::StoreVault { vault } => {
                        vault.validate()?;
                        ensure!(
                            self.expected
                                .vault
                                .as_ref()
                                .and_then(|previous| previous.revision.checked_add(1))
                                == Some(vault.revision),
                            "Invalid backup update operation"
                        );
                    }
                    Change::Enroll { .. } => unreachable!(),
                }
            }
        }
        Ok(())
    }

    fn statement(&self) -> anyhow::Result<Vec<u8>> {
        self.validate()?;
        Ok(serde_json::to_vec(&(DOMAIN, self))?)
    }

    /// Also used as OPAQUE reauthentication's exact-operation binding. Grant
    /// issuance/consumption must compare this digest inside the transaction.
    pub fn digest(&self) -> anyhow::Result<String> {
        Ok(format!("{:x}", Sha256::digest(self.statement()?)))
    }

    pub fn sign(&self, identity: &Identity) -> anyhow::Result<String> {
        Ok(identity.sign_statement(DOMAIN, &self.statement()?))
    }

    pub fn verify(&self, identity: &PublicIdentity, signature: &str) -> anyhow::Result<()> {
        ensure!(
            signature.len() <= 128,
            "Invalid account operation signature"
        );
        identity.verify_statement(DOMAIN, &self.statement()?, signature)
    }
}

/// Public native OPAQUE wire messages, already canonical base64. This binding
/// covers the original request/response as well as the final password file.
/// A renewed signup authorization must preserve this exact effect.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RegistrationTranscript {
    pub account: AccountContext,
    pub scope: Scope,
    pub generation: Uuid,
    pub request: String,
    pub response: String,
    pub upload: String,
}
impl RegistrationTranscript {
    pub fn digest(&self) -> anyhow::Result<String> {
        self.account.validate()?;
        self.scope.validate()?;
        ensure!(
            self.account.origin == self.scope.origin
                && self.account.network == self.scope.network
                && !self.generation.is_nil(),
            "Invalid registration scope"
        );
        super::auth::validate_registration_transcript(&self.request, &self.response, &self.upload)?;
        Ok(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                "wisp-registration-transcript-v1",
                self
            ))?)
        ))
    }
}

/// One binary credential identifier for opaque-ke. Server-only input selection
/// binds each password file to its immutable account scope and generation.
pub fn credential_identifier(scope: &Scope, generation: Uuid) -> anyhow::Result<[u8; 32]> {
    scope.validate()?;
    ensure!(!generation.is_nil(), "Invalid secure credential generation");
    Ok(Sha256::digest(serde_json::to_vec(&(
        "wisp-opaque-credential-v1",
        scope,
        generation,
    ))?)
    .into())
}

/// Password reset is authorized by an email token, not an unavailable account
/// identity. Keep the stable effect separate from a renewable authorization.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ResetEffect {
    pub format: u32,
    pub scope: Scope,
    pub id: Uuid,
    pub expected_generation: Uuid,
    pub generation: Uuid,
    pub registration_sha256: String,
}

impl ResetEffect {
    pub fn digest(&self) -> anyhow::Result<String> {
        self.scope.validate()?;
        ensure!(
            self.format == 1
                && !self.id.is_nil()
                && !self.expected_generation.is_nil()
                && !self.generation.is_nil()
                && self.expected_generation != self.generation
                && is_digest(&self.registration_sha256),
            "Invalid secure password reset effect"
        );
        Ok(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&("wisp-account-reset-effect-v1", self))?)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signup_reservation_requires_identity_and_binds_every_field() {
        let identity = Identity::generate().unwrap();
        let (_, request) = super::super::auth::Registration::start(crate::SecretString::from(
            "synthetic signup password".to_owned(),
        ))
        .unwrap();
        let binding = SignupStartBinding {
            format: 1,
            scope: Scope {
                origin: "https://example.invalid".into(),
                network: Uuid::new_v4(),
                account: Uuid::new_v4(),
            },
            id: Uuid::new_v4(),
            generation: Uuid::new_v4(),
            signup: SignupMetadata {
                username: "synthetic".into(),
                display_name: "Example".into(),
                device_name: "Test".into(),
            },
            device: DeviceBinding::Prospective {
                id: Uuid::new_v4(),
                token_sha256: "a".repeat(64),
            },
            identity: identity.public(),
            request_sha256: super::super::auth::registration_request_digest(&request).unwrap(),
            previous_binding_sha256: None,
        };
        let signature = binding.sign(&identity).unwrap();
        binding.verify(&signature).unwrap();
        assert!(binding.sign(&Identity::generate().unwrap()).is_err());
        for field in 0..13 {
            let mut altered = binding.clone();
            match field {
                0 => altered.scope.account = Uuid::new_v4(),
                1 => altered.scope.network = Uuid::new_v4(),
                2 => altered.scope.origin = "https://other.invalid".into(),
                3 => altered.id = Uuid::new_v4(),
                4 => altered.generation = Uuid::new_v4(),
                5 => altered.signup.username = "someoneelse".into(),
                6 => altered.signup.display_name = "Other name".into(),
                7 => altered.signup.device_name = "Other device".into(),
                8 => {
                    altered.device = DeviceBinding::Prospective {
                        id: Uuid::new_v4(),
                        token_sha256: "a".repeat(64),
                    }
                }
                9 => {
                    altered.device = DeviceBinding::Prospective {
                        id: binding.device.id(),
                        token_sha256: "b".repeat(64),
                    }
                }
                10 => altered.identity = Identity::generate().unwrap().public(),
                11 => altered.request_sha256 = "c".repeat(64),
                _ => altered.previous_binding_sha256 = Some(binding.digest().unwrap()),
            }
            assert!(altered.verify(&signature).is_err(), "field {field}");
            assert_ne!(binding.digest().unwrap(), altered.digest().unwrap());
        }
        assert!(super::super::auth::registration_request_digest("not base64").is_err());
        assert!(super::super::auth::registration_request_digest(&format!("{request} ")).is_err());
    }

    #[test]
    fn proofs_bind_account_device_generation_preconditions_and_exact_change() {
        let identity = Identity::generate().unwrap();
        let generation = Uuid::new_v4();
        let operation = Operation {
            format: 1,
            scope: Scope {
                origin: "https://example.invalid".into(),
                network: Uuid::new_v4(),
                account: Uuid::new_v4(),
            },
            id: Uuid::new_v4(),
            device: DeviceBinding::Existing { id: Uuid::new_v4() },
            expected: Precondition {
                credential_generation: Some(generation),
                wrapper_generation: Some(generation),
                vault: Some(Checkpoint {
                    revision: 1,
                    sha256: "a".repeat(64),
                }),
            },
            change: Change::Rewrap {
                wrapper_sha256: "b".repeat(64),
            },
        };
        let signature = operation.sign(&identity).unwrap();
        operation.verify(&identity.public(), &signature).unwrap();
        for field in 0..8 {
            let mut altered = operation.clone();
            match field {
                0 => altered.scope.account = Uuid::new_v4(),
                1 => altered.scope.network = Uuid::new_v4(),
                2 => altered.scope.origin = "https://other.invalid".into(),
                3 => altered.id = Uuid::new_v4(),
                4 => altered.device = DeviceBinding::Existing { id: Uuid::new_v4() },
                5 => altered.expected.credential_generation = Some(Uuid::new_v4()),
                6 => altered.expected.vault.as_mut().unwrap().sha256 = "c".repeat(64),
                _ => {
                    altered.change = Change::SetRecoveryEmail {
                        email_sha256: "b".repeat(64),
                    }
                }
            }
            assert!(altered.verify(&identity.public(), &signature).is_err());
            assert_ne!(altered.digest().unwrap(), operation.digest().unwrap());
        }
        assert!(
            operation
                .verify(&Identity::generate().unwrap().public(), &signature)
                .is_err()
        );
    }
}
