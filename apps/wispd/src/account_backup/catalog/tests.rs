#![allow(clippy::manual_assert_eq)] // Avoid dumping private journals on assertion failure.
use super::*;
use crate::account_backup::tests::Fixture;
use uuid::Uuid;
use wisp_crypto::{
    SecretString,
    account_vault::{Scope, envelope::Checkpoint},
};
fn password() -> SecretString {
    SecretString::from("synthetic catalog account password".to_owned())
}
async fn signed_in() -> (Fixture, Api, Api) {
    let fixture = Fixture::new().await;
    let mut first = fixture.client("desktop").await;
    first
        .signup("cataloguser", "Catalog user", "Desktop", password())
        .await
        .unwrap();
    let mut phone = fixture.client("phone").await;
    assert!(
        phone
            .login("cataloguser", password(), "Phone")
            .await
            .unwrap()
    );
    (fixture, first, phone)
}
fn entry(origin: &str) -> Entry {
    Entry {
        scope: Scope {
            origin: origin.into(),
            network: Uuid::new_v4(),
            account: Uuid::new_v4(),
        },
        label: "Synthetic saved server".into(),
    }
}
fn envelope(api: &Api, entries: &[Entry], parent: Option<Checkpoint>) -> Envelope {
    let record = api.store.record().unwrap();
    let bundle = record.bundle().unwrap().unwrap();
    let mut catalog = Catalog::empty(bundle.scope.clone());
    catalog.merge(entries).unwrap();
    Envelope::seal(
        &catalog,
        &record.local_key().unwrap(),
        &bundle.identity().unwrap(),
        parent,
    )
    .unwrap()
}
async fn publish(api: &Api, envelope: &Envelope) -> anyhow::Result<Status> {
    api.post(
        PATH,
        &StoreRequest {
            catalog: envelope.clone(),
        },
        true,
        16384,
    )
    .await
}
#[tokio::test]
async fn desktop_publishes_phone_restores_without_target_credentials_or_vault_changes() {
    let (fixture, first, phone) = signed_in().await;
    let before = serde_json::to_vec(&first.store.record().unwrap()).unwrap();
    let known = entry("https://private-server.invalid");
    let saved = first
        .sync_catalog(std::slice::from_ref(&known))
        .await
        .unwrap()
        .unwrap();
    let restored = phone.sync_catalog(&[]).await.unwrap().unwrap();
    assert!(restored == saved && restored.entries == [known]);
    assert!(before == serde_json::to_vec(&first.store.record().unwrap()).unwrap());
    let ciphertext: String = sqlx::query_scalar("SELECT envelope FROM server_catalogs")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert!(
        !ciphertext.contains("private-server")
            && !ciphertext.contains("Synthetic saved server")
            && !ciphertext.contains("device_token")
    );
    assert!(!fixture.folder.path().join("phone/accounts.json").exists());
    let checkpoint = first.store.catalog().unwrap().checkpoint().unwrap();
    first.sync_catalog(&[]).await.unwrap();
    assert!(first.store.catalog().unwrap().checkpoint().unwrap() == checkpoint);
}
#[tokio::test]
async fn simultaneous_writers_catch_up_and_preserve_both_servers() {
    let (_fixture, first, phone) = signed_in().await;
    let a = entry("https://a.invalid");
    let b = entry("https://b.invalid");
    let aa = [a.clone()];
    let bb = [b.clone()];
    let (one, two) = tokio::join!(first.sync_catalog(&aa), phone.sync_catalog(&bb));
    one.unwrap();
    two.unwrap();
    let all = first.sync_catalog(&[]).await.unwrap().unwrap();
    assert!(all.entries == [a, b]);
    assert!(phone.sync_catalog(&[]).await.unwrap().unwrap() == all);
}
#[tokio::test]
async fn exact_retry_is_idempotent_stale_parent_conflicts_and_lost_response_resumes() {
    let (fixture, first, phone) = signed_in().await;
    let a = entry("https://a.invalid");
    let initial = envelope(&first, std::slice::from_ref(&a), None);
    let checkpoint = publish(&first, &initial).await.unwrap().catalog.unwrap();
    assert!(publish(&first, &initial).await.unwrap().catalog == Some(checkpoint));
    let fork = envelope(&phone, &[entry("https://b.invalid")], None);
    let failure = publish(&phone, &fork).await.err().unwrap();
    assert!(
        failure
            .downcast_ref::<Failure>()
            .is_some_and(|f| f.status == 409 && f.code == "catalog_state_changed")
    );
    first.sync_catalog(&[]).await.unwrap();
    *fixture.faults.lock().unwrap() = Some(PATH.into());
    assert!(
        first
            .sync_catalog(&[entry("https://c.invalid")])
            .await
            .is_err()
    );
    let pending = first.store.catalog().unwrap().pending.unwrap();
    let result = first.sync_catalog(&[]).await.unwrap().unwrap();
    assert!(result.entries.len() == 2 && result.entries[0] == a);
    assert!(
        first.store.catalog().unwrap().checkpoint().unwrap()
            == Some(pending.manifest.checkpoint().unwrap())
    );
    let revisions: i64 = sqlx::query_scalar("SELECT count(*) FROM server_catalog_manifests")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(revisions, 2);
}
#[tokio::test]
async fn restored_catalog_rejects_account_substitution_revoked_devices_and_rollback() {
    let (fixture, first, phone) = signed_in().await;
    first
        .sync_catalog(&[entry("https://a.invalid")])
        .await
        .unwrap();
    let initial = first.store.catalog().unwrap().head.unwrap();
    first
        .sync_catalog(&[entry("https://b.invalid")])
        .await
        .unwrap();
    phone.sync_catalog(&[]).await.unwrap();
    let mut outsider = fixture.client("outsider").await;
    outsider
        .signup("differentuser", "Different account", "Other", password())
        .await
        .unwrap();
    assert!(
        outsider
            .get::<ReadResponse>(PATH, catalog::MAX_ENVELOPE_WIRE)
            .await
            .unwrap()
            .catalog
            .is_none()
    );
    assert!(publish(&outsider, &initial).await.is_err());
    let previous = phone.store.catalog().unwrap().checkpoint().unwrap();
    let cp = initial.manifest.checkpoint().unwrap();
    sqlx::query("UPDATE server_catalogs SET revision=?,digest=?,envelope=? WHERE user_id=?")
        .bind(i64::try_from(cp.revision).unwrap())
        .bind(&cp.sha256)
        .bind(serde_json::to_string(&initial).unwrap())
        .bind(initial.manifest.header.scope.account.to_string())
        .execute(&fixture.pool)
        .await
        .unwrap();
    assert!(phone.sync_catalog(&[]).await.is_err());
    assert!(phone.store.catalog().unwrap().checkpoint().unwrap() == previous);
    sqlx::query("UPDATE devices SET revoked_at='2099-01-01T00:00:00Z' WHERE id=?")
        .bind(first.device.unwrap().to_string())
        .execute(&fixture.pool)
        .await
        .unwrap();
    assert!(publish(&first, &initial).await.is_err());
}
#[tokio::test]
async fn signed_history_is_paged_and_tampering_preserves_checkpoint() {
    let (fixture, first, phone) = signed_in().await;
    let a = entry("https://a.invalid");
    first.sync_catalog(std::slice::from_ref(&a)).await.unwrap();
    phone.sync_catalog(&[]).await.unwrap();
    let source = phone
        .store
        .catalog()
        .unwrap()
        .checkpoint()
        .unwrap()
        .unwrap();
    let mut current = source.clone();
    for _ in 0..130 {
        let next = envelope(&first, std::slice::from_ref(&a), Some(current));
        current = publish(&first, &next).await.unwrap().catalog.unwrap();
    }
    let proof: ProofPage = first
        .post(
            &format!("{PATH}/proof"),
            &ProofRequest {
                after: source.clone(),
                through: current.clone(),
            },
            true,
            1024 * 1024,
        )
        .await
        .unwrap();
    assert_eq!(proof.manifests.len(), 128);
    assert!(!proof.complete);
    phone.sync_catalog(&[]).await.unwrap();
    assert!(phone.store.catalog().unwrap().checkpoint().unwrap() == Some(current));
    let mut damaged: serde_json::Value = serde_json::from_str(
        &sqlx::query_scalar::<_, String>("SELECT envelope FROM server_catalogs")
            .fetch_one(&fixture.pool)
            .await
            .unwrap(),
    )
    .unwrap();
    damaged["ciphertext"] = "dGFtcGVyZWQ=".into();
    sqlx::query("UPDATE server_catalogs SET envelope=?")
        .bind(serde_json::to_string(&damaged).unwrap())
        .execute(&fixture.pool)
        .await
        .unwrap();
    assert!(first.sync_catalog(&[]).await.is_err());
    assert!(first.store.catalog().unwrap().checkpoint().unwrap() == Some(source));
}
#[tokio::test]
async fn conflicting_service_binding_does_not_replace_remote_or_local_history() {
    let (_fixture, first, phone) = signed_in().await;
    let known = entry("https://one.invalid");
    first
        .sync_catalog(std::slice::from_ref(&known))
        .await
        .unwrap();
    phone.sync_catalog(&[]).await.unwrap();
    let before = phone.store.catalog().unwrap();
    assert!(
        phone
            .sync_catalog(&[entry("https://one.invalid")])
            .await
            .is_err()
    );
    assert!(phone.store.catalog().unwrap() == before);
    assert!(first.sync_catalog(&[]).await.unwrap().unwrap().entries == [known]);
}

