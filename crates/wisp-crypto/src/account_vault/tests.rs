#![allow(clippy::manual_assert_eq)] // Failed assertions must not print private backup material.
use super::{
    Scope,
    auth::ExportKey,
    bundle::{Bundle, NameCheckpoint, TrustState},
    envelope::{Envelope, KeyEnvelope, VaultKey},
};
use crate::{
    Identity, SecretString,
    profile::Profile,
    roster::{Member, Role, Roster, SignedRoster},
};
use age::secrecy::ExposeSecret;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;
use zeroize::Zeroizing;

#[test]
fn wrapper_matches_independent_node_hkdf_aes_gcm_vector() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    #[derive(serde::Deserialize)]
    struct Vector {
        export_key_base64: String,
        vault_key_base64: String,
        wrapper: KeyEnvelope,
    }
    let vector: Vector = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/account-vault/wrapper-v1.json"
    ))
    .unwrap();
    let export = ExportKey(Zeroizing::new(
        STANDARD
            .decode(vector.export_key_base64)
            .unwrap()
            .try_into()
            .unwrap(),
    ));
    let key = vector
        .wrapper
        .unwrap(
            &vector.wrapper.scope,
            vector.wrapper.credential_generation,
            &export,
        )
        .unwrap();
    let expected = Zeroizing::new(STANDARD.decode(vector.vault_key_base64).unwrap());
    assert!(key.save_private().as_slice() == expected.as_slice());
}

fn fixture() -> (Identity, Bundle) {
    let identity = Identity::generate().unwrap();
    let scope = Scope {
        origin: "https://example.invalid".into(),
        network: Uuid::new_v4(),
        account: Uuid::new_v4(),
    };
    let mut trust = TrustState::default();
    trust.pins.insert(scope.account, identity.public());
    let bundle = Bundle::new(
        scope,
        identity.recovery_key().unwrap(),
        Some(SecretString::from("synthetic media key fixture".to_owned())),
        trust,
    )
    .unwrap();
    (identity, bundle)
}

fn copy(bundle: &Bundle) -> Bundle {
    Bundle::decode_private(
        &bundle.encode_private().unwrap(),
        &bundle.scope,
        &bundle.identity().unwrap().public(),
    )
    .unwrap()
}

#[test]
fn wrapping_and_restore_survive_new_credentials_without_changing_identity_or_payload() {
    let (identity, bundle) = fixture();
    let key = VaultKey::generate().unwrap();
    let old_export = ExportKey(Zeroizing::new([31; 64]));
    let new_export = ExportKey(Zeroizing::new([72; 64]));
    let old_generation = Uuid::new_v4();
    let new_generation = Uuid::new_v4();
    let old_wrap =
        KeyEnvelope::wrap(bundle.scope.clone(), old_generation, &old_export, &key).unwrap();
    let envelope = Envelope::seal(&bundle, &key, None).unwrap();
    let accepted = envelope
        .manifest
        .advance(&bundle.scope, &identity.public(), None)
        .unwrap();
    let restored_key = old_wrap
        .unwrap(&bundle.scope, old_generation, &old_export)
        .unwrap();
    let restored = envelope
        .open(&restored_key, &bundle.scope, &identity.public(), &accepted)
        .unwrap();
    assert!(
        bundle.encode_private().unwrap().as_slice()
            == restored.encode_private().unwrap().as_slice()
    );

    // An email reset cannot unlock the old wrapping generation. A trusted
    // device can explicitly rewrap its stable key after secure reauthentication.
    assert!(
        old_wrap
            .unwrap(&bundle.scope, new_generation, &new_export)
            .is_err()
    );
    assert!(
        old_wrap
            .unwrap(&bundle.scope, old_generation, &new_export)
            .is_err()
    );
    let repaired = KeyEnvelope::wrap(
        bundle.scope.clone(),
        new_generation,
        &new_export,
        &restored_key,
    )
    .unwrap();
    let repaired_key = repaired
        .unwrap(&bundle.scope, new_generation, &new_export)
        .unwrap();
    let repaired_bundle = envelope
        .open(&repaired_key, &bundle.scope, &identity.public(), &accepted)
        .unwrap();
    assert_eq!(
        identity.public(),
        repaired_bundle.identity().unwrap().public()
    );
    assert!(restored_key.save_private().as_slice() == repaired_key.save_private().as_slice());
}

