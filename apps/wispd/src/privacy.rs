//! Client-held identities and TOFU pins. Once configured, any key/network/
//! membership error blocks encryption rather than choosing plaintext transport.
#[cfg(test)]
#[path = "privacy_tests.rs"]
mod tests;
use super::{
    ServerApi,
    account_backup::store::{PrivateBytes, Store},
    decode,
};
use anyhow::{Context, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
};
use uuid::Uuid;
use wisp_crypto::{
    PublicIdentity,
    account_vault::{
        Scope,
        bundle::{Bundle, NameCheckpoint},
        envelope::VaultKey,
    },
    keyring::{Keyring, TrustStore},
    message::{Content, MessageContext},
    profile::{Profile, SignedProfile},
    roster::{Member, Role, Roster, SignedRoster},
};
use wisp_protocol::{ConversationView, EncryptedMessageRequest, Message, Snapshot};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Setup {
    network: Uuid,
    account: Uuid,
    contacts: BTreeMap<Uuid, String>,
    #[serde(default)]
    profile_revisions: BTreeMap<Uuid, u64>,
}

#[derive(Deserialize)]
pub(super) struct Directory {
    pub network: Uuid,
    pub identities: BTreeMap<Uuid, PublicIdentity>,
    #[serde(default)]
    pub channel_identities: BTreeMap<Uuid, PublicIdentity>,
    #[serde(default)]
    pub channel_recipients: BTreeMap<String, BTreeSet<Uuid>>,
    pub rosters: BTreeMap<String, Vec<SignedRoster>>,
    #[serde(default)]
    pub profiles: BTreeMap<Uuid, SignedProfile>,
    #[serde(default)]
    pub pending_admissions: Vec<PendingAdmission>,
}

#[derive(Deserialize)]
pub(super) struct PendingAdmission {
    pub conversation_id: String,
    pub user_id: Uuid,
}

pub(super) struct Vault {
    pub ring: Keyring,
    pub network: Uuid,
    pub account: Uuid,
    pub temporary: PathBuf,
    pub contacts: BTreeMap<Uuid, String>,
    channel_recipients: RwLock<BTreeMap<String, BTreeSet<Uuid>>>,
}

pub(super) struct Privacy {
    root: PathBuf,
    binding: PathBuf,
    account: Uuid,
    store: Result<Arc<Store>, String>,
    active: RwLock<Result<Option<Arc<Vault>>, String>>,
    decrypted: Mutex<BTreeMap<Uuid, Content>>,
    last_error: Mutex<Option<String>>,
    setup_error: Mutex<Option<String>>,
    backup_error: Mutex<Option<String>>,
    enrollment: tokio::sync::Mutex<()>,
    contact_updates: Mutex<()>,
}

pub(super) fn local_path(value: &str) -> anyhow::Result<PathBuf> {
    let path = if value.starts_with("file:") {
        url::Url::parse(value)?
            .to_file_path()
            .map_err(|()| anyhow::anyhow!("Choose a local file"))?
    } else {
        PathBuf::from(value)
    };
    ensure!(path.is_absolute(), "Choose an absolute local file path");
    Ok(path)
}

#[allow(clippy::verbose_bit_mask)] // Octal permission masks are clearer here.
fn private_dir(path: &Path) -> anyhow::Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    }
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_dir() && metadata.permissions().mode() & 0o077 == 0,
        "Privacy storage must be a private directory"
    );
    Ok(())
}

#[allow(clippy::verbose_bit_mask)]
fn read_setup(path: &Path) -> anyhow::Result<Option<Setup>> {
    let mut file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let meta = file.metadata()?;
    ensure!(
        meta.is_file() && meta.permissions().mode() & 0o077 == 0,
        "Insecure privacy configuration"
    );
    let mut bytes = Vec::new();
    Read::by_ref(&mut file).take(4097).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 4096, "Invalid privacy configuration");
    Ok(Some(serde_json::from_slice(&bytes)?))
}

fn write_setup(path: &Path, setup: &Setup, replace: bool) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .context("Missing privacy configuration parent")?;
    private_dir(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&serde_json::to_vec(setup)?)?;
    file.as_file().sync_all()?;
    if replace {
        file.persist(path)?;
    } else {
        file.persist_noclobber(path)?;
    }
    Ok(())
}

