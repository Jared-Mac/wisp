//! Client encryption and public verification primitives. Servers may verify
//! public signatures, but must never receive private identities/recovery keys.
//! Recipient trust is established locally; encryption alone cannot detect a
//! malicious key directory at first contact.
pub mod attachment;
pub mod keyring;
pub mod message;
pub mod profile;
pub mod roster;

use age::secrecy::ExposeSecret;
pub use age::secrecy::SecretString;
use anyhow::{Context, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub const ENCRYPTION_VERSION: u8 = 1;
const RECOVERY_PREFIX: &str = "wisp-recovery-v1:";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicIdentity {
    pub encryption: String,
    pub signing: String,
}

impl PublicIdentity {
    pub fn verify_statement(
        &self,
        domain: &str,
        body: &[u8],
        signature: &str,
    ) -> anyhow::Result<()> {
        let signature = Signature::from_slice(&STANDARD.decode(signature)?)?;
        self.verifying_key()?
            .verify_strict(&signing_input(domain, body), &signature)
            .context("Invalid identity signature")
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        self.recipient()?;
        self.verifying_key()?;
        Ok(())
    }

    pub fn fingerprint(&self) -> anyhow::Result<String> {
        self.validate()?;
        let digest = Sha256::digest(serde_json::to_vec(self)?);
        Ok(digest
            .chunks(4)
            .map(|part| {
                use std::fmt::Write as _;
                part.iter().fold(String::new(), |mut text, byte| {
                    write!(text, "{byte:02x}").expect("formatting into String");
                    text
                })
            })
            .collect::<Vec<_>>()
            .join("-"))
    }

    fn recipient(&self) -> anyhow::Result<age::x25519::Recipient> {
        self.encryption
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid encryption public key"))
    }

    fn verifying_key(&self) -> anyhow::Result<VerifyingKey> {
        let bytes: [u8; 32] = STANDARD
            .decode(&self.signing)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid signing public key length"))?;
        let key = VerifyingKey::from_bytes(&bytes)?;
        if key.is_weak() {
            bail!("Weak signing public key");
        }
        Ok(key)
    }
}

// Deliberately no Debug or public serde implementation for private identities.
pub struct Identity {
    encryption: age::x25519::Identity,
    signing: SigningKey,
}

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
struct RecoveryMaterial {
    encryption: String,
    signing: String,
}

impl Identity {
    #[must_use]
    pub fn sign_statement(&self, domain: &str, body: &[u8]) -> String {
        STANDARD.encode(self.signing.sign(&signing_input(domain, body)).to_bytes())
    }
    pub fn generate() -> anyhow::Result<Self> {
        let mut seed = Zeroizing::new([0_u8; 32]);
        getrandom::getrandom(seed.as_mut())
            .map_err(|_| anyhow::anyhow!("Secure random generator unavailable"))?;
        Ok(Self {
            encryption: age::x25519::Identity::generate(),
            signing: SigningKey::from_bytes(&seed),
        })
    }

    #[must_use]
    pub fn public(&self) -> PublicIdentity {
        PublicIdentity {
            encryption: self.encryption.to_public().to_string(),
            signing: STANDARD.encode(self.signing.verifying_key().to_bytes()),
        }
    }

    /// Returns a secret recovery code. Never log it or include it in snapshots.
    pub fn recovery_key(&self) -> anyhow::Result<SecretString> {
        let material = RecoveryMaterial {
            encryption: self.encryption.to_string().expose_secret().to_owned(),
            signing: STANDARD.encode(Zeroizing::new(self.signing.to_bytes()).as_slice()),
        };
        let bytes = Zeroizing::new(serde_json::to_vec(&material)?);
        Ok(SecretString::from(format!(
            "{RECOVERY_PREFIX}{}",
            STANDARD.encode(&*bytes)
        )))
    }

    pub fn recover(code: &SecretString) -> anyhow::Result<Self> {
        let encoded = code
            .expose_secret()
            .trim()
            .strip_prefix(RECOVERY_PREFIX)
            .context("Unsupported Wisp recovery key")?;
        let decoded = Zeroizing::new(
            STANDARD
                .decode(encoded)
                .context("Invalid recovery key encoding")?,
        );
        let material: RecoveryMaterial = serde_json::from_slice(&decoded)
            .map_err(|_| anyhow::anyhow!("Invalid recovery key"))?;
        let encryption = material
            .encryption
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid recovery encryption key"))?;
        let seed = Zeroizing::new(
            STANDARD
                .decode(&material.signing)
                .context("Invalid recovery signing key")?,
        );
        let seed: &[u8; 32] = seed
            .as_slice()
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid recovery signing key length"))?;
        Ok(Self {
            encryption,
            signing: SigningKey::from_bytes(seed),
        })
    }

    /// Context is bound inside authenticated ciphertext and a sender signature.
    /// It must include server identity, conversation, sender, message and purpose.
    pub fn seal(
        &self,
        context: &str,
        plaintext: &[u8],
        recipients: &[PublicIdentity],
    ) -> anyhow::Result<Vec<u8>> {
        let body = Zeroizing::new(serde_json::to_vec(&SignedContent {
            context: context.to_owned(),
            plaintext: STANDARD.encode(plaintext),
            signature: STANDARD.encode(
                self.signing
                    .sign(&signing_input(context, plaintext))
                    .to_bytes(),
            ),
        })?);
        let mut encrypted = Vec::new();
        encrypt_stream(body.as_slice(), &mut encrypted, recipients)?;
        Ok(encrypted)
    }

    pub fn open(
        &self,
        context: &str,
        ciphertext: &[u8],
        sender: &PublicIdentity,
    ) -> anyhow::Result<Zeroizing<Vec<u8>>> {
        let mut decrypted = Zeroizing::new(Vec::new());
        self.decrypt_stream(ciphertext, &mut *decrypted)?;
        let decoded: SignedContent = serde_json::from_slice(&decrypted)
            .map_err(|_| anyhow::anyhow!("Invalid encrypted message"))?;
        if decoded.context != context {
            bail!("Encrypted message belongs to a different context");
        }
        let plaintext = Zeroizing::new(STANDARD.decode(&decoded.plaintext)?);
        let signature = Signature::from_slice(&STANDARD.decode(&decoded.signature)?)?;
        sender
            .verifying_key()?
            .verify_strict(&signing_input(context, &plaintext), &signature)
            .context("Message sender signature failed")?;
        Ok(plaintext)
    }

    /// Write only to a private temporary destination. Callers must discard all
    /// output on ANY error (including truncated final chunks) before publishing
    /// the completed file. Bind the encrypted file digest in a signed message.
    pub fn decrypt_stream(&self, input: impl Read, mut output: impl Write) -> anyhow::Result<u64> {
        let decryptor = age::Decryptor::new(input).context("Invalid encrypted file")?;
        let mut reader = decryptor
            .decrypt(std::iter::once(&self.encryption as &dyn age::Identity))
            .context("This recovery key cannot decrypt the file")?;
        std::io::copy(&mut reader, &mut output).context("Encrypted file authentication failed")
    }
}

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
struct SignedContent {
    context: String,
    plaintext: String,
    signature: String,
}

fn signing_input(context: &str, plaintext: &[u8]) -> Vec<u8> {
    // Domain separated, length-independent transcript: signatures authenticate
    // a fixed SHA-256 digest rather than duplicating arbitrary-sized content.
    let mut digest = Sha256::new();
    digest.update(b"Wisp signed content v1\0");
    digest.update(Sha256::digest(context.as_bytes()));
    digest.update(Sha256::digest(plaintext));
    digest.finalize().to_vec()
}

/// Standard age streaming encryption: bounded memory, authenticated chunks and
/// an authenticated final chunk. All recipients (including sender for history)
/// must be explicitly supplied and verified by the caller.
pub fn encrypt_stream(
    mut input: impl Read,
    output: impl Write,
    recipients: &[PublicIdentity],
) -> anyhow::Result<u64> {
    if recipients.is_empty() {
        bail!("No verified recipients supplied");
    }
    let keys = recipients
        .iter()
        .map(PublicIdentity::recipient)
        .collect::<anyhow::Result<Vec<_>>>()?;
    let encryptor =
        age::Encryptor::with_recipients(keys.iter().map(|key| key as &dyn age::Recipient))?;
    let mut writer = encryptor.wrap_output(output)?;
    let size = std::io::copy(&mut input, &mut writer)?;
    writer.finish()?;
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_restores_identity_and_large_messages_without_server_secrets() {
        let alice = Identity::generate().unwrap();
        let bob = Identity::generate().unwrap();
        let recovered = Identity::recover(&bob.recovery_key().unwrap()).unwrap();
        assert_eq!(bob.public(), recovered.public());
        let plaintext = "Private message 🙂\n".repeat(200_000);
        let ciphertext = alice
            .seal(
                "server/room/alice/message-1/text",
                plaintext.as_bytes(),
                &[alice.public(), bob.public()],
            )
            .unwrap();
        assert!(!ciphertext.windows(15).any(|w| w == b"Private message"));
        assert_eq!(
            &*recovered
                .open(
                    "server/room/alice/message-1/text",
                    &ciphertext,
                    &alice.public()
                )
                .unwrap(),
            plaintext.as_bytes()
        );
        assert_eq!(
            &*alice
                .open(
                    "server/room/alice/message-1/text",
                    &ciphertext,
                    &alice.public()
                )
                .unwrap(),
            plaintext.as_bytes()
        );
    }

    #[test]
    fn strangers_wrong_senders_relocation_and_tampering_fail_closed() {
        let alice = Identity::generate().unwrap();
        let bob = Identity::generate().unwrap();
        let stranger = Identity::generate().unwrap();
        let cipher = alice
            .seal("dm-1/message-1", b"secret", &[alice.public(), bob.public()])
            .unwrap();
        assert!(
            stranger
                .open("dm-1/message-1", &cipher, &alice.public())
                .is_err()
        );
        assert!(
            bob.open("dm-2/message-1", &cipher, &alice.public())
                .is_err()
        );
        assert!(
            bob.open("dm-1/message-1", &cipher, &stranger.public())
                .is_err()
        );
        for position in [0, cipher.len() / 2, cipher.len() - 1] {
            let mut broken = cipher.clone();
            broken[position] ^= 1;
            assert!(
                bob.open("dm-1/message-1", &broken, &alice.public())
                    .is_err()
            );
        }
        assert!(
            bob.open(
                "dm-1/message-1",
                &cipher[..cipher.len() - 1],
                &alice.public()
            )
            .is_err()
        );
    }

    #[test]
    fn streamed_attachments_detect_truncation_and_recovery_preserves_access() {
        let alice = Identity::generate().unwrap();
        let recovered = Identity::recover(&alice.recovery_key().unwrap()).unwrap();
        let source = vec![42; 4 * 1024 * 1024 + 19];
        let mut cipher = Vec::new();
        assert_eq!(
            encrypt_stream(source.as_slice(), &mut cipher, &[alice.public()]).unwrap(),
            source.len() as u64
        );
        let mut output = Vec::new();
        recovered
            .decrypt_stream(cipher.as_slice(), &mut output)
            .unwrap();
        assert_eq!(source, output);
        assert!(
            recovered
                .decrypt_stream(&cipher[..cipher.len() - 1], std::io::sink())
                .is_err()
        );
        assert!(encrypt_stream(&b"text"[..], std::io::sink(), &[]).is_err());
    }
}
