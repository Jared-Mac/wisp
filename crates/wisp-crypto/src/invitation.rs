//! Version 2 server invitations. A URL fragment contains a fresh secret; the
//! coordination server stores only the lookup ID and authenticated ciphertext.
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use anyhow::{Context, anyhow, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use url::Url;
use uuid::Uuid;
use wisp_protocol::UserSummary;
use zeroize::Zeroizing;

const SALT: &[u8] = b"wisp-server-invite-v2";
pub const MAX_ENVELOPE: usize = 12000;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invitation {
    pub v: u8,
    pub server: String,
    pub id: Uuid,
    pub code: String,
    pub server_name: String,
    pub inviter: UserSummary,
    pub expires_at: DateTime<Utc>,
    pub media_key: Option<String>,
}

pub struct Link {
    pub origin: String,
    secret: Zeroizing<[u8; 32]>,
}

pub fn origin(value: &str) -> anyhow::Result<String> {
    let url = Url::parse(value).map_err(|_| anyhow!("Invalid Wisp server address"))?;
    let local = url
        .host_str()
        .is_some_and(|host| matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1"));
    ensure!(
        url.scheme() == "https" || (url.scheme() == "http" && local),
        "Public Wisp servers require HTTPS"
    );
    ensure!(
        url.username().is_empty() && url.password().is_none() && url.host_str().is_some(),
        "Invalid Wisp server address"
    );
    Ok(url.origin().ascii_serialization())
}

impl Link {
    pub fn create(server: &str) -> anyhow::Result<Self> {
        let mut secret = Zeroizing::new([0_u8; 32]);
        getrandom::getrandom(&mut *secret)
            .map_err(|_| anyhow!("Secure random source unavailable"))?;
        Ok(Self {
            origin: origin(server)?,
            secret,
        })
    }

    pub fn parse(value: &str) -> anyhow::Result<Self> {
        ensure!(value.len() <= 2048, "Invalid Wisp invitation");
        let url = Url::parse(value).map_err(|_| anyhow!("Invalid Wisp invitation"))?;
        ensure!(
            url.path() == "/join/" && url.query().is_none(),
            "Invalid Wisp invitation"
        );
        let encoded = url
            .fragment()
            .and_then(|s| s.strip_prefix("v2."))
            .context("This invitation requires a newer Wisp version")?;
        ensure!(encoded.len() == 43, "Invalid Wisp invitation");
        let bytes = Zeroizing::new(
            URL_SAFE_NO_PAD
                .decode(encoded)
                .map_err(|_| anyhow!("Invalid Wisp invitation"))?,
        );
        let secret: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| anyhow!("Invalid Wisp invitation"))?;
        Ok(Self {
            origin: origin(value)?,
            secret: Zeroizing::new(secret),
        })
    }

    #[must_use]
    pub fn uri(&self) -> String {
        format!(
            "{}/join/#v2.{}",
            self.origin,
            URL_SAFE_NO_PAD.encode(*self.secret)
        )
    }

    fn derive(&self, label: &[u8]) -> Zeroizing<[u8; 32]> {
        let hk = Hkdf::<Sha256>::new(Some(SALT), &*self.secret);
        let mut key = Zeroizing::new([0_u8; 32]);
        hk.expand(label, &mut *key).expect("32-byte HKDF expansion");
        key
    }

    #[must_use]
    pub fn lookup_id(&self) -> String {
        URL_SAFE_NO_PAD.encode(*self.derive(b"lookup"))
    }

    fn aad(&self) -> String {
        format!(
            "wisp-server-invite-v2\n{}\n{}",
            self.origin,
            self.lookup_id()
        )
    }

    pub fn seal(&self, invitation: &Invitation) -> anyhow::Result<String> {
        let mut nonce = [0_u8; 12];
        getrandom::getrandom(&mut nonce)
            .map_err(|_| anyhow!("Secure random source unavailable"))?;
        self.seal_with_nonce(invitation, &nonce)
    }

    fn seal_with_nonce(&self, invitation: &Invitation, nonce: &[u8; 12]) -> anyhow::Result<String> {
        ensure!(
            invitation.v == 2 && origin(&invitation.server)? == self.origin,
            "Invalid invitation issuer"
        );
        let plaintext = Zeroizing::new(serde_json::to_vec(invitation)?);
        ensure!(plaintext.len() <= 8000, "Invitation is too large");
        let key = self.derive(b"encryption");
        let cipher = Aes256Gcm::new_from_slice(&*key).expect("32-byte AES key");
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: &plaintext,
                    aad: self.aad().as_bytes(),
                },
            )
            .map_err(|_| anyhow!("Could not encrypt invitation"))?;
        let mut envelope = Vec::with_capacity(12 + ciphertext.len());
        envelope.extend(nonce);
        envelope.extend(ciphertext);
        Ok(URL_SAFE_NO_PAD.encode(envelope))
    }

    pub fn open(&self, envelope: &str) -> anyhow::Result<Invitation> {
        ensure!(
            envelope.len() <= MAX_ENVELOPE,
            "Invalid encrypted invitation"
        );
        let bytes = URL_SAFE_NO_PAD
            .decode(envelope)
            .map_err(|_| anyhow!("Invalid encrypted invitation"))?;
        ensure!(bytes.len() >= 28, "Invalid encrypted invitation");
        let key = self.derive(b"encryption");
        let cipher = Aes256Gcm::new_from_slice(&*key).expect("32-byte AES key");
        let plaintext = Zeroizing::new(
            cipher
                .decrypt(
                    Nonce::from_slice(&bytes[..12]),
                    Payload {
                        msg: &bytes[12..],
                        aad: self.aad().as_bytes(),
                    },
                )
                .map_err(|_| anyhow!("Invitation verification failed"))?,
        );
        let invitation: Invitation = serde_json::from_slice(&plaintext)
            .map_err(|_| anyhow!("Invalid invitation details"))?;
        ensure!(
            invitation.v == 2 && invitation.server == self.origin,
            "Invalid invitation issuer"
        );
        ensure!(
            !invitation.code.is_empty()
                && invitation.code.len() <= 256
                && invitation.server_name.chars().count() <= 80,
            "Invalid invitation details"
        );
        ensure!(
            invitation
                .media_key
                .as_ref()
                .is_none_or(|key| (16..=1024).contains(&key.len())),
            "Invalid invitation media key"
        );
        Ok(invitation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Invitation {
        Invitation {
            v: 2,
            server: "https://wisp.invalid".into(),
            id: Uuid::nil(),
            code: "test-only-invite".into(),
            server_name: "Test community".into(),
            inviter: UserSummary {
                id: Uuid::nil(),
                display_name: "Example".into(),
            },
            expires_at: "2030-01-01T00:00:00Z".parse().unwrap(),
            media_key: Some("test-only-media-key".into()),
        }
    }
    #[test]
    fn encrypted_invitation_binds_secret_and_origin() {
        let link = Link::create("https://wisp.invalid").unwrap();
        let envelope = link.seal(&sample()).unwrap();
        assert_eq!(
            Link::parse(&link.uri())
                .unwrap()
                .open(&envelope)
                .unwrap()
                .code,
            "test-only-invite"
        );
        assert!(
            !String::from_utf8_lossy(&URL_SAFE_NO_PAD.decode(&envelope).unwrap())
                .contains("test-only-media-key")
        );
        assert!(
            Link::create("https://wisp.invalid")
                .unwrap()
                .open(&envelope)
                .is_err()
        );
        let redirected = Link::parse(&link.uri().replace("wisp.invalid", "other.invalid")).unwrap();
        assert!(redirected.open(&envelope).is_err());
        let mut bytes = URL_SAFE_NO_PAD.decode(envelope).unwrap();
        bytes[15] ^= 1;
        assert!(link.open(&URL_SAFE_NO_PAD.encode(bytes)).is_err());
    }
    #[test]
    fn rejects_ambiguous_and_insecure_links() {
        let link = Link::create("https://wisp.invalid").unwrap().uri();
        for changed in [
            link.replace("https:", "http:"),
            link.replace("/join/", "/other/"),
            link.replace('#', "?tracking=1#"),
            link.replace("wisp.invalid", "user@wisp.invalid"),
            link[..link.len() - 1].to_owned(),
        ] {
            assert!(Link::parse(&changed).is_err());
        }
    }
    #[test]
    fn matches_independent_node_aes_gcm_vector() {
        let vector: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/invitations/v2.json"))
                .unwrap();
        let link = Link::parse(vector["url"].as_str().unwrap()).unwrap();
        assert_eq!(link.lookup_id(), vector["lookup_id"]);
        assert_eq!(
            URL_SAFE_NO_PAD.encode(*link.derive(b"encryption")),
            vector["encryption_key"]
        );
        assert_eq!(link.aad(), vector["aad"]);
        let nonce: [u8; 12] = URL_SAFE_NO_PAD
            .decode(vector["nonce"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(
            serde_json::to_string(&sample()).unwrap(),
            vector["plaintext"]
        );
        assert_eq!(
            link.seal_with_nonce(&sample(), &nonce).unwrap(),
            vector["envelope"]
        );
        assert_eq!(
            link.open(vector["envelope"].as_str().unwrap())
                .unwrap()
                .code,
            "test-only-invite"
        );
    }
}
