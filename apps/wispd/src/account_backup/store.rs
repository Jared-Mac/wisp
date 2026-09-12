use anyhow::{Context, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::Mutex,
};
use uuid::Uuid;
use wisp_crypto::{
    account_vault::{
        Scope,
        auth::{self, ServerPin},
        bundle::{Bundle, MAX_PLAINTEXT, TrustState},
        envelope::{Checkpoint, VaultKey},
        operation::Precondition,
    },
    keyring::TrustStore,
};
use zeroize::Zeroizing;

const MAX_RECORD: u64 = 48 * 1024 * 1024;

/// Private at-rest serialization is deliberate and has no Debug implementation.
/// This wrapper must never appear in public HTTP/IPC response types.
#[derive(Clone)]
pub(crate) struct PrivateBytes(Zeroizing<String>);
impl PrivateBytes {
    pub(crate) fn new(bytes: &[u8]) -> Self {
        Self(Zeroizing::new(STANDARD.encode(bytes)))
    }
    pub(crate) fn decode(&self, limit: usize) -> anyhow::Result<Zeroizing<Vec<u8>>> {
        ensure!(
            self.0.len() <= limit.div_ceil(3) * 4,
            "Private account field exceeds its bound"
        );
        let bytes = Zeroizing::new(
            STANDARD
                .decode(self.0.as_bytes())
                .context("Invalid private account encoding")?,
        );
        ensure!(
            bytes.len() <= limit && STANDARD.encode(&*bytes) == *self.0,
            "Invalid private account encoding"
        );
        Ok(bytes)
    }
    pub(crate) fn json<T: Serialize>(value: &T) -> anyhow::Result<Self> {
        let bytes = Zeroizing::new(serde_json::to_vec(value)?);
        Ok(Self::new(&bytes))
    }
    pub(crate) fn value(&self) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::from_slice(&self.decode(16 * 1024 * 1024)?)?)
    }
}
impl Serialize for PrivateBytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for PrivateBytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self(Zeroizing::new(String::deserialize(deserializer)?)))
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    Signup,
    Login,
    Migration,
    Password,
    Rewrap,
    Email,
    Reset,
    Sync,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StagedDevice {
    pub(crate) id: Uuid,
    pub(crate) token: PrivateBytes,
}
impl StagedDevice {
    pub(crate) fn generate() -> anyhow::Result<Self> {
        use age::secrecy::ExposeSecret;
        let device = wisp_crypto::account_vault::device::ProspectiveDevice::generate()?;
        Ok(Self {
            id: device.id(),
            token: PrivateBytes::new(device.token().expose_secret().as_bytes()),
        })
    }
    pub(crate) fn native(
        &self,
    ) -> anyhow::Result<wisp_crypto::account_vault::device::ProspectiveDevice> {
        let bytes = self.token.decode(128)?;
        let text = std::str::from_utf8(&bytes).context("Invalid staged credential")?;
        wisp_crypto::account_vault::device::ProspectiveDevice::restore_private(
            self.id,
            wisp_crypto::SecretString::from(text.to_owned()),
        )
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Pending {
    pub(crate) kind: Kind,
    pub(crate) id: Uuid,
    pub(crate) scope: Option<Scope>,
    pub(crate) identity: Option<wisp_crypto::PublicIdentity>,
    pub(crate) username: Option<String>,
    pub(crate) generation: Option<Uuid>,
    pub(crate) device: Option<StagedDevice>,
    pub(crate) key: Option<PrivateBytes>,
    pub(crate) bundle: Option<PrivateBytes>,
    /// Public handshake/staging context, including no raw password or export key.
    pub(crate) start: Option<PrivateBytes>,
    /// Exact retry body. Renewable authorization is separate from effect_digest.
    pub(crate) finish: Option<PrivateBytes>,
    pub(crate) effect_digest: Option<String>,
    pub(crate) sent: bool,
    /// Never overwrite an outcome-unknown operation to recover its session.
    pub(crate) recovery: Vec<Pending>,
}
impl Pending {
    pub(crate) fn new(kind: Kind, id: Uuid) -> Self {
        Self {
            kind,
            id,
            scope: None,
            identity: None,
            username: None,
            generation: None,
            device: None,
            key: None,
            bundle: None,
            start: None,
            finish: None,
            effect_digest: None,
            sent: false,
            recovery: Vec::new(),
        }
    }
    fn validate(&self, origin: &str, depth: usize) -> anyhow::Result<()> {
        ensure!(
            !self.id.is_nil() && depth <= 1 && self.recovery.len() <= 8,
            "Invalid pending account operation"
        );
        if let Some(scope) = &self.scope {
            scope.validate()?;
            ensure!(
                scope.origin == origin,
                "Pending operation belongs to another service"
            );
        }
        if let Some(username) = &self.username {
            ensure!(
                auth::canonical_username(username)? == *username,
                "Invalid pending username"
            );
        }
        if let Some(generation) = self.generation {
            ensure!(!generation.is_nil(), "Invalid pending generation");
        }
        if let Some(device) = &self.device {
            device.native()?;
        }
        if let Some(key) = &self.key {
            VaultKey::restore_private(&key.decode(32)?)?;
        }
        if let Some(bundle) = &self.bundle {
            let scope = self
                .scope
                .as_ref()
                .context("Missing staged account scope")?;
            Bundle::decode_private(
                &bundle.decode(MAX_PLAINTEXT)?,
                scope,
                self.identity.as_ref().context("Missing staged identity")?,
            )?;
        }
        for value in [&self.start, &self.finish].into_iter().flatten() {
            let value = value.value()?;
            fn public_transport(value: &serde_json::Value, depth: usize) -> bool {
                if depth > 24 {
                    return false;
                }
                match value {
                    serde_json::Value::Object(map) => map.iter().all(|(key, value)| {
                        !matches!(
                            key.as_str(),
                            "password"
                                | "new_password"
                                | "current_password"
                                | "legacy_password"
                                | "export_key"
                        ) && public_transport(value, depth + 1)
                    }),
                    serde_json::Value::Array(values) => {
                        values.iter().all(|v| public_transport(v, depth + 1))
                    }
                    _ => true,
                }
            }
            ensure!(
                public_transport(&value, 0),
                "Transient authentication secrets cannot be saved in the account journal"
            );
        }
        ensure!(
            !self.sent || self.finish.is_some(),
            "An unknown outcome must retain its exact retry body"
        );
        if let Some(digest) = &self.effect_digest {
            ensure!(
                digest.len() == 64
                    && digest
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "Invalid pending effect digest"
            );
        }
        for recovery in &self.recovery {
            ensure!(
                recovery.kind == Kind::Login && recovery.id != self.id,
                "Invalid nested recovery operation"
            );
            recovery.validate(origin, depth + 1)?;
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    version: u32,
    pub(crate) origin: String,
    pub(crate) network: Option<Uuid>,
    pub(crate) username: Option<String>,
    pub(crate) scope: Option<Scope>,
    pub(crate) identity: Option<wisp_crypto::PublicIdentity>,
    pub(crate) pin: Option<ServerPin>,
    pub(crate) secure: bool,
    pub(crate) revision: u64,
    pub(crate) dirty: u64,
    pub(crate) synced_dirty: u64,
    pub(crate) checkpoint: Option<Checkpoint>,
    pub(crate) expected: Option<Precondition>,
    pub(crate) key: Option<PrivateBytes>,
    pub(crate) key_verified: bool,
    bundle: Option<PrivateBytes>,
    pub(crate) pending: Option<Pending>,
    pub(crate) last_sync: Option<i64>,
}
impl Record {
    fn empty(origin: &str) -> Self {
        Self {
            version: 1,
            origin: origin.to_owned(),
            network: None,
            username: None,
            scope: None,
            identity: None,
            pin: None,
            secure: false,
            revision: 0,
            dirty: 0,
            synced_dirty: 0,
            checkpoint: None,
            expected: None,
            key: None,
            key_verified: false,
            bundle: None,
            pending: None,
            last_sync: None,
        }
    }
    pub(crate) fn bundle(&self) -> anyhow::Result<Option<Bundle>> {
        self.bundle
            .as_ref()
            .map(|bundle| {
                Bundle::decode_private(
                    &bundle.decode(MAX_PLAINTEXT)?,
                    self.scope.as_ref().context("Missing account scope")?,
                    self.identity.as_ref().context("Missing account identity")?,
                )
            })
            .transpose()
    }
    pub(crate) fn set_bundle(&mut self, bundle: &Bundle) -> anyhow::Result<()> {
        bundle.validate()?;
        if let Some(scope) = &self.scope {
            ensure!(scope == &bundle.scope, "Backup belongs to another account");
        }
        let identity = bundle.identity()?.public();
        if let Some(own) = &self.identity {
            ensure!(own == &identity, "Backup identity changed");
        }
        self.identity = Some(identity);
        self.network = Some(bundle.scope.network);
        self.scope = Some(bundle.scope.clone());
        self.bundle = Some(PrivateBytes::new(&bundle.encode_private()?));
        Ok(())
    }
    pub(crate) fn local_key(&self) -> anyhow::Result<VaultKey> {
        VaultKey::restore_private(
            &self
                .key
                .as_ref()
                .context("The encrypted backup is locked on this device")?
                .decode(32)?,
        )
    }
    fn validate(&self, origin: &str) -> anyhow::Result<Option<Bundle>> {
        ensure!(
            self.version == 1 && self.origin == origin && self.synced_dirty <= self.dirty,
            "Invalid private account state"
        );
        auth::AccountContext {
            origin: origin.to_owned(),
            network: Uuid::from_u128(1),
            username: "storage".into(),
        }
        .validate()?;
        if let Some(scope) = &self.scope {
            scope.validate()?;
            ensure!(
                scope.origin == origin && self.network == Some(scope.network),
                "Account service changed"
            );
        }
        ensure!(
            self.network.is_none_or(|id| !id.is_nil()),
            "Invalid account network"
        );
        ensure!(
            !self.key_verified || self.key.is_some(),
            "Verified backup key is missing"
        );
        if let Some(username) = &self.username {
            ensure!(
                auth::canonical_username(username)? == *username,
                "Invalid account username"
            );
        }
        if let Some(expected) = &self.expected {
            expected.validate()?;
        }
        if let Some(checkpoint) = &self.checkpoint {
            checkpoint.validate()?;
        }
        if self.key.is_some() {
            self.local_key()?;
        }
        let bundle = self.bundle()?;
        ensure!(
            bundle.is_none() || (self.key.is_some() && self.scope.is_some()),
            "Incomplete private account backup"
        );
        if let Some(pending) = &self.pending {
            pending.validate(origin, 0)?;
            for pending in std::iter::once(pending).chain(pending.recovery.iter()) {
                ensure!(
                    pending
                        .scope
                        .as_ref()
                        .is_none_or(|scope| self.network == Some(scope.network)),
                    "Pending account network changed"
                );
            }
        }
        Ok(bundle)
    }
}
struct Loaded {
    record: Record,
    bundle: Option<Bundle>,
}
#[derive(PartialEq, Eq)]
struct Stamp {
    inode: u64,
    size: u64,
    modified: std::time::SystemTime,
}
struct Cache {
    stamp: Option<Stamp>,
    loaded: Loaded,
}

pub(crate) struct Store {
    path: PathBuf,
    origin: String,
    cache: Mutex<Option<Cache>>,
}
struct Locked(File);
impl Drop for Locked {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

#[allow(clippy::verbose_bit_mask)]
fn check_file(metadata: &fs::Metadata) -> anyhow::Result<()> {
    ensure!(
        metadata.is_file() && metadata.mode() & 0o077 == 0 && metadata.nlink() == 1,
        "Account storage must be a private regular file without links"
    );
    Ok(())
}
#[allow(clippy::verbose_bit_mask)]
fn private_dir(path: &Path) -> anyhow::Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.into()),
    }
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_dir() && metadata.mode() & 0o077 == 0,
        "Account storage must be a private directory"
    );
    Ok(())
}
impl Store {
    pub(crate) fn at(root: &Path, origin: &str) -> anyhow::Result<Self> {
        ensure!(
            root.is_absolute(),
            "Private account storage must be absolute"
        );
        auth::AccountContext {
            origin: origin.to_owned(),
            network: Uuid::from_u128(1),
            username: "storage".into(),
        }
        .validate()?;
        private_dir(root)?;
        let folder = root.join("account-backup");
        private_dir(&folder)?;
        Ok(Self {
            path: folder.join(format!("{:x}.json", Sha256::digest(origin.as_bytes()))),
            origin: origin.to_owned(),
            cache: Mutex::new(None),
        })
    }
    #[cfg(test)]
    pub(crate) fn private_path(&self) -> &Path {
        &self.path
    }

