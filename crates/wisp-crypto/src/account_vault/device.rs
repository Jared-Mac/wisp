//! Prospective credentials are staged by the client before authentication. Only
//! their public binding is sent until a successful finish activates the hash.
use super::{is_digest, operation::DeviceBinding};
use crate::SecretString;
use age::secrecy::ExposeSecret;
use anyhow::ensure;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::Zeroizing;

pub struct ProspectiveDevice {
    id: Uuid,
    token: SecretString,
}
impl ProspectiveDevice {
    pub fn generate() -> anyhow::Result<Self> {
        let mut bytes = Zeroizing::new([0_u8; 32]);
        getrandom::getrandom(bytes.as_mut())
            .map_err(|_| anyhow::anyhow!("Secure random generator unavailable"))?;
        Ok(Self {
            id: Uuid::new_v4(),
            token: SecretString::from(format!(
                "wisp-device:{}",
                URL_SAFE_NO_PAD.encode(bytes.as_slice())
            )),
        })
    }
    pub fn restore_private(id: Uuid, token: SecretString) -> anyhow::Result<Self> {
        ensure!(!id.is_nil(), "Invalid prospective device");
        let text = token
            .expose_secret()
            .strip_prefix("wisp-device:")
            .ok_or_else(|| anyhow::anyhow!("Invalid prospective device token"))?;
        ensure!(text.len() == 43, "Invalid prospective device token");
        let bytes = Zeroizing::new(
            URL_SAFE_NO_PAD
                .decode(text)
                .map_err(|_| anyhow::anyhow!("Invalid prospective device token"))?,
        );
        ensure!(
            bytes.len() == 32 && URL_SAFE_NO_PAD.encode(&*bytes) == text,
            "Invalid prospective device token"
        );
        Ok(Self { id, token })
    }
    #[must_use]
    pub fn binding(&self) -> DeviceBinding {
        DeviceBinding::Prospective {
            id: self.id,
            token_sha256: format!(
                "{:x}",
                Sha256::digest(self.token.expose_secret().as_bytes())
            ),
        }
    }
    /// Private staged storage and the existing /v1/sessions API only.
    #[must_use]
    pub fn token(&self) -> &SecretString {
        &self.token
    }
    #[must_use]
    pub fn id(&self) -> Uuid {
        self.id
    }
}

/// The signed context uses lowercase hex, while existing `devices.token_hash`
/// stores unpadded URL-safe base64 of the same SHA-256 digest bytes. Hash the
/// UTF-8 token text, never its decoded random bytes or the supplied digest text.
pub fn legacy_token_hash(token_sha256: &str) -> anyhow::Result<String> {
    ensure!(
        is_digest(token_sha256),
        "Invalid prospective credential digest"
    );
    let mut bytes = [0_u8; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte =
            u8::from_str_radix(&token_sha256[i * 2..i * 2 + 2], 16).expect("validated hex digest");
    }
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prospective_hash_matches_existing_server_utf8_token_hash() {
        let device = ProspectiveDevice::generate().unwrap();
        let binding = device.binding();
        let DeviceBinding::Prospective { token_sha256, .. } = &binding else {
            unreachable!()
        };
        let expected =
            URL_SAFE_NO_PAD.encode(Sha256::digest(device.token().expose_secret().as_bytes()));
        assert_eq!(legacy_token_hash(token_sha256).unwrap(), expected);
        let restored = ProspectiveDevice::restore_private(
            device.id(),
            SecretString::from(device.token().expose_secret().to_owned()),
        )
        .unwrap();
        assert_eq!(binding, restored.binding());
        assert_ne!(binding, ProspectiveDevice::generate().unwrap().binding());
        assert!(legacy_token_hash(&token_sha256.to_ascii_uppercase()).is_err());
    }
}
