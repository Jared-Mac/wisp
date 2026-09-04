//! Private local key storage. Import never overwrites a different identity;
//! public-key pinning never silently accepts a key change.
use crate::{Identity, PublicIdentity, SecretString};
use age::secrecy::ExposeSecret;
use anyhow::{Context, bail};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
use uuid::Uuid;
use zeroize::Zeroizing;

pub struct Keyring {
    directory: PathBuf,
    identity: Identity,
}

fn private_directory(path: &Path) -> anyhow::Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
        bail!("Encryption key directory must be a private directory (0700), not a symlink");
    }
    Ok(())
}

fn read_private(path: &Path) -> anyhow::Result<SecretString> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o077 != 0 || metadata.nlink() != 1 {
        bail!("Encryption key file must be a private regular file (0600), without hard links");
    }
    let mut code = Zeroizing::new(String::new());
    Read::by_ref(&mut file)
        .take(16_385)
        .read_to_string(&mut code)?;
    if code.len() > 16_384 {
        bail!("Invalid recovery key file");
    }
    Ok(SecretString::from(std::mem::take(&mut *code)))
}

fn write_new(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .context("Create private file without replacing existing data")?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

impl Keyring {
    /// Explicit first-time enrollment only. Never use this as a missing-key
    /// fallback; an absent recovery file may represent lost existing history.
    pub fn create(root: &Path, account: Uuid) -> anyhow::Result<Self> {
        private_directory(root)?;
        let directory = root.join(account.to_string());
        private_directory(&directory)?;
        let identity = Identity::generate()?;
        write_new(
            &directory.join("recovery.key"),
            identity.recovery_key()?.expose_secret().as_bytes(),
        )?;
        Self::open(root, account)
    }

    /// Load only. A missing or damaged key never creates a replacement identity.
    /// Account UUIDs isolate profiles; caller also isolates pinned network IDs.
    pub fn open(root: &Path, account: Uuid) -> anyhow::Result<Self> {
        private_directory(root)?;
        let directory = root.join(account.to_string());
        private_directory(&directory)?;
        private_directory(&directory.join("verified"))?;
        private_directory(&directory.join("conversations"))?;
        private_directory(&directory.join("room-heads"))?;
        let path = directory.join("recovery.key");
        let identity = Identity::recover(&read_private(&path).context(
            "Cannot read encryption identity; restore its recovery key, do not reset it silently",
        )?)?;
        Ok(Self {
            directory,
            identity,
        })
    }

    #[must_use]
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    /// Verify signed room evolution and retain the last seen head across
    /// restarts. Initial identities use TOFU; later key/roster changes do not.
    pub fn accept_rosters(
        &self,
        network: Uuid,
        conversation: &str,
        account: Uuid,
        chain: &[crate::roster::SignedRoster],
    ) -> anyhow::Result<crate::roster::SignedRoster> {
        let first = chain
            .first()
            .context("Room owner has not initialized encrypted membership")?;
        first.verify_genesis()?;
        anyhow::ensure!(
            first.roster.network == network && first.roster.conversation == conversation,
            "Room belongs to another network or conversation"
        );
        for pair in chain.windows(2) {
            pair[1].verify_successor(&pair[0])?;
        }
        let last = chain.last().expect("nonempty chain");
        let own = last
            .roster
            .members
            .get(&account)
            .context("You are not a member of the encrypted room")?;
        anyhow::ensure!(
            own.identity == self.identity.public(),
            "Your room identity does not match this device"
        );
        let path = self
            .directory
            .join("room-heads")
            .join(format!("{:x}", Sha256::digest(conversation.as_bytes())));
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                let saved: crate::roster::SignedRoster =
                    serde_json::from_str(read_private(&path)?.expose_secret())?;
                anyhow::ensure!(
                    chain.iter().any(|item| item == &saved),
                    "Room history rolled back or forked; refusing to replace its saved identity"
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        for entry in chain {
            for (id, member) in &entry.roster.members {
                if *id == account {
                    anyhow::ensure!(
                        member.identity == self.identity.public(),
                        "Your encryption identity changed in room history"
                    );
                } else {
                    self.trust_first_use(*id, &member.identity)?;
                }
            }
        }
        let mut file = tempfile::NamedTempFile::new_in(self.directory.join("room-heads"))?;
        file.write_all(&serde_json::to_vec(last)?)?;
        file.as_file().sync_all()?;
        file.persist(path)?;
        Ok(last.clone())
    }

    pub fn restore_file(root: &Path, account: Uuid, path: &Path) -> anyhow::Result<()> {
        Self::restore(root, account, &read_private(path)?)
    }

    fn roster_path(&self, conversation: &str) -> PathBuf {
        let digest = Sha256::digest(conversation.as_bytes());
        self.directory
            .join("conversations")
            .join(format!("{digest:x}"))
    }

    fn keys_for(
        &self,
        account: Uuid,
        members: &BTreeSet<Uuid>,
    ) -> anyhow::Result<BTreeMap<Uuid, PublicIdentity>> {
        anyhow::ensure!(
            members.contains(&account),
            "Your account is absent from the chat membership"
        );
        members
            .iter()
            .map(|id| {
                Ok((
                    *id,
                    if *id == account {
                        self.identity.public()
                    } else {
                        self.friend(*id)
                            .context("Verify every chat participant before sending")?
                    },
                ))
            })
            .collect()
    }

    /// Explicit user confirmation of the displayed participant list only. A
    /// server event must never call this automatically, even for known friends.
    pub fn approve_conversation(
        &self,
        conversation: &str,
        account: Uuid,
        members: &BTreeSet<Uuid>,
    ) -> anyhow::Result<()> {
        self.keys_for(account, members)?;
        let path = self.roster_path(conversation);
        if path.try_exists()? {
            read_private(&path)?;
        }
        let mut temporary = tempfile::NamedTempFile::new_in(self.directory.join("conversations"))?;
        temporary.write_all(&serde_json::to_vec(&(conversation, members))?)?;
        temporary.as_file().sync_all()?;
        temporary.persist(path)?;
        Ok(())
    }

    /// Reject both added and removed participants until explicitly reviewed;
    /// otherwise a malicious host could alter even a roster of pinned friends.
    pub fn recipients(
        &self,
        conversation: &str,
        account: Uuid,
        offered: &BTreeSet<Uuid>,
    ) -> anyhow::Result<BTreeMap<Uuid, PublicIdentity>> {
        let (pinned_conversation, pinned): (String, BTreeSet<Uuid>) = serde_json::from_str(
            read_private(&self.roster_path(conversation))
                .context("Review this chat's recipients before sending")?
                .expose_secret(),
        )?;
        anyhow::ensure!(
            pinned_conversation == conversation && &pinned == offered,
            "Chat participants changed; review the recipients before sending"
        );
        self.keys_for(account, offered)
    }

    /// Return the first pinned identity; callers may optionally verify its
    /// fingerprint independently. A pin alone is not independent verification.
    pub fn friend(&self, user: Uuid) -> anyhow::Result<PublicIdentity> {
        let key: PublicIdentity = serde_json::from_str(
            read_private(&self.directory.join("verified").join(user.to_string()))?.expose_secret(),
        )?;
        key.validate()?;
        Ok(key)
    }

    /// Explicit user action only; export is sensitive and never overwrites.
    pub fn export_recovery(&self, destination: &Path) -> anyhow::Result<()> {
        if !destination.is_absolute() {
            bail!("Choose an absolute recovery export path");
        }
        write_new(
            destination,
            self.identity.recovery_key()?.expose_secret().as_bytes(),
        )
    }

    /// Recover BEFORE open on a fresh device. Existing, different identities are
    /// never destroyed; replacement needs a separate, explicit rotation workflow.
    pub fn restore(root: &Path, account: Uuid, recovery: &SecretString) -> anyhow::Result<()> {
        let restored = Identity::recover(recovery)?;
        private_directory(root)?;
        let directory = root.join(account.to_string());
        private_directory(&directory)?;
        let path = directory.join("recovery.key");
        if path.try_exists()? {
            let current = Identity::recover(&read_private(&path)?)?;
            if current.public() != restored.public() {
                bail!("This device already has a different identity; nothing was overwritten");
            }
            return Ok(());
        }
        write_new(&path, restored.recovery_key()?.expose_secret().as_bytes())
    }

    pub fn verified_key(&self, user: Uuid, offered: &PublicIdentity) -> anyhow::Result<()> {
        offered.validate()?;
        let pinned: PublicIdentity = serde_json::from_str(
            read_private(&self.directory.join("verified").join(user.to_string()))?.expose_secret(),
        )?;
        if &pinned != offered {
            bail!("Friend's encryption identity changed; verify it again before sending");
        }
        Ok(())
    }

    /// User-selected trust on first use. Persist the first valid identity and
    /// reject replacements. Never interpret corruption/permission errors as a
    /// missing pin, and never automatically reset a changed identity.
    pub fn trust_first_use(&self, user: Uuid, key: &PublicIdentity) -> anyhow::Result<()> {
        key.validate()?;
        let path = self.directory.join("verified").join(user.to_string());
        match std::fs::symlink_metadata(&path) {
            Ok(_) => self.verified_key(user, key),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match write_new(&path, &serde_json::to_vec(key)?) {
                    Ok(()) => Ok(()),
                    // Another first-contact operation may have pinned a key.
                    Err(_) if path.exists() => self.verified_key(user, key),
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Call ONLY after explicit comparison of the FULL fingerprint through an
    /// independently trusted channel. Not a TOFU auto-accept operation.
    pub fn verify_friend(
        &self,
        user: Uuid,
        key: &PublicIdentity,
        compared_fingerprint: &str,
    ) -> anyhow::Result<()> {
        if key.fingerprint()? != compared_fingerprint {
            bail!("Fingerprint does not match");
        }
        let path = self.directory.join("verified").join(user.to_string());
        if path.try_exists()? {
            return self.verified_key(user, key);
        }
        write_new(&path, &serde_json::to_vec(key)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_contact_is_automatic_but_changed_keys_and_corruption_never_are() {
        let temp = tempfile::tempdir().unwrap();
        let user = Uuid::new_v4();
        let ring = Keyring::create(&temp.path().join("keys"), user).unwrap();
        let friend = Uuid::new_v4();
        let key = Identity::generate().unwrap().public();
        ring.trust_first_use(friend, &key).unwrap();
        let reopened = Keyring::open(&temp.path().join("keys"), user).unwrap();
        reopened.trust_first_use(friend, &key).unwrap();
        assert!(
            reopened
                .trust_first_use(friend, &Identity::generate().unwrap().public())
                .is_err()
        );
        assert_eq!(reopened.friend(friend).unwrap(), key);
        let path = ring.directory.join("verified").join(friend.to_string());
        fs::write(&path, b"corrupted pin").unwrap();
        assert!(reopened.trust_first_use(friend, &key).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"corrupted pin");
    }

    #[test]
    fn recovery_and_pins_persist_without_overwrite_or_auto_trust() {
        let temp = tempfile::tempdir().unwrap();
        let user = Uuid::new_v4();
        let ring = Keyring::create(&temp.path().join("keys"), user).unwrap();
        let public = ring.identity().public();
        let destination = temp.path().join("saved-recovery.key");
        ring.export_recovery(&destination).unwrap();
        assert_eq!(
            fs::metadata(&destination).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(ring.export_recovery(&destination).is_err());
        let root2 = temp.path().join("restored");
        Keyring::restore(&root2, user, &read_private(&destination).unwrap()).unwrap();
        let restored = Keyring::open(&root2, user).unwrap();
        assert_eq!(restored.identity().public(), public);
        assert!(
            Keyring::restore(
                &root2,
                user,
                &Identity::generate().unwrap().recovery_key().unwrap()
            )
            .is_err()
        );
        let friend_id = Uuid::new_v4();
        let friend = Identity::generate().unwrap().public();
        assert!(ring.verified_key(friend_id, &friend).is_err());
        assert!(
            ring.verify_friend(friend_id, &friend, "wrong fingerprint")
                .is_err()
        );
        ring.verify_friend(friend_id, &friend, &friend.fingerprint().unwrap())
            .unwrap();
        Keyring::open(&temp.path().join("keys"), user)
            .unwrap()
            .verified_key(friend_id, &friend)
            .unwrap();
        let changed = Identity::generate().unwrap().public();
        assert!(
            ring.verify_friend(friend_id, &changed, &changed.fingerprint().unwrap())
                .is_err()
        );
        ring.verified_key(friend_id, &friend).unwrap();
        let members = BTreeSet::from([user, friend_id]);
        assert!(ring.recipients("dm", user, &members).is_err());
        ring.approve_conversation("dm", user, &members).unwrap();
        assert_eq!(ring.recipients("dm", user, &members).unwrap().len(), 2);
        assert!(
            ring.recipients("dm", user, &BTreeSet::from([user]))
                .is_err()
        );
        assert!(ring.recipients("another-dm", user, &members).is_err());
        assert!(
            ring.recipients(
                "dm",
                user,
                &BTreeSet::from([user, friend_id, Uuid::new_v4()])
            )
            .is_err()
        );
    }

    #[test]
    fn insecure_corrupted_or_symlinked_keys_are_not_replaced() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("keys");
        let user = Uuid::new_v4();
        assert!(Keyring::open(&root, user).is_err());
        let ring = Keyring::create(&root, user).unwrap();
        let key = ring.directory.join("recovery.key");
        fs::set_permissions(&key, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(Keyring::open(&root, user).is_err());
        fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&key, b"corrupted").unwrap();
        assert!(Keyring::open(&root, user).is_err());
        assert_eq!(fs::read(&key).unwrap(), b"corrupted");
        fs::remove_file(&key).unwrap();
        assert!(Keyring::open(&root, user).is_err());
        assert!(!key.exists());
        let target = temp.path().join("target");
        fs::write(&target, b"untouched").unwrap();
        std::os::unix::fs::symlink(&target, &key).unwrap();
        assert!(Keyring::open(&root, user).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"untouched");
    }
}
