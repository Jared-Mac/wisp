use super::{Daemon, chat_images, decode, string_arg};
use anyhow::{Context, bail};
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};
use tokio::io::AsyncWriteExt;
use tracing::warn;
use wisp_protocol::{BeginFileUpload, CHAT_FILE_CHUNK_BYTES, FileUploadStatus};

struct PartialDownload {
    path: PathBuf,
    complete: bool,
}
impl Drop for PartialDownload {
    fn drop(&mut self) {
        if !self.complete {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl Daemon {
    fn transfer_progress(&self, id: uuid::Uuid, direction: &str, bytes: u64, total: u64) {
        self.emit(
            "file_transfer_progress",
            json!({"id":id,"direction":direction,"bytes":bytes,"total":total}),
            self.next_seq(0),
        );
    }

    pub(super) async fn send_chunked_file(
        &self,
        token: uuid::Uuid,
        draft: chat_images::AttachmentDraft,
        conversation_id: String,
        caption: String,
        keep: bool,
    ) -> anyhow::Result<Option<Value>> {
        // Check the original file descriptor before resuming a partially sent file.
        let _ = draft.chunk(draft.size).await?;
        let mut status: FileUploadStatus = decode(
            self.api
                .request(reqwest::Method::POST, "/v1/file-uploads")
                .json(&BeginFileUpload {
                    id: token,
                    conversation_id,
                    file_name: draft.file_name.clone(),
                    size: draft.size,
                    caption,
                    keep,
                })
                .send()
                .await?,
        )
        .await?;
        if status.received_bytes > draft.size {
            bail!("Invalid upload offset from server");
        }
        self.transfer_progress(token, "upload", status.received_bytes, draft.size);
        while status.message_id.is_none() && status.received_bytes < draft.size {
            let bytes = draft.chunk(status.received_bytes).await?;
            let previous = status.received_bytes;
            status = decode(
                self.api
                    .request(
                        reqwest::Method::PUT,
                        &format!("/v1/file-uploads/{token}/chunks/{}", status.next_chunk),
                    )
                    .timeout(Duration::from_secs(120))
                    .header("content-type", "application/octet-stream")
                    .body(bytes)
                    .send()
                    .await?,
            )
            .await?;
            if status.received_bytes <= previous || status.received_bytes > draft.size {
                bail!("Upload made no progress");
            }
            self.transfer_progress(token, "upload", status.received_bytes, draft.size);
        }
        let _: wisp_protocol::Message = decode(
            self.api
                .request(
                    reqwest::Method::POST,
                    &format!("/v1/file-uploads/{token}/complete"),
                )
                .send()
                .await?,
        )
        .await?;
        self.chat_images.discard(token).await;
        if let Err(error) = self.refresh("message_created").await {
            warn!(%error, "file sent but snapshot refresh failed");
        }
        Ok(None)
    }

    pub(super) async fn save_streamed_file(&self, args: &Value) -> anyhow::Result<Option<Value>> {
        let id: uuid::Uuid = string_arg(args, "message_id")?.parse()?;
        let message = self
            .state
            .read()
            .await
            .messages
            .iter()
            .find(|message| message.id == id && message.content_type == "application/octet-stream")
            .cloned()
            .context("File is not in your visible chat history")?;
        if message.payload["expired"] == true {
            bail!("This file has expired");
        }
        let name = message.payload["file_name"]
            .as_str()
            .context("Missing filename")?;
        if !wisp_protocol::valid_chat_file_name(name) {
            bail!("Invalid filename");
        }
        let expected = message.payload["size"]
            .as_u64()
            .context("Missing file size")?;
        // A separate client has a read-idle timeout, not a total file-duration limit.
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(120))
            .build()?;
        let auth = self
            .api
            .token
            .read()
            .map_err(|_| anyhow::anyhow!("Session lock unavailable"))?
            .clone();
        let mut response = client
            .get(format!("{}/v1/messages/{id}/file", self.api.base_url))
            .bearer_auth(auth)
            .send()
            .await?
            .error_for_status()?;
        let directory = tokio::process::Command::new("xdg-user-dir")
            .arg("DOWNLOAD")
            .output()
            .await
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|value| PathBuf::from(value.trim()))
            .filter(|path| path.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Downloads")))
            .context("No Downloads directory available")?
            .join("Wisp")
            .join(uuid::Uuid::new_v4().to_string());
        tokio::fs::create_dir_all(&directory).await?;
        let path = directory.join(name);
        let mut partial = PartialDownload {
            path: path.with_file_name(format!("{}.part", uuid::Uuid::new_v4())),
            complete: false,
        };
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&partial.path)
            .await?;
        let mut received = 0_u64;
        let mut last_report = 0_u64;
        while let Some(bytes) = response.chunk().await? {
            received = received
                .checked_add(bytes.len() as u64)
                .context("Invalid file length")?;
            if received > expected {
                bail!("Downloaded file is larger than its declared size");
            }
            file.write_all(&bytes).await?;
            if received - last_report >= CHAT_FILE_CHUNK_BYTES as u64 {
                self.transfer_progress(id, "download", received, expected);
                last_report = received;
            }
        }
        if received != expected {
            bail!("Download interrupted; try saving the file again");
        }
        file.flush().await?;
        drop(file);
        tokio::fs::rename(&partial.path, &path).await?;
        partial.complete = true;
        self.transfer_progress(id, "download", received, expected);
        Ok(Some(
            json!({"message_id":id,"url":chat_images::file_url(&path)?,"directory_url":chat_images::file_url(&directory)?}),
        ))
    }
}
