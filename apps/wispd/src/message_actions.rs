use super::{
    Daemon, LinkedServer, ServerApi, chat_images::AttachmentDraft, decode, privacy::Privacy,
    privacy_transfers::receive_attachment, string_arg,
};
use anyhow::{Context, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::io::AsyncWriteExt;
use tracing::warn;
use uuid::Uuid;
use wisp_crypto::message::Content;
use wisp_protocol::{
    BeginEncryptedUpload, BeginFileUpload, CommandEnvelope, FileUploadStatus, ForwardedFrom,
    Message, MessageContext, ReplyReference, SendImageMessageRequest, SendMessageRequest,
};

enum Session<'a> {
    Primary(&'a Daemon),
    Linked(Arc<LinkedServer>),
}
impl Session<'_> {
    fn api(&self) -> &ServerApi {
        match self {
            Self::Primary(d) => &d.api,
            Self::Linked(s) => &s.api,
        }
    }
    fn privacy(&self) -> &Privacy {
        match self {
            Self::Primary(d) => &d.privacy,
            Self::Linked(s) => &s.privacy,
        }
    }
    async fn message(&self, id: Uuid) -> anyhow::Result<Message> {
        let raw: Message = decode(
            self.api()
                .request(reqwest::Method::GET, &format!("/v1/messages/{id}"))
                .send()
                .await?,
        )
        .await?;
        ensure!(
            raw.encryption_version <= 1,
            "Unsupported message encryption version"
        );
        let mut snapshot = self.api().snapshot().await?;
        ensure!(
            raw.encryption_version != 0
                || (!snapshot.chat_encryption_required && self.privacy().active()?.is_none()),
            "Unencrypted message cannot be used while encrypted chat is required"
        );
        snapshot.messages.retain(|m| m.id != id);
        snapshot.messages.push(raw);
        self.privacy()
            .decrypt_snapshot(self.api(), &mut snapshot)
            .await;
        let message = snapshot
            .messages
            .into_iter()
            .find(|m| m.id == id)
            .context("Message is no longer available in your chat history")?;
        if message.encryption_version == 1 {
            self.privacy().content(id)?;
        }
        ensure!(
            matches!(
                message.content_type.as_str(),
                "text/plain" | "image/png" | "application/octet-stream"
            ),
            "This message cannot be replied to or forwarded"
        );
        Ok(message)
    }
}

fn author(name: &str) -> String {
    let value: String = name.chars().filter(|c| !c.is_control()).take(100).collect();
    if value.trim().is_empty() {
        "Unknown".into()
    } else {
        value
    }
}

pub(super) fn reference(message: &Message) -> ReplyReference {
    let text = if message.content_type == "text/plain" {
        message.payload.as_str().unwrap_or_default().to_owned()
    } else {
        format!(
            "{} · {}",
            if message.content_type == "image/png" {
                "Image"
            } else {
                message.payload["file_name"].as_str().unwrap_or("File")
            },
            message.payload["caption"].as_str().unwrap_or_default()
        )
    };
    ReplyReference {
        message_id: message.id,
        sender_name: author(&message.sender.display_name),
        preview: text.chars().take(240).collect(),
    }
}

fn forwarding_context(original: &Message) -> MessageContext {
    MessageContext {
        reply_to: None,
        forwarded_from: Some(
            original
                .context
                .as_ref()
                .and_then(|c| c.forwarded_from.clone())
                .unwrap_or(ForwardedFrom {
                    sender_name: author(&original.sender.display_name),
                }),
        ),
    }
}