    fn initialized(&self) -> anyhow::Result<bool> {
        match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(self.path.with_extension("initialized"))
        {
            Ok(mut file) => {
                check_file(&file.metadata()?)?;
                let mut bytes = Vec::new();
                Read::by_ref(&mut file).take(64).read_to_end(&mut bytes)?;
                ensure!(
                    bytes == b"wisp-private-account-v1\n",
                    "Invalid account storage marker"
                );
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
    fn mark_initialized(&self) -> anyhow::Result<()> {
        if self.initialized()? {
            return Ok(());
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.path.with_extension("initialized"))?;
        file.write_all(b"wisp-private-account-v1\n")?;
        file.sync_all()?;
        Ok(())
    }
    fn lock(&self, exclusive: bool) -> anyhow::Result<Locked> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(self.path.with_extension("lock"))?;
        check_file(&file.metadata()?)?;
        if exclusive {
            file.lock()?;
        } else {
            file.lock_shared()?;
        }
        Ok(Locked(file))
    }
    fn refresh<'a>(&self, cache: &'a mut Option<Cache>) -> anyhow::Result<&'a mut Cache> {
        let mut file = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&self.path)
        {
            Ok(file) => Some(file),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        ensure!(
            file.is_some() || !self.initialized()?,
            "Private account state is missing; restore it before continuing"
        );
        let stamp = file
            .as_ref()
            .map(|file| -> anyhow::Result<Stamp> {
                let meta = file.metadata()?;
                check_file(&meta)?;
                ensure!(
                    meta.len() <= MAX_RECORD,
                    "Private account record is too large"
                );
                Ok(Stamp {
                    inode: meta.ino(),
                    size: meta.len(),
                    modified: meta.modified()?,
                })
            })
            .transpose()?;
        ensure!(
            !(stamp.is_none() && cache.as_ref().is_some_and(|cache| cache.stamp.is_some())),
            "Private account state disappeared; existing keys were preserved"
        );
        if cache.as_ref().is_none_or(|cache| cache.stamp != stamp) {
            let record = if let Some(file) = &mut file {
                let mut bytes = Zeroizing::new(Vec::new());
                file.take(MAX_RECORD + 1).read_to_end(&mut bytes)?;
                ensure!(
                    bytes.len() <= MAX_RECORD as usize,
                    "Private account record is too large"
                );
                serde_json::from_slice::<Record>(&bytes)
                    .context("Private account state is damaged; existing keys were preserved")?
            } else {
                Record::empty(&self.origin)
            };
            let bundle = record.validate(&self.origin)?;
            *cache = Some(Cache {
                stamp,
                loaded: Loaded { record, bundle },
            });
        }
        Ok(cache.as_mut().expect("loaded private state"))
    }
    pub(crate) fn read<T>(
        &self,
        read: impl FnOnce(&Record, Option<&Bundle>) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let _lock = self.lock(false)?;
        let mut cache = self.cache.lock().expect("account storage lock");
        let loaded = &self.refresh(&mut cache)?.loaded;
        read(&loaded.record, loaded.bundle.as_ref())
    }
    pub(crate) fn record(&self) -> anyhow::Result<Record> {
        self.read(|record, _| Ok(record.clone()))
    }
    pub(crate) fn update<T>(
        &self,
        expected_revision: Option<u64>,
        change: impl FnOnce(&mut Record) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        self.edit(expected_revision, |record| Ok((change(record)?, true)))
    }
    fn edit<T>(
        &self,
        expected_revision: Option<u64>,
        change: impl FnOnce(&mut Record) -> anyhow::Result<(T, bool)>,
    ) -> anyhow::Result<T> {
        let _lock = self.lock(true)?;
        let mut cache = self.cache.lock().expect("account storage lock");
        let loaded = &self.refresh(&mut cache)?.loaded;
        if let Some(expected) = expected_revision {
            ensure!(
                loaded.record.revision == expected,
                "Account state changed; retry with its current state"
            );
        }
        let mut candidate = loaded.record.clone();
        let (result, changed) = change(&mut candidate)?;
        if !changed {
            return Ok(result);
        }
        ensure!(
            !loaded.record.secure || candidate.secure,
            "A secure account cannot return to classic authentication"
        );
        if let Some(scope) = &loaded.record.scope {
            ensure!(
                candidate.scope.as_ref() == Some(scope),
                "Account scope cannot change"
            );
        }
        if let Some(network) = loaded.record.network {
            ensure!(
                candidate.network == Some(network),
                "Account network cannot change"
            );
        }
        if loaded.record.key_verified {
            ensure!(
                candidate.key_verified,
                "A verified backup key cannot become unverified"
            );
            ensure!(
                candidate.local_key()?.save_private().as_slice()
                    == loaded.record.local_key()?.save_private().as_slice(),
                "Account backup key cannot change"
            );
        }
        if let Some(identity) = &loaded.record.identity {
            ensure!(
                candidate.identity.as_ref() == Some(identity),
                "Account encryption identity cannot change"
            );
        }
        if let Some(username) = &loaded.record.username {
            ensure!(
                candidate.username.as_ref() == Some(username),
                "Sign-in account cannot change during an operation"
            );
        }
        ensure!(
            candidate.dirty >= loaded.record.dirty,
            "Backup change counter cannot rewind"
        );
        if let (Some(old), Some(next)) = (&loaded.record.pending, &candidate.pending)
            && old.id == next.id
            && old.sent
        {
            ensure!(
                next.sent && next.finish.is_some(),
                "An unknown outcome cannot return to an unsubmitted state"
            );
            ensure!(
                next.kind == old.kind && next.effect_digest == old.effect_digest,
                "An unknown operation cannot change its intended effect"
            );
        }
        if let Some(pin) = &loaded.record.pin {
            ensure!(
                candidate.pin.as_ref() == Some(pin),
                "Authentication server identity changed"
            );
        }
        candidate.revision = loaded
            .record
            .revision
            .checked_add(1)
            .context("Invalid account revision")?;
        let bundle = candidate.validate(&self.origin)?;
        let bytes = Zeroizing::new(serde_json::to_vec(&candidate)?);
        ensure!(
            bytes.len() <= MAX_RECORD as usize,
            "Private account record is too large"
        );
        let parent = self
            .path
            .parent()
            .context("Missing account storage directory")?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(&self.path)
            .context("Could not save account state; previous keys were preserved")?;
        self.mark_initialized()?;
        File::open(parent)?.sync_all()?;
        let meta = fs::metadata(&self.path)?;
        *cache = Some(Cache {
            stamp: Some(Stamp {
                inode: meta.ino(),
                size: meta.len(),
                modified: meta.modified()?,
            }),
            loaded: Loaded {
                record: candidate,
                bundle,
            },
        });
        Ok(result)
    }
}
impl TrustStore for Store {
    fn snapshot(&self) -> anyhow::Result<TrustState> {
        self.read(|_, bundle| {
            Ok(bundle
                .context("Restore the account backup before using encryption")?
                .trust
                .clone())
        })
    }
    fn update(
        &self,
        change: &mut dyn FnMut(&mut TrustState) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.edit(None, |record| {
            let current = record
                .bundle()?
                .context("Restore the account backup before using encryption")?;
            let mut trust = current.trust.clone();
            change(&mut trust)?;
            if trust == current.trust {
                return Ok(((), false));
            }
            let candidate = Bundle::new(
                current.scope.clone(),
                current.identity()?.recovery_key()?,
                current.media_key().cloned(),
                trust,
            )?;
            record.set_bundle(&candidate)?;
            record.dirty = record
                .dirty
                .checked_add(1)
                .context("Invalid backup revision")?;
            Ok(((), true))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_crypto::{Identity, keyring::Keyring};
    fn fixture() -> (tempfile::TempDir, Store, Scope) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("privacy");
        let scope = Scope {
            origin: "https://example.invalid".into(),
            network: Uuid::new_v4(),
            account: Uuid::new_v4(),
        };
        let store = Store::at(&root, &scope.origin).unwrap();
        let identity = Identity::generate().unwrap();
        let mut trust = TrustState::default();
        trust.pins.insert(scope.account, identity.public());
        let bundle =
            Bundle::new(scope.clone(), identity.recovery_key().unwrap(), None, trust).unwrap();
        let key = VaultKey::generate().unwrap();
        store
            .update(None, |record| {
                record.username = Some("synthetic".into());
                record.secure = true;
                record.key = Some(PrivateBytes::new(&*key.save_private()));
                record.set_bundle(&bundle)
            })
            .unwrap();
        (dir, store, scope)
    }
    #[test]
    fn durable_trust_preserves_identity_and_rejects_conflicts_stale_commits_and_scope_changes() {
        let (dir, store, scope) = fixture();
        let store = std::sync::Arc::new(store);
        let identity = store.read(|_, bundle| bundle.unwrap().identity()).unwrap();
        let public = identity.public();
        let ring = Keyring::portable(identity, store.clone());
        let before = store.record().unwrap();
        let friend = Uuid::new_v4();
        let friend_key = Identity::generate().unwrap().public();
        ring.trust_first_use(friend, &friend_key).unwrap();
        let reopened = Store::at(&dir.path().join("privacy"), &scope.origin).unwrap();
        assert_eq!(reopened.snapshot().unwrap().pins[&friend], friend_key);
        assert_eq!(
            reopened
                .read(|_, bundle| Ok(bundle.unwrap().identity()?.public()))
                .unwrap(),
            public
        );
        assert!(
            ring.trust_first_use(friend, &Identity::generate().unwrap().public())
                .is_err()
        );
        assert!(store.update(Some(before.revision), |_| Ok(())).is_err());
        assert!(
            store
                .update(None, |record| {
                    record.scope.as_mut().unwrap().account = Uuid::new_v4();
                    Ok(())
                })
                .is_err()
        );
        assert!(
            store
                .update(None, |record| {
                    record.secure = false;
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(store.snapshot().unwrap().pins[&friend], friend_key);
    }
    #[test]
    fn failure_before_persistence_does_not_publish_changes_and_external_updates_refresh_cache() {
        let (dir, store, scope) = fixture();
        let before = fs::read(&store.path).unwrap();
        assert!(
            store
                .update(None, |record| {
                    record.username = Some("changed".into());
                    anyhow::bail!("Synthetic failure");
                    #[allow(unreachable_code)]
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(fs::read(&store.path).unwrap(), before);
        let second = Store::at(&dir.path().join("privacy"), &scope.origin).unwrap();
        second
            .update(None, |record| {
                record.dirty += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(store.record().unwrap().dirty, 1);
        fs::write(&store.path, b"damaged").unwrap();
        assert!(store.record().is_err());
    }
    #[test]
    fn journal_rejects_passwords_export_keys_and_lost_unknown_retry_bodies() {
        let (_dir, store, _scope) = fixture();
        for name in [
            "password",
            "legacy_password",
            "current_password",
            "new_password",
            "export_key",
        ] {
            let mut pending = Pending::new(Kind::Login, Uuid::new_v4());
            pending.start =
                Some(PrivateBytes::json(&serde_json::json!({name:"synthetic secret"})).unwrap());
            assert!(
                store
                    .update(None, |record| {
                        record.pending = Some(pending);
                        Ok(())
                    })
                    .is_err()
            );
        }
        let mut pending = Pending::new(Kind::Login, Uuid::new_v4());
        pending.sent = true;
        assert!(
            store
                .update(None, |record| {
                    record.pending = Some(pending);
                    Ok(())
                })
                .is_err()
        );
        assert!(store.record().unwrap().pending.is_none());
    }
    #[test]
    fn independent_writers_merge_trust_and_unchanged_reads_do_not_rewrite() {
        let (dir, store, scope) = fixture();
        let root = dir.path().join("privacy");
        let first = std::sync::Arc::new(store);
        let second = std::sync::Arc::new(Store::at(&root, &scope.origin).unwrap());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let mut workers = Vec::new();
        for store in [first.clone(), second] {
            let barrier = barrier.clone();
            workers.push(std::thread::spawn(move || {
                let friend = Uuid::new_v4();
                let pin = Identity::generate().unwrap().public();
                barrier.wait();
                TrustStore::update(&*store, &mut |trust| {
                    trust.pins.insert(friend, pin.clone());
                    Ok(())
                })
                .unwrap();
                (friend, pin)
            }));
        }
        for worker in workers {
            let (friend, pin) = worker.join().unwrap();
            assert_eq!(first.snapshot().unwrap().pins[&friend], pin);
        }
        let revision = first.record().unwrap().revision;
        TrustStore::update(&*first, &mut |_| Ok(())).unwrap();
        assert_eq!(first.record().unwrap().revision, revision);
        assert_eq!(first.record().unwrap().dirty, 2);
    }
    #[test]
    fn missing_record_on_cold_start_and_nonprivate_files_fail_closed() {
        let (dir, store, scope) = fixture();
        fs::set_permissions(&store.path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(store.record().is_err());
        fs::set_permissions(&store.path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::remove_file(&store.path).unwrap();
        let reopened = Store::at(&dir.path().join("privacy"), &scope.origin).unwrap();
        assert!(reopened.record().is_err());
    }
    #[test]
    fn verified_key_network_and_unknown_effect_cannot_be_replaced() {
        let (_dir, store, _scope) = fixture();
        store
            .update(None, |record| {
                record.key_verified = true;
                Ok(())
            })
            .unwrap();
        assert!(
            store
                .update(None, |record| {
                    record.key = Some(PrivateBytes::new(&*VaultKey::generate()?.save_private()));
                    Ok(())
                })
                .is_err()
        );
        assert!(
            store
                .update(None, |record| {
                    record.network = Some(Uuid::new_v4());
                    Ok(())
                })
                .is_err()
        );
        let mut pending = Pending::new(Kind::Login, Uuid::new_v4());
        pending.sent = true;
        pending.finish =
            Some(PrivateBytes::json(&serde_json::json!({"attempt":pending.id})).unwrap());
        pending.effect_digest = Some("a".repeat(64));
        store
            .update(None, |record| {
                record.pending = Some(pending);
                Ok(())
            })
            .unwrap();
        assert!(
            store
                .update(None, |record| {
                    record.pending.as_mut().unwrap().effect_digest = Some("b".repeat(64));
                    Ok(())
                })
                .is_err()
        );
    }
}