#[test]
fn wrapper_and_payload_tampering_or_cross_scope_substitution_fail() {
    let (identity, bundle) = fixture();
    let key = VaultKey::generate().unwrap();
    let export = ExportKey(Zeroizing::new([31; 64]));
    let generation = Uuid::new_v4();
    let wrapper = KeyEnvelope::wrap(bundle.scope.clone(), generation, &export, &key).unwrap();
    let envelope = Envelope::seal(&bundle, &key, None).unwrap();
    let accepted = envelope.manifest.checkpoint().unwrap();
    for field in 0..6 {
        let mut altered = wrapper.clone();
        match field {
            0 => altered.scope.account = Uuid::new_v4(),
            1 => altered.scope.network = Uuid::new_v4(),
            2 => altered.scope.origin = "https://other.invalid".into(),
            3 => altered.credential_generation = Uuid::new_v4(),
            4 => altered.ciphertext.replace_range(
                ..1,
                if altered.ciphertext.starts_with('A') {
                    "B"
                } else {
                    "A"
                },
            ),
            _ => altered.nonce.replace_range(
                ..1,
                if altered.nonce.starts_with('A') {
                    "B"
                } else {
                    "A"
                },
            ),
        }
        assert!(altered.unwrap(&bundle.scope, generation, &export).is_err());
    }
    for field in 0..7 {
        let mut altered = envelope.clone();
        match field {
            0 => altered.manifest.header.scope.account = Uuid::new_v4(),
            1 => altered.manifest.header.scope.network = Uuid::new_v4(),
            2 => altered.manifest.header.scope.origin = "https://other.invalid".into(),
            3 => altered.manifest.header.signer = Identity::generate().unwrap().public(),
            4 => altered.manifest.header.revision += 1,
            5 => altered.ciphertext.replace_range(
                ..1,
                if altered.ciphertext.starts_with('A') {
                    "B"
                } else {
                    "A"
                },
            ),
            _ => altered.manifest.signature = "invalid".into(),
        }
        assert!(
            altered
                .open(&key, &bundle.scope, &identity.public(), &accepted)
                .is_err()
        );
    }
    assert!(
        envelope
            .open(
                &VaultKey::generate().unwrap(),
                &bundle.scope,
                &identity.public(),
                &accepted
            )
            .is_err()
    );
}

#[test]
fn random_nonces_and_manifest_checkpoints_reject_rollback_forks_and_skipped_history() {
    let (identity, bundle) = fixture();
    let key = VaultKey::generate().unwrap();
    let first = Envelope::seal(&bundle, &key, None).unwrap();
    let checkpoint = first
        .manifest
        .advance(&bundle.scope, &identity.public(), None)
        .unwrap();
    let second = Envelope::seal(&bundle, &key, Some(checkpoint.clone())).unwrap();
    let second_checkpoint = second
        .manifest
        .advance(&bundle.scope, &identity.public(), Some(&checkpoint))
        .unwrap();
    let third = Envelope::seal(&bundle, &key, Some(second_checkpoint.clone())).unwrap();
    let fork = Envelope::seal(&bundle, &key, Some(checkpoint.clone())).unwrap();
    assert_ne!(first.manifest.header.nonce, second.manifest.header.nonce);
    assert_ne!(second.manifest.header.nonce, fork.manifest.header.nonce);
    assert!(
        first
            .manifest
            .advance(&bundle.scope, &identity.public(), Some(&second_checkpoint))
            .is_err()
    );
    assert!(
        third
            .manifest
            .advance(&bundle.scope, &identity.public(), Some(&checkpoint))
            .is_err()
    );
    assert!(
        fork.manifest
            .advance(&bundle.scope, &identity.public(), Some(&second_checkpoint))
            .is_err()
    );
    assert_eq!(
        second_checkpoint,
        second
            .manifest
            .advance(&bundle.scope, &identity.public(), Some(&second_checkpoint))
            .unwrap()
    );
}

