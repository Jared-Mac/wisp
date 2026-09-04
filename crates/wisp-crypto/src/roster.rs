//! Authenticated room membership. A server may relay but cannot authorize
//! transitions after the first roster/identities have been pinned locally.
use crate::{Identity, PublicIdentity};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use uuid::Uuid;

const DOMAIN: &str = "wisp-room-roster-v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Host,
    Admin,
    Member,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub identity: PublicIdentity,
    pub role: Role,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roster {
    pub network: Uuid,
    pub conversation: String,
    pub revision: u64,
    pub previous: Option<String>,
    pub actor: Uuid,
    pub members: BTreeMap<Uuid, Member>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedRoster {
    pub roster: Roster,
    pub signature: String,
}

impl Roster {
    pub fn sign(self, identity: &Identity) -> anyhow::Result<SignedRoster> {
        // The actor may remove themselves in a future leave operation; their
        // authorization always comes from the predecessor, not this roster.
        let signature = identity.sign_statement(DOMAIN, &serde_json::to_vec(&self)?);
        Ok(SignedRoster {
            roster: self,
            signature,
        })
    }
}

impl SignedRoster {
    pub fn hash(&self) -> anyhow::Result<String> {
        Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(self)?)))
    }

    /// A genesis can only establish TOFU, not independently verified identity.
    /// Caller must match expected network/conversation/current account and pin
    /// its hash before accepting later updates. Never re-bootstrap on errors.
    pub fn verify_genesis(&self) -> anyhow::Result<()> {
        ensure!(
            self.roster.revision == 0 && self.roster.previous.is_none(),
            "Expected initial room identity"
        );
        self.validate_members()?;
        let actor = self
            .roster
            .members
            .get(&self.roster.actor)
            .context("Initial signer must belong to the room")?;
        ensure!(
            actor.role == Role::Host
                || (actor.role == Role::Admin
                    && !self.roster.members.values().any(|m| m.role == Role::Host))
                || self
                    .roster
                    .members
                    .values()
                    .all(|member| member.role == Role::Member),
            "Initial roster must be signed by its owner"
        );
        actor
            .identity
            .verify_statement(DOMAIN, &serde_json::to_vec(&self.roster)?, &self.signature)
    }

    pub fn verify_successor(&self, previous: &Self) -> anyhow::Result<()> {
        let old = &previous.roster;
        let next = &self.roster;
        ensure!(
            next.network == old.network && next.conversation == old.conversation,
            "Room identity changed"
        );
        ensure!(
            old.revision.checked_add(1) == Some(next.revision)
                && next.previous.as_deref() == Some(previous.hash()?.as_str()),
            "Room history is missing, replayed, or forked"
        );
        self.validate_members()?;
        let actor = old
            .members
            .get(&next.actor)
            .context("Room change signer was not a member")?;
        ensure!(
            matches!(actor.role, Role::Host | Role::Admin),
            "Only an existing room owner or admin can change membership"
        );
        actor
            .identity
            .verify_statement(DOMAIN, &serde_json::to_vec(next)?, &self.signature)?;
        for (id, member) in &next.members {
            if let Some(previous) = old.members.get(id) {
                ensure!(
                    member.identity == previous.identity,
                    "A membership update cannot replace a friend's identity"
                );
            }
        }
        // Owner changes need an explicit ownership-transfer design, not a
        // membership update. Only the existing owner may grant/revoke admins.
        for (id, member) in &old.members {
            if member.role == Role::Host {
                ensure!(
                    next.members.get(id) == Some(member),
                    "Room owner cannot be replaced or removed"
                );
            }
            if actor.role == Role::Admin && member.role != Role::Member {
                ensure!(
                    next.members.get(id) == Some(member),
                    "Admins cannot remove/demote owners or other admins"
                );
            }
        }
        for (id, member) in &next.members {
            if member.role == Role::Host {
                ensure!(
                    old.members.get(id) == Some(member),
                    "A membership update cannot create an owner"
                );
            }
            if actor.role == Role::Admin && old.members.get(id) != Some(member) {
                ensure!(
                    member.role == Role::Member,
                    "Only the owner may grant admin access"
                );
            }
        }
        Ok(())
    }

    fn validate_members(&self) -> anyhow::Result<()> {
        ensure!(
            !self.roster.members.is_empty(),
            "Room must have participants"
        );
        ensure!(
            self.roster
                .members
                .values()
                .filter(|m| m.role == Role::Host)
                .count()
                <= 1,
            "Room cannot have multiple owners"
        );
        for member in self.roster.members.values() {
            member.identity.validate()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authorized_invites_are_automatic_but_provider_injection_and_replay_fail() {
        let host = Identity::generate().unwrap();
        let friend = Identity::generate().unwrap();
        let attacker = Identity::generate().unwrap();
        let h = Uuid::new_v4();
        let f = Uuid::new_v4();
        let genesis = Roster {
            network: Uuid::new_v4(),
            conversation: "room".into(),
            revision: 0,
            previous: None,
            actor: h,
            members: BTreeMap::from([(
                h,
                Member {
                    identity: host.public(),
                    role: Role::Host,
                },
            )]),
        }
        .sign(&host)
        .unwrap();
        genesis.verify_genesis().unwrap();
        let mut invited = genesis.roster.clone();
        invited.revision = 1;
        invited.previous = Some(genesis.hash().unwrap());
        invited.members.insert(
            f,
            Member {
                identity: friend.public(),
                role: Role::Member,
            },
        );
        let legitimate = invited.clone().sign(&host).unwrap();
        legitimate.verify_successor(&genesis).unwrap();
        assert!(
            invited
                .clone()
                .sign(&attacker)
                .unwrap()
                .verify_successor(&genesis)
                .is_err()
        );
        assert!(genesis.verify_successor(&legitimate).is_err());
        let mut promotion = legitimate.roster.clone();
        promotion.revision = 2;
        promotion.previous = Some(legitimate.hash().unwrap());
        promotion.members.get_mut(&f).unwrap().role = Role::Admin;
        let promoted = promotion.clone().sign(&host).unwrap();
        promoted.verify_successor(&legitimate).unwrap();
        promotion.actor = f;
        assert!(
            promotion
                .sign(&friend)
                .unwrap()
                .verify_successor(&legitimate)
                .is_err()
        );
        let mut changed = promoted.roster.clone();
        changed.revision = 3;
        changed.previous = Some(promoted.hash().unwrap());
        changed.actor = f;
        changed.members.get_mut(&h).unwrap().identity = attacker.public();
        assert!(
            changed
                .sign(&friend)
                .unwrap()
                .verify_successor(&promoted)
                .is_err()
        );
    }
}
