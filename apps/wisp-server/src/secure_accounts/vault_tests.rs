use super::authentication_tests::{Fixture, existing_proof, fixture, post, signed_in};
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use tower::ServiceExt;
use wisp_crypto::account_vault::{
    envelope::{Envelope, KeyEnvelope},
    operation::{Change, DeviceBinding, Operation},
};

async fn get(fixture: &Fixture, path: &str, session: &str) -> (StatusCode, Value) {
    let response = router(fixture.state.clone())
        .oneshot(
            Request::get(path)
                .header("authorization", format!("Bearer {session}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let bytes = to_bytes(response.into_body(), 12 * 1024 * 1024)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn expected(fixture: &Fixture) -> Precondition {
    let mut tx = fixture.state.pool.begin().await.unwrap();
    let expected = current_precondition(&mut tx, fixture.scope.account)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    expected
}

fn update(fixture: &Fixture, device: Uuid, previous: Precondition) -> (Operation, Value) {
    let envelope = Envelope::seal(&fixture.bundle, &fixture.key, previous.vault.clone()).unwrap();
    let operation = Operation {
        format: 1,
        scope: fixture.scope.clone(),
        id: Uuid::new_v4(),
        device: DeviceBinding::Existing { id: device },
        expected: previous,
        change: Change::StoreVault {
            vault: envelope.manifest.checkpoint().unwrap(),
        },
    };
    let signature = operation.sign(&fixture.bundle.identity().unwrap()).unwrap();
    let body = json!({"operation":operation,"signature":signature,"vault":envelope});
    (operation, body)
}

#[tokio::test]
async fn conditional_updates_have_one_winner_and_signed_proofs_preserve_the_original_checkpoint() {
    let fixture = fixture().await;
    let (device, session) = signed_in(&fixture).await;
    let previous = expected(&fixture).await;
    let (one, first) = update(&fixture, device.id(), previous.clone());
    let (_, second) = update(&fixture, device.id(), previous.clone());
    let (a, b) = tokio::join!(
        post(
            &fixture.state,
            "/v3/accounts/vault",
            first.clone(),
            Some(&session)
        ),
        post(&fixture.state, "/v3/accounts/vault", second, Some(&session))
    );
    assert!(
        (a.0 == StatusCode::OK && b.0 == StatusCode::CONFLICT)
            || (b.0 == StatusCode::OK && a.0 == StatusCode::CONFLICT)
    );
    let (status, read) = get(&fixture, "/v3/accounts/vault", &session).await;
    assert_eq!(status, StatusCode::OK);
    let envelope: Envelope = serde_json::from_value(read["vault"].clone()).unwrap();
    let (status, page) = post(
        &fixture.state,
        "/v3/accounts/vault/proof",
        json!({"after":previous.vault,"through":envelope.manifest.checkpoint().unwrap()}),
        Some(&session),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["complete"], true);
    assert_eq!(page["manifests"].as_array().unwrap().len(), 1);
    let accepted = envelope
        .manifest
        .advance(
            &fixture.scope,
            &fixture.bundle.identity().unwrap().public(),
            previous.vault.as_ref(),
        )
        .unwrap();
    let restored = envelope
        .open(
            &fixture.key,
            &fixture.scope,
            &fixture.bundle.identity().unwrap().public(),
            &accepted,
        )
        .unwrap();
    let same = restored.encode_private().unwrap().as_slice()
        == fixture.bundle.encode_private().unwrap().as_slice();
    assert!(same, "Restored bundle changed");
    if a.0 == StatusCode::OK {
        let (status, retried) =
            post(&fixture.state, "/v3/accounts/vault", first, Some(&session)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(retried["operation_id"], one.id.to_string());
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM account_vault_manifests")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
    let (status, _) = post(
        &fixture.state,
        "/v3/accounts/vault/proof",
        json!({"after":{"revision":1,"sha256":"a".repeat(64)},"through":accepted}),
        Some(&session),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn vault_signatures_scope_and_historical_receipts_cannot_replace_current_state() {
    let fixture = fixture().await;
    let (device, session) = signed_in(&fixture).await;
    let previous = expected(&fixture).await;
    let (operation, body) = update(&fixture, device.id(), previous.clone());
    let mut bad = body.clone();
    bad["signature"] = json!("invalid");
    assert_eq!(
        post(&fixture.state, "/v3/accounts/vault", bad, Some(&session))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        post(&fixture.state, "/v3/accounts/vault", body.clone(), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(
            &fixture.state,
            "/v3/accounts/vault",
            body.clone(),
            Some(&session)
        )
        .await
        .0,
        StatusCode::OK
    );
    let next_expected = expected(&fixture).await;
    let (_, next) = update(&fixture, device.id(), next_expected);
    assert_eq!(
        post(&fixture.state, "/v3/accounts/vault", next, Some(&session))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        post(&fixture.state, "/v3/accounts/vault", body, Some(&session))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(expected(&fixture).await.vault.unwrap().revision, 3);
    let (status, receipt) = get(
        &fixture,
        &format!("/v3/accounts/operations/{}", operation.id),
        &session,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(receipt["result"]["vault"]["revision"], 2);
    let other = Uuid::new_v4();
    let other_device = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,display_name,username) VALUES(?,'Other synthetic user','otherfixture')").bind(other.to_string()).execute(&fixture.state.pool).await.unwrap();
    sqlx::query("INSERT INTO devices(id,user_id,name,token_hash,created_at) VALUES(?,?,'Test','synthetic-token-hash',?)").bind(other_device.to_string()).bind(other.to_string()).bind(Utc::now().to_rfc3339()).execute(&fixture.state.pool).await.unwrap();
    sqlx::query(
        "INSERT INTO sessions(id,device_id,token_hash,created_at,expires_at) VALUES(?,?,?,?,?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(other_device.to_string())
    .bind(token_hash("other-test-session"))
    .bind(Utc::now().to_rfc3339())
    .bind((Utc::now() + ChronoDuration::minutes(5)).to_rfc3339())
    .execute(&fixture.state.pool)
    .await
    .unwrap();
    assert_eq!(
        get(
            &fixture,
            &format!("/v3/accounts/operations/{}", operation.id),
            "other-test-session"
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn rewrap_requires_exact_one_use_grant_and_never_changes_payload() {
    let fixture = fixture().await;
    let (device, session) = signed_in(&fixture).await;
    let previous = expected(&fixture).await;
    let (attempt, proof) = existing_proof(&fixture, &session, None).await;
    assert_eq!(
        post(
            &fixture.state,
            "/v3/auth/unlock/finish",
            json!({"attempt":attempt,"finalization":proof.finalization}),
            Some(&session)
        )
        .await
        .0,
        StatusCode::OK
    );
    let wrapper = KeyEnvelope::wrap(
        fixture.scope.clone(),
        previous.credential_generation.unwrap(),
        &proof.export_key,
        &fixture.key,
    )
    .unwrap();
    let operation = Operation {
        format: 1,
        scope: fixture.scope.clone(),
        id: Uuid::new_v4(),
        device: DeviceBinding::Existing { id: device.id() },
        expected: previous,
        change: Change::Rewrap {
            wrapper_sha256: wrapper.digest().unwrap(),
        },
    };
    let signature = operation.sign(&fixture.bundle.identity().unwrap()).unwrap();
    let mut body = json!({"operation":operation,"signature":signature,"wrapper":wrapper,"grant":"a".repeat(43)});
    assert_eq!(
        post(
            &fixture.state,
            "/v3/accounts/vault/rewrap",
            body.clone(),
            Some(&session)
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (attempt, proof) = existing_proof(&fixture, &session, Some(&operation)).await;
    let (status, granted) = post(
        &fixture.state,
        "/v3/auth/reauth/finish",
        json!({"attempt":attempt,"finalization":proof.finalization}),
        Some(&session),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    body["grant"] = granted["grant"].clone();
    let before: String = sqlx::query_scalar("SELECT envelope FROM account_vaults")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(
        post(
            &fixture.state,
            "/v3/accounts/vault/rewrap",
            body.clone(),
            Some(&session)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        post(
            &fixture.state,
            "/v3/accounts/vault/rewrap",
            body,
            Some(&session)
        )
        .await
        .0,
        StatusCode::OK
    );
    let after: String = sqlx::query_scalar("SELECT envelope FROM account_vaults")
        .fetch_one(&fixture.state.pool)
        .await
        .unwrap();
    assert_eq!(before, after);
    let consumed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM secure_reauth_grants WHERE consumed_at IS NOT NULL",
    )
    .fetch_one(&fixture.state.pool)
    .await
    .unwrap();
    assert_eq!(consumed, 1);
}
