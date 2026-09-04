use super::{
    ApiError, AppState, ChronoDuration, Duration, HeaderMap, Json, Message, Path, Response, Row,
    SendMessageRequest, SqlitePool, SqliteRow, StoredAttachment, UserId, Utc, Uuid, Value,
    authenticate_headers, ensure_conversation_member, message_from_row, parse_uuid,
    persist_message,
};
use axum::{
    body::{Body, Bytes},
    extract::State,
};
use chrono::DateTime;
use serde_json::json;
use tracing::warn;
use wisp_protocol::{BeginFileUpload, CHAT_FILE_CHUNK_BYTES, FileUploadStatus, SetFileRetention};

fn deadline(size: u64, created: DateTime<Utc>, keep: bool) -> Option<DateTime<Utc>> {
    if keep {
        None
    } else {
        wisp_protocol::file_retention_hours(size)
            .map(|hours| created + ChronoDuration::hours(hours))
    }
}

fn status(row: &SqliteRow) -> Result<FileUploadStatus, ApiError> {
    Ok(FileUploadStatus {
        received_bytes: u64::try_from(row.get::<i64, _>("received_bytes"))
            .map_err(ApiError::internal)?,
        next_chunk: u64::try_from(row.get::<i64, _>("next_chunk")).map_err(ApiError::internal)?,
        message_id: row
            .get::<Option<String>, _>("message_id")
            .map(|id| parse_uuid(&id))
            .transpose()?,
    })
}

async fn owned(state: &AppState, user: UserId, id: Uuid) -> Result<SqliteRow, ApiError> {
    let row = sqlx::query("SELECT * FROM file_uploads WHERE id = ? AND owner_id = ?")
        .bind(id.to_string())
        .bind(user.to_string())
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("Upload unavailable"))?;
    ensure_conversation_member(&state.pool, &row.get::<String, _>("conversation_id"), user).await?;
    Ok(row)
}

pub(super) async fn begin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BeginFileUpload>,
) -> Result<Json<FileUploadStatus>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    ensure_conversation_member(&state.pool, &request.conversation_id, user).await?;
    if !wisp_protocol::valid_chat_file_name(&request.file_name) {
        return Err(ApiError::bad_request("invalid_file", "Invalid filename"));
    }
    let size = i64::try_from(request.size).map_err(|_| {
        ApiError::bad_request("invalid_size", "File size cannot be represented by storage")
    })?;
    sqlx::query("INSERT OR IGNORE INTO file_uploads(id, owner_id, conversation_id, file_name, caption, size, keep, last_activity_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(request.id.to_string()).bind(user.to_string()).bind(&request.conversation_id).bind(&request.file_name)
        .bind(&request.caption).bind(size).bind(request.keep).bind(Utc::now().to_rfc3339())
        .execute(&state.pool).await.map_err(ApiError::internal)?;
    let row = owned(&state, user, request.id).await?;
    if row.get::<String, _>("conversation_id") != request.conversation_id
        || row.get::<String, _>("file_name") != request.file_name
        || row.get::<i64, _>("size") != size
    {
        return Err(ApiError::conflict(
            "upload_changed",
            "File changed; attach it again",
        ));
    }
    // A retry may update its caption/keep choice before committing, never after.
    sqlx::query("UPDATE file_uploads SET caption = ?, keep = ?, last_activity_at = ? WHERE id = ? AND message_id IS NULL")
        .bind(request.caption).bind(request.keep).bind(Utc::now().to_rfc3339()).bind(request.id.to_string())
        .execute(&state.pool).await.map_err(ApiError::internal)?;
    Ok(Json(status(&row)?))
}

