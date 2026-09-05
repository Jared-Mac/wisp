use super::{CommandEnvelope, ServerApi, decode, ensure_ok, privacy, string_arg};
use anyhow::{Context, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::io::Read;
use uuid::Uuid;
use wisp_crypto::message::{Content, MessageContext};
use wisp_protocol::{REACTION_CONTENT_TYPE, ReactionRequest};

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "list_emojis" | "upload_emoji" | "remove_emoji" | "emoji_image" | "toggle_reaction"
    )
}

pub(super) async fn command(
    api: &ServerApi,
    privacy: &privacy::Privacy,
    command: &CommandEnvelope,
) -> anyhow::Result<Value> {
    let args = &command.args;
    match command.name.as_str() {
        "list_emojis" => {
            decode(
                api.request(reqwest::Method::GET, "/v1/emojis")
                    .query(&[("after", args["after"].as_str().unwrap_or_default())])
                    .send()
                    .await?,
            )
            .await
        }
        "upload_emoji" => {
            let path = privacy::local_path(&string_arg(args, "path")?)?;
            let bytes = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
                let file = std::fs::File::open(path)?;
                ensure!(file.metadata()?.is_file(), "Choose an image file");
                let mut bytes = Vec::new();
                file.take(2_097_153).read_to_end(&mut bytes)?;
                ensure!(bytes.len() <= 2_097_152, "Choose an image under 2 MB");
                Ok(bytes)
            })
            .await??;
            decode(api.request(reqwest::Method::POST,"/v1/emojis").json(&json!({"name":string_arg(args,"name")?,"scope":string_arg(args,"scope")?,"data":STANDARD.encode(bytes)})).send().await?).await
        }
        "remove_emoji" => {
            let id: Uuid = string_arg(args, "emoji_id")?.parse()?;
            decode(
                api.request(reqwest::Method::DELETE, &format!("/v1/emojis/{id}"))
                    .send()
                    .await?,
            )
            .await
        }
        "emoji_image" => {
            let id: Uuid = string_arg(args, "emoji_id")?.parse()?;
            let response = api
                .request(reqwest::Method::GET, &format!("/v1/emojis/{id}"))
                .send()
                .await?;
            ensure!(response.status().is_success(), "Emoji image unavailable");
            let bytes = response.bytes().await?;
            ensure!(bytes.len() <= 512 * 1024, "Emoji image is too large");
            // Decode and normalize before exposing data to the UI, even when the server is untrusted.
            let png = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
                let mut reader = image::ImageReader::with_format(
                    std::io::Cursor::new(bytes),
                    image::ImageFormat::Png,
                );
                let mut limits = image::Limits::default();
                limits.max_image_width = Some(128);
                limits.max_image_height = Some(128);
                limits.max_alloc = Some(1024 * 1024);
                reader.limits(limits);
                let image = reader.decode()?;
                let mut out = std::io::Cursor::new(Vec::new());
                image.write_to(&mut out, image::ImageFormat::Png)?;
                Ok(out.into_inner())
            })
            .await??;
            Ok(json!({"url":format!("data:image/png;base64,{}",STANDARD.encode(png))}))
        }
        "toggle_reaction" => toggle_reaction(api, privacy, args).await,
        _ => unreachable!(),
    }
}

#[allow(clippy::too_many_lines)] // Keep original-audience validation beside the encrypted mutation.
async fn toggle_reaction(
    api: &ServerApi,
    privacy: &privacy::Privacy,
    args: &Value,
) -> anyhow::Result<Value> {
    let target: Uuid = string_arg(args, "message_id")?.parse()?;
    let emoji = string_arg(args, "emoji")?;
    ensure!(
        !emoji.is_empty() && emoji.len() <= 200 && !emoji.chars().any(char::is_control),
        "Choose an emoji"
    );
    let raw = api.snapshot().await?;
    let mut snapshot = raw.clone();
    privacy.decrypt_snapshot(api, &mut snapshot).await;
    let user = snapshot.self_state.user.id;
    let existing: Vec<_> = snapshot
        .reactions
        .iter()
        .filter(|r| {
            r.target_id == target
                && r.message.sender.id == user
                && r.message.content_type == REACTION_CONTENT_TYPE
                && r.message.payload["target"].as_str() == Some(&target.to_string())
                && r.message.payload["emoji"].as_str() == Some(&emoji)
        })
        .collect();
    if !existing.is_empty() {
        for reaction in existing {
            ensure_ok(
                api.request(
                    reqwest::Method::DELETE,
                    &format!("/v1/reactions/{}", reaction.message.id),
                )
                .send()
                .await?,
            )
            .await?;
        }
        return Ok(json!({"added":false}));
    }
    let message = raw
        .messages
        .iter()
        .find(|m| m.id == target)
        .context("Message is no longer available")?;
    let id = Uuid::new_v4();
    let request = if let Some(vault) = privacy.active()? {
        let conversation = raw
            .conversations
            .iter()
            .find(|c| c.id == message.conversation_id)
            .context("Conversation unavailable")?;
        let (_, roster) = privacy.recipients(api, conversation).await?;
        ensure!(
            message.encryption_version == 1,
            "Cannot react to unverified message content"
        );
        let directory = privacy.directory(api, &vault).await?;
        let hash = message.payload["roster_hash"]
            .as_str()
            .context("Missing original room identity")?;
        let original = directory
            .rosters
            .get(&message.conversation_id)
            .context("Missing original room")?
            .iter()
            .find(|r| r.hash().ok().as_deref() == Some(hash))
            .context("Original room identity is not trusted")?;
        let sender = &original
            .roster
            .members
            .get(&message.sender.id)
            .context("Original sender unavailable")?
            .identity;
        let binding = MessageContext {
            network: vault.network,
            conversation: message.conversation_id.clone(),
            sender: message.sender.id,
            message: target,
            roster: hash.into(),
        };
        let (_, mut recipients) = binding.open_with_recipients(
            vault.ring.identity(),
            vault.account,
            sender,
            &STANDARD.decode(
                message.payload["ciphertext"]
                    .as_str()
                    .context("Missing original ciphertext")?,
            )?,
        )?;
        recipients.retain(|id, key| {
            roster
                .roster
                .members
                .get(id)
                .is_some_and(|m| &m.identity == key)
        });
        let encrypted = privacy::Privacy::seal_to(
            &vault,
            &roster,
            id,
            Content {
                content_type: REACTION_CONTENT_TYPE.into(),
                payload: json!({"target":target,"emoji":emoji}),
                attachment: None,
            },
            &recipients,
        )?;
        ReactionRequest {
            id,
            emoji: None,
            encrypted: Some(encrypted),
        }
    } else {
        ensure!(
            !raw.chat_encryption_required,
            "Chat encryption is not ready"
        );
        ReactionRequest {
            id,
            emoji: Some(emoji),
            encrypted: None,
        }
    };
    ensure_ok(
        api.request(
            reqwest::Method::PUT,
            &format!("/v1/messages/{target}/reactions"),
        )
        .json(&request)
        .send()
        .await?,
    )
    .await?;
    Ok(json!({"added":true}))
}