impl Privacy {
    pub fn new(server: &str, account: Uuid) -> Self {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .unwrap_or_else(|| PathBuf::from("/nonexistent"));
        let root = std::env::var_os("WISP_PRIVACY_DIR")
            .map_or_else(|| config.join("wisp").join("privacy"), PathBuf::from);
        Self::at(root, server, account)
    }

    pub(super) fn at(root: PathBuf, server: &str, account: Uuid) -> Self {
        let binding = root.join(format!("{:x}.json", Sha256::digest(server.as_bytes())));
        let store = Store::at(&root, server)
            .map(Arc::new)
            .map_err(|error| error.to_string());
        let active = (|| -> anyhow::Result<Option<Arc<Vault>>> {
            let store = store
                .as_ref()
                .map_err(|error| anyhow::anyhow!(error.clone()))?;
            let record = store.record()?;
            if let Some(scope) = &record.scope {
                ensure!(
                    scope.account == account,
                    "Server changed your account identity"
                );
            }
            if store.read(|_, bundle| Ok(bundle.is_some()))? {
                return Ok(Some(Arc::new(Self::load_portable(&root, store)?)));
            }
            ensure!(
                !record.secure,
                "Sign in with your secure password to restore this account's encrypted backup"
            );
            let Some(setup) = read_setup(&binding)? else {
                return Ok(None);
            };
            ensure!(
                setup.account == account,
                "Server changed your account identity"
            );
            Ok(Some(Arc::new(Self::load(&root, &setup, store)?)))
        })()
        .map_err(|e| e.to_string());
        Self {
            root,
            binding,
            account,
            store,
            active: RwLock::new(active),
            decrypted: Mutex::new(BTreeMap::new()),
            last_error: Mutex::new(None),
            setup_error: Mutex::new(None),
            backup_error: Mutex::new(None),
            enrollment: tokio::sync::Mutex::new(()),
            contact_updates: Mutex::new(()),
        }
    }

    fn load(root: &Path, setup: &Setup, store: &Arc<Store>) -> anyhow::Result<Vault> {
        // Convert existing private files once, before this daemon starts using
        // the account. The source files stay recoverable; all subsequent trust
        // changes share the same atomic portable store on every live keyring.
        if !store.read(|_, bundle| Ok(bundle.is_some()))? {
            private_dir(root)?;
            let network = root.join(setup.network.to_string());
            private_dir(&network)?;
            let ring = Keyring::open(&network, setup.account)?;
            let mut trust = ring.export_trust()?;
            trust.pins.insert(setup.account, ring.identity().public());
            for (account, name) in &setup.contacts {
                trust.names.insert(
                    *account,
                    NameCheckpoint {
                        display_name: name.clone(),
                        revision: setup.profile_revisions.get(account).copied().unwrap_or(0),
                    },
                );
            }
            let scope = Scope {
                origin: store.record()?.origin,
                network: setup.network,
                account: setup.account,
            };
            let bundle = Bundle::new(scope, ring.identity().recovery_key()?, None, trust)?;
            let key = VaultKey::generate()?;
            store.update(None, |record| {
                ensure!(
                    record.bundle()?.is_none() && !record.secure,
                    "Account storage changed during local import"
                );
                record.key = Some(PrivateBytes::new(&*key.save_private()));
                record.set_bundle(&bundle)
            })?;
        }
        Self::load_portable(root, store)
    }

    fn load_portable(root: &Path, store: &Arc<Store>) -> anyhow::Result<Vault> {
        let (scope, identity, contacts) = store.read(|record, bundle| {
            let bundle = bundle.context("Restore the account backup before using encryption")?;
            ensure!(
                record.scope.as_ref() == Some(&bundle.scope),
                "Private account scope changed"
            );
            Ok((
                bundle.scope.clone(),
                bundle.identity()?,
                bundle
                    .trust
                    .names
                    .iter()
                    .map(|(id, name)| (*id, name.display_name.clone()))
                    .collect(),
            ))
        })?;
        let network = root.join(scope.network.to_string());
        private_dir(&network)?;
        let temporary = network.join("temporary");
        private_dir(&temporary)?;
        Ok(Vault {
            ring: Keyring::portable(identity, store.clone()),
            network: scope.network,
            account: scope.account,
            temporary,
            contacts,
            channel_recipients: RwLock::new(BTreeMap::new()),
        })
    }

