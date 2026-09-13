//! Authenticated portable vault encryption and client-only key wrapping.
use super::{
    Scope,
    auth::ExportKey,
    bundle::{Bundle, MAX_PLAINTEXT},
    is_digest,
};
use crate::PublicIdentity;
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use anyhow::ensure;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::Zeroizing;

const FORMAT: u32 = 1;
const MANIFEST_DOMAIN: &str = "wisp-account-vault-manifest-v1";
const WRAP_DOMAIN: &str = "wisp-account-vault-wrap-v1";
const PAYLOAD_DOMAIN: &str = "wisp-account-vault-payload-v1";
pub const MAX_CIPHERTEXT: usize = MAX_PLAINTEXT + 16;
pub const MAX_ENVELOPE_WIRE: usize = (MAX_CIPHERTEXT.div_ceil(3) * 4) + 16_384;

pub(super) fn random<const N: usize>() -> anyhow::Result<Zeroizing<[u8; N]>> {
    let mut bytes = Zeroizing::new([0; N]);
    getrandom::getrandom(bytes.as_mut())
        .map_err(|_| anyhow::anyhow!("Secure random generator unavailable"))?;
    Ok(bytes)
}

pub(super) fn encoded(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

pub(super) fn decoded(value: &str, max: usize) -> anyhow::Result<Zeroizing<Vec<u8>>> {
    ensure!(
        value.len() <= max.div_ceil(3) * 4,
        "Invalid encrypted backup size"
    );
    let bytes = Zeroizing::new(
        STANDARD
            .decode(value)
            .map_err(|_| anyhow::anyhow!("Invalid encrypted backup encoding"))?,
    );
    ensure!(
        bytes.len() <= max && encoded(&bytes) == value,
        "Invalid encrypted backup encoding"
    );
    Ok(bytes)
}

pub(super) fn nonce(value: &str) -> anyhow::Result<[u8; 12]> {
    decoded(value, 12)?
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid backup nonce"))
}

pub(super) fn derive(
    secret: &[u8],
    domain: &str,
    scope: &Scope,
) -> anyhow::Result<Zeroizing<[u8; 32]>> {
    let info = serde_json::to_vec(&(domain, FORMAT, scope))?;
    let mut key = Zeroizing::new([0; 32]);
    Hkdf::<Sha256>::new(Some(b"wisp-account-vault-hkdf-v1"), secret)
        .expand(&info, key.as_mut())
        .map_err(|_| anyhow::anyhow!("Could not derive backup encryption key"))?;
    Ok(key)
}

/// Stable random payload key, retained only in private device storage. It is
/// never a device credential and never serialized into a public API response.
pub struct VaultKey(Zeroizing<[u8; 32]>);
impl VaultKey {
    pub fn generate() -> anyhow::Result<Self> {
        Ok(Self(random()?))
    }
    pub fn restore_private(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(Self(Zeroizing::new(bytes.try_into().map_err(|_| {
            anyhow::anyhow!("Invalid private vault key")
        })?)))
    }
    #[must_use]
    pub fn save_private(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(*self.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct KeyEnvelope {
    pub format: u32,
    pub scope: Scope,
    pub credential_generation: Uuid,
    pub nonce: String,
    pub ciphertext: String,
}
impl KeyEnvelope {
    fn aad(&self) -> anyhow::Result<Vec<u8>> {
        self.scope.validate()?;
        ensure!(
            self.format == FORMAT && !self.credential_generation.is_nil(),
            "Invalid backup key wrapper"
        );
        nonce(&self.nonce)?;
        Ok(serde_json::to_vec(&(
            WRAP_DOMAIN,
            self.format,
            &self.scope,
            self.credential_generation,
            &self.nonce,
        ))?)
    }

    pub fn validate(&self, expected: &Scope, generation: Uuid) -> anyhow::Result<()> {
        self.aad()?;
        ensure!(
            &self.scope == expected
                && self.credential_generation == generation
                && decoded(&self.ciphertext, 48)?.len() == 48,
            "Backup is locked under a different credential or account"
        );
        Ok(())
    }

    pub fn wrap(
        scope: Scope,
        generation: Uuid,
        export: &ExportKey,
        key: &VaultKey,
    ) -> anyhow::Result<Self> {
        let mut envelope = Self {
            format: FORMAT,
            scope,
            credential_generation: generation,
            nonce: encoded(random::<12>()?.as_slice()),
            ciphertext: String::new(),
        };
        let aad = envelope.aad()?;
        let wrapping = derive(export.0.as_slice(), WRAP_DOMAIN, &envelope.scope)?;
        let ciphertext = Aes256Gcm::new_from_slice(wrapping.as_slice())
            .expect("AES-256 key")
            .encrypt(
                Nonce::from_slice(&nonce(&envelope.nonce)?),
                Payload {
                    msg: key.0.as_slice(),
                    aad: &aad,
                },
            )
            .map_err(|_| anyhow::anyhow!("Could not wrap backup key"))?;
        envelope.ciphertext = encoded(&ciphertext);
        Ok(envelope)
    }

    pub fn unwrap(
        &self,
        expected: &Scope,
        generation: Uuid,
        export: &ExportKey,
    ) -> anyhow::Result<VaultKey> {
        self.validate(expected, generation)?;
        let wrapping = derive(export.0.as_slice(), WRAP_DOMAIN, expected)?;
        let plaintext = Zeroizing::new(
            Aes256Gcm::new_from_slice(wrapping.as_slice())
                .expect("AES-256 key")
                .decrypt(
                    Nonce::from_slice(&nonce(&self.nonce)?),
                    Payload {
                        msg: &decoded(&self.ciphertext, 48)?,
                        aad: &self.aad()?,
                    },
                )
                .map_err(|_| anyhow::anyhow!("Could not unlock encrypted backup"))?,
        );
        VaultKey::restore_private(&plaintext)
    }

    pub fn digest(&self) -> anyhow::Result<String> {
        self.validate(&self.scope, self.credential_generation)?;
        Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(self)?)))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub revision: u64,
    pub sha256: String,
}
impl Checkpoint {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.revision > 0 && i64::try_from(self.revision).is_ok() && is_digest(&self.sha256),
            "Invalid backup checkpoint"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Header {
    pub format: u32,
    pub scope: Scope,
    pub revision: u64,
    pub parent: Option<Checkpoint>,
    pub signer: PublicIdentity,
    pub nonce: String,
}
impl Header {
    pub(super) fn validate(&self) -> anyhow::Result<()> {
        self.scope.validate()?;
        ensure!(
            self.format == FORMAT && self.revision > 0 && i64::try_from(self.revision).is_ok(),
            "Unsupported or invalid backup header"
        );
        if let Some(parent) = &self.parent {
            parent.validate()?;
            ensure!(
                parent.revision.checked_add(1) == Some(self.revision),
                "Invalid backup parent"
            );
        } else {
            ensure!(self.revision == 1, "Missing backup parent");
        }
        ensure!(
            self.signer.encryption.len() <= 128 && self.signer.signing.len() <= 64,
            "Invalid backup signer"
        );
        self.signer.validate()?;
        nonce(&self.nonce)?;
        Ok(())
    }
    fn aad(&self) -> anyhow::Result<Vec<u8>> {
        self.validate()?;
        Ok(serde_json::to_vec(&(PAYLOAD_DOMAIN, self))?)
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
        ensure!(
            is_digest(&self.ciphertext_sha256),
            "Invalid backup ciphertext digest"
        );
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
            "Backup belongs to another identity"
        );
        own.verify_statement(MANIFEST_DOMAIN, &self.statement()?, &self.signature)
    }

    pub fn checkpoint(&self) -> anyhow::Result<Checkpoint> {
        Ok(Checkpoint {
            revision: self.header.revision,
            sha256: format!("{:x}", Sha256::digest(self.statement()?)),
        })
    }

    /// Accept equal current state or an immediate authenticated successor only.
    /// For larger gaps the server supplies the bounded signed manifest chain.
    pub fn advance(
        &self,
        scope: &Scope,
        own: &PublicIdentity,
        from: Option<&Checkpoint>,
    ) -> anyhow::Result<Checkpoint> {
        self.verify(scope, own)?;
        let next = self.checkpoint()?;
        if from == Some(&next) {
            return Ok(next);
        }
        ensure!(
            self.header.parent.as_ref() == from,
            "Backup history rolled back, forked or is missing a connecting proof"
        );
        Ok(next)
    }
}

/// Public data only. Large ciphertext must not be included in application logs.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub manifest: SignedManifest,
    pub ciphertext: String,
}
impl Envelope {
    pub fn seal(
        bundle: &Bundle,
        key: &VaultKey,
        parent: Option<Checkpoint>,
    ) -> anyhow::Result<Self> {
        let identity = bundle.identity()?;
        let header = Header {
            format: FORMAT,
            scope: bundle.scope.clone(),
            revision: match &parent {
                Some(parent) => parent
                    .revision
                    .checked_add(1)
                    .ok_or_else(|| anyhow::anyhow!("Backup revision exhausted"))?,
                None => 1,
            },
            parent,
            signer: identity.public(),
            nonce: encoded(random::<12>()?.as_slice()),
        };
        let payload_key = derive(key.0.as_slice(), PAYLOAD_DOMAIN, &bundle.scope)?;
        let plaintext = bundle.encode_private()?;
        let ciphertext = Aes256Gcm::new_from_slice(payload_key.as_slice())
            .expect("AES-256 key")
            .encrypt(
                Nonce::from_slice(&nonce(&header.nonce)?),
                Payload {
                    msg: &plaintext,
                    aad: &header.aad()?,
                },
            )
            .map_err(|_| anyhow::anyhow!("Could not encrypt backup"))?;
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
            "Encrypted backup digest mismatch"
        );
        Ok(())
    }

    /// `accepted` must have passed advance/chain validation against the retained
    /// local checkpoint (or explicit fresh restore). Decryption cannot erase it.
    pub fn open(
        &self,
        key: &VaultKey,
        scope: &Scope,
        own: &PublicIdentity,
        accepted: &Checkpoint,
    ) -> anyhow::Result<Bundle> {
        self.verify(scope, own)?;
        ensure!(
            &self.manifest.checkpoint()? == accepted,
            "Backup differs from accepted checkpoint"
        );
        let payload_key = derive(key.0.as_slice(), PAYLOAD_DOMAIN, scope)?;
        let plaintext = Zeroizing::new(
            Aes256Gcm::new_from_slice(payload_key.as_slice())
                .expect("AES-256 key")
                .decrypt(
                    Nonce::from_slice(&nonce(&self.manifest.header.nonce)?),
                    Payload {
                        msg: &decoded(&self.ciphertext, MAX_CIPHERTEXT)?,
                        aad: &self.manifest.header.aad()?,
                    },
                )
                .map_err(|_| anyhow::anyhow!("Could not decrypt authenticated backup"))?,
        );
        Bundle::decode_private(&plaintext, scope, own)
    }
}
