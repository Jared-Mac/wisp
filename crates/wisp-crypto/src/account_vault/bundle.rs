//! Typed portable state. Missing fields contribute nothing; they never erase
//! another device's keys or trust checkpoints. No filesystem paths or sessions.
use super::{Scope, is_digest, strict_json};
use crate::{Identity, PublicIdentity, SecretString, profile::SignedProfile, roster::SignedRoster};
use age::secrecy::ExposeSecret;
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use uuid::Uuid;
use zeroize::Zeroizing;

pub const MAX_PLAINTEXT: usize = 8 * 1024 * 1024;
const MAX_ENTRIES: usize = 4096;
const MAX_PROOF: usize = 4096;
const FORMAT: u32 = 1;

struct PrivateWriter(Zeroizing<Vec<u8>>);
impl Write for PrivateWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_PLAINTEXT.saturating_sub(self.0.len()) {
            return Err(std::io::Error::other("Backup exceeds size limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NameCheckpoint {
    pub display_name: String,
    /// Zero means a first-use name, not a verified signed-profile revision.
    pub revision: u64,
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TrustState {
    pub pins: BTreeMap<Uuid, PublicIdentity>,
    pub names: BTreeMap<Uuid, NameCheckpoint>,
    pub profiles: BTreeMap<Uuid, SignedProfile>,
    /// Retained legacy metadata, not an additional modern send-policy guard.
    pub legacy_approvals: BTreeMap<String, BTreeSet<Uuid>>,
    /// Previously accepted checkpoints. Restore is not current authorization.
    pub room_heads: BTreeMap<String, SignedRoster>,
}

/// No Debug, Clone or public serde: encoding plaintext is always explicit.
pub struct Bundle {
    pub scope: Scope,
    pub trust: TrustState,
    recovery: SecretString,
    media_key: Option<SecretString>,
}

#[derive(Serialize)]
struct WireOut<'a> {
    format: u32,
    scope: &'a Scope,
    recovery: &'a str,
    media_key: Option<&'a str>,
    trust: &'a TrustState,
}

struct PrivateText(SecretString);
impl<'de> Deserialize<'de> for PrivateText {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        String::deserialize(decoder).map(|text| Self(SecretString::from(text)))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireIn {
    format: u32,
    scope: Scope,
    recovery: PrivateText,
    media_key: Option<PrivateText>,
    trust: TrustState,
}

fn conversation(value: &str) -> anyhow::Result<()> {
    ensure!(
        !value.is_empty() && value.len() <= 1024 && !value.chars().any(char::is_control),
        "Invalid backup conversation"
    );
    Ok(())
}

fn public_identity(identity: &PublicIdentity) -> anyhow::Result<()> {
    ensure!(
        identity.encryption.len() <= 128 && identity.signing.len() <= 64,
        "Invalid backup identity"
    );
    identity.validate()
}

impl TrustState {
    pub fn validate(&self, scope: &Scope, own: &PublicIdentity) -> anyhow::Result<()> {
        for count in [
            self.pins.len(),
            self.names.len(),
            self.profiles.len(),
            self.legacy_approvals.len(),
            self.room_heads.len(),
        ] {
            ensure!(count <= MAX_ENTRIES, "Backup has too many trust entries");
        }
        ensure!(
            self.pins.get(&scope.account) == Some(own),
            "Backup's own identity pin is missing or different"
        );
        for (id, key) in &self.pins {
            ensure!(!id.is_nil(), "Invalid backup account");
            public_identity(key)?;
        }
        for (id, name) in &self.names {
            ensure!(
                !id.is_nil()
                    && (name.revision == 0 || self.pins.contains_key(id))
                    && name.display_name.trim() == name.display_name
                    && !name.display_name.is_empty()
                    && name.display_name.chars().count() <= 80
                    && !name.display_name.chars().any(char::is_control)
                    && i64::try_from(name.revision).is_ok(),
                "Invalid backup name checkpoint"
            );
        }
        for (id, profile) in &self.profiles {
            ensure!(
                profile.profile.account == *id
                    && profile.profile.network == scope.network
                    && profile.signature.len() <= 128,
                "Invalid backup signed profile"
            );
            profile.verify(
                self.pins
                    .get(id)
                    .context("Signed profile has no identity pin")?,
            )?;
            let name = self
                .names
                .get(id)
                .context("Signed profile has no name checkpoint")?;
            ensure!(
                name.revision >= profile.profile.revision
                    && (name.revision != profile.profile.revision
                        || name.display_name == profile.profile.display_name),
                "Conflicting backup profile checkpoint"
            );
        }
        for (id, members) in &self.legacy_approvals {
            conversation(id)?;
            ensure!(
                !members.is_empty()
                    && members.len() <= MAX_ENTRIES
                    && members.contains(&scope.account)
                    && members.iter().all(|id| self.pins.contains_key(id)),
                "Invalid legacy recipient approval"
            );
        }
        for (id, head) in &self.room_heads {
            self.checkpoint(scope, id, head)?;
        }
        Ok(())
    }

    fn checkpoint(&self, scope: &Scope, id: &str, head: &SignedRoster) -> anyhow::Result<()> {
        conversation(id)?;
        let r = &head.roster;
        ensure!(
            r.network == scope.network
                && r.conversation == id
                && r.members.len() <= MAX_ENTRIES
                && i64::try_from(r.revision).is_ok()
                && head.signature.len() <= 128
                && if r.revision == 0 {
                    r.previous.is_none()
                } else {
                    r.previous.as_deref().is_some_and(is_digest)
                },
            "Invalid backup room checkpoint"
        );
        for (id, member) in &r.members {
            ensure!(
                self.pins.get(id) == Some(&member.identity),
                "Room checkpoint conflicts with an identity pin"
            );
        }
        let actor = self
            .pins
            .get(&r.actor)
            .context("Room checkpoint has no signer pin")?;
        if r.revision == 0 {
            head.verify_genesis()?;
        }
        head.verify_checkpoint_signature(actor)
    }
}

impl Bundle {
    pub fn new(
        scope: Scope,
        recovery: SecretString,
        media_key: Option<SecretString>,
        trust: TrustState,
    ) -> anyhow::Result<Self> {
        let bundle = Self {
            scope,
            trust,
            recovery,
            media_key,
        };
        // Ensure an accepted detached candidate can be saved and restored by
        // either client, including aggregate limits spanning separate maps.
        bundle.encode_private()?;
        Ok(bundle)
    }

    pub fn identity(&self) -> anyhow::Result<Identity> {
        Identity::recover(&self.recovery)
    }

    #[must_use]
    pub fn media_key(&self) -> Option<&SecretString> {
        self.media_key.as_ref()
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        self.scope.validate()?;
        ensure!(
            self.recovery.expose_secret().len() <= 4096,
            "Invalid backup recovery material"
        );
        if let Some(key) = &self.media_key {
            let value = key.expose_secret();
            ensure!(
                (16..=4096).contains(&value.len()) && !value.chars().any(char::is_control),
                "Invalid backup media key"
            );
        }
        self.trust.validate(&self.scope, &self.identity()?.public())
    }

    /// Private local storage / authenticated encryption only; never send this
    /// result to the account service or put it in a snapshot/log.
    pub fn encode_private(&self) -> anyhow::Result<Zeroizing<Vec<u8>>> {
        self.validate()?;
        // Fixed bounded capacity avoids leaving plaintext in reallocated Vec
        // buffers. This is best-effort cleanup, not a total-memory-erasure claim.
        let mut writer = PrivateWriter(Zeroizing::new(Vec::with_capacity(MAX_PLAINTEXT)));
        serde_json::to_writer(
            &mut writer,
            &WireOut {
                format: FORMAT,
                scope: &self.scope,
                recovery: self.recovery.expose_secret(),
                media_key: self.media_key.as_ref().map(ExposeSecret::expose_secret),
                trust: &self.trust,
            },
        )
        .map_err(|_| anyhow::anyhow!("Invalid or oversized private backup"))?;
        let bytes = writer.0;
        strict_json::validate(&bytes)?;
        Ok(bytes)
    }

    pub fn decode_private(
        bytes: &[u8],
        expected: &Scope,
        own: &PublicIdentity,
    ) -> anyhow::Result<Self> {
        ensure!(
            bytes.len() <= MAX_PLAINTEXT,
            "Encrypted backup is too large"
        );
        strict_json::validate(bytes)?;
        let decoded: WireIn =
            serde_json::from_slice(bytes).map_err(|_| anyhow::anyhow!("Invalid private backup"))?;
        ensure!(
            decoded.format == FORMAT && &decoded.scope == expected,
            "Unsupported or mismatched backup scope"
        );
        let bundle = Self::new(
            decoded.scope,
            decoded.recovery.0,
            decoded.media_key.map(|v| v.0),
            decoded.trust,
        )?;
        ensure!(
            bundle.identity()?.public() == *own,
            "Backup identity differs from this account"
        );
        Ok(bundle)
    }

    /// Construct a detached candidate; the caller must durably commit it before
    /// publishing live state. Proofs connect checkpoints; revision alone cannot.
    #[allow(clippy::too_many_lines)] // Keep the detached conflict checks together.
    pub fn merge(
        &self,
        other: &Self,
        proofs: &BTreeMap<String, Vec<SignedRoster>>,
    ) -> anyhow::Result<Self> {
        self.validate()?;
        other.validate()?;
        ensure!(
            self.scope == other.scope && self.identity()?.public() == other.identity()?.public(),
            "Cannot merge different backup identities"
        );
        ensure!(proofs.len() <= MAX_ENTRIES, "Too many room proofs");
        let media_key = match (&self.media_key, &other.media_key) {
            (Some(a), Some(b)) => {
                ensure!(
                    a.expose_secret() == b.expose_secret(),
                    "Conflicting media keys need explicit reconciliation"
                );
                Some(SecretString::from(a.expose_secret().to_owned()))
            }
            (Some(a), None) | (None, Some(a)) => {
                Some(SecretString::from(a.expose_secret().to_owned()))
            }
            (None, None) => None,
        };
        let mut trust = self.trust.clone();
        for (id, key) in &other.trust.pins {
            ensure!(
                trust.pins.get(id).is_none_or(|saved| saved == key),
                "Conflicting identity pins"
            );
            trust.pins.insert(*id, key.clone());
        }
        for (id, name) in &other.trust.names {
            match trust.names.get(id) {
                Some(saved) if saved.revision == name.revision => {
                    ensure!(saved == name, "Conflicting name checkpoints");
                }
                Some(saved) if saved.revision > name.revision => (),
                _ => {
                    trust.names.insert(*id, name.clone());
                }
            }
        }
        for (id, profile) in &other.trust.profiles {
            match trust.profiles.get(id) {
                Some(saved) if saved.profile.revision == profile.profile.revision => {
                    ensure!(saved == profile, "Conflicting signed profiles");
                }
                Some(saved) if saved.profile.revision > profile.profile.revision => (),
                _ => {
                    trust.profiles.insert(*id, profile.clone());
                }
            }
        }
        for (id, approval) in &other.trust.legacy_approvals {
            ensure!(
                trust
                    .legacy_approvals
                    .get(id)
                    .is_none_or(|saved| saved == approval),
                "Conflicting legacy recipient approvals"
            );
            trust.legacy_approvals.insert(id.clone(), approval.clone());
        }
        for (id, head) in &other.trust.room_heads {
            if let Some(saved) = trust.room_heads.get(id) {
                if saved == head {
                    continue;
                }
                let (older, newer) = if saved.roster.revision < head.roster.revision {
                    (saved, head)
                } else {
                    (head, saved)
                };
                ensure!(
                    older.roster.revision < newer.roster.revision,
                    "Conflicting room checkpoints"
                );
                let chain = proofs
                    .get(id)
                    .context("A connecting room proof is required")?;
                ensure!(
                    chain.len() >= 2
                        && chain.len() <= MAX_PROOF
                        && chain.first() == Some(older)
                        && chain.last() == Some(newer),
                    "Room proof does not connect the saved checkpoints"
                );
                for item in chain {
                    trust.checkpoint(&self.scope, id, item)?;
                }
                for pair in chain.windows(2) {
                    pair[1].verify_successor(&pair[0])?;
                }
                let newer = newer.clone();
                trust.room_heads.insert(id.clone(), newer);
            } else {
                trust.room_heads.insert(id.clone(), head.clone());
            }
        }
        Self::new(
            self.scope.clone(),
            SecretString::from(self.recovery.expose_secret().to_owned()),
            media_key,
            trust,
        )
    }
}