    pub(crate) fn backup_store(&self) -> anyhow::Result<Arc<Store>> {
        self.store
            .as_ref()
            .cloned()
            .map_err(|error| anyhow::anyhow!(error.clone()))
    }
    pub(crate) fn reload_backup(&self) -> anyhow::Result<()> {
        let store = self.backup_store()?;
        let vault = Self::load_portable(&self.root, &store)?;
        ensure!(vault.account == self.account, "Account identity changed");
        *self.active.write().expect("privacy state lock") = Ok(Some(Arc::new(vault)));
        Ok(())
    }

    pub(crate) fn capture_media_key(&self, media: Option<String>) -> anyhow::Result<()> {
        self.backup_store()?.capture_media_key(media)
    }

    pub(crate) fn backup_error(&self) -> Option<String> {
        self.backup_error.lock().expect("backup error lock").clone()
    }
    pub(crate) fn set_backup_error(&self, error: Option<String>) {
        *self.backup_error.lock().expect("backup error lock") = error;
    }
    pub fn active(&self) -> anyhow::Result<Option<Arc<Vault>>> {
        let active = self
            .active
            .read()
            .expect("privacy state lock")
            .clone()
            .map_err(anyhow::Error::msg)?;
        if active.is_none() && self.backup_store()?.record()?.secure {
            anyhow::bail!(
                "Unlock and restore your encrypted account backup in Profile settings before using chat or voice"
            );
        }
        Ok(active)
    }

    pub fn status(&self) -> Value {
        match self.active() {
            Ok(Some(vault)) => {
                json!({"configured":true,"error":self.last_error.lock().expect("privacy error lock").clone(),"fingerprint":vault.ring.identity().public().fingerprint().ok(),"network":vault.network,"trust":"first_use","warning":"Recovery keys and this device must remain private. Old plaintext history is not encrypted retroactively."})
            }
            Ok(None) => {
                json!({"configured":false,"error":self.setup_error.lock().expect("privacy setup lock").clone(),"warning":"Chat encryption is set up automatically when this account connects. Private keys stay on this device."})
            }
            Err(_) => {
                json!({"configured":true,"error":"Encryption identity could not be loaded. Restore its recovery key; sending is blocked."})
            }
        }
    }

    /// Enroll a new account without a file picker. `Keyring::create` persists the
    /// private recovery identity before its public identity is registered.
    /// Retrying interrupted enrollment reuses that same identity.
    pub async fn initialize(&self, api: &ServerApi) -> anyhow::Result<()> {
        let _enrollment = self.enrollment.lock().await;
        let result = match self.active() {
            Ok(Some(_)) => Ok(()),
            Ok(None) => self.enable_inner(api, None, None).await.map(|_| ()),
            Err(error) => Err(error.context("Restore this account's existing recovery file")),
        };
        *self.setup_error.lock().expect("privacy setup lock") =
            result.as_ref().err().map(ToString::to_string);
        result
    }

    pub async fn enable(
        &self,
        api: &ServerApi,
        backup: &Path,
        recovery: Option<&Path>,
    ) -> anyhow::Result<Value> {
        let _enrollment = self.enrollment.lock().await;
        self.enable_inner(api, Some(backup), recovery).await
    }

