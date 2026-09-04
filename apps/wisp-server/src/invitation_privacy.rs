//! A pending invitation carries an owner's pre-signed membership transition.
//! It is applied only on explicit acceptance, in the same transaction as joining.
use super::{ApiError, Uuid};
use sqlx::Row;
use wisp_crypto::roster::{Role, SignedRoster};
use wisp_protocol::InviteToRoom;

pub(super) async fn membership_offer(
    db: &mut sqlx::SqliteConnection,
    network: Uuid,
    actor: Uuid,
    request: &InviteToRoom,
) -> Result<Option<String>, ApiError> {
    let conversation: String = sqlx::query_scalar("SELECT c.id FROM hangouts h JOIN conversations c ON (c.hangout_id=h.id OR c.spot_id=h.spot_id) WHERE h.id=?")
        .bind(request.hangout_id.to_string()).fetch_one(&mut *db).await.map_err(ApiError::internal)?;
    let current: Option<String> = sqlx::query_scalar("SELECT signed_roster FROM chat_rosters WHERE conversation_id=? ORDER BY revision DESC LIMIT 1")
        .bind(&conversation).fetch_optional(&mut *db).await.map_err(ApiError::internal)?;
    let Some(current) = current else {
        if request.encrypted_membership.is_some() {
            return Err(ApiError::conflict(
                "missing_roster",
                "Initialize encrypted membership before inviting",
            ));
        }
        return Ok(None);
    };
    let previous: SignedRoster = serde_json::from_str(&current).map_err(ApiError::internal)?;
    if previous.roster.members.contains_key(&request.user_id) {
        return Ok(None);
    }
    let signed: SignedRoster =
        serde_json::from_value(request.encrypted_membership.clone().ok_or_else(|| {
            ApiError::conflict(
                "signed_membership_required",
                "Update Wisp to send an encrypted room invitation",
            )
        })?)
        .map_err(|_| ApiError::bad_request("invalid_roster", "Invalid invitation membership"))?;
    let roster = &signed.roster;
    let adds_only_recipient = roster.members.len() == previous.roster.members.len() + 1
        && previous
            .roster
            .members
            .iter()
            .all(|(id, member)| roster.members.get(id) == Some(member))
        && roster
            .members
            .get(&request.user_id)
            .is_some_and(|member| member.role == Role::Member);
    if roster.network != network
        || roster.conversation != conversation
        || roster.actor != actor
        || !adds_only_recipient
    {
        return Err(ApiError::forbidden(
            "Voice invitation may only admit its recipient",
        ));
    }
    signed
        .verify_successor(&previous)
        .map_err(|_| ApiError::conflict("room_changed", "Room changed; send a new invitation"))?;
    let row = sqlx::query("SELECT public_identity FROM chat_identities WHERE user_id=?")
        .bind(request.user_id.to_string())
        .fetch_optional(&mut *db)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| {
            ApiError::conflict(
                "missing_identity",
                "Your friend must enable encryption first",
            )
        })?;
    let key: wisp_crypto::PublicIdentity =
        serde_json::from_str(&row.get::<String, _>("public_identity"))
            .map_err(ApiError::internal)?;
    if roster.members[&request.user_id].identity != key {
        return Err(ApiError::conflict(
            "identity_changed",
            "Friend's encryption identity changed",
        ));
    }
    Ok(Some(
        serde_json::to_string(&signed).map_err(ApiError::internal)?,
    ))
}
