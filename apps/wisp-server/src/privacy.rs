use super::{
    ApiError, AppState, HeaderMap, Json, Message, Path, Row, State, StoredAttachment, Utc, Uuid,
    Value, authenticate_headers, ensure_conversation_member, own_message, persist_message,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::json;
use wisp_protocol::{BeginEncryptedUpload, EncryptedMessageRequest, SendMessageRequest};

/// Refuse a privacy-required startup with legacy content still present. This
/// checks active rows, NOT freed `SQLite` pages, WAL files or historical backups.
pub(super) async fn ensure_ciphertext_storage(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
    let legacy: i64 = sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM messages WHERE NOT (encryption_version=1 AND content_type='application/vnd.wisp.encrypted+json') AND content_type!='application/vnd.wisp.room-invitation+json') + (SELECT COUNT(*) FROM chat_images) + (SELECT COUNT(*) FROM chat_files WHERE length(data)>0) + (SELECT COUNT(*) FROM file_uploads WHERE encryption_version!=1)")
        .fetch_one(pool).await?;
    anyhow::ensure!(
        legacy == 0,
        "Ciphertext-only startup refused: legacy chat content exists. Complete and verify client-side migration before moving storage to a provider. Nothing was deleted."
    );
    Ok(())
}

pub(super) fn require_legacy_allowed(state: &AppState) -> Result<(), ApiError> {
    if state.config.require_chat_e2ee {
        return Err(ApiError::forbidden(
            "This server requires end-to-end encrypted chat; update and configure your client",
        ));
    }
    Ok(())
}

/// Legacy clients cannot change membership once clients have signed a roster.
pub(super) async fn require_unsigned_room(
    pool: &sqlx::SqlitePool,
    conversation: &str,
) -> Result<(), ApiError> {
    let signed: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM chat_rosters WHERE conversation_id=?)")
            .bind(conversation)
            .fetch_one(pool)
            .await
            .map_err(ApiError::internal)?;
    if signed {
        return Err(ApiError::conflict(
            "signed_membership_required",
            "Update Wisp to authorize encrypted room membership with your client key",
        ));
    }
    Ok(())
}

pub(super) async fn validate_roster(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    conversation: &str,
    sender: Uuid,
    hash: &str,
) -> Result<(), ApiError> {
    let encoded:Option<String>=sqlx::query_scalar("SELECT signed_roster FROM chat_rosters WHERE conversation_id=? ORDER BY revision DESC LIMIT 1").bind(conversation).fetch_optional(&mut **tx).await.map_err(ApiError::internal)?;
    let roster: wisp_crypto::roster::SignedRoster =
        serde_json::from_str(&encoded.ok_or_else(|| {
            ApiError::conflict(
                "room_identity_required",
                "Initialize signed room membership before sending",
            )
        })?)
        .map_err(ApiError::internal)?;
    if roster.hash().map_err(ApiError::internal)? != hash
        || !roster.roster.members.contains_key(&sender)
    {
        return Err(ApiError::conflict(
            "room_changed",
            "Encrypted room membership changed; refresh before sending",
        ));
    }
    Ok(())
}

fn validate(request: &EncryptedMessageRequest) -> Result<(), ApiError> {
    let bytes = STANDARD.decode(&request.ciphertext).map_err(|_| {
        ApiError::bad_request("invalid_envelope", "Invalid encrypted message encoding")
    })?;
    if !bytes.starts_with(b"age-encryption.org/v1\n") || bytes.len() < 100 {
        return Err(ApiError::bad_request(
            "invalid_envelope",
            "Expected an age encrypted message",
        ));
    }
    Ok(())
}

pub(super) fn stored(request: EncryptedMessageRequest) -> SendMessageRequest {
    SendMessageRequest {
        conversation_id: request.conversation_id,
        content_type: "application/vnd.wisp.encrypted+json".into(),
        encryption_version: 1,
        payload: json!({"id":request.id,"ciphertext":request.ciphertext,"roster_hash":request.roster_hash}),
    }
}

pub(super) async fn send(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<EncryptedMessageRequest>,
) -> Result<Json<Message>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    ensure_conversation_member(&state.pool, &request.conversation_id, user).await?;
    validate(&request)?;
    persist_message(&state, user, stored(request), None)
        .await
        .map(Json)
}

