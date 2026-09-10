use super::{ApiError, AppState, HeaderMap, Json, Path, Row, State, Utc, Uuid, Value};
use axum::response::{IntoResponse, Response};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::json;
use wisp_protocol::soundboard::{MAX_SOUNDS, MAX_WAV_BYTES, valid_name, validate};

pub(super) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    super::authenticate_headers(&state, &headers).await?;
    let rows = sqlx::query("SELECT s.id,s.owner_id,s.name,s.duration_ms,u.display_name FROM soundboard_sounds s JOIN users u ON u.id=s.owner_id ORDER BY s.name")
        .fetch_all(&state.pool).await.map_err(ApiError::internal)?;
    Ok(Json(
        json!({"sounds":rows.iter().map(|r| json!({"id":r.get::<String,_>("id"),"owner_id":r.get::<String,_>("owner_id"),"owner_name":r.get::<String,_>("display_name"),"name":r.get::<String,_>("name"),"duration_ms":r.get::<i64,_>("duration_ms")})).collect::<Vec<_>>(),"limit":MAX_SOUNDS}),
    ))
}
#[derive(Deserialize)]
pub(super) struct Upload {
    name: String,
    data: String,
}
pub(super) async fn upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Upload>,
) -> Result<Json<Value>, ApiError> {
    let owner = super::authenticate_headers(&state, &headers).await?;
    let name = request.name.trim();
    if !valid_name(name) {
        return Err(ApiError::bad_request(
            "invalid_sound_name",
            "Use a name of 1–32 characters",
        ));
    }
    if request.data.len() > MAX_WAV_BYTES.div_ceil(3) * 4 {
        return Err(ApiError::bad_request(
            "sound_too_large",
            "Sounds must be 10 seconds or shorter",
        ));
    }
    let bytes = STANDARD
        .decode(&request.data)
        .map_err(|_| ApiError::bad_request("invalid_sound", "Invalid sound data"))?;
    let duration = validate(&bytes).filter(|d| *d > 0).ok_or_else(|| {
        ApiError::bad_request(
            "invalid_sound",
            "Choose a valid audio clip up to 10 seconds long",
        )
    })?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    // Acquire the write lock before checking capacity; concurrent uploads cannot exceed it.
    sqlx::query("UPDATE users SET id=id WHERE id=?")
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM soundboard_sounds")
        .fetch_one(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    if count >= MAX_SOUNDS {
        return Err(ApiError::conflict(
            "soundboard_full",
            "This server has 64 sounds. Remove a sound before adding another.",
        ));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO soundboard_sounds(id,owner_id,name,wav,duration_ms,created_at) VALUES (?,?,?,?,?,?)")
        .bind(id.to_string()).bind(owner.to_string()).bind(name).bind(bytes).bind(duration).bind(Utc::now().to_rfc3339())
        .execute(&mut *tx).await.map_err(|e| if e.as_database_error().is_some_and(sqlx::error::DatabaseError::is_unique_violation) {ApiError::conflict("sound_name_taken","A sound with that name already exists")} else {ApiError::internal(e)})?;
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .emit("soundboard_changed", json!({"changed":true}))
        .await;
    Ok(Json(
        json!({"id":id,"name":name,"owner_id":owner,"duration_ms":duration}),
    ))
}
pub(super) async fn audio(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    super::authenticate_headers(&state, &headers).await?;
    let wav: Vec<u8> = sqlx::query_scalar("SELECT wav FROM soundboard_sounds WHERE id=?")
        .bind(id.to_string())
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("Sound no longer available"))?;
    Ok((
        [
            ("content-type", "audio/wav"),
            ("cache-control", "private, no-store"),
            ("x-content-type-options", "nosniff"),
        ],
        wav,
    )
        .into_response())
}
pub(super) async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let user = super::authenticate_headers(&state, &headers).await?;
    let count=sqlx::query("DELETE FROM soundboard_sounds WHERE id=? AND (owner_id=? OR EXISTS(SELECT 1 FROM server_identity WHERE owner_user_id=?) OR EXISTS(SELECT 1 FROM server_admins WHERE user_id=?))")
        .bind(id.to_string()).bind(user.to_string()).bind(user.to_string()).bind(user.to_string()).execute(&state.pool).await.map_err(ApiError::internal)?.rows_affected();
    if count == 0 {
        return Err(ApiError::forbidden(
            "Only the uploader or a server administrator can remove this sound",
        ));
    }
    state
        .emit("soundboard_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}
