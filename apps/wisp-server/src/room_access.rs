//! Server-wide room discovery is separate from signed encrypted-chat access.
use super::{ApiError, ConversationKind, ConversationView, Row, SqlitePool, UserId, Utc};
use std::collections::BTreeMap;

pub(super) async fn queue_public_admissions(
    db: &mut sqlx::SqliteConnection,
) -> Result<(), ApiError> {
    // Keep signed membership intact. An existing authorized client completes
    // these admissions once the joining account has enrolled its public key.
    sqlx::query("INSERT OR IGNORE INTO pending_room_admissions(conversation_id,user_id,invited_by,created_at,automatic) SELECT c.id,u.id,si.owner_user_id,?,1 FROM conversations c JOIN spots s ON s.id=c.spot_id CROSS JOIN users u CROSS JOIN server_identity si WHERE s.private=0 AND u.username IS NOT NULL AND NOT EXISTS(SELECT 1 FROM accessible_conversation_members cm WHERE cm.conversation_id=c.id AND cm.user_id=u.id)")
        .bind(Utc::now().to_rfc3339()).execute(&mut *db).await.map_err(ApiError::internal)?;
    super::channel_access::queue_admissions(db).await?;
    Ok(())
}

pub(super) async fn ensure_voice_access(
    pool: &SqlitePool,
    spot: &str,
    user: UserId,
) -> Result<(), ApiError> {
    let allowed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM spots s JOIN conversations c ON c.spot_id=s.id WHERE s.id=? AND (s.private=0 OR EXISTS(SELECT 1 FROM accessible_conversation_members cm WHERE cm.conversation_id=c.id AND cm.user_id=?)))")
        .bind(spot).bind(user.to_string()).fetch_one(pool).await.map_err(ApiError::internal)?;
    if !allowed {
        return Err(ApiError::forbidden("This room is invite-only"));
    }
    Ok(())
}

pub(super) async fn preview(
    pool: &SqlitePool,
    conversation: &str,
) -> Result<Option<ConversationView>, ApiError> {
    let row = sqlx::query("SELECT c.id,c.label,s.id spot_id,s.category_id,cc.name category_name FROM conversations c JOIN spots s ON s.id=c.spot_id LEFT JOIN channel_categories cc ON cc.id=s.category_id WHERE c.id=? AND s.private=0")
        .bind(conversation).fetch_optional(pool).await.map_err(ApiError::internal)?;
    Ok(row.map(|row| ConversationView {
        id: row.get("id"),
        kind: ConversationKind::Hangout,
        label: row.get("label"),
        spot_id: Some(row.get("spot_id")),
        members: Vec::new(),
        last_message: None,
        unread_count: 0,
        tab_closed: false,
        history_cleared_at: None,
        can_clear_for_everyone: false,
        self_role: String::new(),
        member_roles: BTreeMap::default(),
        server_channel: false,
        category_id: row.get("category_id"),
        category_name: row.get("category_name"),
        pending_access: true,
    }))
}
