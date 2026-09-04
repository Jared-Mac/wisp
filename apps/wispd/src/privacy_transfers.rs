use super::{
    Daemon, chat_images, decode, ensure_ok,
    privacy::{Privacy, Vault},
};
use anyhow::{Context, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
use wisp_crypto::{
    message::Content,
    roster::{Member, Role, SignedRoster},
};
use wisp_protocol::{BeginEncryptedUpload, CommandEnvelope, FileUploadStatus, Message};

impl Daemon {
    pub(super) async fn decrypt_attachment(
        &self,
        id: Uuid,
    ) -> anyhow::Result<tempfile::NamedTempFile> {
        let vault = self
            .privacy
            .active()?
            .context("Restore the encryption recovery key first")?;
        let content = self.privacy.content(id)?;
        let manifest = content
            .attachment
            .context("Missing authenticated attachment manifest")?;
        receive_attachment(&self.api, vault, id, manifest, |received, total| {
            self.transfer_progress(id, "download", received, total);
        })
        .await
    }

    pub(super) async fn cache_encrypted_image(
        &self,
        id: Uuid,
        path: &std::path::Path,
    ) -> anyhow::Result<()> {
        let content = self.privacy.content(id)?;
        ensure!(
            content.content_type == "image/png"
                && content.attachment.as_ref().is_some_and(
                    |m| m.plaintext_bytes <= wisp_protocol::MAX_CHAT_IMAGE_BYTES as u64
                ),
            "Invalid encrypted image size"
        );
        let temporary = self.decrypt_attachment(id).await?;
        let destination = path.to_path_buf();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let reader = image::ImageReader::with_format(
                std::io::BufReader::new(temporary.reopen()?),
                image::ImageFormat::Png,
            );
            let (width, height) = reader.into_dimensions()?;
            ensure!(
                u64::from(width) * u64::from(height) <= wisp_protocol::MAX_CHAT_IMAGE_PIXELS,
                "Image dimensions exceed preview limits"
            );
            let mut output = tempfile::NamedTempFile::new_in(
                destination.parent().context("Missing image cache parent")?,
            )?;
            std::io::copy(&mut temporary.reopen()?, output.as_file_mut())?;
            output.as_file().sync_all()?;
            output.persist_noclobber(destination)?;
            Ok(())
        })
        .await??;
        Ok(())
    }

    pub(super) async fn save_encrypted_file(
        &self,
        message: &Message,
    ) -> anyhow::Result<Option<Value>> {
        use std::{os::unix::fs::DirBuilderExt, path::PathBuf};
        let content = self.privacy.content(message.id)?;
        let name = content.payload["file_name"]
            .as_str()
            .context("Missing authenticated filename")?
            .to_owned();
        ensure!(
            wisp_protocol::valid_chat_file_name(&name),
            "Invalid filename"
        );
        let file = self.decrypt_attachment(message.id).await?;
        let directory = tokio::process::Command::new("xdg-user-dir")
            .arg("DOWNLOAD")
            .output()
            .await
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|p| PathBuf::from(p.trim()))
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Downloads")))
            .context("Downloads directory unavailable")?
            .join("Wisp")
            .join(Uuid::new_v4().to_string());
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&directory)?;
        let path = directory.join(name);
        let destination = path.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let mut output = tempfile::NamedTempFile::new_in(
                destination.parent().context("Missing download directory")?,
            )?;
            std::io::copy(&mut file.reopen()?, output.as_file_mut())?;
            output.as_file().sync_all()?;
            output.persist_noclobber(destination)?;
            Ok(())
        })
        .await??;
        Ok(Some(
            json!({"message_id":message.id,"url":chat_images::file_url(&path)?,"directory_url":chat_images::file_url(&directory)?}),
        ))
    }

    async fn encrypted_recipients(&self, id: &str) -> anyhow::Result<(Arc<Vault>, SignedRoster)> {
        let snapshot = self.api.snapshot().await?;
        let conversation = snapshot
            .conversations
            .iter()
            .find(|c| c.id == id)
            .context("Conversation is not available")?;
        self.privacy.recipients(&self.api, conversation).await
    }

    pub(super) async fn send_chat_text(
        &self,
        conversation_id: String,
        text: String,
    ) -> anyhow::Result<()> {
        if self.privacy.active()?.is_none() {
            return self.api.send_message(conversation_id, text).await;
        }
        let (vault, roster) = self.encrypted_recipients(&conversation_id).await?;
        let request = Privacy::seal(
            &vault,
            &roster,
            Uuid::new_v4(),
            Content {
                content_type: "text/plain".into(),
                payload: json!(text),
                attachment: None,
            },
        )?;
        let _: Message = decode(
            self.api
                .request(reqwest::Method::POST, "/v1/e2ee/messages")
                .json(&request)
                .send()
                .await?,
        )
        .await?;
        Ok(())
    }

    pub(super) async fn edit_encrypted_message(
        &self,
        id: Uuid,
        text: String,
    ) -> anyhow::Result<()> {
        let message = self
            .state
            .read()
            .await
            .messages
            .iter()
            .find(|m| m.id == id)
            .cloned()
            .context("Message is not visible")?;
        let (vault, roster) = self.encrypted_recipients(&message.conversation_id).await?;
        ensure!(
            message.sender.id == vault.account,
            "Only your messages can be edited"
        );
        let mut content = if message.encryption_version == 1 {
            self.privacy.content(id)?
        } else {
            ensure!(
                message.content_type == "text/plain",
                "Old attachments require complete migration, not only an encrypted caption"
            );
            Content {
                content_type: "text/plain".into(),
                payload: message.payload,
                attachment: None,
            }
        };
        if content.content_type == "text/plain" {
            content.payload = json!(text);
        } else {
            content.payload["caption"] = json!(text);
        }
        // Do not redistribute an old message to newly admitted participants.
        let raw = self
            .api
            .snapshot()
            .await?
            .messages
            .into_iter()
            .find(|m| m.id == id)
            .context("Message was removed")?;
        let request = if message.encryption_version == 1 {
            let binding = wisp_crypto::message::MessageContext {
                network: vault.network,
                conversation: raw.conversation_id,
                sender: vault.account,
                message: id,
                roster: raw.payload["roster_hash"]
                    .as_str()
                    .context("Missing original signed roster")?
                    .into(),
            };
            let (_, mut recipients) = binding.open_with_recipients(
                vault.ring.identity(),
                vault.account,
                &vault.ring.identity().public(),
                &STANDARD.decode(
                    raw.payload["ciphertext"]
                        .as_str()
                        .context("Missing original ciphertext")?,
                )?,
            )?;
            recipients.retain(|id, key| {
                roster
                    .roster
                    .members
                    .get(id)
                    .is_some_and(|member| &member.identity == key)
            });
            Privacy::seal_to(&vault, &roster, id, content, &recipients)?
        } else {
            Privacy::seal(&vault, &roster, id, content)?
        };
        ensure_ok(
            self.api
                .request(reqwest::Method::PUT, &format!("/v1/e2ee/messages/{id}"))
                .json(&request)
                .send()
                .await?,
        )
        .await
    }

    pub(super) async fn change_encrypted_room(
        &self,
        command: &CommandEnvelope,
    ) -> anyhow::Result<()> {
        let id = super::string_arg(&command.args, "conversation_id")?;
        let target: Uuid = super::string_arg(&command.args, "user_id")?.parse()?;
        let (vault, previous) = self.encrypted_recipients(&id).await?;
        let mut roster = previous.roster.clone();
        roster.actor = vault.account;
        roster.revision = roster
            .revision
            .checked_add(1)
            .context("Room version overflow")?;
        roster.previous = Some(previous.hash()?);
        if command.name == "invite_to_room" {
            if roster.members.contains_key(&target) {
                return Ok(());
            }
            let directory = self.privacy.directory(&self.api, &vault).await?;
            let identity = directory
                .identities
                .get(&target)
                .context("This friend needs to enable encrypted chat first")?
                .clone();
            roster.members.insert(
                target,
                Member {
                    identity,
                    role: Role::Member,
                },
            );
        } else {
            let admin = command.args["admin"]
                .as_bool()
                .context("Admin choice required")?;
            roster
                .members
                .get_mut(&target)
                .context("Friend is not in this room")?
                .role = if admin { Role::Admin } else { Role::Member };
        }
        let signed = roster.sign(vault.ring.identity())?;
        signed.verify_successor(&previous)?;
        ensure_ok(
            self.api
                .request(reqwest::Method::POST, "/v1/e2ee/roster")
                .json(&signed)
                .send()
                .await?,
        )
        .await
    }

    pub(super) async fn send_encrypted_attachment(
        &self,
        token: Uuid,
        draft: chat_images::AttachmentDraft,
        conversation_id: String,
        caption: String,
        keep: bool,
    ) -> anyhow::Result<Option<Value>> {
        let (vault, roster) = self.encrypted_recipients(&conversation_id).await?;
        let recipients = roster
            .roster
            .members
            .values()
            .map(|m| m.identity.clone())
            .collect::<Vec<_>>();
        let directory = vault.temporary.clone();
        let original = draft.clone();
        let prepared = tokio::task::spawn_blocking(move || {
            wisp_crypto::attachment::prepare(original.reader(), &recipients, &directory)
        })
        .await??;
        let size = prepared.manifest.ciphertext_bytes;
        // Randomized ciphertext must never resume over an older upload's bytes.
        let upload = Uuid::new_v4();
        let message = Privacy::seal(
            &vault,
            &roster,
            upload,
            Content {
                content_type: if draft.is_image {
                    "image/png"
                } else {
                    "application/octet-stream"
                }
                .into(),
                payload: json!({"file_name":draft.file_name,"size":draft.size,"caption":caption}),
                attachment: Some(prepared.manifest),
            },
        )?;
        let mut status: FileUploadStatus = decode(
            self.api
                .request(reqwest::Method::POST, "/v1/e2ee/file-uploads")
                .json(&BeginEncryptedUpload {
                    upload_id: upload,
                    size,
                    plaintext_size: Some(draft.size),
                    keep,
                    message,
                })
                .send()
                .await?,
        )
        .await?;
        self.transfer_progress(token, "upload", 0, size);
        while status.received_bytes < size {
            let length = usize::try_from(
                (size - status.received_bytes).min(wisp_protocol::CHAT_FILE_CHUNK_BYTES as u64),
            )?;
            let mut bytes = vec![0; length];
            std::os::unix::fs::FileExt::read_exact_at(
                prepared.file.as_file(),
                &mut bytes,
                status.received_bytes,
            )?;
            let previous = status.received_bytes;
            status = decode(
                self.api
                    .request(
                        reqwest::Method::PUT,
                        &format!("/v1/file-uploads/{upload}/chunks/{}", status.next_chunk),
                    )
                    .timeout(Duration::from_secs(120))
                    .body(bytes)
                    .send()
                    .await?,
            )
            .await?;
            ensure!(
                status.received_bytes == previous + length as u64,
                "Unexpected upload offset"
            );
            self.transfer_progress(token, "upload", status.received_bytes, size);
        }
        let _: Message = decode(
            self.api
                .request(
                    reqwest::Method::POST,
                    &format!("/v1/file-uploads/{upload}/complete"),
                )
                .send()
                .await?,
        )
        .await?;
        self.chat_images.discard(token).await;
        self.refresh("message_created").await?;
        Ok(None)
    }
}

