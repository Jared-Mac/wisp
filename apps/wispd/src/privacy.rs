//! Client-held identities and TOFU pins. Once configured, any key/network/
//! membership error blocks encryption rather than choosing plaintext transport.
#[cfg(test)]
#[path = "privacy_tests.rs"]
mod tests;
use super::{ServerApi, decode};
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
    keyring::Keyring,
    message::{Content, MessageContext},
    roster::{Member, Role, Roster, SignedRoster},
};
use wisp_protocol::{ConversationView, EncryptedMessageRequest, Message, Snapshot};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Setup {
    network: Uuid,
    account: Uuid,
    contacts: BTreeMap<Uuid, String>,
}

#[derive(Deserialize)]
pub(super) struct Directory {
    pub network: Uuid,
    pub identities: BTreeMap<Uuid, PublicIdentity>,
    pub rosters: BTreeMap<String, Vec<SignedRoster>>,
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
}

pub(super) struct Privacy {
    root: PathBuf,
    binding: PathBuf,
    account: Uuid,
    active: RwLock<Result<Option<Arc<Vault>>, String>>,
    decrypted: Mutex<BTreeMap<Uuid, Content>>,
    last_error: Mutex<Option<String>>,
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

    fn at(root: PathBuf, server: &str, account: Uuid) -> Self {
        let binding = root.join(format!("{:x}.json", Sha256::digest(server.as_bytes())));
        let active = (|| -> anyhow::Result<Option<Arc<Vault>>> {
            let Some(setup) = read_setup(&binding)? else {
                return Ok(None);
            };
            ensure!(
                setup.account == account,
                "Server changed your account identity"
            );
            Ok(Some(Arc::new(Self::load(&root, &setup)?)))
        })()
        .map_err(|e| e.to_string());
        Self {
            root,
            binding,
            account,
            active: RwLock::new(active),
            decrypted: Mutex::new(BTreeMap::new()),
            last_error: Mutex::new(None),
        }
    }

    fn load(root: &Path, setup: &Setup) -> anyhow::Result<Vault> {
        private_dir(root)?;
        let network = root.join(setup.network.to_string());
        private_dir(&network)?;
        let ring = Keyring::open(&network, setup.account)?;
        let temporary = network.join("temporary");
        private_dir(&temporary)?;
        Ok(Vault {
            ring,
            network: setup.network,
            account: setup.account,
            temporary,
            contacts: setup.contacts.clone(),
        })
    }

    pub fn active(&self) -> anyhow::Result<Option<Arc<Vault>>> {
        self.active
            .read()
            .expect("privacy state lock")
            .clone()
            .map_err(anyhow::Error::msg)
    }

    pub fn status(&self) -> Value {
        match self.active() {
            Ok(Some(vault)) => {
                json!({"configured":true,"error":self.last_error.lock().expect("privacy error lock").clone(),"fingerprint":vault.ring.identity().public().fingerprint().ok(),"network":vault.network,"trust":"first_use","warning":"Recovery keys and this device must remain private. Old plaintext history is not encrypted retroactively."})
            }
            Ok(None) => {
                json!({"configured":false,"warning":"Chat encryption is not configured. Media encryption is separate."})
            }
            Err(_) => {
                json!({"configured":true,"error":"Encryption identity could not be loaded. Restore its recovery key; sending is blocked."})
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub async fn enable(
        &self,
        api: &ServerApi,
        backup: &Path,
        recovery: Option<&Path>,
    ) -> anyhow::Result<Value> {
        let url = url::Url::parse(&api.base_url)?;
        ensure!(
            url.scheme() == "https"
                || matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
            "Encryption setup requires HTTPS (except isolated localhost testing)"
        );
        let directory: Directory = decode(
            api.request(reqwest::Method::GET, "/v1/e2ee/state")
                .send()
                .await?,
        )
        .await?;
        // Enrollment is explicit. It never changes an already pinned network.
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
            Keyring::create(&network, self.account)?
        };
        ring.export_recovery(backup)?;
        let identity = ring.identity().public();
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
            }
        };
        if read_setup(&self.binding)?.is_none() {
            write_setup(&self.binding, &setup, false)?;
        }
        *self.active.write().expect("privacy state lock") =
            Ok(Some(Arc::new(Self::load(&self.root, &setup)?)));
        Ok(self.status())
    }

    /// Trust-on-first-use applies when the authenticated account intentionally
    /// gains a new friend. Existing contacts are never removed or replaced,
    /// and the keyring separately refuses changes to an already pinned key.
    fn sync_contacts(&self, snapshot: &Snapshot) -> anyhow::Result<bool> {
        let Some(vault) = self.active()? else {
            return Ok(false);
        };
        let mut setup = read_setup(&self.binding)?.context("Missing privacy binding")?;
        ensure!(
            setup.network == vault.network && setup.account == vault.account,
            "Privacy binding changed"
        );
        let mut changed = false;
        for friend in &snapshot.friends {
            if let std::collections::btree_map::Entry::Vacant(entry) =
                setup.contacts.entry(friend.user.id)
            {
                entry.insert(friend.user.display_name.clone());
                changed = true;
            }
        }
        if changed {
            write_setup(&self.binding, &setup, true)?;
            *self.active.write().expect("privacy state lock") =
                Ok(Some(Arc::new(Self::load(&self.root, &setup)?)));
        }
        Ok(changed)
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
                || !friend_ids.contains(&pending.user_id)
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
        let directory: Directory = decode(
            api.request(reqwest::Method::GET, "/v1/e2ee/state")
                .send()
                .await?,
        )
        .await?;
        Self::verify_directory(vault, &directory)?;
        Ok(directory)
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
                .all(|id| vault.contacts.contains_key(id))
                && directory.rosters.values().flatten().all(|roster| roster
                    .roster
                    .members
                    .keys()
                    .all(|id| vault.contacts.contains_key(id))),
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
        let binding = MessageContext {
            network: vault.network,
            conversation: roster.roster.conversation.clone(),
            sender: vault.account,
            message: id,
            roster: roster.hash()?,
        };
        Ok(EncryptedMessageRequest {
            id,
            conversation_id: binding.conversation.clone(),
            roster_hash: binding.roster.clone(),
            ciphertext: STANDARD.encode(binding.seal(
                vault.ring.identity(),
                recipients,
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
        if let Ok(Some(vault)) = self.active() {
            Self::restore_contact_names(&vault, snapshot);
        }
        if !snapshot.chat_encryption_required
            && matches!(self.active(), Ok(None))
            && !snapshot.messages.iter().any(|m| m.encryption_version != 0)
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
            for (conversation, chain) in &directory.rosters {
                vault
                    .ring
                    .accept_rosters(vault.network, conversation, vault.account, chain)?;
            }
            Ok::<_, anyhow::Error>((vault, directory))
        }
        .await;
        *self.last_error.lock().expect("privacy error lock") =
            result.as_ref().err().map(ToString::to_string);
        self.decrypted.lock().expect("decrypted cache").clear();
        let block_plaintext =
            snapshot.chat_encryption_required || !matches!(self.active(), Ok(None));
        let decode_message = |message: &mut Message| {
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
        for invite in &mut snapshot.room_invitations {
            restore(&mut invite.from);
        }
    }
}
