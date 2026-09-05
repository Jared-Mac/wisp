//! Public-only directory and signed membership relay. No private keys exist in
//! this module. Clients independently pin keys and verify the complete chain.
use super::{
    ApiError, AppState, HeaderMap, Json, Row, State, Uuid, Value, authenticate_headers,
    ensure_conversation_member,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use wisp_crypto::{
    PublicIdentity,
    roster::{Role, SignedRoster},
};

pub(super) async fn network(state: &AppState) -> Result<Uuid, ApiError> {
    sqlx::query_scalar::<_, String>("SELECT network_id FROM chat_network WHERE id=1")
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::internal)?
        .parse()
        .map_err(ApiError::internal)
}

pub(super) async fn directory(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    let identities=sqlx::query("SELECT ci.user_id,ci.public_identity FROM chat_identities ci WHERE ci.user_id=? OR EXISTS(SELECT 1 FROM friendships f WHERE (f.first_user_id=? AND f.second_user_id=ci.user_id) OR (f.second_user_id=? AND f.first_user_id=ci.user_id))")
        .bind(user.to_string()).bind(user.to_string()).bind(user.to_string()).fetch_all(&state.pool).await.map_err(ApiError::internal)?;
    let mut profiles = BTreeMap::<String, Value>::new();
    let profile_rows = sqlx::query("SELECT p.user_id,p.signed_profile FROM account_profiles p WHERE p.user_id=? OR EXISTS(SELECT 1 FROM friendships f WHERE (f.first_user_id=? AND f.second_user_id=p.user_id) OR (f.second_user_id=? AND f.first_user_id=p.user_id))")
        .bind(user.to_string()).bind(user.to_string()).bind(user.to_string()).fetch_all(&state.pool).await.map_err(ApiError::internal)?;
    for row in profile_rows {
        profiles.insert(
            row.get("user_id"),
            serde_json::from_str(&row.get::<String, _>("signed_profile"))
                .map_err(ApiError::internal)?,
        );
    }
    let mut keys = BTreeMap::<String, Value>::new();
    for row in identities {
        keys.insert(
            row.get("user_id"),
            serde_json::from_str(&row.get::<String, _>("public_identity"))
                .map_err(ApiError::internal)?,
        );
    }
    let rows=sqlx::query("SELECT cr.conversation_id,cr.signed_roster FROM chat_rosters cr JOIN conversation_members cm ON cm.conversation_id=cr.conversation_id AND cm.user_id=? ORDER BY cr.conversation_id,cr.revision")
        .bind(user.to_string()).fetch_all(&state.pool).await.map_err(ApiError::internal)?;
    let mut rosters = BTreeMap::<String, Vec<Value>>::new();
    for row in rows {
        rosters.entry(row.get("conversation_id")).or_default().push(
            serde_json::from_str(&row.get::<String, _>("signed_roster"))
                .map_err(ApiError::internal)?,
        );
    }
    let pending = sqlx::query("SELECT p.conversation_id,p.user_id FROM pending_room_admissions p JOIN conversation_members cm ON cm.conversation_id=p.conversation_id AND cm.user_id=? WHERE cm.role IN ('host','admin') ORDER BY p.created_at")
        .bind(user.to_string())
        .fetch_all(&state.pool)
        .await
        .map_err(ApiError::internal)?
        .into_iter()
        .map(|row| json!({"conversation_id":row.get::<String,_>("conversation_id"),"user_id":row.get::<String,_>("user_id")}))
        .collect::<Vec<_>>();
    Ok(Json(
        json!({"network":network(&state).await?,"required":state.config.require_chat_e2ee,"identities":keys,"profiles":profiles,"rosters":rosters,"pending_admissions":pending}),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PublishIdentity {
    identity: PublicIdentity,
    signature: String,
}

pub(super) async fn publish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PublishIdentity>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    request
        .identity
        .validate()
        .map_err(|_| ApiError::bad_request("invalid_identity", "Invalid public identity"))?;
    let statement = serde_json::to_vec(&(network(&state).await?, user, &request.identity))
        .map_err(ApiError::internal)?;
    request
        .identity
        .verify_statement("wisp-account-key-v1", &statement, &request.signature)
        .map_err(|_| ApiError::bad_request("invalid_identity", "Public identity proof failed"))?;
    let identity = serde_json::to_string(&request.identity).map_err(ApiError::internal)?;
    sqlx::query("INSERT OR IGNORE INTO chat_identities(user_id,public_identity) VALUES (?,?)")
        .bind(user.to_string())
        .bind(&identity)
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    let existing: String =
        sqlx::query_scalar("SELECT public_identity FROM chat_identities WHERE user_id=?")
            .bind(user.to_string())
            .fetch_one(&state.pool)
            .await
            .map_err(ApiError::internal)?;
    if existing != identity {
        return Err(ApiError::conflict(
            "identity_changed",
            "This account already has a different encryption identity; restore its recovery key",
        ));
    }
    state
        .emit("chat_identity_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn update_roster(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SignedRoster>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    let network = network(&state).await?;
    ensure_conversation_member(&state.pool, &request.roster.conversation, user).await?;
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    apply_roster(&mut tx, network, user, &request).await?;
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .emit("chat_roster_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}

#[allow(clippy::too_many_lines)]
pub(super) async fn apply_roster(
    tx: &mut sqlx::SqliteConnection,
    network: Uuid,
    user: Uuid,
    request: &SignedRoster,
) -> Result<(), ApiError> {
    let roster = &request.roster;
    if roster.actor != user || roster.network != network {
        return Err(ApiError::forbidden("Invalid room change identity"));
    }
    // Acquire write lock before checking the predecessor, preventing forks.
    sqlx::query("UPDATE conversations SET label=label WHERE id=?")
        .bind(&roster.conversation)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    let previous:Option<String>=sqlx::query_scalar("SELECT signed_roster FROM chat_rosters WHERE conversation_id=? ORDER BY revision DESC LIMIT 1").bind(&roster.conversation).fetch_optional(&mut *tx).await.map_err(ApiError::internal)?;
    if let Some(previous) = previous {
        let previous: SignedRoster = serde_json::from_str(&previous).map_err(ApiError::internal)?;
        if &previous == request {
            return Ok(());
        }
        let server_room: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM conversations c WHERE c.id=? AND (c.spot_id IS NOT NULL OR EXISTS(SELECT 1 FROM server_channels sc WHERE sc.conversation_id=c.id)))")
            .bind(&roster.conversation).fetch_one(&mut *tx).await.map_err(ApiError::internal)?;
        if server_room {
            for (id, old) in &previous.roster.members {
                if roster
                    .members
                    .get(id)
                    .is_some_and(|new| new.role != old.role)
                {
                    return Err(ApiError::forbidden(
                        "Room roles cannot be changed; use Server settings",
                    ));
                }
            }
            let manager: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM server_identity WHERE owner_user_id=?) OR EXISTS(SELECT 1 FROM server_admins WHERE user_id=?)")
                .bind(user.to_string()).bind(user.to_string()).fetch_one(&mut *tx).await.map_err(ApiError::internal)?;
            if !manager {
                if !previous
                    .roster
                    .members
                    .iter()
                    .all(|(id, member)| roster.members.get(id) == Some(member))
                {
                    return Err(ApiError::forbidden("Server administrator required"));
                }
                for (id, member) in &roster.members {
                    if previous.roster.members.contains_key(id) {
                        continue;
                    }
                    let authorized: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pending_room_admissions p WHERE p.conversation_id=? AND p.user_id=? AND (EXISTS(SELECT 1 FROM server_identity WHERE owner_user_id=p.invited_by) OR EXISTS(SELECT 1 FROM server_admins WHERE user_id=p.invited_by)))")
                        .bind(&roster.conversation).bind(id.to_string()).fetch_one(&mut *tx).await.map_err(ApiError::internal)?;
                    if !authorized || member.role != Role::Member {
                        return Err(ApiError::forbidden(
                            "Server administrator approval required for encrypted access",
                        ));
                    }
                }
            }
            if roster.members.iter().any(|(id, member)| {
                !previous.roster.members.contains_key(id) && member.role != Role::Member
            }) {
                return Err(ApiError::forbidden(
                    "Room roles cannot be granted; use Server settings",
                ));
            }
        }
        request.verify_successor(&previous).map_err(|_| {
            ApiError::conflict(
                "invalid_room_transition",
                "Room membership update is not authorized by its predecessor",
            )
        })?;
    } else {
        request.verify_genesis().map_err(|_| {
            ApiError::bad_request("invalid_room_identity", "Invalid initial room signature")
        })?;
        let members =
            sqlx::query("SELECT user_id,role FROM conversation_members WHERE conversation_id=?")
                .bind(&roster.conversation)
                .fetch_all(&mut *tx)
                .await
                .map_err(ApiError::internal)?;
        if members.len() != roster.members.len() {
            return Err(ApiError::conflict(
                "room_changed",
                "Initial membership does not match",
            ));
        }
        for row in members {
            let id: Uuid = row
                .get::<String, _>("user_id")
                .parse()
                .map_err(ApiError::internal)?;
            let role = row.get::<String, _>("role");
            if roster
                .members
                .get(&id)
                .is_none_or(|m| role_name(&m.role) != role)
            {
                return Err(ApiError::conflict(
                    "room_changed",
                    "Initial membership does not match",
                ));
            }
        }
    }
    for (id, member) in &roster.members {
        let key: Option<String> =
            sqlx::query_scalar("SELECT public_identity FROM chat_identities WHERE user_id=?")
                .bind(id.to_string())
                .fetch_optional(&mut *tx)
                .await
                .map_err(ApiError::internal)?;
        let key: PublicIdentity = serde_json::from_str(&key.ok_or_else(|| {
            ApiError::conflict(
                "missing_identity",
                "Every participant must enable encryption first",
            )
        })?)
        .map_err(ApiError::internal)?;
        if key != member.identity {
            return Err(ApiError::conflict(
                "identity_changed",
                "Participant encryption identity does not match",
            ));
        }
        if *id != user {
            let friend:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM friendships WHERE (first_user_id=? AND second_user_id=?) OR (first_user_id=? AND second_user_id=?))").bind(user.to_string()).bind(id.to_string()).bind(id.to_string()).bind(user.to_string()).fetch_one(&mut *tx).await.map_err(ApiError::internal)?;
            if !friend {
                return Err(ApiError::forbidden("Only friends can be added to a room"));
            }
        }
    }
    // Retain read markers/history state for existing members.
    let current: Vec<String> =
        sqlx::query_scalar("SELECT user_id FROM conversation_members WHERE conversation_id=?")
            .bind(&roster.conversation)
            .fetch_all(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    for id in current {
        if !roster.members.keys().any(|member| member.to_string() == id) {
            sqlx::query("DELETE FROM conversation_members WHERE conversation_id=? AND user_id=?")
                .bind(&roster.conversation)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(ApiError::internal)?;
        }
    }
    for (id, member) in &roster.members {
        sqlx::query("INSERT INTO conversation_members(conversation_id,user_id,role,joined_at,history_cleared_at) VALUES (?,?,?,?,?) ON CONFLICT(conversation_id,user_id) DO UPDATE SET role=excluded.role")
        .bind(&roster.conversation).bind(id.to_string()).bind(role_name(&member.role)).bind(chrono::Utc::now().to_rfc3339()).bind(chrono::Utc::now().to_rfc3339()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    }
    sqlx::query("INSERT INTO chat_rosters(conversation_id,revision,signed_roster) VALUES (?,?,?)")
        .bind(&roster.conversation)
        .bind(i64::try_from(roster.revision).map_err(ApiError::internal)?)
        .bind(serde_json::to_string(&request).map_err(ApiError::internal)?)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    for id in roster.members.keys() {
        sqlx::query("DELETE FROM pending_room_admissions WHERE conversation_id=? AND user_id=?")
            .bind(&roster.conversation)
            .bind(id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(ApiError::internal)?;
    }
    Ok(())
}

fn role_name(role: &Role) -> &'static str {
    match role {
        Role::Host => "host",
        Role::Admin => "admin",
        Role::Member => "member",
    }
}