#[test]
fn desktop_profile_floor_and_android_signed_profile_survive_both_merge_directions() {
    let (_, mut desktop) = fixture();
    let friend = Identity::generate().unwrap();
    let friend_id = Uuid::new_v4();
    desktop.trust.pins.insert(friend_id, friend.public());
    desktop.trust.names.insert(
        friend_id,
        NameCheckpoint {
            display_name: "Later name".into(),
            revision: 3,
        },
    );
    let mut android = copy(&desktop);
    android.trust.names.insert(
        friend_id,
        NameCheckpoint {
            display_name: "Earlier name".into(),
            revision: 2,
        },
    );
    let signed = Profile {
        network: desktop.scope.network,
        account: friend_id,
        revision: 2,
        display_name: "Earlier name".into(),
    }
    .sign(&friend)
    .unwrap();
    android.trust.profiles.insert(friend_id, signed.clone());
    desktop.trust.legacy_approvals.insert(
        "chat".into(),
        BTreeSet::from([desktop.scope.account, friend_id]),
    );
    for merged in [
        desktop.merge(&android, &BTreeMap::new()).unwrap(),
        android.merge(&desktop, &BTreeMap::new()).unwrap(),
    ] {
        assert_eq!(merged.trust.names[&friend_id].revision, 3);
        assert_eq!(merged.trust.names[&friend_id].display_name, "Later name");
        assert_eq!(merged.trust.profiles[&friend_id], signed);
        assert_eq!(
            merged.trust.legacy_approvals,
            desktop.trust.legacy_approvals
        );
        let round_trip = copy(&merged);
        assert!(round_trip.trust == merged.trust);
    }
    let mut conflict = copy(&android);
    conflict
        .trust
        .names
        .get_mut(&friend_id)
        .unwrap()
        .display_name = "Conflicting name".into();
    conflict.trust.profiles.clear();
    assert!(android.merge(&conflict, &BTreeMap::new()).is_err());
}

#[test]
fn missing_media_is_not_deletion_and_conflicts_never_mutate_saved_state() {
    let (identity, bundle) = fixture();
    let before = bundle.encode_private().unwrap();
    let missing = Bundle::new(
        bundle.scope.clone(),
        identity.recovery_key().unwrap(),
        None,
        bundle.trust.clone(),
    )
    .unwrap();
    let merged = missing.merge(&bundle, &BTreeMap::new()).unwrap();
    assert!(
        merged.media_key().unwrap().expose_secret() == bundle.media_key().unwrap().expose_secret()
    );
    let conflicting = Bundle::new(
        bundle.scope.clone(),
        identity.recovery_key().unwrap(),
        Some(SecretString::from(
            "different synthetic media key".to_owned(),
        )),
        bundle.trust.clone(),
    )
    .unwrap();
    assert!(bundle.merge(&conflicting, &BTreeMap::new()).is_err());
    let mut altered = copy(&bundle);
    let friend = Uuid::new_v4();
    altered
        .trust
        .pins
        .insert(friend, Identity::generate().unwrap().public());
    let mut second = copy(&bundle);
    second
        .trust
        .pins
        .insert(friend, Identity::generate().unwrap().public());
    assert!(altered.merge(&second, &BTreeMap::new()).is_err());
    assert!(before.as_slice() == bundle.encode_private().unwrap().as_slice());
}

