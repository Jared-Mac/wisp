use super::*;
use uuid::Uuid;

fn fixture() -> (Catalog, VaultKey, Identity) {
    let scope = Scope {
        origin: "https://primary.invalid".into(),
        network: Uuid::new_v4(),
        account: Uuid::new_v4(),
    };
    let mut catalog = Catalog::empty(scope.clone());
    catalog
        .merge(&[Entry {
            scope,
            label: "Primary".into(),
        }])
        .unwrap();
    (
        catalog,
        VaultKey::generate().unwrap(),
        Identity::generate().unwrap(),
    )
}
#[test]
fn round_trip_rejects_other_scope_identity_key_and_tampering() {
    let (catalog, key, identity) = fixture();
    let envelope = Envelope::seal(&catalog, &key, &identity, None).unwrap();
    let checkpoint = envelope
        .manifest
        .advance(&catalog.scope, &identity.public(), None)
        .unwrap();
    let decoded = envelope
        .open(&key, &catalog.scope, &identity.public(), &checkpoint)
        .unwrap();
    assert!(decoded == catalog);
    for altered in [
        Scope {
            origin: "https://other.invalid".into(),
            ..catalog.scope.clone()
        },
        Scope {
            network: Uuid::new_v4(),
            ..catalog.scope.clone()
        },
        Scope {
            account: Uuid::new_v4(),
            ..catalog.scope.clone()
        },
    ] {
        assert!(
            envelope
                .open(&key, &altered, &identity.public(), &checkpoint)
                .is_err()
        );
    }
    assert!(
        envelope
            .open(
                &VaultKey::generate().unwrap(),
                &catalog.scope,
                &identity.public(),
                &checkpoint
            )
            .is_err()
    );
    assert!(
        envelope
            .verify(&catalog.scope, &Identity::generate().unwrap().public())
            .is_err()
    );
    let mut altered = envelope.clone();
    altered.ciphertext.replace_range(
        0..1,
        if altered.ciphertext.starts_with('A') {
            "B"
        } else {
            "A"
        },
    );
    assert!(altered.verify(&catalog.scope, &identity.public()).is_err());
    let mut altered = envelope.clone();
    altered.manifest.header.nonce = encoded(&[0; 12]);
    assert!(altered.verify(&catalog.scope, &identity.public()).is_err());
}
#[test]
fn history_rejects_forks_gaps_rollback_and_vault_signature_reuse() {
    let (catalog, key, identity) = fixture();
    let first = Envelope::seal(&catalog, &key, &identity, None).unwrap();
    let parent = first.manifest.checkpoint().unwrap();
    let second = Envelope::seal(&catalog, &key, &identity, Some(parent.clone())).unwrap();
    let next = second
        .manifest
        .advance(&catalog.scope, &identity.public(), Some(&parent))
        .unwrap();
    assert!(
        second
            .manifest
            .advance(&catalog.scope, &identity.public(), None)
            .is_err()
    );
    assert!(
        first
            .manifest
            .advance(&catalog.scope, &identity.public(), Some(&next))
            .is_err()
    );
    let fork = Envelope::seal(&catalog, &key, &identity, None).unwrap();
    assert!(
        second
            .manifest
            .advance(
                &catalog.scope,
                &identity.public(),
                Some(&fork.manifest.checkpoint().unwrap())
            )
            .is_err()
    );
    let vault_manifest: super::super::envelope::SignedManifest =
        serde_json::from_value(serde_json::to_value(&first.manifest).unwrap()).unwrap();
    assert!(
        vault_manifest
            .verify(&catalog.scope, &identity.public())
            .is_err()
    );
}
#[test]
fn merges_preserve_remote_entries_labels_and_fail_atomically_for_account_conflicts() {
    let (mut catalog, _, _) = fixture();
    let entry = Entry {
        scope: Scope {
            origin: "https://another.invalid".into(),
            network: Uuid::new_v4(),
            account: Uuid::new_v4(),
        },
        label: "Another".into(),
    };
    assert!(catalog.merge(std::slice::from_ref(&entry)).unwrap());
    assert!(
        !catalog
            .merge(&[Entry {
                label: "Local nickname".into(),
                ..entry.clone()
            }])
            .unwrap()
    );
    assert_eq!(catalog.entries[0].label, "Another");
    let previous = catalog.clone();
    assert!(
        catalog
            .merge(&[Entry {
                scope: Scope {
                    account: Uuid::new_v4(),
                    ..entry.scope
                },
                label: "Conflict".into()
            }])
            .is_err()
    );
    assert!(catalog == previous);
}
#[test]
fn bounds_and_unknown_fields_prevent_credentials_or_ambiguous_origins() {
    let (catalog, _, _) = fixture();
    let mut value = serde_json::to_value(&catalog).unwrap();
    value["entries"][0]["device_token"] = "forbidden".into();
    assert!(Catalog::decode_private(&serde_json::to_vec(&value).unwrap(), &catalog.scope).is_err());
    for origin in [
        "http://example.invalid",
        "https://example.invalid/",
        "https://user@example.invalid",
        "https://example.invalid/path",
        "https://example.invalid?token=hidden",
    ] {
        let mut entry = catalog.entries[0].clone();
        entry.scope.origin = origin.into();
        assert!(entry.validate().is_err());
    }
    let mut too_many = catalog.clone();
    too_many.entries = vec![catalog.entries[0].clone(); MAX_ENTRIES + 1];
    assert!(too_many.validate().is_err());
    let mut entry = catalog.entries[0].clone();
    entry.label = "x".repeat(161);
    assert!(entry.validate().is_err());
    entry.label = "bad\nlabel".into();
    assert!(entry.validate().is_err());
    assert!(Catalog::decode_private(&vec![b' '; MAX_PLAINTEXT + 1], &catalog.scope).is_err());
    let repeated = format!(
        "{{\"format\":1,\"format\":1,\"scope\":{},\"entries\":[]}}",
        serde_json::to_string(&catalog.scope).unwrap()
    );
    assert!(Catalog::decode_private(repeated.as_bytes(), &catalog.scope).is_err());
}
