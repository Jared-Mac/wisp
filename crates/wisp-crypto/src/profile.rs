//! Account-authorized display names, bound to a network and monotonic revision.
use crate::{Identity, PublicIdentity};
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const DOMAIN: &str = "wisp-account-profile-v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub network: Uuid,
    pub account: Uuid,
    pub revision: u64,
    pub display_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedProfile {
    pub profile: Profile,
    pub signature: String,
}

impl Profile {
    fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.revision > 0 && i64::try_from(self.revision).is_ok(),
            "Invalid profile revision"
        );
        let name = &self.display_name;
        ensure!(
            name.trim() == name
                && !name.is_empty()
                && name.chars().count() <= 80
                && !name.chars().any(char::is_control),
            "Display name must contain 1–80 printable characters"
        );
        Ok(())
    }

    pub fn sign(self, identity: &Identity) -> anyhow::Result<SignedProfile> {
        self.validate()?;
        let signature = identity.sign_statement(DOMAIN, &serde_json::to_vec(&self)?);
        Ok(SignedProfile {
            profile: self,
            signature,
        })
    }
}

impl SignedProfile {
    pub fn verify(&self, identity: &PublicIdentity) -> anyhow::Result<()> {
        self.profile.validate()?;
        identity.verify_statement(DOMAIN, &serde_json::to_vec(&self.profile)?, &self.signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_signature_binds_every_field_and_identity() {
        let identity = Identity::generate().unwrap();
        let signed = Profile {
            network: Uuid::new_v4(),
            account: Uuid::new_v4(),
            revision: 1,
            display_name: "New name".into(),
        }
        .sign(&identity)
        .unwrap();
        signed.verify(&identity.public()).unwrap();
        assert!(
            signed
                .verify(&Identity::generate().unwrap().public())
                .is_err()
        );
        for field in 0..4 {
            let mut altered = signed.clone();
            match field {
                0 => altered.profile.network = Uuid::new_v4(),
                1 => altered.profile.account = Uuid::new_v4(),
                2 => altered.profile.revision += 1,
                _ => altered.profile.display_name = "Impostor".into(),
            }
            assert!(altered.verify(&identity.public()).is_err());
        }
    }
}
