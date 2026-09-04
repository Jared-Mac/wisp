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
        match self.content_type.as_str() {
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
                        content_type: "text/plain".into(),
                        payload: serde_json::json!("Private"),
                        attachment: None
                    }
                )
                .is_err()
        );
    }
}
