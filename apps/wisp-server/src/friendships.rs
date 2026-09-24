use crate::{
    ApiError, AppState, HeaderMap, Json, Path, Row, State, UserId, Utc, Value, add_friendship,
    authenticate_headers, json,
};

// Membership in this server is public to its authenticated members. Presence,
// private rooms, credentials and account login names are deliberately excluded.
pub(super) async fn people(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?.to_string();
    let rows = sqlx::query(
        "SELECT u.id,u.display_name,u.server_member,CASE WHEN u.id=?1 THEN 'self'
         WHEN EXISTS(SELECT 1 FROM friendships f WHERE
           (f.first_user_id=?1 AND f.second_user_id=u.id) OR
           (f.second_user_id=?1 AND f.first_user_id=u.id)) THEN 'friend'
         WHEN EXISTS(SELECT 1 FROM friend_requests r WHERE r.sender_id=u.id AND r.recipient_id=?1) THEN 'incoming'
         WHEN EXISTS(SELECT 1 FROM friend_requests r WHERE r.sender_id=?1 AND r.recipient_id=u.id) THEN 'outgoing'
         ELSE 'none' END AS relationship
         FROM users u WHERE u.id=?1 OR (u.server_member=1 AND EXISTS(SELECT 1 FROM users actor WHERE actor.id=?1 AND actor.server_member=1)) OR EXISTS(SELECT 1 FROM friendships f WHERE (f.first_user_id=?1 AND f.second_user_id=u.id) OR (f.second_user_id=?1 AND f.first_user_id=u.id)) OR EXISTS(SELECT 1 FROM friend_requests r WHERE (r.sender_id=?1 AND r.recipient_id=u.id) OR (r.recipient_id=?1 AND r.sender_id=u.id)) ORDER BY u.display_name COLLATE NOCASE,u.id",
    ).bind(&user).fetch_all(&state.pool).await.map_err(ApiError::internal)?;
    Ok(Json(json!({"people": rows.into_iter().map(|row| json!({
        "id":row.get::<String,_>("id"),
        "display_name":row.get::<String,_>("display_name"),
        "relationship":row.get::<String,_>("relationship"),
        "server_member":row.get::<bool,_>("server_member")
    })).collect::<Vec<_>>()})))
}

async fn member(state: &AppState, user: UserId, other: UserId) -> Result<(), ApiError> {
    if user == other {
        return Err(ApiError::bad_request(
            "self_request",
            "This is your own account",
        ));
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id=?)")
        .bind(other.to_string())
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    if !exists
        || super::account_membership::blocked(&state.pool, user, other).await?
        || !super::account_membership::can_view_person(&state.pool, user, other).await?
    {
        return Err(ApiError::not_found("Server member not found"));
    }
    Ok(())
}

pub(super) async fn send(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(other): Path<UserId>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    member(&state, user, other).await?;
    super::account_membership::rate(&state, format!("friend-request:{user}"), 60).await?;
    // A single write serializes duplicate/crossed requests and acceptance.
    let result = sqlx::query(
        "INSERT OR IGNORE INTO friend_requests(sender_id,recipient_id,created_at)
        SELECT ?1,?2,?3 WHERE NOT EXISTS(SELECT 1 FROM friendships WHERE
        (first_user_id=?1 AND second_user_id=?2) OR (first_user_id=?2 AND second_user_id=?1)) AND NOT EXISTS(SELECT 1 FROM account_blocks WHERE (user_id=?1 AND blocked_id=?2) OR (user_id=?2 AND blocked_id=?1))",
    )
    .bind(user.to_string())
    .bind(other.to_string())
    .bind(Utc::now().to_rfc3339())
    .execute(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    if result.rows_affected() > 0 {
        state
            .emit("friend_requests_changed", json!({"changed":true}))
            .await;
    }
    people(State(state), headers).await
}

pub(super) async fn accept(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(other): Path<UserId>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    member(&state, user, other).await?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    let removed = sqlx::query("DELETE FROM friend_requests WHERE sender_id=? AND recipient_id=?")
        .bind(other.to_string())
        .bind(user.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?
        .rows_affected();
    if removed == 0 {
        let friends: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM friendships WHERE
            (first_user_id=?1 AND second_user_id=?2) OR (first_user_id=?2 AND second_user_id=?1))",
        )
        .bind(user.to_string())
        .bind(other.to_string())
        .fetch_one(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
        if !friends {
            return Err(ApiError::conflict(
                "request_unavailable",
                "This friend request is no longer available",
            ));
        }
    } else {
        add_friendship(&mut tx, user, other).await?;
    }
    tx.commit().await.map_err(ApiError::internal)?;
    if removed > 0 {
        state
            .emit("friendship_changed", json!({"changed":true}))
            .await;
    }
    people(State(state), headers).await
}

pub(super) async fn dismiss(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(other): Path<UserId>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    member(&state, user, other).await?;
    let result = sqlx::query(
        "DELETE FROM friend_requests WHERE
        (sender_id=?1 AND recipient_id=?2) OR (sender_id=?2 AND recipient_id=?1)",
    )
    .bind(user.to_string())
    .bind(other.to_string())
    .execute(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    if result.rows_affected() > 0 {
        state
            .emit("friend_requests_changed", json!({"changed":true}))
            .await;
    }
    people(State(state), headers).await
}

/// Remove only the authenticated account's friendship edge. Chat history,
/// room membership, trusted identities and blocks have independent lifetimes.
pub(super) async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(other): Path<UserId>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    if user == other {
        return Err(ApiError::bad_request(
            "self_request",
            "This is your own account",
        ));
    }
    // Do not require directory visibility: after removal, an account-only
    // friend may disappear from it. Retries must still succeed without exposing
    // whether arbitrary account IDs exist. The first write serializes accepts.
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    let removed = sqlx::query("DELETE FROM friendships WHERE (first_user_id=?1 AND second_user_id=?2) OR (first_user_id=?2 AND second_user_id=?1)")
        .bind(user.to_string()).bind(other.to_string()).execute(&mut *tx).await.map_err(ApiError::internal)?.rows_affected();
    let requests = sqlx::query("DELETE FROM friend_requests WHERE (sender_id=?1 AND recipient_id=?2) OR (sender_id=?2 AND recipient_id=?1)")
        .bind(user.to_string()).bind(other.to_string()).execute(&mut *tx).await.map_err(ApiError::internal)?.rows_affected();
    tx.commit().await.map_err(ApiError::internal)?;
    if removed > 0 || requests > 0 {
        state
            .emit("friendship_changed", json!({"changed":true}))
            .await;
    }
    people(State(state), headers).await
}