pub(super) async fn chunk(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, index)): Path<(Uuid, u64)>,
    bytes: Bytes,
) -> Result<Json<FileUploadStatus>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    owned(&state, user, id).await?;
    if bytes.is_empty() || bytes.len() > CHAT_FILE_CHUNK_BYTES {
        return Err(ApiError::bad_request(
            "invalid_chunk",
            "Invalid transfer chunk",
        ));
    }
    let index = i64::try_from(index).map_err(ApiError::internal)?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    // Take the write lock before reading offsets; simultaneous retries cannot append twice.
    sqlx::query("UPDATE file_uploads SET last_activity_at = ? WHERE id = ? AND owner_id = ?")
        .bind(Utc::now().to_rfc3339())
        .bind(id.to_string())
        .bind(user.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    let row = sqlx::query("SELECT * FROM file_uploads WHERE id = ? AND owner_id = ?")
        .bind(id.to_string())
        .bind(user.to_string())
        .fetch_optional(&mut *tx)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("Upload expired"))?;
    let next: i64 = row.get("next_chunk");
    if row.get::<Option<String>, _>("message_id").is_some() {
        return Err(ApiError::conflict(
            "upload_complete",
            "Upload already completed",
        ));
    }
    if index < next {
        let previous: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT data FROM file_chunks WHERE upload_id = ? AND chunk_index = ?",
        )
        .bind(id.to_string())
        .bind(index)
        .fetch_optional(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
        if previous.as_deref() != Some(bytes.as_ref()) {
            return Err(ApiError::conflict(
                "chunk_changed",
                "Retry data does not match",
            ));
        }
    } else {
        let received: i64 = row.get("received_bytes");
        let size: i64 = row.get("size");
        let remaining = size - received;
        let expected = remaining.min(i64::try_from(CHAT_FILE_CHUNK_BYTES).unwrap_or(i64::MAX));
        if index != next || i64::try_from(bytes.len()).map_err(ApiError::internal)? != expected {
            return Err(ApiError::conflict(
                "chunk_offset",
                "Chunk offset or length does not match",
            ));
        }
        sqlx::query("INSERT INTO file_chunks(upload_id, chunk_index, data) VALUES (?, ?, ?)")
            .bind(id.to_string())
            .bind(index)
            .bind(bytes.as_ref())
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
        sqlx::query("UPDATE file_uploads SET received_bytes = received_bytes + ?, next_chunk = next_chunk + 1 WHERE id = ?")
            .bind(i64::try_from(bytes.len()).map_err(ApiError::internal)?).bind(id.to_string()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    }
    let row = sqlx::query("SELECT * FROM file_uploads WHERE id = ?")
        .bind(id.to_string())
        .fetch_one(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    Ok(Json(status(&row)?))
}

pub(super) async fn complete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Message>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    let row = owned(&state, user, id).await?;
    if let Some(message_id) = row.get::<Option<String>, _>("message_id") {
        let message = sqlx::query("SELECT m.*, u.display_name FROM messages m JOIN users u ON u.id = m.sender_id WHERE m.id = ?")
            .bind(message_id).fetch_one(&state.pool).await.map_err(ApiError::internal)?;
        return Ok(Json(message_from_row(&message)?));
    }
    let size = row.get::<i64, _>("size");
    if row.get::<i64, _>("received_bytes") != size {
        return Err(ApiError::conflict(
            "incomplete_upload",
            "Upload is not complete",
        ));
    }
    let keep: bool = row.get("keep");
    let expires = deadline(
        u64::try_from(size).map_err(ApiError::internal)?,
        Utc::now(),
        keep,
    );
    persist_message(&state, user, SendMessageRequest {
        conversation_id: row.get("conversation_id"), content_type: "application/octet-stream".into(), encryption_version: 0,
        payload: json!({"file_name":row.get::<String,_>("file_name"), "size":size, "caption":row.get::<String,_>("caption"), "keep":keep, "expires_at":expires, "expired":false}),
    }, Some(StoredAttachment::Upload(id))).await.map(Json)
}

