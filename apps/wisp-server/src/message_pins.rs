use super::{
    ApiError, AppState, HeaderMap, Json, Message, Path, Query, UserId, Utc, Uuid, Value,
    authenticate_headers, ensure_conversation_member, message_from_row, server_conversation,
    server_management,
};
use axum::extract::State;
use serde::Deserialize;
use serde_json::json;

pub(super) async fn visible_message(
    state: &AppState,
    user: UserId,
    id: Uuid,
) -> Result<Message, ApiError> {
    let row = sqlx::query("SELECT m.*, u.display_name FROM messages m JOIN users u ON u.id=m.sender_id JOIN accessible_conversation_members cm ON cm.conversation_id=m.conversation_id WHERE m.id=? AND cm.user_id=? AND m.created_at>COALESCE(cm.history_cleared_at, '') AND (json_extract(m.payload,'$.recipient_ids') IS NULL OR EXISTS(SELECT 1 FROM json_each(m.payload,'$.recipient_ids') recipient WHERE recipient.value=cm.user_id))")
        .bind(id.to_string()).bind(user.to_string()).fetch_optional(&state.pool).await.map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("Message is unavailable"))?;
    message_from_row(&row)
}

pub(super) async fn get_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Message>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    visible_message(&state, user, id).await.map(Json)
}

#[derive(Deserialize)]
pub(super) struct PinsQuery {
    conversation_id: String,
}

async fn can_manage(
    state: &AppState,
    user: UserId,
    conversation_id: &str,
) -> Result<bool, ApiError> {
    ensure_conversation_member(&state.pool, conversation_id, user).await?;
    if server_conversation(&state.pool, conversation_id).await? {
        server_management::is_manager(&state.pool, user).await
    } else {
        Ok(true)
    }
}

pub(super) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PinsQuery>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    let manager = can_manage(&state, user, &query.conversation_id).await?;
    let rows = sqlx::query("SELECT m.*, u.display_name FROM message_pins p JOIN messages m ON m.id=p.message_id JOIN users u ON u.id=m.sender_id JOIN accessible_conversation_members cm ON cm.conversation_id=m.conversation_id WHERE p.conversation_id=? AND cm.user_id=? AND m.created_at>COALESCE(cm.history_cleared_at, '') AND (json_extract(m.payload,'$.recipient_ids') IS NULL OR EXISTS(SELECT 1 FROM json_each(m.payload,'$.recipient_ids') recipient WHERE recipient.value=cm.user_id)) ORDER BY p.pinned_at DESC, p.message_id")
        .bind(&query.conversation_id).bind(user.to_string()).fetch_all(&state.pool).await.map_err(ApiError::internal)?;
    let messages: Vec<Message> = rows
        .iter()
        .map(message_from_row)
        .collect::<Result<_, _>>()?;
    Ok(Json(json!({"messages":messages,"can_manage":manager})))
}

#[derive(Deserialize)]
pub(super) struct PinRequest {
    pinned: bool,
}

pub(super) async fn set(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<PinRequest>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    let message = visible_message(&state, user, id).await?;
    if !can_manage(&state, user, &message.conversation_id).await? {
        return Err(ApiError::forbidden(
            "Only server administrators can manage pins in rooms and channels",
        ));
    }
    if request.pinned {
        sqlx::query("INSERT INTO message_pins(message_id,conversation_id,pinned_by,pinned_at) VALUES (?,?,?,?) ON CONFLICT(message_id) DO NOTHING")
            .bind(id.to_string()).bind(&message.conversation_id).bind(user.to_string()).bind(Utc::now().to_rfc3339())
            .execute(&state.pool).await.map_err(ApiError::internal)?;
    } else {
        sqlx::query("DELETE FROM message_pins WHERE message_id=?")
            .bind(id.to_string())
            .execute(&state.pool)
            .await
            .map_err(ApiError::internal)?;
    }
    state.emit("pins_changed", json!({"changed":true})).await;
    Ok(Json(json!({"pinned":request.pinned})))
}
