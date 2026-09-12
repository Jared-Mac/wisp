//! Readable aliases wrap a v2 invitation without storing its fragment secret.
//! The word is reserved while active; twelve random digits are the secret.
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use anyhow::{anyhow, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use url::Url;
use zeroize::Zeroizing;

/// Curated ordinary words only. Hyphens separate words when the pool grows.
pub const WORDS: &[&str] = &[
    "acorn", "apple", "apron", "arrow", "atlas", "badge", "bamboo", "basket", "beacon", "berry",
    "birch", "bloom", "boat", "book", "bottle", "breeze", "brook", "bubble", "bucket", "button",
    "cabin", "cactus", "candle", "canvas", "carrot", "cedar", "cherry", "citrus", "cloud",
    "clover", "cobalt", "cocoa", "comet", "coral", "cotton", "crayon", "cricket", "crystal",
    "daisy", "dawn", "delta", "denim", "dune", "elm", "ember", "fern", "field", "finch", "flame",
    "flannel", "flint", "flower", "forest", "fossil", "fountain", "frost", "garden", "ginger",
    "glacier", "glade", "globe", "granite", "grape", "grove", "harbor", "hazel", "heron", "honey",
    "indigo", "iris", "ivy", "jade", "jasmine", "jigsaw", "juniper", "kettle", "kite", "kiwi",
    "lagoon", "lake", "lantern", "larch", "laurel", "leaf", "lemon", "lilac", "linen", "lotus",
    "lumen", "maple", "marble", "marigold", "meadow", "melon", "mint", "moon", "moss", "mountain",
    "nectar", "nest", "notebook", "nova", "nutmeg", "oak", "ocean", "olive", "opal", "orbit",
    "orchid", "otter", "owl", "paddle", "paper", "peach", "pearl", "pebble", "pencil", "petal",
    "pine", "planet", "plum", "pond", "poppy", "prism", "puzzle", "quartz", "quilt", "rain",
    "reed", "ribbon", "river", "robin", "rocket", "rose", "rowan", "ruby", "saffron", "sage",
    "sail", "sand", "seed", "shell", "silver", "sky", "slate", "snow", "solar", "sparrow",
    "sprout", "star", "stone", "stream", "summit", "sun", "sunrise", "tea", "teal", "thistle",
    "thyme", "tide", "timber", "toast", "topaz", "trail", "tree", "tulip", "tundra", "valley",
    "velvet", "violet", "walnut", "water", "wave", "willow", "wind", "wren",
];
#[must_use]
pub fn valid_label(label: &str) -> bool {
    let words: Vec<_> = label.split('-').collect();
    (1..=4).contains(&words.len()) && words.iter().all(|word| WORDS.contains(word))
}

#[must_use]
pub fn valid_code(code: &str) -> bool {
    let Some(index) = code.find(|c: char| c.is_ascii_digit()) else {
        return false;
    };
    let (label, digits) = code.split_at(index);
    valid_label(label)
        && digits.len() == 12
        && digits.bytes().all(|b| b.is_ascii_digit())
        && !digits.contains("69")
        && !digits.contains("666")
}

pub struct ShortLink {
    pub origin: String,
    code: Zeroizing<String>,
    key: Zeroizing<[u8; 32]>,
}
impl ShortLink {
    pub fn create(origin: &str, label: &str) -> anyhow::Result<Self> {
        ensure!(valid_label(label), "Invalid invitation word");
        loop {
            let mut digits = String::new();
            while digits.len() < 12 {
                let mut byte = [0];
                getrandom::getrandom(&mut byte)
                    .map_err(|_| anyhow!("Secure random source unavailable"))?;
                if byte[0] < 250 {
                    digits.push(char::from(b'0' + byte[0] % 10));
                }
            }
            let code = format!("{label}{digits}");
            if valid_code(&code) {
                return Self::from_code(origin, code);
            }
        }
    }
    fn from_code(origin: &str, code: String) -> anyhow::Result<Self> {
        ensure!(valid_code(&code), "Invalid invitation code");
        let code = Zeroizing::new(code);
        let mut key = Zeroizing::new([0; 32]);
        // The numeric code is deliberately speakable. Memory-hard derivation
        // limits offline guessing if encrypted invitation records are stolen.
        argon2::Argon2::default()
            .hash_password_into(code.as_bytes(), b"wisp-short-invite-v1", &mut *key)
            .map_err(|_| anyhow!("Could not prepare invitation"))?;
        Ok(Self {
            origin: crate::invitation::origin(origin)?,
            code,
            key,
        })
    }
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        ensure!(value.len() <= 2048, "Invalid invitation");
        let url = Url::parse(value).map_err(|_| anyhow!("Invalid invitation"))?;
        ensure!(
            url.query().is_none() && url.fragment().is_none(),
            "Invalid invitation"
        );
        Self::from_code(
            &crate::invitation::origin(value)?,
            url.path().trim_start_matches('/').to_owned(),
        )
    }
    #[must_use]
    pub fn uri(&self) -> String {
        format!("{}/{}", self.origin, self.code.as_str())
    }
    #[must_use]
    pub fn lookup_id(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(b"wisp-short-invite-v1\n");
        hash.update(*self.key);
        URL_SAFE_NO_PAD.encode(hash.finalize())
    }
    fn aad(&self) -> String {
        format!(
            "wisp-short-invite-v1\n{}\n{}",
            self.origin,
            self.lookup_id()
        )
    }
    pub fn seal(&self, uri: &str) -> anyhow::Result<String> {
        crate::invitation::Link::parse(uri)?;
        let mut nonce = [0; 12];
        getrandom::getrandom(&mut nonce)
            .map_err(|_| anyhow!("Secure random source unavailable"))?;
        let cipher = Aes256Gcm::new_from_slice(&*self.key).expect("32-byte AES key");
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: uri.as_bytes(),
                    aad: self.aad().as_bytes(),
                },
            )
            .map_err(|_| anyhow!("Could not encrypt invitation"))?;
        let mut bytes = nonce.to_vec();
        bytes.extend(ciphertext);
        Ok(URL_SAFE_NO_PAD.encode(bytes))
    }
    pub fn open(&self, envelope: &str) -> anyhow::Result<String> {
        ensure!(envelope.len() <= 4096, "Invalid invitation");
        let bytes = URL_SAFE_NO_PAD
            .decode(envelope)
            .map_err(|_| anyhow!("Invalid invitation"))?;
        ensure!(bytes.len() >= 28, "Invalid invitation");
        let cipher = Aes256Gcm::new_from_slice(&*self.key).expect("32-byte AES key");
        let plain = cipher
            .decrypt(
                Nonce::from_slice(&bytes[..12]),
                Payload {
                    msg: &bytes[12..],
                    aad: self.aad().as_bytes(),
                },
            )
            .map_err(|_| anyhow!("Invitation verification failed"))?;
        let uri = String::from_utf8(plain).map_err(|_| anyhow!("Invalid invitation"))?;
        crate::invitation::Link::parse(&uri)?;
        Ok(uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_independent_python_argon2_and_aes_gcm_vector() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/invitations/readable-v1.json"
        ))
        .unwrap();
        let link = ShortLink::parse(fixture["url"].as_str().unwrap()).unwrap();
        assert_eq!(link.lookup_id(), fixture["lookup_id"]);
        assert_eq!(URL_SAFE_NO_PAD.encode(*link.key), fixture["encryption_key"]);
        assert_eq!(link.aad(), fixture["aad"]);
        assert_eq!(
            link.open(fixture["envelope"].as_str().unwrap()).unwrap(),
            fixture["plaintext"]
        );
    }
    #[test]
    fn readable_alias_is_private_and_binds_origin() {
        let link = ShortLink::create("https://wisp.invalid", "tea").unwrap();
        let uri = crate::invitation::Link::create("https://server.invalid")
            .unwrap()
            .uri();
        let encrypted = link.seal(&uri).unwrap();
        assert_eq!(
            ShortLink::parse(&link.uri())
                .unwrap()
                .open(&encrypted)
                .unwrap(),
            uri
        );
        assert!(
            ShortLink::parse(&link.uri().replace("wisp.invalid", "other.invalid"))
                .unwrap()
                .open(&encrypted)
                .is_err()
        );
        assert!(
            !String::from_utf8_lossy(&URL_SAFE_NO_PAD.decode(&encrypted).unwrap())
                .contains("server.invalid")
        );
        assert!(ShortLink::parse(&(link.uri() + "?tracking=1")).is_err());
        assert!(valid_code("tea123456781234"));
        assert!(!valid_code("tea123456991234"));
        assert!(!valid_code("tea123466661234"));
        assert!(!valid_code("unknown123456781234"));
    }
}