pub(super) async fn download(
    state: &AppState,
    headers: &HeaderMap,
    id: Uuid,
) -> Result<Option<Response>, ApiError> {
    let user = authenticate_headers(state, headers).await?;
    let row = sqlx::query("SELECT cf.upload_id, m.payload FROM chat_files cf JOIN messages m ON m.id = cf.message_id JOIN conversation_members cm ON cm.conversation_id = m.conversation_id WHERE m.id = ? AND cm.user_id = ? AND m.created_at > COALESCE(cm.history_cleared_at, '')")
        .bind(id.to_string()).bind(user.to_string()).fetch_optional(&state.pool).await.map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("File is unavailable or expired"))?;
    let payload: Value =
        serde_json::from_str(&row.get::<String, _>("payload")).map_err(ApiError::internal)?;
    if is_expired(&payload, Utc::now()) {
        return Err(ApiError::not_found("File expired"));
    }
    let Some(upload_id) = row.get::<Option<String>, _>("upload_id") else {
        return Ok(None);
    };
    let size = payload["size"]
        .as_u64()
        .ok_or_else(|| ApiError::internal("missing file size"))?;
    let pool = state.pool.clone();
    let stream = futures_util::stream::try_unfold(
        (pool, upload_id, 0_i64, size),
        |(pool, upload_id, index, remaining)| async move {
            if remaining == 0 {
                return Ok::<_, std::io::Error>(None);
            }
            let bytes: Option<Vec<u8>> = sqlx::query_scalar("SELECT fc.data FROM file_chunks fc JOIN file_uploads fu ON fu.id = fc.upload_id WHERE fc.upload_id = ? AND fc.chunk_index = ? AND fu.message_id IS NOT NULL")
            .bind(&upload_id).bind(index).fetch_optional(&pool).await.map_err(std::io::Error::other)?;
            let bytes = bytes
                .ok_or_else(|| std::io::Error::other("File expired or deleted during download"))?;
            let remaining = remaining
                .checked_sub(bytes.len() as u64)
                .ok_or_else(|| std::io::Error::other("Invalid file length"))?;
            Ok(Some((bytes, (pool, upload_id, index + 1, remaining))))
        },
    );
    let response = Response::builder()
        .header("content-type", "application/octet-stream")
        .header("content-disposition", "attachment")
        .header("cache-control", "private, no-store")
        .header("x-content-type-options", "nosniff")
        .header("content-length", size)
        .body(Body::from_stream(stream))
        .map_err(ApiError::internal)?;
    Ok(Some(response))
}

fn is_expired(payload: &Value, now: DateTime<Utc>) -> bool {
    payload["expired"] == true
        || (payload["keep"] != true
            && payload["expires_at"]
                .as_str()
                .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
                .is_some_and(|date| date <= now))
}