    #[allow(clippy::too_many_lines)]
    async fn enable_inner(
        &self,
        api: &ServerApi,
        backup: Option<&Path>,
        recovery: Option<&Path>,
    ) -> anyhow::Result<Value> {
        let url = url::Url::parse(&api.base_url)?;
        ensure!(
            url.scheme() == "https"
                || matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
            "Encryption setup requires HTTPS (except isolated localhost testing)"
        );
        // Real accounts must prove their authentication mode before legacy
        // enrollment can create or import an identity. Secure identities come
        // exclusively from the verified account backup.
        let real_account = matches!(
            &*api.auth.read().expect("account credential lock"),
            super::AuthMethod::Device { .. }
        );
        if real_account {
            let backup = super::account_backup_commands::connect(api, self).await?;
            if matches!(
                backup.status().await?,
                super::account_backup::api::Status::Secure { .. }
            ) {
                self.reload_backup().context(
                    "Unlock and restore your encrypted account backup in Profile settings",
                )?;
                return Ok(self.status());
            }
        }
        let directory: Directory = decode(
            api.request(reqwest::Method::GET, "/v1/e2ee/state")
                .send()
                .await?,
        )
        .await?;
        // Enrollment never changes an already pinned network.
        if let Ok(Some(existing)) = self.active() {
            ensure!(
                existing.network == directory.network,
                "Server network identity changed"
            );
            return Ok(self.status());
        }
        if let Some(setup) = read_setup(&self.binding)? {
            ensure!(
                setup.network == directory.network && setup.account == self.account,
                "Server identity changed; refusing recovery into a different network"
            );
            ensure!(
                recovery.is_some(),
                "Restore the existing recovery file; automatic identity replacement is forbidden"
            );
        }
        ensure!(self.root.is_absolute(), "Privacy storage must be absolute");
        let parent = self.root.parent().context("Missing privacy parent")?;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)?;
        private_dir(&self.root)?;
        let network = self.root.join(directory.network.to_string());
        private_dir(&network)?;
        if let Some(recovery) = recovery {
            Keyring::restore_file(&network, self.account, recovery)?;
        }
        let ring = if network
            .join(self.account.to_string())
            .join("recovery.key")
            .try_exists()?
        {
            Keyring::open(&network, self.account)?
        } else {
            ensure!(
                !directory.identities.contains_key(&self.account),
                "This account already has encryption keys. Choose Restore recovery file instead of creating new keys."
            );
            ensure!(
                !network.join(self.account.to_string()).try_exists()?,
                "This device's encryption keys are missing. Restore its existing recovery file."
            );
            Keyring::create(&network, self.account)?
        };
        let identity = ring.identity().public();
        if let Some(registered) = directory.identities.get(&self.account) {
            ensure!(
                registered == &identity,
                "This account uses different encryption keys. Restore its existing recovery file."
            );
        }
        if let Some(backup) = backup {
            ring.export_recovery(backup)?;
        }
        let signature = ring.identity().sign_statement(
            "wisp-account-key-v1",
            &serde_json::to_vec(&(directory.network, self.account, &identity))?,
        );
        super::ensure_ok(
            api.request(reqwest::Method::POST, "/v1/e2ee/identity")
                .json(&json!({"identity":identity,"signature":signature}))
                .send()
                .await?,
        )
        .await?;
        let setup = if let Some(existing) = read_setup(&self.binding)? {
            existing
        } else {
            let snapshot = api.snapshot().await?;
            ensure!(
                snapshot.self_state.user.id == self.account,
                "Server changed your account during setup"
            );
            let mut contacts: BTreeMap<_, _> = snapshot
                .friends
                .into_iter()
                .map(|friend| (friend.user.id, friend.user.display_name))
                .collect();
            contacts.insert(self.account, snapshot.self_state.user.display_name);
            Setup {
                network: directory.network,
                account: self.account,
                contacts,
                profile_revisions: BTreeMap::new(),
            }
        };
        if read_setup(&self.binding)?.is_none() {
            write_setup(&self.binding, &setup, false)?;
        }
        *self.active.write().expect("privacy state lock") = Ok(Some(Arc::new(Self::load(
            &self.root,
            &setup,
            &self.backup_store()?,
        )?)));
        *self.setup_error.lock().expect("privacy setup lock") = None;
        Ok(self.status())
    }

    /// Trust-on-first-use applies when the authenticated account intentionally
    /// gains a new friend. Existing contacts are never removed or replaced,
    /// and the keyring separately refuses changes to an already pinned key.
    fn sync_contacts(&self, snapshot: &Snapshot) -> anyhow::Result<bool> {
        let _guard = self.contact_updates.lock().expect("contact update lock");
        if self.active()?.is_none() {
            return Ok(false);
        }
        let store = self.backup_store()?;
        let mut changed = false;
        TrustStore::update(&*store, &mut |trust| {
            for friend in &snapshot.friends {
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    trust.names.entry(friend.user.id)
                {
                    entry.insert(NameCheckpoint {
                        display_name: friend.user.display_name.clone(),
                        revision: 0,
                    });
                    changed = true;
                }
            }
            Ok(())
        })?;
        if changed {
            self.reload_backup()?;
        }
        Ok(changed)
    }

    /// Room names follow verified signed membership; a snapshot never renames an
    /// existing trust entry or discards a higher signed-profile revision floor.
    fn sync_room_names(
        &self,
        vault: &Vault,
        directory: &Directory,
        snapshot: &Snapshot,
    ) -> anyhow::Result<()> {
        let _guard = self.contact_updates.lock().expect("contact update lock");
        let store = self.backup_store()?;
        store.read(|record, _| {
            ensure!(
                record
                    .scope
                    .as_ref()
                    .is_some_and(|s| s.network == vault.network && s.account == vault.account),
                "Privacy binding changed"
            );
            Ok(())
        })?;
        TrustStore::update(&*store, &mut |trust| {
            for conversation in &snapshot.conversations {
                let Some(roster) = directory
                    .rosters
                    .get(&conversation.id)
                    .and_then(|chain| chain.last())
                else {
                    continue;
                };
                for member in &conversation.members {
                    if !roster.roster.members.contains_key(&member.id)
                        || member.display_name.trim().is_empty()
                        || member.display_name.chars().count() > 80
                        || member.display_name.chars().any(char::is_control)
                    {
                        continue;
                    }
                    trust
                        .names
                        .entry(member.id)
                        .or_insert_with(|| NameCheckpoint {
                            display_name: member.display_name.clone(),
                            revision: 0,
                        });
                }
            }
            Ok(())
        })?;
        self.reload_backup()
    }

    /// Complete accepted encrypted-room account invitations with an
    /// owner/admin signature. At most one admission per room is published per
    /// pass so every signature is based on the latest roster and snapshot.
    pub async fn reconcile_pending_admissions(
        &self,
        api: &ServerApi,
        snapshot: &Snapshot,
    ) -> anyhow::Result<bool> {
        self.sync_contacts(snapshot)?;
        let Some(vault) = self.active()? else {
            return Ok(false);
        };
        let directory = self.directory(api, &vault).await?;
        let friend_ids = snapshot
            .friends
            .iter()
            .map(|friend| friend.user.id)
            .collect::<BTreeSet<_>>();
        let mut attempted = BTreeSet::new();
        for pending in directory.pending_admissions {
            if !attempted.insert(pending.conversation_id.clone())
                || (!friend_ids.contains(&pending.user_id)
                    && !directory
                        .channel_recipients
                        .get(&pending.conversation_id)
                        .is_some_and(|ids| ids.contains(&pending.user_id)))
                || !directory.identities.contains_key(&pending.user_id)
            {
                continue;
            }
            let Some(conversation) = snapshot
                .conversations
                .iter()
                .find(|conversation| conversation.id == pending.conversation_id)
            else {
                continue;
            };
            if !matches!(
                conversation
                    .member_roles
                    .get(&vault.account)
                    .map(String::as_str),
                Some("host" | "admin")
            ) {
                continue;
            }
            let Some(signed) = self
                .invite_member(api, conversation, pending.user_id)
                .await?
            else {
                continue;
            };
            super::ensure_ok(
                api.request(reqwest::Method::POST, "/v1/e2ee/roster")
                    .json(&signed)
                    .send()
                    .await?,
            )
            .await?;
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn directory(&self, api: &ServerApi, vault: &Vault) -> anyhow::Result<Directory> {
        let mut directory: Directory = decode(
            api.request(reqwest::Method::GET, "/v1/e2ee/state")
                .send()
                .await?,
        )
        .await?;
        Self::verify_directory(vault, &directory)?;
        // Server-channel membership is distinct from friendship. Pin eligible
        // account keys on first use; key replacements still fail closed.
        for (id, key) in &directory.channel_identities {
            if *id == vault.account {
                ensure!(
                    key == &vault.ring.identity().public(),
                    "Server changed your encryption identity"
                );
            } else {
                vault.ring.trust_first_use(*id, key)?;
            }
        }
        *vault
            .channel_recipients
            .write()
            .expect("channel audience lock") = directory.channel_recipients.clone();
        directory
            .identities
            .extend(directory.channel_identities.clone());
        self.sync_signed_profiles(vault, &directory)?;
        Ok(directory)
    }

    pub fn signed_profile(&self, name: String, revision: u64) -> anyhow::Result<SignedProfile> {
        let vault = self
            .active()?
            .context("Restore account encryption before changing your display name")?;
        Profile {
            network: vault.network,
            account: vault.account,
            revision,
            display_name: name,
        }
        .sign(vault.ring.identity())
    }

    fn sync_signed_profiles(&self, vault: &Vault, directory: &Directory) -> anyhow::Result<()> {
        let _guard = self.contact_updates.lock().expect("contact update lock");
        let store = self.backup_store()?;
        TrustStore::update(&*store, &mut |trust| {
            for (account, signed) in &directory.profiles {
                ensure!(
                    signed.profile.network == vault.network && signed.profile.account == *account,
                    "Invalid profile account"
                );
                let saved = trust
                    .names
                    .get(account)
                    .context("Invalid profile account")?;
                let key = directory
                    .identities
                    .get(account)
                    .context("Missing profile identity")?;
                signed.verify(key)?;
                ensure!(
                    signed.profile.revision >= saved.revision,
                    "Account profile rollback blocked"
                );
                if signed.profile.revision == saved.revision {
                    ensure!(
                        saved.display_name == signed.profile.display_name,
                        "Conflicting account profile blocked"
                    );
                } else {
                    trust.names.insert(
                        *account,
                        NameCheckpoint {
                            display_name: signed.profile.display_name.clone(),
                            revision: signed.profile.revision,
                        },
                    );
                }
                if let Some(previous) = trust.profiles.get(account) {
                    ensure!(
                        previous.profile.revision <= signed.profile.revision,
                        "Signed profile rollback blocked"
                    );
                    if previous.profile.revision == signed.profile.revision {
                        ensure!(previous == signed, "Conflicting signed profile blocked");
                    }
                }
                trust.profiles.insert(*account, signed.clone());
            }
            Ok(())
        })?;
        self.reload_backup()
    }

    fn verify_directory(vault: &Vault, directory: &Directory) -> anyhow::Result<()> {
        ensure!(
            directory.network == vault.network,
            "Server encryption network changed; sending blocked"
        );
        ensure!(
            directory
                .identities
                .keys()
                .all(|id| vault.contacts.contains_key(id)),
            "Friend account roster changed. Refusing automatic enrollment of a new account; verify it before changing your trusted contacts"
        );
        for (id, key) in &directory.identities {
            if *id == vault.account {
                ensure!(
                    key == &vault.ring.identity().public(),
                    "Server changed your encryption identity"
                );
            } else {
                vault.ring.trust_first_use(*id, key)?;
            }
        }
        // A room owner can admit someone who is not every member's direct
        // friend. Authorize those identities through the signed room chain,
        // including saved heads and key pins, rather than the friend list.
        for (conversation, chain) in &directory.rosters {
            vault
                .ring
                .accept_rosters(vault.network, conversation, vault.account, chain)?;
        }
        Ok(())
    }

    pub async fn recipients(
        &self,
        api: &ServerApi,
        conversation: &ConversationView,
    ) -> anyhow::Result<(Arc<Vault>, SignedRoster)> {
        let vault = self
            .active()?
            .context("Chat encryption is not configured")?;
        let mut directory = self.directory(api, &vault).await?;
        if !directory.rosters.contains_key(&conversation.id) {
            let mut members = BTreeMap::new();
            for member in &conversation.members {
                let role = match conversation
                    .member_roles
                    .get(&member.id)
                    .map(String::as_str)
                {
                    Some("host") => Role::Host,
                    Some("admin") => Role::Admin,
                    _ => Role::Member,
                };
                let identity = directory
                    .identities
                    .get(&member.id)
                    .with_context(|| {
                        format!(
                            "{} needs to enable encrypted chat first",
                            member.display_name
                        )
                    })?
                    .clone();
                members.insert(member.id, Member { identity, role });
            }
            let initial = Roster {
                network: vault.network,
                conversation: conversation.id.clone(),
                revision: 0,
                previous: None,
                actor: vault.account,
                members,
            }
            .sign(vault.ring.identity())?;
            initial
                .verify_genesis()
                .context("The room owner must initialize encrypted chat first")?;
            super::ensure_ok(
                api.request(reqwest::Method::POST, "/v1/e2ee/roster")
                    .json(&initial)
                    .send()
                    .await?,
            )
            .await?;
            directory
                .rosters
                .insert(conversation.id.clone(), vec![initial]);
        }
        let chain = &directory.rosters[&conversation.id];
        let latest =
            vault
                .ring
                .accept_rosters(vault.network, &conversation.id, vault.account, chain)?;
        let actual: std::collections::BTreeSet<_> =
            conversation.members.iter().map(|m| m.id).collect();
        ensure!(
            actual == latest.roster.members.keys().copied().collect(),
            "Room membership changed without a signed update; ask its owner to update Wisp"
        );
        Ok((vault, latest))
    }

    /// Prepare (but do not publish) the membership addition carried by a voice
    /// invite. The server applies this signature only when the friend accepts.
    pub async fn invite_member(
        &self,
        api: &ServerApi,
        conversation: &ConversationView,
        target: Uuid,
    ) -> anyhow::Result<Option<Value>> {
        let (vault, previous) = self.recipients(api, conversation).await?;
        if previous.roster.members.contains_key(&target) {
            return Ok(None);
        }
        let directory = self.directory(api, &vault).await?;
        let identity = directory
            .identities
            .get(&target)
            .context("Your friend needs to enable encrypted chat first")?
            .clone();
        let mut roster = previous.roster.clone();
        roster.actor = vault.account;
        roster.revision = roster
            .revision
            .checked_add(1)
            .context("Room version overflow")?;
        roster.previous = Some(previous.hash()?);
        roster.members.insert(
            target,
            Member {
                identity,
                role: Role::Member,
            },
        );
        let signed = roster.sign(vault.ring.identity())?;
        signed.verify_successor(&previous)?;
        Ok(Some(serde_json::to_value(signed)?))
    }

    pub fn seal(
        vault: &Vault,
        roster: &SignedRoster,
        id: Uuid,
        content: Content,
    ) -> anyhow::Result<EncryptedMessageRequest> {
        let recipients = roster
            .roster
            .members
            .iter()
            .map(|(id, m)| (*id, m.identity.clone()))
            .collect();
        Self::seal_to(vault, roster, id, content, &recipients)
    }

    pub fn seal_to(
        vault: &Vault,
        roster: &SignedRoster,
        id: Uuid,
        content: Content,
        recipients: &BTreeMap<Uuid, PublicIdentity>,
    ) -> anyhow::Result<EncryptedMessageRequest> {
        ensure!(
            recipients.iter().all(|(id, key)| roster
                .roster
                .members
                .get(id)
                .is_some_and(|member| &member.identity == key)),
            "Message recipients must belong to the current signed room"
        );
        let audiences = vault
            .channel_recipients
            .read()
            .expect("channel audience lock");
        let recipients = recipients
            .iter()
            .filter(|(id, _)| {
                audiences
                    .get(&roster.roster.conversation)
                    .is_none_or(|allowed| allowed.contains(id))
            })
            .map(|(id, key)| (*id, key.clone()))
            .collect::<BTreeMap<_, _>>();
        ensure!(
            recipients.contains_key(&vault.account),
            "You no longer have access to this channel"
        );
        let binding = MessageContext {
            network: vault.network,
            conversation: roster.roster.conversation.clone(),
            sender: vault.account,
            message: id,
            roster: roster.hash()?,
        };
        Ok(EncryptedMessageRequest {
            recipient_ids: Some(recipients.keys().copied().collect()),
            id,
            conversation_id: binding.conversation.clone(),
            roster_hash: binding.roster.clone(),
            ciphertext: STANDARD.encode(binding.seal(
                vault.ring.identity(),
                &recipients,
                content,
            )?),
        })
    }

    pub fn content(&self, id: Uuid) -> anyhow::Result<Content> {
        self.decrypted
            .lock()
            .expect("decrypted cache")
            .get(&id)
            .cloned()
            .context("Encrypted message is not in the local history")
    }

    #[allow(clippy::too_many_lines)] // Keep authenticated decode and redaction together.
    pub async fn decrypt_snapshot(&self, api: &ServerApi, snapshot: &mut Snapshot) {
        if !snapshot.chat_encryption_required
            && matches!(self.active(), Ok(None))
            && !snapshot.messages.iter().any(|m| m.encryption_version != 0)
            && !snapshot
                .reactions
                .iter()
                .any(|r| r.message.encryption_version != 0)
            && !snapshot.conversations.iter().any(|c| {
                c.last_message
                    .as_ref()
                    .is_some_and(|m| m.encryption_version != 0)
            })
        {
            *self.last_error.lock().expect("privacy error lock") = None;
            self.decrypted.lock().expect("decrypted cache").clear();
            return;
        }
        let result = async {
            let vault = self
                .active()?
                .context("Restore or enable chat encryption to read this message")?;
            let directory = self.directory(api, &vault).await?;
            self.sync_room_names(&vault, &directory, snapshot)?;
            let vault = self.active()?.context("Missing account encryption")?;
            Ok::<_, anyhow::Error>((vault, directory))
        }
        .await;
        // Always redact unrecognized names, including when directory validation
        // fails. Preserve the raw names only long enough to enroll verified peers.
        if let Ok(Some(vault)) = self.active() {
            Self::restore_contact_names(&vault, snapshot);
        }
        *self.last_error.lock().expect("privacy error lock") =
            result.as_ref().err().map(ToString::to_string);
        self.decrypted.lock().expect("decrypted cache").clear();
        let block_plaintext =
            snapshot.chat_encryption_required || !matches!(self.active(), Ok(None));
        let decode_message = |message: &mut Message| {
            if message.encryption_version != 0 || block_plaintext {
                message.context = None;
            }
            if message.encryption_version == 0 {
                // A later malicious server must not bypass sender authentication
                // by replaying the legacy wire type. Invite cards are explicitly
                // public coordination metadata, never trusted message text.
                if block_plaintext
                    && message.content_type != "application/vnd.wisp.room-invitation+json"
                {
                    message.content_type = "text/plain".into();
                    message.payload =
                        json!("[Unencrypted message blocked — encrypted chat is required]");
                }
                return;
            }
            let content = (|| -> anyhow::Result<Content> {
                ensure!(
                    message.encryption_version == 1,
                    "Unsupported message encryption version"
                );
                let (vault, directory) = result
                    .as_ref()
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                let hash = message.payload["roster_hash"]
                    .as_str()
                    .context("Missing encrypted room identity")?;
                let chain = directory
                    .rosters
                    .get(&message.conversation_id)
                    .context("Missing signed room history")?;
                let roster = chain
                    .iter()
                    .find(|r| r.hash().ok().as_deref() == Some(hash))
                    .context("Unrecognized room membership signature")?;
                let sender = roster
                    .roster
                    .members
                    .get(&message.sender.id)
                    .context("Sender was not an authorized room member")?;
                let context = MessageContext {
                    network: vault.network,
                    conversation: message.conversation_id.clone(),
                    sender: message.sender.id,
                    message: message.id,
                    roster: hash.into(),
                };
                context.open(
                    vault.ring.identity(),
                    vault.account,
                    &sender.identity,
                    &STANDARD.decode(
                        message.payload["ciphertext"]
                            .as_str()
                            .context("Missing ciphertext")?,
                    )?,
                )
            })();
            if let Ok(content) = content {
                self.decrypted
                    .lock()
                    .expect("decrypted cache")
                    .insert(message.id, content.clone());
                message.context = content.context;
                let mut payload = content.payload;
                if payload.is_object() {
                    for key in ["keep", "expires_at", "expired"] {
                        if let Some(value) = message.payload.get(key) {
                            payload[key] = value.clone();
                        }
                    }
                }
                message.content_type = content.content_type;
                message.payload = payload;
            } else {
                message.content_type = "text/plain".into();
                message.payload =
                    json!("[Encrypted message unavailable — check Settings → Privacy]");
            }
        };
        for message in &mut snapshot.messages {
            decode_message(message);
        }
        for reaction in &mut snapshot.reactions {
            decode_message(&mut reaction.message);
        }
        snapshot.reactions.retain(|reaction| {
            reaction.message.content_type == wisp_protocol::REACTION_CONTENT_TYPE
                && reaction.message.payload["target"].as_str()
                    == Some(&reaction.target_id.to_string())
        });
        for conversation in &mut snapshot.conversations {
            if let Some(message) = conversation.last_message.as_mut() {
                decode_message(message);
            }
        }
    }

    fn restore_contact_names(vault: &Vault, snapshot: &mut Snapshot) {
        let restore = |person: &mut wisp_protocol::UserSummary| {
            person.display_name = vault
                .contacts
                .get(&person.id)
                .cloned()
                .unwrap_or_else(|| "Unrecognized account".into());
        };
        restore(&mut snapshot.self_state.user);
        snapshot
            .friends
            .retain(|friend| vault.contacts.contains_key(&friend.user.id));
        for friend in &mut snapshot.friends {
            restore(&mut friend.user);
        }
        for room in &mut snapshot.hangouts {
            for person in &mut room.members {
                restore(person);
            }
        }
        for conversation in &mut snapshot.conversations {
            for person in &mut conversation.members {
                restore(person);
            }
            if conversation.kind == wisp_protocol::ConversationKind::Direct {
                conversation.label = conversation
                    .members
                    .iter()
                    .filter(|p| p.id != vault.account)
                    .map(|p| p.display_name.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
            }
            if let Some(message) = &mut conversation.last_message {
                restore(&mut message.sender);
            }
        }
        for message in &mut snapshot.messages {
            restore(&mut message.sender);
        }
        for reaction in &mut snapshot.reactions {
            restore(&mut reaction.message.sender);
        }
        for invite in &mut snapshot.room_invitations {
            restore(&mut invite.from);
        }
    }
}