/// Testable transport shared by image previews, clipboard copies and downloads.
/// The manifest must come from authenticated decrypted message content.
pub(super) async fn receive_attachment(
    api: &super::ServerApi,
    vault: Arc<Vault>,
    id: Uuid,
    manifest: wisp_crypto::attachment::Manifest,
    progress: impl Fn(u64, u64),
) -> anyhow::Result<tempfile::NamedTempFile> {
    use tokio::io::AsyncWriteExt;
    let temporary = tempfile::NamedTempFile::new_in(&vault.temporary)?;
    let mut file = tokio::fs::File::from_std(temporary.reopen()?);
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(120))
        .build()?;
    let auth = api.token.read().expect("session token lock").clone();
    let mut response = client
        .get(format!("{}/v1/messages/{id}/file", api.base_url))
        .bearer_auth(auth)
        .send()
        .await?
        .error_for_status()?;
    let mut received = 0_u64;
    while let Some(bytes) = response.chunk().await? {
        received = received
            .checked_add(bytes.len() as u64)
            .context("Invalid encrypted file length")?;
        ensure!(
            received <= manifest.ciphertext_bytes,
            "Encrypted download exceeds its signed size"
        );
        file.write_all(&bytes).await?;
        progress(received, manifest.ciphertext_bytes);
    }
    ensure!(
        received == manifest.ciphertext_bytes,
        "Encrypted file download interrupted"
    );
    file.flush().await?;
    drop(file);
    tokio::task::spawn_blocking(move || {
        wisp_crypto::attachment::decrypt(
            vault.ring.identity(),
            temporary.reopen()?,
            &manifest,
            &vault.temporary,
        )
    })
    .await?
}
