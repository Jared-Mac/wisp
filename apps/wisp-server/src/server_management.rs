use super::{
    ApiError, AppState, HeaderMap, Json, Row, State, Utc, Uuid, Value, authenticate_headers,
    ensure_friendship, load_conversation,
};
use axum::extract::Path;
use serde_json::json;
use sqlx::SqlitePool;
use std::collections::BTreeSet;
use wisp_protocol::{
    CreateServerCategoryRequest, CreateServerChannelRequest, RenameServerItemRequest,
    SetServerAdminRequest, UpdateServerChannelRequest, UpdateServerProfileRequest,
    UpdateServerRoomRequest,
};

async fn is_owner(pool: &SqlitePool, user: Uuid) -> Result<bool, ApiError> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_identity WHERE owner_user_id=?)")
        .bind(user.to_string())
        .fetch_one(pool)
        .await
        .map_err(ApiError::internal)
}

pub(super) async fn is_manager(pool: &SqlitePool, user: Uuid) -> Result<bool, ApiError> {
    if is_owner(pool, user).await? {
        return Ok(true);
    }
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_admins WHERE user_id=?)")
        .bind(user.to_string())
        .fetch_one(pool)
        .await
        .map_err(ApiError::internal)
}

pub(super) async fn require_manager(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Uuid, ApiError> {
    let user = authenticate_headers(state, headers).await?;
    if !is_manager(&state.pool, user).await? {
        return Err(ApiError::forbidden(
            "Server owner or administrator required",
        ));
    }
    Ok(user)
}

fn valid_name(name: &str, maximum: usize) -> Result<&str, ApiError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > maximum || name.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "invalid_name",
            format!("Name must contain 1–{maximum} printable characters"),
        ));
    }
    Ok(name)
}

fn unique_name_error(error: sqlx::Error, code: &'static str, message: &'static str) -> ApiError {
    if error
        .as_database_error()
        .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
    {
        ApiError::conflict(code, message)
    } else {
        ApiError::internal(error)
    }
}

async fn ensure_category(pool: &SqlitePool, id: Option<&str>) -> Result<(), ApiError> {
    let Some(id) = id else { return Ok(()) };
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM channel_categories WHERE id=?)")
            .bind(id)
            .fetch_one(pool)
            .await
            .map_err(ApiError::internal)?;
    if !exists {
        return Err(ApiError::bad_request(
            "invalid_category",
            "Category does not exist",
        ));
    }
    Ok(())
}

pub(super) async fn settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let actor = require_manager(&state, &headers).await?;
    let owner = is_owner(&state.pool, actor).await?;
    let name: String = sqlx::query_scalar("SELECT name FROM server_identity WHERE id=1")
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    let members = sqlx::query("SELECT u.id,u.display_name,CASE WHEN si.owner_user_id IS NOT NULL THEN 'owner' WHEN sa.user_id IS NOT NULL THEN 'admin' ELSE 'member' END role FROM users u LEFT JOIN server_identity si ON si.owner_user_id=u.id LEFT JOIN server_admins sa ON sa.user_id=u.id WHERE u.username IS NOT NULL ORDER BY CASE role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 ELSE 2 END,u.display_name COLLATE NOCASE")
        .fetch_all(&state.pool).await.map_err(ApiError::internal)?
        .into_iter().map(|row| json!({"id":row.get::<String,_>("id"),"display_name":row.get::<String,_>("display_name"),"role":row.get::<String,_>("role")})).collect::<Vec<_>>();
    let categories = sqlx::query("SELECT id,name,position FROM channel_categories ORDER BY position,name COLLATE NOCASE")
        .fetch_all(&state.pool).await.map_err(ApiError::internal)?
        .into_iter().map(|row| json!({"id":row.get::<String,_>("id"),"name":row.get::<String,_>("name"),"position":row.get::<i64,_>("position")})).collect::<Vec<_>>();
    let channels = sqlx::query("SELECT sc.conversation_id,c.label,sc.category_id,sc.position FROM server_channels sc JOIN conversations c ON c.id=sc.conversation_id ORDER BY sc.position,c.label COLLATE NOCASE")
        .fetch_all(&state.pool).await.map_err(ApiError::internal)?
        .into_iter().map(|row| json!({"id":row.get::<String,_>("conversation_id"),"name":row.get::<String,_>("label"),"category_id":row.get::<Option<String>,_>("category_id"),"position":row.get::<i64,_>("position")})).collect::<Vec<_>>();
    let rooms = sqlx::query("SELECT c.id,s.name,s.category_id,EXISTS(SELECT 1 FROM hangouts h WHERE h.spot_id=s.id AND h.ended_at IS NULL) active FROM spots s JOIN conversations c ON c.spot_id=s.id ORDER BY s.name COLLATE NOCASE")
        .fetch_all(&state.pool).await.map_err(ApiError::internal)?
        .into_iter().map(|row| json!({"id":row.get::<String,_>("id"),"name":row.get::<String,_>("name"),"category_id":row.get::<Option<String>,_>("category_id"),"active":row.get::<bool,_>("active")})).collect::<Vec<_>>();
    Ok(Json(
        json!({"name":name,"role":if owner {"owner"} else {"admin"},"members":members,"categories":categories,"channels":channels,"rooms":rooms}),
    ))
}

