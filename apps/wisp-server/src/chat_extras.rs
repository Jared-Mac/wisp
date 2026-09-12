use super::{ApiError, AppState, HeaderMap, Json, Path, Query, Row, State, Utc, Uuid, Value};
use axum::response::{IntoResponse, Response};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::json;
use std::io::Cursor;
use wisp_protocol::{Message, MessageReaction, REACTION_CONTENT_TYPE, ReactionRequest};

#[derive(Deserialize)]
pub(super) struct EmojiQuery {
    #[serde(default)]
    after: String,
}

pub(super) async fn emojis(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<EmojiQuery>,
) -> Result<Json<Value>, ApiError> {
    let user = super::authenticate_headers(&state, &headers).await?;
    let rows = sqlx::query("SELECT id,name,scope,owner_id FROM custom_emojis WHERE removed_at IS NULL AND ((scope='server' AND EXISTS(SELECT 1 FROM users WHERE id=?1 AND server_member=1)) OR owner_id=?1) AND id>?2 ORDER BY id LIMIT 101")
        .bind(user.to_string()).bind(query.after).fetch_all(&state.pool).await.map_err(ApiError::internal)?;
    let entries: Vec<_> = rows.iter().take(100).map(|r| json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"scope":r.get::<String,_>("scope"),"owner_id":r.get::<String,_>("owner_id")})).collect();
    let next = if rows.len() > 100 {
        entries.last().map(|e| e["id"].clone())
    } else {
        None
    };
    Ok(Json(json!({"emojis":entries,"next":next})))
}

#[derive(Deserialize)]
pub(super) struct EmojiUpload {
    name: String,
    scope: String,
    data: String,
}

pub(super) async fn upload_emoji(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<EmojiUpload>,
) -> Result<Json<Value>, ApiError> {
    let owner = super::authenticate_headers(&state, &headers).await?;
    let name = request.name.to_lowercase();
    if !(2..=32).contains(&name.len())
        || !name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
        || !matches!(request.scope.as_str(), "account" | "server")
    {
        return Err(ApiError::bad_request(
            "invalid_emoji",
            "Use 2–32 letters, numbers or underscores and choose an emoji library",
        ));
    }
    if request.scope == "server" {
        super::server_management::require_manager(&state, &headers).await?;
    }
    if request.data.len() > 2_800_000 {
        return Err(ApiError::bad_request(
            "emoji_too_large",
            "Choose an image under 2 MB",
        ));
    }
    let bytes = STANDARD
        .decode(&request.data)
        .map_err(|_| ApiError::bad_request("invalid_image", "Invalid emoji image"))?;
    let png = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
        let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let decoded = reader.decode()?.thumbnail(128, 128);
        let mut png = Cursor::new(Vec::new());
        decoded.write_to(&mut png, image::ImageFormat::Png)?;
        Ok(png.into_inner())
    })
    .await
    .map_err(ApiError::internal)?
    .map_err(|_| {
        ApiError::bad_request(
            "invalid_image",
            "Choose a PNG, JPEG, GIF or WebP image up to 4096 × 4096",
        )
    })?;
    let id = Uuid::new_v4();
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    sqlx::query("UPDATE users SET id=id WHERE id=?")
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    if request.scope == "server" {
        let allowed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_identity WHERE owner_user_id=?) OR EXISTS(SELECT 1 FROM server_admins WHERE user_id=?)").bind(owner.to_string()).bind(owner.to_string()).fetch_one(&mut *tx).await.map_err(ApiError::internal)?;
        if !allowed {
            return Err(ApiError::forbidden("Server administrator required"));
        }
    }
    sqlx::query(
        "INSERT INTO custom_emojis(id,owner_id,scope,name,png,created_at) VALUES (?,?,?,?,?,?)",
    )
    .bind(id.to_string())
    .bind(owner.to_string())
    .bind(&request.scope)
    .bind(&name)
    .bind(png)
    .bind(Utc::now().to_rfc3339())
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if e.as_database_error()
            .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
        {
            ApiError::conflict(
                "emoji_name_taken",
                "An emoji with that name already exists in this library",
            )
        } else {
            ApiError::internal(e)
        }
    })?;
    tx.commit().await.map_err(ApiError::internal)?;
    state.emit("emojis_changed", json!({"changed":true})).await;
    Ok(Json(
        json!({"id":id,"name":name,"scope":request.scope,"owner_id":owner}),
    ))
}

