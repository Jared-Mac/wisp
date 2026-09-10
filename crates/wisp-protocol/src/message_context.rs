use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A sender-provided quotation, not a signature by the quoted author.
/// Encrypted chats carry this only inside the authenticated ciphertext.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<ReplyReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forwarded_from: Option<ForwardedFrom>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplyReference {
    pub message_id: Uuid,
    pub sender_name: String,
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForwardedFrom {
    pub sender_name: String,
}

impl MessageContext {
    pub fn validate(&self) -> Result<(), &'static str> {
        fn valid_name(name: &str) -> bool {
            !name.trim().is_empty() && name.len() <= 400 && !name.chars().any(char::is_control)
        }
        if let Some(reply) = &self.reply_to
            && (!valid_name(&reply.sender_name) || reply.preview.len() > 1200)
        {
            return Err("Invalid reply preview");
        }
        if let Some(forward) = &self.forwarded_from
            && !valid_name(&forward.sender_name)
        {
            return Err("Invalid forwarding attribution");
        }
        if self.reply_to.is_some() && self.forwarded_from.is_some() {
            return Err("A forwarded message cannot also carry its original reply context");
        }
        Ok(())
    }
}
