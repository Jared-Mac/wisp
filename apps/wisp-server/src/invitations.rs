use super::{
    ApiError, AppState, ChronoDuration, HangoutId, HeaderMap, Json, Path, Row, SqlitePool, State,
    UserId, UserSummary, Utc, Uuid, Value, active_hangout_for_tx, add_member, authenticate_headers,
    end_if_empty, find_or_create_direct_tx, json, leave_active_in_transaction, parse_uuid,
};
use wisp_protocol::{InviteToRoom, RespondRoomInvitation, RoomInvitation};

// Rechecked under the acceptance transaction: invites cannot outlive the sender's
// presence or bypass friendship and persistent-room administration.
async fn authorize(
    db: &mut sqlx::SqliteConnection,
    actor: UserId,
    recipient: UserId,
    hangout: HangoutId,
) -> Result<Option<String>, ApiError> {
    if actor == recipient {
        return Err(ApiError::bad_request(
            "self_invite",
            "Choose a friend to invite",
        ));
    }
    let room = sqlx::query("SELECT h.spot_id FROM hangouts h JOIN hangout_members m ON m.hangout_id = h.id WHERE h.id = ? AND h.ended_at IS NULL AND m.user_id = ? AND m.left_at IS NULL")
        .bind(hangout.to_string()).bind(actor.to_string()).fetch_optional(&mut *db).await.map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::conflict("room_unavailable", "Your friend is no longer in this voice room"))?;
    let friend: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM circle_members a JOIN circle_members b ON b.circle_id = a.circle_id WHERE a.user_id = ? AND b.user_id = ?)")
        .bind(actor.to_string()).bind(recipient.to_string()).fetch_one(&mut *db).await.map_err(ApiError::internal)?;
    if !friend {
        return Err(ApiError::forbidden("Only friends can be invited"));
    }
    if let Some(spot) = room.get::<Option<String>, _>("spot_id") {
        let conversation = format!("spot:{spot}");
        let member: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM conversation_members WHERE conversation_id = ? AND user_id = ?)")
            .bind(&conversation).bind(recipient.to_string()).fetch_one(&mut *db).await.map_err(ApiError::internal)?;
        let role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM conversation_members WHERE conversation_id = ? AND user_id = ?",
        )
        .bind(&conversation)
        .bind(actor.to_string())
        .fetch_optional(&mut *db)
        .await
        .map_err(ApiError::internal)?;
        if role.is_none() || (!member && !matches!(role.as_deref(), Some("host" | "admin"))) {
            return Err(ApiError::forbidden(
                "Only room owners or admins can invite friends without room access",
            ));
        }
        return Ok(Some(conversation));
    }
    Ok(None)
}