#[tokio::test]
async fn password_change_keeps_catalog_readable_on_a_fresh_device() {
    let (fixture, first, _phone) = signed_in().await;
    let known = entry("https://saved.invalid");
    first
        .sync_catalog(std::slice::from_ref(&known))
        .await
        .unwrap();
    let new_password = || SecretString::from("synthetic changed catalog password".to_owned());
    first
        .password_change(password(), new_password())
        .await
        .unwrap();
    let mut fresh = fixture.client("fresh").await;
    assert!(
        fresh
            .login("cataloguser", new_password(), "Fresh device")
            .await
            .unwrap()
    );
    assert!(fresh.sync_catalog(&[]).await.unwrap().unwrap().entries == [known]);
}

#[tokio::test]
async fn unusable_storage_prevents_publication_and_missing_history_fails_closed() {
    let (fixture, first, _phone) = signed_in().await;
    let journal = std::fs::read_dir(fixture.folder.path().join("desktop/account-backup"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "json"))
        .unwrap();
    let sidecar = journal.with_extension("catalog");
    std::fs::create_dir(&sidecar).unwrap();
    assert!(
        first
            .sync_catalog(&[entry("https://saved.invalid")])
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM server_catalogs")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    std::fs::remove_dir(&sidecar).unwrap();
    first
        .sync_catalog(&[entry("https://saved.invalid")])
        .await
        .unwrap();
    std::fs::remove_file(&sidecar).unwrap();
    assert!(first.sync_catalog(&[]).await.is_err());
    assert!(first.store.record().unwrap().key_verified);
}