pub(super) async fn update_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateServerProfileRequest>,
) -> Result<Json<Value>, ApiError> {
    require_manager(&state, &headers).await?;
    let name = valid_name(&request.name, 60)?;
    let changed = sqlx::query("UPDATE server_identity SET name=? WHERE id=1")
        .bind(name)
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    if changed.rows_affected() == 0 {
        return Err(ApiError::not_found("Server identity is not configured"));
    }
    state
        .emit("server_settings_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true,"name":name})))
}

pub(super) async fn set_admin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetServerAdminRequest>,
) -> Result<Json<Value>, ApiError> {
    let actor = authenticate_headers(&state, &headers).await?;
    if !is_owner(&state.pool, actor).await? {
        return Err(ApiError::forbidden(
            "Only the server owner can manage server administrators",
        ));
    }
    if is_owner(&state.pool, request.user_id).await? {
        return Err(ApiError::forbidden(
            "Server ownership cannot be changed here",
        ));
    }
    if request.admin {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id=? AND username IS NOT NULL)",
        )
        .bind(request.user_id.to_string())
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::internal)?;
        if !exists {
            return Err(ApiError::not_found("Account does not exist"));
        }
        sqlx::query("INSERT INTO server_admins(user_id,granted_by,granted_at) VALUES (?,?,?) ON CONFLICT(user_id) DO NOTHING")
            .bind(request.user_id.to_string()).bind(actor.to_string()).bind(Utc::now().to_rfc3339()).execute(&state.pool).await.map_err(ApiError::internal)?;
    } else {
        sqlx::query("DELETE FROM server_admins WHERE user_id=?")
            .bind(request.user_id.to_string())
            .execute(&state.pool)
            .await
            .map_err(ApiError::internal)?;
    }
    state
        .emit("server_settings_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn create_category(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateServerCategoryRequest>,
) -> Result<Json<Value>, ApiError> {
    let actor = require_manager(&state, &headers).await?;
    let name = valid_name(&request.name, 60)?;
    let id = Uuid::new_v4().to_string();
    let position: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(position),-1)+1 FROM channel_categories")
            .fetch_one(&state.pool)
            .await
            .map_err(ApiError::internal)?;
    sqlx::query(
        "INSERT INTO channel_categories(id,name,position,created_by,created_at) VALUES (?,?,?,?,?)",
    )
    .bind(&id)
    .bind(name)
    .bind(position)
    .bind(actor.to_string())
    .bind(Utc::now().to_rfc3339())
    .execute(&state.pool)
    .await
    .map_err(|error| {
        unique_name_error(
            error,
            "category_exists",
            "A category already uses that name",
        )
    })?;
    state
        .emit("server_settings_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"id":id,"name":name,"position":position})))
}

