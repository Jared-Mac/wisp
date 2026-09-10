//! Structured domain separation; callers pin both the network identity and
//! recipient roster locally rather than trusting server-supplied membership.
use crate::{Identity, PublicIdentity};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageContext {
    pub network: Uuid,
    pub conversation: String,
    pub sender: Uuid,
    pub message: Uuid,
    pub roster: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Content {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<wisp_protocol::MessageContext>,
    pub content_type: String,
    pub payload: Value,
    pub attachment: Option<crate::attachment::Manifest>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    recipients: BTreeMap<Uuid, PublicIdentity>,
    content: Content,
}

impl MessageContext {
    fn transcript(&self) -> anyhow::Result<String> {
        // JSON encoding avoids separator ambiguity in conversation IDs.
        Ok(serde_json::to_string(&("wisp-chat-v1", self))?)
    }

    pub fn seal(
        &self,
        identity: &Identity,
        recipients: &BTreeMap<Uuid, PublicIdentity>,
        content: Content,
    ) -> anyhow::Result<Vec<u8>> {
        ensure!(
            recipients.get(&self.sender) == Some(&identity.public()),
            "Sender identity missing from verified recipients"
        );
        content.validate()?;
        let body = zeroize::Zeroizing::new(serde_json::to_vec(&Envelope {
            recipients: recipients.clone(),
            content,
        })?);
        identity.seal(
            &self.transcript()?,
            &body,
            &recipients.values().cloned().collect::<Vec<_>>(),
        )
    }

    pub fn open(
        &self,
        identity: &Identity,
        recipient: Uuid,
        sender: &PublicIdentity,
        cipher: &[u8],
    ) -> anyhow::Result<Content> {
        self.open_with_recipients(identity, recipient, sender, cipher)
            .map(|(content, _)| content)
    }

    /// Return the authenticated original audience for privacy-preserving edits.
    pub fn open_with_recipients(
        &self,
        identity: &Identity,
        recipient: Uuid,
        sender: &PublicIdentity,
        cipher: &[u8],
    ) -> anyhow::Result<(Content, BTreeMap<Uuid, PublicIdentity>)> {
        let body = identity.open(&self.transcript()?, cipher, sender)?;
        let envelope: Envelope =
            serde_json::from_slice(&body).context("Invalid encrypted chat content")?;
        ensure!(
            envelope.recipients.get(&recipient) == Some(&identity.public()),
            "Recipient not bound to this message"
        );
        ensure!(
            envelope.recipients.get(&self.sender) == Some(sender),
            "Sender not bound to this message"
        );
        envelope.content.validate()?;
        Ok((envelope.content, envelope.recipients))
    }
}

impl Content {
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Some(context) = &self.context {
            context.validate().map_err(anyhow::Error::msg)?;
        }
        match self.content_type.as_str() {
            "application/vnd.wisp.reaction+json" => {
                ensure!(
                    self.attachment.is_none(),
                    "Reaction cannot contain an attachment"
                );
                let target = self.payload["target"]
                    .as_str()
                    .context("Missing reaction target")?;
                target.parse::<Uuid>().context("Invalid reaction target")?;
                let emoji = self.payload["emoji"]
                    .as_str()
                    .context("Missing reaction emoji")?;
                ensure!(
                    !emoji.is_empty() && emoji.len() <= 200 && !emoji.chars().any(char::is_control),
                    "Invalid reaction emoji"
                );
            }
            "text/plain" => {
                ensure!(
                    self.attachment.is_none(),
                    "Text cannot contain an attachment"
                );
                ensure!(
                    !self
                        .payload
                        .as_str()
                        .context("Invalid encrypted text")?
                        .trim()
                        .is_empty(),
                    "Message must not be empty"
                );
            }
            "image/png" | "application/octet-stream" => {
                let attachment = self
                    .attachment
                    .as_ref()
                    .context("Missing signed attachment manifest")?;
                ensure!(self.payload.is_object(), "Invalid attachment metadata");
                let name = self.payload["file_name"]
                    .as_str()
                    .context("Missing encrypted filename")?;
                ensure!(
                    !name.is_empty()
                        && name != "."
                        && name != ".."
                        && !name.contains(['/', '\\', '\0'])
                        && !name.chars().any(char::is_control),
                    "Invalid encrypted filename"
                );
                ensure!(
                    self.payload["caption"].is_string(),
                    "Missing encrypted caption"
                );
                ensure!(
                    self.payload["size"].as_u64() == Some(attachment.plaintext_bytes),
                    "Attachment size does not match signed manifest"
                );
            }
            _ => anyhow::bail!("Unsupported encrypted content type"),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reply_metadata_is_encrypted_authenticated_and_ordinary_messages_stay_compatible() {
        let alice = Identity::generate().unwrap();
        let bob = Identity::generate().unwrap();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let binding = MessageContext {
            network: Uuid::new_v4(),
            conversation: "chat".into(),
            sender: a,
            message: Uuid::new_v4(),
            roster: "roster".into(),
        };
        let recipients = BTreeMap::from([(a, alice.public()), (b, bob.public())]);
        let plain = Content {
            content_type: "text/plain".into(),
            payload: serde_json::json!("Reply body"),
            attachment: None,
            context: None,
        };
        let encoded = serde_json::to_value(&plain).unwrap();
        assert!(encoded.get("context").is_none());
        let mut reply = plain.clone();
        reply.context = Some(wisp_protocol::MessageContext {
            reply_to: Some(wisp_protocol::ReplyReference {
                message_id: Uuid::new_v4(),
                sender_name: "Quoted person".into(),
                preview: "Private quoted preview".into(),
            }),
            forwarded_from: None,
        });
        let mut ciphertext = binding.seal(&alice, &recipients, reply.clone()).unwrap();
        assert!(!ciphertext.windows(6).any(|w| w == b"Quoted"));
        assert_eq!(
            binding
                .open(&bob, b, &alice.public(), &ciphertext)
                .unwrap()
                .context,
            reply.context
        );
        let index = ciphertext.len() / 2;
        ciphertext[index] ^= 1;
        assert!(binding.open(&bob, b, &alice.public(), &ciphertext).is_err());
        reply.context.as_mut().unwrap().forwarded_from = Some(wisp_protocol::ForwardedFrom {
            sender_name: "Person".into(),
        });
        assert!(
            reply.validate().is_err(),
            "Forward must drop original reply context"
        );
        reply.context.as_mut().unwrap().forwarded_from = None;
        reply
            .context
            .as_mut()
            .unwrap()
            .reply_to
            .as_mut()
            .unwrap()
            .preview = "x".repeat(1201);
        assert!(reply.validate().is_err(), "Oversized preview rejected");
    }

    #[test]
    fn binds_network_conversation_sender_message_and_recipient_without_ambiguity() {
        let alice = Identity::generate().unwrap();
        let bob = Identity::generate().unwrap();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let context = MessageContext {
            network: Uuid::new_v4(),
            conversation: "dm:/a/b".into(),
            sender: a,
            message: Uuid::new_v4(),
            roster: "initial-roster-hash".into(),
        };
        let recipients = BTreeMap::from([(a, alice.public()), (b, bob.public())]);
        let cipher = context
            .seal(
                &alice,
                &recipients,
                Content {
                    context: None,
                    content_type: "text/plain".into(),
                    payload: serde_json::json!("Private"),
                    attachment: None,
                },
            )
            .unwrap();
        assert_eq!(
            context
                .open(&bob, b, &alice.public(), &cipher)
                .unwrap()
                .payload,
            "Private"
        );
        for field in ["network", "conversation", "sender", "message", "roster"] {
            let mut changed = serde_json::to_value(&context).unwrap();
            changed[field] = serde_json::json!(Uuid::new_v4().to_string());
            let changed: MessageContext = serde_json::from_value(changed).unwrap();
            assert!(changed.open(&bob, b, &alice.public(), &cipher).is_err());
        }
        assert!(
            context
                .open(&bob, Uuid::new_v4(), &alice.public(), &cipher)
                .is_err()
        );
        assert!(
            context
                .seal(
                    &alice,
                    &BTreeMap::from([(b, bob.public())]),
                    Content {
                        context: None,
                        content_type: "text/plain".into(),
                        payload: serde_json::json!("Private"),
                        attachment: None
                    }
                )
                .is_err()
        );
    }
}