#[test]
fn individually_valid_backups_cannot_merge_beyond_the_portable_aggregate_budget() {
    let (_, mut left) = fixture();
    let own = left.identity().unwrap().public();
    let mut members = BTreeSet::from([left.scope.account]);
    for _ in 0..999 {
        let id = Uuid::new_v4();
        left.trust.pins.insert(id, own.clone());
        members.insert(id);
    }
    let mut right = copy(&left);
    for i in 0..75 {
        left.trust
            .legacy_approvals
            .insert(format!("left-{i}"), members.clone());
        right
            .trust
            .legacy_approvals
            .insert(format!("right-{i}"), members.clone());
    }
    let left_before = left.encode_private().unwrap();
    let right_before = right.encode_private().unwrap();
    assert!(left.merge(&right, &BTreeMap::new()).is_err());
    assert!(left_before.as_slice() == left.encode_private().unwrap().as_slice());
    assert!(right_before.as_slice() == right.encode_private().unwrap().as_slice());
}

fn room(bundle: &Bundle, identity: &Identity) -> SignedRoster {
    Roster {
        network: bundle.scope.network,
        conversation: "room".into(),
        revision: 0,
        previous: None,
        actor: bundle.scope.account,
        members: BTreeMap::from([(
            bundle.scope.account,
            Member {
                identity: identity.public(),
                role: Role::Host,
            },
        )]),
    }
    .sign(identity)
    .unwrap()
}

#[test]
fn room_checkpoint_requires_connecting_authorized_proof_and_preserves_departed_rooms() {
    let (identity, mut old) = fixture();
    let genesis = room(&old, &identity);
    old.trust.room_heads.insert("room".into(), genesis.clone());
    let mut newer = copy(&old);
    let friend = Identity::generate().unwrap();
    let friend_id = Uuid::new_v4();
    newer.trust.pins.insert(friend_id, friend.public());
    let mut next = genesis.roster.clone();
    next.revision = 1;
    next.previous = Some(genesis.hash().unwrap());
    next.members.insert(
        friend_id,
        Member {
            identity: friend.public(),
            role: Role::Member,
        },
    );
    let next = next.sign(&identity).unwrap();
    newer.trust.room_heads.insert("room".into(), next.clone());
    assert!(old.merge(&newer, &BTreeMap::new()).is_err());
    let proofs = BTreeMap::from([("room".into(), vec![genesis.clone(), next.clone()])]);
    let merged = old.merge(&newer, &proofs).unwrap();
    assert_eq!(merged.trust.room_heads["room"], next);
    let mut absent = copy(&newer);
    absent.trust.room_heads.clear();
    assert_eq!(
        merged
            .merge(&absent, &BTreeMap::new())
            .unwrap()
            .trust
            .room_heads["room"],
        next
    );

    // A pinned member can sign a syntactically valid checkpoint, but cannot
    // authorize its advancement. The connecting proof must establish authority.
    let mut unauthorized = next.roster.clone();
    unauthorized.revision = 2;
    unauthorized.previous = Some(next.hash().unwrap());
    unauthorized.actor = friend_id;
    let unauthorized = unauthorized.sign(&friend).unwrap();
    let mut fork = copy(&newer);
    fork.trust
        .room_heads
        .insert("room".into(), unauthorized.clone());
    fork.validate().unwrap();
    assert!(
        newer
            .merge(
                &fork,
                &BTreeMap::from([("room".into(), vec![next, unauthorized])])
            )
            .is_err()
    );
}

#[test]
fn private_decoder_rejects_duplicate_fields_unknown_versions_and_different_identity() {
    let (identity, bundle) = fixture();
    let bytes = bundle.encode_private().unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    let duplicate = Zeroizing::new(text.replacen("\"format\":1", "\"format\":1,\"format\":1", 1));
    let unsupported = Zeroizing::new(text.replacen("\"format\":1", "\"format\":99", 1));
    for invalid in [duplicate.as_bytes(), unsupported.as_bytes()] {
        assert!(Bundle::decode_private(invalid, &bundle.scope, &identity.public()).is_err());
    }
    assert!(
        Bundle::decode_private(
            &bytes,
            &bundle.scope,
            &Identity::generate().unwrap().public()
        )
        .is_err()
    );
}