pub(super) async fn edit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<EncryptedMessageRequest>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    let row = own_message(&state.pool, user, id).await?;
    validate(&request)?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    sqlx::query("UPDATE messages SET payload=payload WHERE id=?")
        .bind(id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    validate_roster(
        &mut tx,
        &request.conversation_id,
        user,
        &request.roster_hash,
    )
    .await?;
    if request.id != id || request.conversation_id != row.get::<String, _>("conversation_id") {
        return Err(ApiError::bad_request(
            "invalid_envelope",
            "Encrypted edit context does not match",
        ));
    }
    if row.get::<String, _>("content_type") == "application/vnd.wisp.room-invitation+json" {
        return Err(ApiError::bad_request(
            "invitation_not_editable",
            "Voice invitations cannot be edited",
        ));
    }
    let mut payload: Value =
        serde_json::from_str(&row.get::<String, _>("payload")).map_err(ApiError::internal)?;
    if row.get::<i64, _>("encryption_version") != 1 {
        // Historical files need a byte-level migration, not a caption-only edit.
        if row.get::<String, _>("content_type") != "text/plain" {
            return Err(ApiError::bad_request(
                "attachment_migration_required",
                "Re-encrypt the attachment before migrating its message",
            ));
        }
        payload = json!({"id":id});
    }
    if row.get::<i64, _>("encryption_version") == 1 {
        // Preserve concurrent retention updates; do not rewrite stale payload.
        sqlx::query("UPDATE messages SET payload=json_set(payload,'$.ciphertext',?,'$.roster_hash',?),edited_at=? WHERE id=? AND sender_id=?")
            .bind(&request.ciphertext).bind(&request.roster_hash).bind(Utc::now().to_rfc3339()).bind(id.to_string()).bind(user.to_string()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    } else {
        payload["ciphertext"] = json!(request.ciphertext);
        payload["roster_hash"] = json!(request.roster_hash);
        sqlx::query("UPDATE messages SET payload=?,content_type='application/vnd.wisp.encrypted+json',encryption_version=1,edited_at=? WHERE id=? AND sender_id=?")
            .bind(payload.to_string()).bind(Utc::now().to_rfc3339()).bind(id.to_string()).bind(user.to_string()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    }
    tx.commit().await.map_err(ApiError::internal)?;
    state.emit("message_updated", json!({"changed":true})).await;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn begin_upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BeginEncryptedUpload>,
) -> Result<Json<wisp_protocol::FileUploadStatus>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    ensure_conversation_member(&state.pool, &request.message.conversation_id, user).await?;
    validate(&request.message)?;
    let size = i64::try_from(request.size).map_err(ApiError::internal)?;
    let plaintext_size = i64::try_from(request.plaintext_size.unwrap_or(request.size))
        .map_err(ApiError::internal)?;
    if plaintext_size > size {
        return Err(ApiError::bad_request(
            "invalid_size",
            "Plaintext size cannot exceed encrypted file size",
        ));
    }
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    sqlx::query("INSERT OR IGNORE INTO file_uploads(id,owner_id,conversation_id,file_name,caption,size,keep,last_activity_at,encryption_version,encrypted_message_id,plaintext_size) VALUES (?,?,?,'Encrypted attachment',?,?,?, ?,1,?,?)")
        .bind(request.upload_id.to_string()).bind(user.to_string()).bind(&request.message.conversation_id).bind(&request.message.ciphertext).bind(size).bind(request.keep).bind(Utc::now().to_rfc3339()).bind(request.message.id.to_string()).bind(plaintext_size).execute(&mut *tx).await.map_err(ApiError::internal)?;
    validate_roster(
        &mut tx,
        &request.message.conversation_id,
        user,
        &request.message.roster_hash,
    )
    .await?;
    let row = sqlx::query("SELECT * FROM file_uploads WHERE id=? AND owner_id=?")
        .bind(request.upload_id.to_string())
        .bind(user.to_string())
        .fetch_optional(&mut *tx)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::forbidden("Upload belongs to another user"))?;
    if row.get::<i64, _>("size") != size
        || row.get::<Option<i64>, _>("plaintext_size") != Some(plaintext_size)
        || row.get::<String, _>("conversation_id") != request.message.conversation_id
        || row
            .get::<Option<String>, _>("encrypted_message_id")
            .as_deref()
            != Some(&request.message.id.to_string())
        || row.get::<i64, _>("encryption_version") != 1
    {
        return Err(ApiError::conflict(
            "upload_changed",
            "Encrypted attachment changed; attach it again",
        ));
    }
    sqlx::query(
        "UPDATE file_uploads SET caption=?,keep=?,roster_hash=? WHERE id=? AND message_id IS NULL",
    )
    .bind(&request.message.ciphertext)
    .bind(request.keep)
    .bind(&request.message.roster_hash)
    .bind(request.upload_id.to_string())
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;
    let response = wisp_protocol::FileUploadStatus {
        received_bytes: u64::try_from(row.get::<i64, _>("received_bytes"))
            .map_err(ApiError::internal)?,
        next_chunk: u64::try_from(row.get::<i64, _>("next_chunk")).map_err(ApiError::internal)?,
        message_id: row
            .get::<Option<String>, _>("message_id")
            .map(|s| s.parse())
            .transpose()
            .map_err(ApiError::internal)?,
    };
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(response))
}

pub(super) async fn complete_upload(
    state: &AppState,
    user: Uuid,
    row: &sqlx::sqlite::SqliteRow,
    id: Uuid,
    expires: Option<chrono::DateTime<Utc>>,
) -> Result<Message, ApiError> {
    let request = EncryptedMessageRequest {
        roster_hash: row.get("roster_hash"),
        id: row
            .get::<String, _>("encrypted_message_id")
            .parse()
            .map_err(ApiError::internal)?,
        conversation_id: row.get("conversation_id"),
        ciphertext: row.get("caption"),
    };
    let mut request = stored(request);
    request.payload["size"] = json!(row.get::<i64, _>("size"));
    request.payload["retention_size"] = json!(
        row.get::<Option<i64>, _>("plaintext_size")
            .unwrap_or_else(|| row.get("size"))
    );
    request.payload["keep"] = json!(row.get::<bool, _>("keep"));
    request.payload["expires_at"] = json!(expires);
    request.payload["expired"] = json!(false);
    persist_message(state, user, request, Some(StoredAttachment::Upload(id))).await
}