impl Daemon {
    async fn message_session(&self, id: &str) -> anyhow::Result<Session<'_>> {
        if id.is_empty() || id == self.primary_server.id {
            return Ok(Session::Primary(self));
        }
        Ok(Session::Linked(
            self.linked_servers
                .read()
                .await
                .get(id)
                .cloned()
                .context("Server is not connected")?,
        ))
    }

    pub(super) async fn message_attachment_action(
        &self,
        command: &CommandEnvelope,
    ) -> anyhow::Result<Value> {
        use super::chat_images;
        use std::{os::unix::fs::DirBuilderExt, path::PathBuf};
        let _cache = self.chat_images.cache_operation.lock().await;
        let id: Uuid = string_arg(&command.args, "message_id")?.parse()?;
        let session = self
            .message_session(command.args["server_id"].as_str().unwrap_or_default())
            .await?;
        let message = session.message(id).await?;
        let image = command.name != "save_chat_file";
        ensure!(
            message.content_type
                == if image {
                    "image/png"
                } else {
                    "application/octet-stream"
                },
            "Wrong attachment type"
        );
        let path = if image {
            chat_images::cache_path(id, Some(&message.conversation_id))?
        } else {
            let name = message.payload["file_name"]
                .as_str()
                .context("Missing filename")?;
            ensure!(
                wisp_protocol::valid_chat_file_name(name),
                "Invalid filename"
            );
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
            directory.join(name)
        };
        if !path.is_file() {
            let file = download(&session, &message).await?;
            let destination = path.clone();
            tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                if image {
                    ensure!(
                        file.as_file().metadata()?.len()
                            <= wisp_protocol::MAX_CHAT_IMAGE_BYTES as u64,
                        "Image exceeds 12 MB"
                    );
                    let (w, h) = image::ImageReader::with_format(
                        std::io::BufReader::new(file.reopen()?),
                        image::ImageFormat::Png,
                    )
                    .into_dimensions()?;
                    ensure!(
                        u64::from(w) * u64::from(h) <= wisp_protocol::MAX_CHAT_IMAGE_PIXELS,
                        "Image dimensions exceed preview limits"
                    );
                }
                let mut output = tempfile::NamedTempFile::new_in(
                    destination
                        .parent()
                        .context("Missing attachment directory")?,
                )?;
                std::io::copy(&mut file.reopen()?, output.as_file_mut())?;
                output.as_file().sync_all()?;
                output.persist_noclobber(destination)?;
                Ok(())
            })
            .await??;
        }
        if command.name == "copy_chat_image" {
            self.chat_images.copy_image(path).await?;
            return Ok(json!({"message_id":id,"copied":true}));
        }
        Ok(
            json!({"message_id":id,"url":chat_images::file_url(&path)?,"directory_url":chat_images::file_url(path.parent().context("Missing attachment directory")?)?}),
        )
    }

    pub(super) async fn message_action(&self, command: &CommandEnvelope) -> anyhow::Result<Value> {
        let args = &command.args;
        let destination = self
            .message_session(args["server_id"].as_str().unwrap_or_default())
            .await?;
        if command.name == "list_pins" {
            let conversation_id = string_arg(args, "conversation_id")?;
            let mut result: Value = decode(
                destination
                    .api()
                    .request(reqwest::Method::GET, "/v1/pins")
                    .query(&[("conversation_id", &conversation_id)])
                    .send()
                    .await?,
            )
            .await?;
            let pinned: Vec<Message> = serde_json::from_value(result["messages"].clone())?;
            let ids: Vec<Uuid> = pinned.iter().map(|m| m.id).collect();
            let mut snapshot = destination.api().snapshot().await?;
            snapshot.messages.retain(|m| !ids.contains(&m.id));
            snapshot.messages.extend(pinned);
            destination
                .privacy()
                .decrypt_snapshot(destination.api(), &mut snapshot)
                .await;
            result["messages"] = json!(
                ids.iter()
                    .filter_map(|id| snapshot.messages.iter().find(|m| &m.id == id))
                    .collect::<Vec<_>>()
            );
            return Ok(result);
        }
        if command.name == "load_message" {
            let message = destination
                .message(string_arg(args, "message_id")?.parse()?)
                .await?;
            ensure!(
                message.conversation_id == string_arg(args, "conversation_id")?,
                "Message belongs to another conversation"
            );
            return Ok(json!(message));
        }
        if command.name == "set_message_pin" {
            let id: Uuid = string_arg(args, "message_id")?.parse()?;
            return decode(
                destination
                    .api()
                    .request(reqwest::Method::PUT, &format!("/v1/messages/{id}/pin"))
                    .json(
                        &json!({"pinned":args["pinned"].as_bool().context("Choose pin or unpin")?}),
                    )
                    .send()
                    .await?,
            )
            .await;
        }
        self.send_message_action(command, destination).await
    }

    async fn send_message_action(
        &self,
        command: &CommandEnvelope,
        destination: Session<'_>,
    ) -> anyhow::Result<Value> {
        let args = &command.args;
        let forwarding = command.name == "forward_message";
        let source = if forwarding {
            self.message_session(args["source_server_id"].as_str().unwrap_or_default())
                .await?
        } else {
            self.message_session(args["server_id"].as_str().unwrap_or_default())
                .await?
        };
        let original = source
            .message(string_arg(args, if forwarding { "message_id" } else { "reply_to" })?.parse()?)
            .await?;
        let conversation_id = if forwarding && args["friend"].as_str().is_some() {
            destination
                .api()
                .create_direct(string_arg(args, "friend")?)
                .await?
                .id
        } else {
            string_arg(args, "conversation_id")?
        };
        if !forwarding {
            ensure!(
                original.conversation_id == conversation_id,
                "Replies must stay in the original conversation"
            );
        }
        let context = if forwarding {
            forwarding_context(&original)
        } else {
            MessageContext {
                reply_to: Some(reference(&original)),
                forwarded_from: None,
            }
        };
        let token: Option<Uuid> = args["token"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::parse)
            .transpose()?;
        let mut temporary = None;
        let draft = if forwarding && original.content_type != "text/plain" {
            temporary = Some(download(&source, &original).await?);
            Some(AttachmentDraft::from_temporary(
                temporary.as_ref().unwrap(),
                original.payload["file_name"]
                    .as_str()
                    .unwrap_or("Image.png")
                    .to_owned(),
                original.content_type == "image/png",
            )?)
        } else if !forwarding {
            match token {
                Some(token) => Some(self.chat_images.draft(token).await?),
                None => None,
            }
        } else {
            None
        };
        let text = if forwarding {
            if original.content_type == "text/plain" {
                original.payload.as_str().unwrap_or_default()
            } else {
                original.payload["caption"].as_str().unwrap_or_default()
            }
            .to_owned()
        } else {
            args["text"].as_str().unwrap_or_default().trim().to_owned()
        };
        ensure!(
            draft.is_some() || !text.trim().is_empty(),
            "Message must not be empty"
        );
        let transfer_id = token.unwrap_or_else(Uuid::new_v4);
        let sent = send_content(
            &destination,
            &conversation_id,
            text,
            context,
            draft,
            !forwarding && args["keep"] == true,
            |bytes, total| self.transfer_progress(transfer_id, "upload", bytes, total),
        )
        .await?;
        if !forwarding && let Some(token) = token {
            self.chat_images.discard(token).await;
        }
        drop(temporary);
        let refresh = match &destination {
            Session::Primary(_) => self.refresh("message_created").await,
            Session::Linked(s) => self.refresh_linked(s, "message_created").await,
        };
        if let Err(error) = refresh {
            warn!(%error, "message action sent but snapshot refresh failed");
        }
        Ok(json!({"message_id": sent.id, "conversation_id": conversation_id}))
    }
}