pub(super) async fn retention(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<SetFileRetention>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    // Serialize keep/expiry/edit races. Any member with visible history may keep a file.
    sqlx::query("UPDATE messages SET payload = payload WHERE id = ?")
        .bind(id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    let row = sqlx::query("SELECT m.payload, m.created_at FROM messages m JOIN conversation_members cm ON cm.conversation_id = m.conversation_id JOIN chat_files cf ON cf.message_id = m.id WHERE m.id = ? AND cm.user_id = ? AND m.created_at > COALESCE(cm.history_cleared_at, '')")
        .bind(id.to_string()).bind(user.to_string()).fetch_optional(&mut *tx).await.map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("File unavailable"))?;
    let payload: Value =
        serde_json::from_str(&row.get::<String, _>("payload")).map_err(ApiError::internal)?;
    if is_expired(&payload, Utc::now()) {
        return Err(ApiError::not_found("Expired files cannot be kept"));
    }
    let created = DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
        .map_err(ApiError::internal)?
        .with_timezone(&Utc);
    let expires = deadline(payload["size"].as_u64().unwrap_or(0), created, request.keep)
        .map(|date| date.to_rfc3339());
    sqlx::query("UPDATE messages SET payload = json_set(payload, '$.keep', json(?), '$.expires_at', ?) WHERE id = ?")
        .bind(if request.keep { "true" } else { "false" }).bind(expires).bind(id.to_string()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    cleanup(&state.pool, Utc::now())
        .await
        .map_err(ApiError::internal)?;
    state
        .emit("file_retention_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn cleanup(pool: &SqlitePool, now: DateTime<Utc>) -> anyhow::Result<u64> {
    let mut tx = pool.begin().await?;
    let changed = sqlx::query("UPDATE messages SET payload = json_set(payload, '$.expired', json('true')) WHERE content_type = 'application/octet-stream' AND json_extract(payload, '$.keep') IS NOT 1 AND json_extract(payload, '$.expired') IS NOT 1 AND julianday(json_extract(payload, '$.expires_at')) <= julianday(?)")
        .bind(now.to_rfc3339()).execute(&mut *tx).await?.rows_affected();
    // Removing chunks and legacy bytes leaves a visible 'expired' marker in chat.
    sqlx::query("DELETE FROM file_uploads WHERE message_id IN (SELECT id FROM messages WHERE content_type = 'application/octet-stream' AND json_extract(payload, '$.expired') = 1) OR (message_id IS NULL AND last_activity_at < ?)")
        .bind((now - ChronoDuration::hours(24)).to_rfc3339()).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM chat_files WHERE message_id IN (SELECT id FROM messages WHERE content_type = 'application/octet-stream' AND json_extract(payload, '$.expired') = 1)").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(changed)
}

impl AppState {
    pub async fn maintain_attachments(self) {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            match cleanup(&self.pool, Utc::now()).await {
                Ok(count) if count > 0 => self.emit("files_expired", json!({"changed":true})).await,
                Ok(_) => (),
                Err(error) => warn!(%error, "attachment expiry failed; will retry"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{chat_headers, test_config};
    use crate::{
        CHARLIE_ID, JARED_ID, TYLER_ID, clear_history_for, find_or_create_direct, get_chat_file,
        load_recent_messages,
    };
    use futures_util::StreamExt;

    #[test]
    fn exact_decimal_gb_boundaries_and_kept_files() {
        let now = Utc::now();
        for (size, hours) in [
            (0, None),
            (1_000_000_000, None),
            (1_000_000_001, Some(48)),
            (5_000_000_000, Some(48)),
            (5_000_000_001, Some(24)),
            (10_000_000_000, Some(24)),
        ] {
            assert_eq!(
                deadline(size, now, false),
                hours.map(|n| now + ChronoDuration::hours(n))
            );
            assert_eq!(deadline(size, now, true), None);
        }
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn chunks_resume_stream_and_complete_idempotently_above_old_limit() {
        let state = AppState::new(test_config()).await.unwrap();
        let user = Uuid::parse_str(JARED_ID).unwrap();
        let peer = Uuid::parse_str(TYLER_ID).unwrap();
        let conversation_id = find_or_create_direct(&state.pool, user, peer)
            .await
            .unwrap();
        let request = BeginFileUpload {
            id: Uuid::new_v4(),
            conversation_id,
            file_name: "large.bin".into(),
            size: (CHAT_FILE_CHUNK_BYTES * 8) as u64,
            caption: "A large file".into(),
            keep: false,
        };
        let id = request.id;
        assert_eq!(
            begin(
                State(state.clone()),
                chat_headers(JARED_ID),
                Json(request.clone())
            )
            .await
            .unwrap()
            .0
            .received_bytes,
            0
        );
        assert!(
            complete(State(state.clone()), chat_headers(JARED_ID), Path(id))
                .await
                .is_err()
        );
        assert!(
            begin(
                State(state.clone()),
                chat_headers(TYLER_ID),
                Json(request.clone())
            )
            .await
            .is_err()
        );
        let bytes = Bytes::from(vec![42; CHAT_FILE_CHUNK_BYTES]);
        assert!(
            chunk(
                State(state.clone()),
                chat_headers(TYLER_ID),
                Path((id, 0)),
                bytes.clone()
            )
            .await
            .is_err()
        );
        assert!(
            chunk(
                State(state.clone()),
                chat_headers(JARED_ID),
                Path((id, 1)),
                bytes.clone()
            )
            .await
            .is_err()
        );
        for index in 0..8 {
            let result = chunk(
                State(state.clone()),
                chat_headers(JARED_ID),
                Path((id, index)),
                bytes.clone(),
            )
            .await
            .unwrap()
            .0;
            assert_eq!(result.next_chunk, index + 1);
        }
        // Repeated network requests do not append bytes twice.
        assert_eq!(
            chunk(
                State(state.clone()),
                chat_headers(JARED_ID),
                Path((id, 7)),
                bytes
            )
            .await
            .unwrap()
            .0
            .next_chunk,
            8
        );
        assert_eq!(
            begin(
                State(state.clone()),
                chat_headers(JARED_ID),
                Json(request.clone())
            )
            .await
            .unwrap()
            .0
            .received_bytes,
            request.size
        );
        let message = complete(State(state.clone()), chat_headers(JARED_ID), Path(id))
            .await
            .unwrap()
            .0;
        assert_eq!(
            complete(State(state.clone()), chat_headers(JARED_ID), Path(id))
                .await
                .unwrap()
                .0
                .id,
            message.id
        );
        assert!(
            download(&state, &chat_headers(CHARLIE_ID), message.id)
                .await
                .is_err()
        );
        let response = download(&state, &chat_headers(TYLER_ID), message.id)
            .await
            .unwrap()
            .unwrap();
        let mut stream = response.into_body().into_data_stream();
        let mut received = 0;
        while let Some(bytes) = stream.next().await {
            let bytes = bytes.unwrap();
            assert!(bytes.len() <= CHAT_FILE_CHUNK_BYTES);
            assert!(bytes.iter().all(|byte| *byte == 42));
            received += bytes.len() as u64;
        }
        assert_eq!(received, request.size);
        // Common DM clearing deletes both metadata and every chunk, including kept files.
        let _ = retention(
            State(state.clone()),
            chat_headers(TYLER_ID),
            Path(message.id),
            Json(SetFileRetention { keep: true }),
        )
        .await
        .unwrap();
        clear_history_for(&state.pool, user, &request.conversation_id)
            .await
            .unwrap();
        assert!(
            get_chat_file(
                State(state.clone()),
                chat_headers(TYLER_ID),
                Path(message.id)
            )
            .await
            .is_ok()
        );
        clear_history_for(&state.pool, peer, &request.conversation_id)
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM file_chunks")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        assert!(
            load_recent_messages(&state.pool, peer)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn expiry_keep_override_and_abandoned_upload_cleanup() {
        let state = AppState::new(test_config()).await.unwrap();
        let user = Uuid::parse_str(JARED_ID).unwrap();
        let peer = Uuid::parse_str(TYLER_ID).unwrap();
        let conversation_id = find_or_create_direct(&state.pool, user, peer)
            .await
            .unwrap();
        let created = Utc::now() - ChronoDuration::hours(25);
        // Policy metadata is virtualized; tests need not allocate six gigabytes.
        let message = persist_message(&state, user, SendMessageRequest {conversation_id:conversation_id.clone(),content_type:"application/octet-stream".into(),encryption_version:0,
            payload:json!({"file_name":"large.bin","size":6_000_000_000_u64,"caption":"kept","keep":true,"expires_at":null,"expired":false})}, Some(StoredAttachment::File(vec![1]))).await.unwrap();
        sqlx::query("UPDATE messages SET created_at = ? WHERE id = ?")
            .bind(created.to_rfc3339())
            .bind(message.id.to_string())
            .execute(&state.pool)
            .await
            .unwrap();
        assert_eq!(cleanup(&state.pool, Utc::now()).await.unwrap(), 0);
        assert!(
            retention(
                State(state.clone()),
                chat_headers(CHARLIE_ID),
                Path(message.id),
                Json(SetFileRetention { keep: false })
            )
            .await
            .is_err()
        );
        let _ = retention(
            State(state.clone()),
            chat_headers(TYLER_ID),
            Path(message.id),
            Json(SetFileRetention { keep: false }),
        )
        .await
        .unwrap();
        assert!(
            get_chat_file(
                State(state.clone()),
                chat_headers(JARED_ID),
                Path(message.id)
            )
            .await
            .is_err()
        );
        assert_eq!(
            load_recent_messages(&state.pool, user).await.unwrap()[0].payload["expired"],
            true
        );
        assert!(
            retention(
                State(state.clone()),
                chat_headers(JARED_ID),
                Path(message.id),
                Json(SetFileRetention { keep: true })
            )
            .await
            .is_err()
        );
        let pending = BeginFileUpload {
            id: Uuid::new_v4(),
            conversation_id,
            file_name: "abandoned.bin".into(),
            size: 10_000_000_000,
            caption: String::new(),
            keep: true,
        };
        let _ = begin(
            State(state.clone()),
            chat_headers(JARED_ID),
            Json(pending.clone()),
        )
        .await
        .unwrap();
        sqlx::query("UPDATE file_uploads SET last_activity_at = ?")
            .bind(created.to_rfc3339())
            .execute(&state.pool)
            .await
            .unwrap();
        cleanup(&state.pool, Utc::now()).await.unwrap();
        assert!(owned(&state, user, pending.id).await.is_err());
    }
}