pub(super) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<InviteToRoom>,
) -> Result<Json<Value>, ApiError> {
    let actor = authenticate_headers(&state, &headers).await?;
    let now = Utc::now();
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    sqlx::query("DELETE FROM room_invitations WHERE expires_at <= ?")
        .bind(now.to_rfc3339())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    authorize(&mut tx, actor, request.user_id, request.hangout_id).await?;
    if active_hangout_for_tx(&mut tx, request.user_id).await? == Some(request.hangout_id) {
        return Err(ApiError::conflict(
            "already_in_room",
            "Your friend is already in this room",
        ));
    }
    if let Some(id) = sqlx::query_scalar::<_, String>("SELECT id FROM room_invitations WHERE hangout_id = ? AND recipient_id = ? AND status = 'pending'")
        .bind(request.hangout_id.to_string()).bind(request.user_id.to_string()).fetch_optional(&mut *tx).await.map_err(ApiError::internal)? {
        return Ok(Json(json!({"id":id,"already_pending":true})));
    }
    let recent: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM room_invitations WHERE sender_id = ? AND created_at > ?",
    )
    .bind(actor.to_string())
    .bind((now - ChronoDuration::minutes(1)).to_rfc3339())
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::internal)?;
    if recent >= 5 {
        return Err(ApiError::conflict(
            "invite_rate_limit",
            "Please wait before sending more invitations",
        ));
    }
    let id = Uuid::new_v4();
    let conversation = find_or_create_direct_tx(&mut tx, actor, request.user_id).await?;
    let label: Option<String> = sqlx::query_scalar("SELECT label FROM hangouts WHERE id = ?")
        .bind(request.hangout_id.to_string())
        .fetch_one(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    let payload = json!({"invitation_id":id,"hangout_id":request.hangout_id,"room_label":label.unwrap_or_else(|| "Voice room".into()),"expires_at":now + ChronoDuration::minutes(5),"status":"pending"});
    let message_id = Uuid::new_v4();
    sqlx::query("INSERT INTO messages(id, conversation_id, sender_id, created_at, content_type, payload) VALUES (?, ?, ?, ?, 'application/vnd.wisp.room-invitation+json', ?)")
        .bind(message_id.to_string()).bind(&conversation).bind(actor.to_string()).bind(now.to_rfc3339()).bind(payload.to_string()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    sqlx::query("UPDATE conversation_members SET tab_closed = 0 WHERE conversation_id = ?")
        .bind(&conversation)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query("INSERT INTO room_invitations(id, message_id, hangout_id, sender_id, recipient_id, created_at, expires_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(id.to_string()).bind(message_id.to_string()).bind(request.hangout_id.to_string()).bind(actor.to_string()).bind(request.user_id.to_string())
        .bind(now.to_rfc3339()).bind((now + ChronoDuration::minutes(5)).to_rfc3339()).execute(&mut *tx).await.map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    state.emit("room_invited", json!({"changed":true})).await;
    Ok(Json(
        json!({"id":id,"conversation_id":conversation,"already_pending":false}),
    ))
}

pub(super) async fn load(pool: &SqlitePool, user: UserId) -> Result<Vec<RoomInvitation>, ApiError> {
    let rows = sqlx::query("SELECT i.id, i.hangout_id, h.label, i.sender_id, u.display_name, i.expires_at, msg.conversation_id FROM room_invitations i JOIN messages msg ON msg.id = i.message_id JOIN hangouts h ON h.id = i.hangout_id JOIN users u ON u.id = i.sender_id WHERE i.recipient_id = ? AND i.status = 'pending' AND i.expires_at > ? AND h.ended_at IS NULL AND EXISTS(SELECT 1 FROM hangout_members m WHERE m.hangout_id = h.id AND m.user_id = i.sender_id AND m.left_at IS NULL) ORDER BY i.created_at")
        .bind(user.to_string()).bind(Utc::now().to_rfc3339()).fetch_all(pool).await.map_err(ApiError::internal)?;
    rows.iter()
        .map(|row| {
            Ok(RoomInvitation {
                id: parse_uuid(&row.get::<String, _>("id"))?,
                hangout_id: parse_uuid(&row.get::<String, _>("hangout_id"))?,
                conversation_id: row.get("conversation_id"),
                room_label: row
                    .get::<Option<String>, _>("label")
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "Voice room".into()),
                from: UserSummary {
                    id: parse_uuid(&row.get::<String, _>("sender_id"))?,
                    display_name: row.get("display_name"),
                },
                expires_at: row
                    .get::<String, _>("expires_at")
                    .parse()
                    .map_err(ApiError::internal)?,
            })
        })
        .collect()
}

pub(super) async fn respond(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<RespondRoomInvitation>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    let now = Utc::now().to_rfc3339();
    let mut tx = state.pool.begin().await.map_err(ApiError::internal)?;
    sqlx::query("UPDATE room_invitations SET status = status WHERE id = ? AND recipient_id = ?")
        .bind(id.to_string())
        .bind(user.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    let row = sqlx::query("SELECT hangout_id, sender_id, status, message_id FROM room_invitations WHERE id = ? AND recipient_id = ? AND expires_at > ?")
        .bind(id.to_string()).bind(user.to_string()).bind(&now).fetch_optional(&mut *tx).await.map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("Invitation expired or unavailable"))?;
    let hangout = parse_uuid(&row.get::<String, _>("hangout_id"))?;
    let previous: String = row.get("status");
    let wanted = if request.accept {
        "accepted"
    } else {
        "dismissed"
    };
    if previous != "pending" {
        if previous != wanted {
            return Err(ApiError::conflict(
                "invite_handled",
                "Invitation already handled",
            ));
        }
        // A retry must not join again after the recipient leaves.
        return Ok(Json(json!({"already_handled":true})));
    }
    if request.accept {
        if let Some(conversation) = authorize(
            &mut tx,
            parse_uuid(&row.get::<String, _>("sender_id"))?,
            user,
            hangout,
        )
        .await?
        {
            sqlx::query("INSERT INTO conversation_members(conversation_id, user_id, joined_at, role) VALUES (?, ?, ?, 'member') ON CONFLICT(conversation_id, user_id) DO NOTHING")
                .bind(conversation).bind(user.to_string()).bind(&now).execute(&mut *tx).await.map_err(ApiError::internal)?;
        }
        let old = active_hangout_for_tx(&mut tx, user).await?;
        if old != Some(hangout) {
            leave_active_in_transaction(&mut tx, user).await?;
            add_member(&mut tx, hangout, user).await?;
            if let Some(old) = old {
                end_if_empty(&mut tx, old).await?;
            }
        }
    }
    sqlx::query("UPDATE room_invitations SET status = ? WHERE id = ?")
        .bind(wanted)
        .bind(id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    sqlx::query("UPDATE messages SET payload = json_set(payload, '$.status', ?) WHERE id = ?")
        .bind(wanted)
        .bind(row.get::<String, _>("message_id"))
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;
    state
        .emit("room_invitation_responded", json!({"changed":true}))
        .await;
    Ok(Json(
        json!({"hangout_id": if request.accept { Some(hangout) } else { None }}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_config;
    use crate::text_tests::{request, value};
    use crate::{
        CHARLIE_ID, JARED_ID, Router, StatusCode, TYLER_ID, active_hangout_for,
        load_recent_messages, router,
    };

    async fn setup() -> (AppState, Router, Value) {
        let state = AppState::new(test_config()).await.unwrap();
        let app = router(state.clone());
        let room = value(
            request(
                &app,
                "POST",
                "/v1/spots/join",
                JARED_ID,
                json!({"spot_id":"Porch"}),
            )
            .await,
        )
        .await;
        (state, app, room["hangout_id"].clone())
    }
    async fn invite(app: &Router, room: &Value, user: &str) -> Value {
        value(
            request(
                app,
                "POST",
                "/v1/room-invitations",
                JARED_ID,
                json!({"hangout_id":room,"user_id":user}),
            )
            .await,
        )
        .await
    }
    #[tokio::test]
    async fn invitation_is_private_chat_card_and_acceptance_is_explicit_and_idempotent() {
        let (state, app, room) = setup().await;
        let tyler = Uuid::parse_str(TYLER_ID).unwrap();
        let first = invite(&app, &room, TYLER_ID).await;
        let second = invite(&app, &room, TYLER_ID).await;
        assert_eq!(first["id"], second["id"]);
        assert_eq!(second["already_pending"], true);
        assert_eq!(active_hangout_for(&state.pool, tyler).await.unwrap(), None);
        assert_eq!(load(&state.pool, tyler).await.unwrap().len(), 1);
        assert!(
            load(&state.pool, Uuid::parse_str(CHARLIE_ID).unwrap())
                .await
                .unwrap()
                .is_empty()
        );
        let messages = load_recent_messages(&state.pool, tyler).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].content_type,
            "application/vnd.wisp.room-invitation+json"
        );
        assert_eq!(
            messages[0].conversation_id,
            first["conversation_id"].as_str().unwrap()
        );
        let path = format!(
            "/v1/room-invitations/{}/respond",
            first["id"].as_str().unwrap()
        );
        assert_eq!(
            request(&app, "POST", &path, CHARLIE_ID, json!({"accept":true}))
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
        value(request(&app, "POST", &path, TYLER_ID, json!({"accept":true})).await).await;
        assert_eq!(
            active_hangout_for(&state.pool, tyler)
                .await
                .unwrap()
                .unwrap()
                .to_string(),
            room.as_str().unwrap()
        );
        assert!(load(&state.pool, tyler).await.unwrap().is_empty());
        assert_eq!(
            load_recent_messages(&state.pool, tyler).await.unwrap()[0].payload["status"],
            "accepted"
        );
        value(request(&app, "POST", "/v1/hangouts/leave", TYLER_ID, json!({})).await).await;
        assert_eq!(
            value(request(&app, "POST", &path, TYLER_ID, json!({"accept":true})).await).await["already_handled"],
            true
        );
        assert_eq!(active_hangout_for(&state.pool, tyler).await.unwrap(), None);
    }
    #[tokio::test]
    async fn dismissal_expiry_departure_and_edit_are_safe() {
        let (state, app, room) = setup().await;
        let tyler = Uuid::parse_str(TYLER_ID).unwrap();
        let first = invite(&app, &room, TYLER_ID).await;
        let path = format!(
            "/v1/room-invitations/{}/respond",
            first["id"].as_str().unwrap()
        );
        let message = load_recent_messages(&state.pool, tyler)
            .await
            .unwrap()
            .remove(0);
        assert_eq!(
            request(
                &app,
                "PATCH",
                &format!("/v1/messages/{}", message.id),
                JARED_ID,
                json!({"text":"fake room"})
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
        value(request(&app, "POST", &path, TYLER_ID, json!({"accept":false})).await).await;
        assert_eq!(active_hangout_for(&state.pool, tyler).await.unwrap(), None);
        assert_eq!(
            request(&app, "POST", &path, TYLER_ID, json!({"accept":true}))
                .await
                .status(),
            StatusCode::CONFLICT
        );
        let second = invite(&app, &room, TYLER_ID).await;
        sqlx::query("UPDATE room_invitations SET expires_at = '2000-01-01T00:00:00Z' WHERE id = ?")
            .bind(second["id"].as_str().unwrap())
            .execute(&state.pool)
            .await
            .unwrap();
        assert_eq!(
            request(
                &app,
                "POST",
                &format!(
                    "/v1/room-invitations/{}/respond",
                    second["id"].as_str().unwrap()
                ),
                TYLER_ID,
                json!({"accept":true})
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        let third = invite(&app, &room, TYLER_ID).await;
        value(request(&app, "POST", "/v1/hangouts/leave", JARED_ID, json!({})).await).await;
        assert!(load(&state.pool, tyler).await.unwrap().is_empty());
        assert_eq!(
            request(
                &app,
                "POST",
                &format!(
                    "/v1/room-invitations/{}/respond",
                    third["id"].as_str().unwrap()
                ),
                TYLER_ID,
                json!({"accept":true})
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(active_hangout_for(&state.pool, tyler).await.unwrap(), None);
    }
    #[tokio::test]
    async fn nonmembers_require_admin_and_revoked_authority_is_rechecked() {
        let (state, app, room) = setup().await;
        sqlx::query(
            "DELETE FROM conversation_members WHERE user_id = ? AND conversation_id LIKE 'spot:%'",
        )
        .bind(TYLER_ID)
        .execute(&state.pool)
        .await
        .unwrap();
        value(
            request(
                &app,
                "POST",
                "/v1/hangouts/join",
                CHARLIE_ID,
                json!({"hangout_id":room}),
            )
            .await,
        )
        .await;
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/room-invitations",
                CHARLIE_ID,
                json!({"hangout_id":room,"user_id":TYLER_ID})
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        let invitation = invite(&app, &room, TYLER_ID).await;
        sqlx::query("UPDATE conversation_members SET role='member' WHERE user_id=? AND conversation_id LIKE 'spot:%'").bind(JARED_ID).execute(&state.pool).await.unwrap();
        assert_eq!(
            request(
                &app,
                "POST",
                &format!(
                    "/v1/room-invitations/{}/respond",
                    invitation["id"].as_str().unwrap()
                ),
                TYLER_ID,
                json!({"accept":true})
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            active_hangout_for(&state.pool, Uuid::parse_str(TYLER_ID).unwrap())
                .await
                .unwrap(),
            None
        );
    }
}
