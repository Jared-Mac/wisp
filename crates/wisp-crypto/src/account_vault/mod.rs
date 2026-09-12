//! Versioned, client-held account vault encryption. Wire types and application
//! integration remain under review; private keys must never cross the API.
pub mod auth;
pub mod bundle;
pub mod device;
pub mod envelope;
pub mod operation;
mod strict_json;
#[cfg(test)]
mod tests;

use anyhow::ensure;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub origin: String,
    pub network: Uuid,
    pub account: Uuid,
}

impl Scope {
    pub fn validate(&self) -> anyhow::Result<()> {
        auth::AccountContext {
            origin: self.origin.clone(),
            network: self.network,
            username: "scope".into(),
        }
        .validate()?;
        ensure!(!self.account.is_nil(), "Invalid backup account");
        Ok(())
    }
}

pub(super) fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