pub(super) async fn rename_category(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<RenameServerItemRequest>,
) -> Result<Json<Value>, ApiError> {
    require_manager(&state, &headers).await?;
    let name = valid_name(&request.name, 60)?;
    let changed = sqlx::query("UPDATE channel_categories SET name=? WHERE id=?")
        .bind(name)
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|error| {
            unique_name_error(
                error,
                "category_exists",
                "A category already uses that name",
            )
        })?;
    if changed.rows_affected() == 0 {
        return Err(ApiError::not_found("Category does not exist"));
    }
    state
        .emit("server_settings_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn delete_category(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require_manager(&state, &headers).await?;
    let changed = sqlx::query("DELETE FROM channel_categories WHERE id=?")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    if changed.rows_affected() == 0 {
        return Err(ApiError::not_found("Category does not exist"));
    }
    state
        .emit("server_settings_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn create_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateServerChannelRequest>,
) -> Result<Json<Value>, ApiError> {
    let actor = require_manager(&state, &headers).await?;
    let name = valid_name(&request.name, 80)?;
    ensure_category(&state.pool, request.category_id.as_deref()).await?;
    let members = request.member_ids.into_iter().collect::<BTreeSet<_>>();
    for member in &members {
        ensure_friendship(&state.pool, actor, *member).await?;
    }
    let mut required_identities = members.clone();
    required_identities.insert(actor);
    for member in &required_identities {
        let configured: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM chat_identities WHERE user_id=?)")
                .bind(member.to_string())
                .fetch_one(&state.pool)
                .await
                .map_err(ApiError::internal)?;
        if !configured {
            return Err(ApiError::conflict(
                "missing_identity",
                "Every channel member must set up encrypted chat first",
            ));
        }
    }
    let id = format!("channel:{}", Uuid::new_v4());
    let now = Utc::now().to_rfc3339();
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    sqlx::query("INSERT INTO conversations(id,kind,label,created_at) VALUES (?,'circle',?,?)")
        .bind(&id)
        .bind(name)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    let position: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(position),-1)+1 FROM server_channels")
            .fetch_one(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    sqlx::query("INSERT INTO server_channels(conversation_id,category_id,position,created_by,created_at) VALUES (?,?,?,?,?)")
        .bind(&id).bind(request.category_id).bind(position).bind(actor.to_string()).bind(&now).execute(&mut *tx).await.map_err(ApiError::internal)?;
    sqlx::query("INSERT INTO conversation_members(conversation_id,user_id,joined_at,role) VALUES (?,?,?,'admin')")
        .bind(&id).bind(actor.to_string()).bind(&now).execute(&mut *tx).await.map_err(ApiError::internal)?;
    for member in members {
        if member == actor {
            continue;
        }
        sqlx::query("INSERT INTO conversation_members(conversation_id,user_id,joined_at,role) VALUES (?,?,?,'member')")
            .bind(&id).bind(member.to_string()).bind(&now).execute(&mut *tx).await.map_err(ApiError::internal)?;
    }
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .emit("server_settings_changed", json!({"changed":true}))
        .await;
    Ok(Json(
        serde_json::to_value(load_conversation(&state.pool, actor, &id).await?)
            .map_err(ApiError::internal)?,
    ))
}