async fn download(
    session: &Session<'_>,
    message: &Message,
) -> anyhow::Result<tempfile::NamedTempFile> {
    ensure!(message.payload["expired"] != true, "This file has expired");
    if message.encryption_version == 1 {
        let vault = session
            .privacy()
            .active()?
            .context("Restore your encryption key first")?;
        let manifest = session
            .privacy()
            .content(message.id)?
            .attachment
            .context("Missing authenticated attachment")?;
        return receive_attachment(session.api(), vault, message.id, manifest, |_, _| {}).await;
    }
    let image = message.content_type == "image/png";
    let limit = if image {
        wisp_protocol::MAX_CHAT_IMAGE_BYTES as u64
    } else {
        message.payload["size"]
            .as_u64()
            .context("Missing file size")?
    };
    let mut response = session
        .api()
        .request(
            reqwest::Method::GET,
            &format!(
                "/v1/messages/{}/{}",
                message.id,
                if image { "image" } else { "file" }
            ),
        )
        .timeout(Duration::from_secs(120))
        .send()
        .await?
        .error_for_status()?;
    let temporary = tempfile::NamedTempFile::new()?;
    let mut file = tokio::fs::File::from_std(temporary.reopen()?);
    let mut received = 0u64;
    while let Some(chunk) = response.chunk().await? {
        received = received
            .checked_add(chunk.len() as u64)
            .context("Invalid download size")?;
        ensure!(received <= limit, "File exceeds its expected size");
        file.write_all(&chunk).await?;
    }
    ensure!(image || received == limit, "File download was interrupted");
    file.flush().await?;
    Ok(temporary)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
async fn send_content(
    session: &Session<'_>,
    conversation_id: &str,
    text: String,
    metadata: MessageContext,
    draft: Option<AttachmentDraft>,
    keep: bool,
    progress: impl Fn(u64, u64),
) -> anyhow::Result<Message> {
    metadata.validate().map_err(anyhow::Error::msg)?;
    let snapshot = session.api().snapshot().await?;
    let conversation = snapshot
        .conversations
        .iter()
        .find(|c| c.id == conversation_id && !c.pending_access)
        .context("Chat is not available for sending")?;
    let vault = session.privacy().active()?;
    ensure!(
        vault.is_some() || !snapshot.chat_encryption_required,
        "Chat encryption is not ready"
    );
    let api = session.api();
    if let Some(vault) = vault {
        let (_, roster) = session.privacy().recipients(api, conversation).await?;
        let id = Uuid::new_v4();
        let mut content = Content {
            content_type: "text/plain".into(),
            payload: json!(text),
            attachment: None,
            context: Some(metadata),
        };
        let prepared = if let Some(draft) = draft {
            let members = roster
                .roster
                .members
                .values()
                .map(|m| m.identity.clone())
                .collect::<Vec<_>>();
            let directory = vault.temporary.clone();
            content.content_type = if draft.is_image {
                "image/png"
            } else {
                "application/octet-stream"
            }
            .into();
            content.payload = json!({"file_name":draft.file_name,"caption":text,"size":draft.size});
            let encrypted = tokio::task::spawn_blocking(move || {
                wisp_crypto::attachment::prepare(draft.reader(), &members, &directory)
            })
            .await??;
            content.attachment = Some(encrypted.manifest.clone());
            Some(encrypted)
        } else {
            None
        };
        let message = Privacy::seal(&vault, &roster, id, content)?;
        if let Some(prepared) = prepared {
            let size = prepared.manifest.ciphertext_bytes;
            let status: FileUploadStatus = decode(
                api.request(reqwest::Method::POST, "/v1/e2ee/file-uploads")
                    .json(&BeginEncryptedUpload {
                        upload_id: id,
                        size,
                        plaintext_size: Some(prepared.manifest.plaintext_bytes),
                        keep,
                        message,
                    })
                    .send()
                    .await?,
            )
            .await?;
            let draft =
                AttachmentDraft::from_temporary(&prepared.file, "ciphertext".into(), false)?;
            upload(api, id, draft, status, progress).await
        } else {
            decode(
                api.request(reqwest::Method::POST, "/v1/e2ee/messages")
                    .json(&message)
                    .send()
                    .await?,
            )
            .await
        }
    } else if let Some(draft) = draft {
        if draft.is_image {
            ensure!(
                draft.size <= wisp_protocol::MAX_CHAT_IMAGE_BYTES as u64,
                "Image exceeds 12 MB"
            );
            let bytes = tokio::task::spawn_blocking(move || {
                let mut bytes = Vec::new();
                std::io::Read::read_to_end(&mut draft.reader(), &mut bytes)?;
                Ok::<_, std::io::Error>(bytes)
            })
            .await??;
            decode(
                api.request(reqwest::Method::POST, "/v1/messages/image")
                    .json(&SendImageMessageRequest {
                        conversation_id: conversation_id.into(),
                        caption: text,
                        png_base64: STANDARD.encode(bytes),
                        context: Some(metadata),
                    })
                    .send()
                    .await?,
            )
            .await
        } else {
            let id = Uuid::new_v4();
            let status: FileUploadStatus = decode(
                api.request(reqwest::Method::POST, "/v1/file-uploads")
                    .json(&BeginFileUpload {
                        id,
                        conversation_id: conversation_id.into(),
                        file_name: draft.file_name.clone(),
                        size: draft.size,
                        caption: text,
                        keep,
                        context: Some(metadata),
                    })
                    .send()
                    .await?,
            )
            .await?;
            upload(api, id, draft, status, progress).await
        }
    } else {
        decode(
            api.request(reqwest::Method::POST, "/v1/messages")
                .json(&SendMessageRequest {
                    conversation_id: conversation_id.into(),
                    content_type: "text/plain".into(),
                    payload: json!(text),
                    encryption_version: 0,
                    context: Some(metadata),
                })
                .send()
                .await?,
        )
        .await
    }
}

async fn upload(
    api: &ServerApi,
    id: Uuid,
    draft: AttachmentDraft,
    mut status: FileUploadStatus,
    progress: impl Fn(u64, u64),
) -> anyhow::Result<Message> {
    ensure!(
        status.received_bytes <= draft.size,
        "Unexpected upload offset"
    );
    progress(status.received_bytes, draft.size);
    while status.received_bytes < draft.size {
        let bytes = draft.chunk(status.received_bytes).await?;
        let expected = status.received_bytes + bytes.len() as u64;
        status = decode(
            api.request(
                reqwest::Method::PUT,
                &format!("/v1/file-uploads/{id}/chunks/{}", status.next_chunk),
            )
            .timeout(Duration::from_secs(120))
            .body(bytes)
            .send()
            .await?,
        )
        .await?;
        ensure!(
            status.received_bytes == expected,
            "Unexpected upload offset"
        );
        progress(status.received_bytes, draft.size);
    }
    decode(
        api.request(
            reqwest::Method::POST,
            &format!("/v1/file-uploads/{id}/complete"),
        )
        .send()
        .await?,
    )
    .await
}

#[cfg(test)]
#[path = "message_actions_tests.rs"]
mod tests;