pub(super) async fn emoji_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    super::authenticate_headers(&state, &headers).await?;
    // Removed library entries remain renderable in existing chat history.
    let png: Vec<u8> = sqlx::query_scalar("SELECT png FROM custom_emojis WHERE id=?")
        .bind(id.to_string())
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("Emoji unavailable"))?;
    Ok((
        [
            ("content-type", "image/png"),
            ("cache-control", "private, max-age=86400"),
            ("x-content-type-options", "nosniff"),
        ],
        png,
    )
        .into_response())
}

pub(super) async fn remove_emoji(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let user = super::authenticate_headers(&state, &headers).await?;
    let manager = super::server_management::is_manager(&state.pool, user).await?;
    let count = sqlx::query("UPDATE custom_emojis SET removed_at=? WHERE id=? AND removed_at IS NULL AND ((scope='account' AND owner_id=?) OR (scope='server' AND ? AND (EXISTS(SELECT 1 FROM server_identity WHERE owner_user_id=?) OR EXISTS(SELECT 1 FROM server_admins WHERE user_id=?))))")
        .bind(Utc::now().to_rfc3339()).bind(id.to_string()).bind(user.to_string()).bind(manager).bind(user.to_string()).bind(user.to_string()).execute(&state.pool).await.map_err(ApiError::internal)?.rows_affected();
    if count == 0 {
        return Err(ApiError::forbidden(
            "Only the account owner or a server administrator can remove this emoji",
        ));
    }
    state.emit("emojis_changed", json!({"changed":true})).await;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn react(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(target): Path<Uuid>,
    Json(request): Json<ReactionRequest>,
) -> Result<Json<Value>, ApiError> {
    let user = super::authenticate_headers(&state, &headers).await?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    sqlx::query("UPDATE users SET id=id WHERE id=?")
        .bind(user.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    let message = sqlx::query("SELECT m.conversation_id,m.content_type FROM messages m JOIN accessible_conversation_members cm ON cm.conversation_id=m.conversation_id AND cm.user_id=? WHERE m.id=? AND m.created_at>COALESCE(cm.history_cleared_at,'') AND (json_extract(m.payload,'$.recipient_ids') IS NULL OR EXISTS(SELECT 1 FROM json_each(m.payload,'$.recipient_ids') recipient WHERE recipient.value=cm.user_id))")
        .bind(user.to_string()).bind(target.to_string()).fetch_optional(&mut *tx).await.map_err(ApiError::internal)?.ok_or_else(|| ApiError::not_found("Message unavailable"))?;
    if message.get::<String, _>("content_type") == "application/vnd.wisp.room-invitation+json" {
        return Err(ApiError::bad_request(
            "invalid_reaction_target",
            "Invitations do not support reactions",
        ));
    }
    let conversation: String = message.get("conversation_id");
    super::account_membership::require_unblocked_chat_tx(&mut tx, user, &conversation).await?;
    let (kind, payload, version) = if let Some(encrypted) = request.encrypted {
        if encrypted.id != request.id
            || encrypted.conversation_id != conversation
            || request.emoji.is_some()
        {
            return Err(ApiError::bad_request(
                "invalid_reaction",
                "Reaction context does not match",
            ));
        }
        super::privacy::validate(&encrypted)?;
        super::privacy::validate_roster(
            &mut tx,
            &conversation,
            user,
            &encrypted.roster_hash,
            encrypted.recipient_ids.as_deref(),
        )
        .await?;
        let stored = super::privacy::stored(encrypted);
        (stored.content_type, stored.payload, 1)
    } else {
        super::privacy::require_legacy_allowed(&state)?;
        let emoji = request.emoji.unwrap_or_default();
        if emoji.is_empty() || emoji.len() > 200 || emoji.chars().any(char::is_control) {
            return Err(ApiError::bad_request("invalid_emoji", "Choose an emoji"));
        }
        (
            REACTION_CONTENT_TYPE.to_owned(),
            json!({"target":target,"emoji":emoji}),
            0,
        )
    };
    let encoded_payload = payload.to_string();
    let existing =
        sqlx::query("SELECT target_id,sender_id,payload FROM message_reactions WHERE id=?")
            .bind(request.id.to_string())
            .fetch_optional(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    if let Some(existing) = existing {
        if existing.get::<String, _>("target_id") != target.to_string()
            || existing.get::<String, _>("sender_id") != user.to_string()
            || existing.get::<String, _>("payload") != encoded_payload
        {
            return Err(ApiError::conflict(
                "reaction_changed",
                "Reaction identifier already used",
            ));
        }
    } else {
        sqlx::query("INSERT INTO message_reactions(id,target_id,sender_id,created_at,content_type,payload,encryption_version) VALUES (?,?,?,?,?,?,?)")
            .bind(request.id.to_string()).bind(target.to_string()).bind(user.to_string()).bind(Utc::now().to_rfc3339()).bind(kind).bind(encoded_payload).bind(version).execute(&mut *tx).await.map_err(ApiError::internal)?;
    }
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .emit("reactions_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn unreact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let user = super::authenticate_headers(&state, &headers).await?;
    sqlx::query("DELETE FROM message_reactions WHERE id=? AND sender_id=?")
        .bind(id.to_string())
        .bind(user.to_string())
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    state
        .emit("reactions_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn load_reactions(
    pool: &sqlx::SqlitePool,
    messages: &[Message],
    user: Uuid,
) -> Result<Vec<MessageReaction>, ApiError> {
    if messages.is_empty() {
        return Ok(Vec::new());
    }
    let mut query = sqlx::QueryBuilder::new(
        "SELECT r.*,m.conversation_id,u.display_name FROM message_reactions r JOIN messages m ON m.id=r.target_id JOIN users u ON u.id=r.sender_id WHERE r.target_id IN (",
    );
    let mut ids = query.separated(",");
    for message in messages {
        ids.push_bind(message.id.to_string());
    }
    query.push(") AND (json_extract(r.payload,'$.recipient_ids') IS NULL OR EXISTS(SELECT 1 FROM json_each(r.payload,'$.recipient_ids') recipient WHERE recipient.value=");
    query.push_bind(user.to_string());
    query.push(")) ORDER BY r.created_at,r.id");
    query
        .build()
        .fetch_all(pool)
        .await
        .map_err(ApiError::internal)?
        .iter()
        .map(|row| {
            Ok(MessageReaction {
                target_id: super::parse_uuid(&row.get::<String, _>("target_id"))?,
                message: Message {
                    context: None,
                    id: super::parse_uuid(&row.get::<String, _>("id"))?,
                    conversation_id: row.get("conversation_id"),
                    sender: wisp_protocol::UserSummary {
                        id: super::parse_uuid(&row.get::<String, _>("sender_id"))?,
                        display_name: row.get("display_name"),
                    },
                    created_at: row
                        .get::<String, _>("created_at")
                        .parse()
                        .map_err(ApiError::internal)?,
                    content_type: row.get("content_type"),
                    payload: serde_json::from_str(&row.get::<String, _>("payload"))
                        .map_err(ApiError::internal)?,
                    encryption_version: row.get("encryption_version"),
                    edited_at: None,
                },
            })
        })
        .collect()
}