pub(super) async fn update_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateServerChannelRequest>,
) -> Result<Json<Value>, ApiError> {
    require_manager(&state, &headers).await?;
    let name = valid_name(&request.name, 80)?;
    ensure_category(&state.pool, request.category_id.as_deref()).await?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    let changed = sqlx::query("UPDATE conversations SET label=? WHERE id=? AND EXISTS(SELECT 1 FROM server_channels WHERE conversation_id=?)")
        .bind(name).bind(&id).bind(&id).execute(&mut *tx).await.map_err(ApiError::internal)?;
    if changed.rows_affected() == 0 {
        return Err(ApiError::not_found("Channel does not exist"));
    }
    sqlx::query("UPDATE server_channels SET category_id=? WHERE conversation_id=?")
        .bind(request.category_id)
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .emit("server_settings_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn delete_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require_manager(&state, &headers).await?;
    let changed = sqlx::query("DELETE FROM conversations WHERE id=? AND EXISTS(SELECT 1 FROM server_channels WHERE conversation_id=?)")
        .bind(&id).bind(&id).execute(&state.pool).await.map_err(ApiError::internal)?;
    if changed.rows_affected() == 0 {
        return Err(ApiError::not_found("Channel does not exist"));
    }
    state
        .emit("server_settings_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn rename_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateServerRoomRequest>,
) -> Result<Json<Value>, ApiError> {
    require_manager(&state, &headers).await?;
    let name = valid_name(&request.name, 60)?;
    ensure_category(&state.pool, request.category_id.as_deref()).await?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    let spot: Option<String> =
        sqlx::query_scalar("SELECT spot_id FROM conversations WHERE id=? AND spot_id IS NOT NULL")
            .bind(&id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    let spot = spot.ok_or_else(|| ApiError::not_found("Room does not exist"))?;
    sqlx::query("UPDATE spots SET name=? WHERE id=?")
        .bind(name)
        .bind(&spot)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            unique_name_error(error, "room_exists", "A room already uses that name")
        })?;
    sqlx::query("UPDATE spots SET category_id=? WHERE id=?")
        .bind(request.category_id)
        .bind(&spot)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query("UPDATE conversations SET label=? WHERE id=?")
        .bind(name)
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query("UPDATE hangouts SET label=? WHERE spot_id=? AND ended_at IS NULL")
        .bind(name)
        .bind(&spot)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .emit("server_settings_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::{
        TEST_MEMBER_A_ID, TEST_MEMBER_B_ID, TEST_OWNER_ID, TEST_ROOM_ID,
        ensure_conversation_member, router,
        tests::test_config,
        text_tests::{request, value},
    };
    use axum::http::StatusCode;

    async fn setup() -> (AppState, axum::Router) {
        let state = AppState::new(test_config()).await.unwrap();
        for (id, username) in [
            (TEST_OWNER_ID, "owner"),
            (TEST_MEMBER_A_ID, "member-a"),
            (TEST_MEMBER_B_ID, "member-b"),
        ] {
            sqlx::query("UPDATE users SET username=?,password_hash='test' WHERE id=?")
                .bind(username)
                .bind(id)
                .execute(&state.pool)
                .await
                .unwrap();
        }
        sqlx::query("INSERT INTO server_identity(id,owner_user_id) VALUES (1,?)")
            .bind(TEST_OWNER_ID)
            .execute(&state.pool)
            .await
            .unwrap();
        for id in [TEST_MEMBER_A_ID, TEST_MEMBER_B_ID] {
            sqlx::query("INSERT INTO chat_identities(user_id,public_identity) VALUES (?,?)")
                .bind(id)
                .bind(format!("test-public-identity-{id}"))
                .execute(&state.pool)
                .await
                .unwrap();
        }
        (state.clone(), router(state))
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn owner_grants_persistent_admin_and_admin_manages_server_content() {
        let (state, app) = setup().await;
        assert_eq!(
            request(
                &app,
                "GET",
                "/v1/server/settings",
                TEST_MEMBER_B_ID,
                json!({})
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/server/admins",
                TEST_OWNER_ID,
                json!({
                    "user_id":TEST_MEMBER_A_ID,"admin":true
                })
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert!(
            is_manager(&state.pool, TEST_MEMBER_A_ID.parse().unwrap())
                .await
                .unwrap()
        );
        let settings = value(
            request(
                &app,
                "GET",
                "/v1/server/settings",
                TEST_MEMBER_A_ID,
                json!({}),
            )
            .await,
        )
        .await;
        assert_eq!(settings["role"], "admin");
        assert_eq!(settings["name"], "Wisp server");
        assert_eq!(
            request(
                &app,
                "PATCH",
                "/v1/server/settings",
                TEST_MEMBER_A_ID,
                json!({"name":"Northstar"}),
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            state
                .snapshot(TEST_OWNER_ID.parse().unwrap())
                .await
                .unwrap()
                .server_name,
            "Northstar"
        );

        let category = value(
            request(
                &app,
                "POST",
                "/v1/server/categories",
                TEST_MEMBER_A_ID,
                json!({
                    "name":"Projects"
                }),
            )
            .await,
        )
        .await;
        let category_id = category["id"].as_str().unwrap();
        let channel = value(
            request(
                &app,
                "POST",
                "/v1/server/channels",
                TEST_MEMBER_A_ID,
                json!({
                    "name":"Build notes","category_id":category_id,"member_ids":[TEST_MEMBER_B_ID]
                }),
            )
            .await,
        )
        .await;
        assert_eq!(channel["server_channel"], true);
        assert_eq!(channel["category_name"], "Projects");
        let channel_id = channel["id"].as_str().unwrap();
        assert!(
            ensure_conversation_member(&state.pool, channel_id, TEST_MEMBER_B_ID.parse().unwrap())
                .await
                .is_ok()
        );

        let room_id = format!("spot:{TEST_ROOM_ID}");
        assert_eq!(
            request(
                &app,
                "PATCH",
                &format!("/v1/server/rooms/{room_id}"),
                TEST_MEMBER_A_ID,
                json!({
                    "name":"Renamed room","category_id":category_id
                })
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT name FROM spots WHERE id=?")
                .bind(TEST_ROOM_ID)
                .fetch_one(&state.pool)
                .await
                .unwrap(),
            "Renamed room"
        );
        let categorized = load_conversation(&state.pool, TEST_OWNER_ID.parse().unwrap(), &room_id)
            .await
            .unwrap();
        assert_eq!(categorized.category_name.as_deref(), Some("Projects"));
        sqlx::query("INSERT INTO hangouts(id,livekit_room,label,created_at,ended_at,spot_id) VALUES (?,?,?,?,?,?)")
            .bind(Uuid::new_v4().to_string())
            .bind(format!("ended-{}", Uuid::new_v4()))
            .bind("Renamed room")
            .bind(Utc::now().to_rfc3339())
            .bind(Utc::now().to_rfc3339())
            .bind(TEST_ROOM_ID)
            .execute(&state.pool)
            .await
            .unwrap();
        assert_eq!(
            request(
                &app,
                "DELETE",
                &format!("/v1/server/rooms/{room_id}"),
                TEST_MEMBER_A_ID,
                json!({})
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM spots WHERE id=?")
                .bind(TEST_ROOM_ID)
                .fetch_one(&state.pool)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            request(
                &app,
                "DELETE",
                &format!("/v1/server/channels/{channel_id}"),
                TEST_MEMBER_A_ID,
                json!({})
            )
            .await
            .status(),
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn admins_cannot_manage_admins_or_change_owner() {
        let (_, app) = setup().await;
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/server/admins",
                TEST_OWNER_ID,
                json!({
                    "user_id":TEST_MEMBER_A_ID,"admin":true
                })
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/server/admins",
                TEST_MEMBER_A_ID,
                json!({
                    "user_id":TEST_MEMBER_B_ID,"admin":true
                })
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/server/admins",
                TEST_OWNER_ID,
                json!({
                    "user_id":TEST_OWNER_ID,"admin":false
                })
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
}

pub(super) async fn delete_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require_manager(&state, &headers).await?;
    let spot: Option<String> =
        sqlx::query_scalar("SELECT spot_id FROM conversations WHERE id=? AND spot_id IS NOT NULL")
            .bind(&id)
            .fetch_optional(&state.pool)
            .await
            .map_err(ApiError::internal)?;
    let spot = spot.ok_or_else(|| ApiError::not_found("Room does not exist"))?;
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM hangouts WHERE spot_id=? AND ended_at IS NULL)",
    )
    .bind(&spot)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    if active {
        return Err(ApiError::conflict(
            "room_active",
            "Everyone must leave the room before it can be deleted",
        ));
    }
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    // The original spot foreign key predates cascade deletion. Once everyone
    // has left, remove its ended media-session records before deleting the
    // durable room; spot deletion then cascades through room chat/history.
    sqlx::query("DELETE FROM hangouts WHERE spot_id=?")
        .bind(&spot)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query("DELETE FROM spots WHERE id=?")
        .bind(spot)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .emit("server_settings_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}
