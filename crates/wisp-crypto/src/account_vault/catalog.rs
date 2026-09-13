//! Separately encrypted discovery metadata. Entries never grant service access.
use super::{
    Scope,
    envelope::{Checkpoint, Header, VaultKey, decoded, derive, encoded, nonce, random},
    is_digest, strict_json,
};
use crate::{Identity, PublicIdentity};
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use zeroize::Zeroizing;

pub const VERSION: u32 = 1;
pub const MAX_ENTRIES: usize = 4096;
pub const MAX_PLAINTEXT: usize = 1024 * 1024;
pub const MAX_CIPHERTEXT: usize = MAX_PLAINTEXT + 16;
pub const MAX_ENVELOPE_WIRE: usize = MAX_CIPHERTEXT.div_ceil(3) * 4 + 16_384;
pub const MAX_PROOF_PAGE: usize = 128;
const PAYLOAD_DOMAIN: &str = "wisp-server-catalog-payload-v1";
const MANIFEST_DOMAIN: &str = "wisp-server-catalog-manifest-v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub scope: Scope,
    pub label: String,
}
impl Entry {
    pub fn validate(&self) -> anyhow::Result<()> {
        self.scope.validate()?;
        ensure!(
            !self.label.is_empty()
                && self.label.len() <= 160
                && self.label.trim() == self.label
                && !self.label.chars().any(char::is_control),
            "Invalid server label"
        );
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub format: u32,
    pub scope: Scope,
    pub entries: Vec<Entry>,
}
impl Catalog {
    #[must_use]
    pub fn empty(scope: Scope) -> Self {
        Self {
            format: VERSION,
            scope,
            entries: Vec::new(),
        }
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        self.scope.validate()?;
        ensure!(
            self.format == VERSION && self.entries.len() <= MAX_ENTRIES,
            "Invalid server catalog"
        );
        let mut previous: Option<&str> = None;
        for entry in &self.entries {
            entry.validate()?;
            ensure!(
                previous.is_none_or(|p| p < entry.scope.origin.as_str()),
                "Server catalog must have unique sorted origins"
            );
            previous = Some(&entry.scope.origin);
        }
        Ok(())
    }
    /// Append known services without replacing a published account binding or
    /// label. Local nickname/order/hide preferences are deliberately separate.
    /// A failed merge leaves the receiver intact.
    pub fn merge(&mut self, entries: &[Entry]) -> anyhow::Result<bool> {
        self.validate()?;
        ensure!(entries.len() <= MAX_ENTRIES, "Too many server entries");
        let mut merged: BTreeMap<_, _> = self
            .entries
            .iter()
            .map(|e| (e.scope.origin.clone(), e.clone()))
            .collect();
        for entry in entries {
            entry.validate()?;
            if let Some(existing) = merged.get(&entry.scope.origin) {
                ensure!(
                    existing.scope == entry.scope,
                    "A saved server belongs to a different account or network"
                );
            } else {
                merged.insert(entry.scope.origin.clone(), entry.clone());
            }
        }
        let candidate = Self {
            format: self.format,
            scope: self.scope.clone(),
            entries: merged.into_values().collect(),
        };
        candidate.encode_private()?;
        let changed = *self != candidate;
        *self = candidate;
        Ok(changed)
    }
    pub fn encode_private(&self) -> anyhow::Result<Zeroizing<Vec<u8>>> {
        self.validate()?;
        let bytes = Zeroizing::new(serde_json::to_vec(self)?);
        ensure!(
            bytes.len() <= MAX_PLAINTEXT,
            "Server catalog exceeds its size limit"
        );
        Ok(bytes)
    }
    pub fn decode_private(bytes: &[u8], expected: &Scope) -> anyhow::Result<Self> {
        ensure!(
            bytes.len() <= MAX_PLAINTEXT,
            "Server catalog exceeds its size limit"
        );
        strict_json::validate(bytes)?;
        let catalog: Self =
            serde_json::from_slice(bytes).map_err(|_| anyhow::anyhow!("Invalid server catalog"))?;
        catalog.validate()?;
        ensure!(
            &catalog.scope == expected,
            "Server catalog belongs to another account"
        );
        Ok(catalog)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SignedManifest {
    pub header: Header,
    pub ciphertext_sha256: String,
    pub signature: String,
}
impl SignedManifest {
    fn statement(&self) -> anyhow::Result<Vec<u8>> {
        self.header.validate()?;
        ensure!(is_digest(&self.ciphertext_sha256), "Invalid catalog digest");
        Ok(serde_json::to_vec(&(
            MANIFEST_DOMAIN,
            &self.header,
            &self.ciphertext_sha256,
        ))?)
    }
    pub fn verify(&self, scope: &Scope, own: &PublicIdentity) -> anyhow::Result<()> {
        ensure!(
            &self.header.scope == scope
                && &self.header.signer == own
                && self.signature.len() <= 128,
            "Server catalog belongs to another identity"
        );
        own.verify_statement(MANIFEST_DOMAIN, &self.statement()?, &self.signature)
    }
    pub fn checkpoint(&self) -> anyhow::Result<Checkpoint> {
        Ok(Checkpoint {
            revision: self.header.revision,
            sha256: format!("{:x}", Sha256::digest(self.statement()?)),
        })
    }
    pub fn advance(
        &self,
        scope: &Scope,
        own: &PublicIdentity,
        from: Option<&Checkpoint>,
    ) -> anyhow::Result<Checkpoint> {
        self.verify(scope, own)?;
        let next = self.checkpoint()?;
        if from != Some(&next) {
            ensure!(
                self.header.parent.as_ref() == from,
                "Server catalog history rolled back, forked or is incomplete"
            );
        }
        Ok(next)
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub manifest: SignedManifest,
    pub ciphertext: String,
}
impl Envelope {
    pub fn seal(
        catalog: &Catalog,
        key: &VaultKey,
        identity: &Identity,
        parent: Option<Checkpoint>,
    ) -> anyhow::Result<Self> {
        let header = Header {
            format: VERSION,
            scope: catalog.scope.clone(),
            revision: match &parent {
                Some(p) => p
                    .revision
                    .checked_add(1)
                    .ok_or_else(|| anyhow::anyhow!("Catalog revision exhausted"))?,
                None => 1,
            },
            parent,
            signer: identity.public(),
            nonce: encoded(random::<12>()?.as_slice()),
        };
        header.validate()?;
        let payload_key = derive(
            key.save_private().as_slice(),
            PAYLOAD_DOMAIN,
            &catalog.scope,
        )?;
        let aad = serde_json::to_vec(&(PAYLOAD_DOMAIN, &header))?;
        let ciphertext = Aes256Gcm::new_from_slice(payload_key.as_slice())
            .expect("AES-256 key")
            .encrypt(
                Nonce::from_slice(&nonce(&header.nonce)?),
                Payload {
                    msg: &catalog.encode_private()?,
                    aad: &aad,
                },
            )
            .map_err(|_| anyhow::anyhow!("Could not encrypt server catalog"))?;
        let mut manifest = SignedManifest {
            header,
            ciphertext_sha256: format!("{:x}", Sha256::digest(&ciphertext)),
            signature: String::new(),
        };
        manifest.signature = identity.sign_statement(MANIFEST_DOMAIN, &manifest.statement()?);
        Ok(Self {
            manifest,
            ciphertext: encoded(&ciphertext),
        })
    }
    pub fn verify(&self, scope: &Scope, own: &PublicIdentity) -> anyhow::Result<()> {
        self.manifest.verify(scope, own)?;
        let ciphertext = decoded(&self.ciphertext, MAX_CIPHERTEXT)?;
        ensure!(
            ciphertext.len() >= 16
                && format!("{:x}", Sha256::digest(&ciphertext)) == self.manifest.ciphertext_sha256,
            "Catalog ciphertext changed"
        );
        Ok(())
    }
    /// Verify the connecting signed history before supplying `accepted`. On a
    /// fresh restore the authenticated head is the initial checkpoint.
    pub fn open(
        &self,
        key: &VaultKey,
        scope: &Scope,
        own: &PublicIdentity,
        accepted: &Checkpoint,
    ) -> anyhow::Result<Catalog> {
        self.verify(scope, own)?;
        ensure!(
            &self.manifest.checkpoint()? == accepted,
            "Catalog differs from accepted checkpoint"
        );
        let payload_key = derive(key.save_private().as_slice(), PAYLOAD_DOMAIN, scope)?;
        let aad = serde_json::to_vec(&(PAYLOAD_DOMAIN, &self.manifest.header))?;
        let plaintext = Zeroizing::new(
            Aes256Gcm::new_from_slice(payload_key.as_slice())
                .expect("AES-256 key")
                .decrypt(
                    Nonce::from_slice(&nonce(&self.manifest.header.nonce)?),
                    Payload {
                        msg: &decoded(&self.ciphertext, MAX_CIPHERTEXT)?,
                        aad: &aad,
                    },
                )
                .map_err(|_| anyhow::anyhow!("Could not decrypt server catalog"))?,
        );
        Catalog::decode_private(&plaintext, scope)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub version: u32,
    pub scope: Scope,
    pub catalog: Option<Checkpoint>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadResponse {
    pub version: u32,
    pub scope: Scope,
    pub catalog: Option<Envelope>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreRequest {
    pub catalog: Envelope,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofRequest {
    pub after: Checkpoint,
    pub through: Checkpoint,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofPage {
    pub manifests: Vec<SignedManifest>,
    pub next: Checkpoint,
    pub complete: bool,
}

#[cfg(test)]
mod tests;
