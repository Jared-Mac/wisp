//! A closed control connection is not proof that its voice session has ended.
//! Give reconnects time, then verify absence from `LiveKit` before retiring a row.
use super::{
    ApiError, AppState, Duration, Row, Uuid, Value, WebSocket, WsMessage, end_if_empty,
    member_moderation, parse_uuid,
};
use chrono::Utc;
use futures_util::{SinkExt, stream::SplitSink};
use serde::Deserialize;
use serde_json::json;
use tracing::warn;

const RECONNECT_GRACE: Duration = Duration::from_secs(30);

pub(super) async fn send_frame(
    sender: &mut SplitSink<WebSocket, WsMessage>,
    frame: WsMessage,
) -> bool {
    matches!(
        tokio::time::timeout(Duration::from_secs(5), sender.send(frame)).await,
        Ok(Ok(()))
    )
}

#[derive(Deserialize)]
struct Participants {
    #[serde(default)]
    participants: Vec<Participant>,
}

#[derive(Deserialize)]
struct Participant {
    identity: String,
}

async fn absent_from_media(state: &AppState, room: &str, user: Uuid) -> Result<bool, ApiError> {
    let response =
        member_moderation::room_request(state, room, "ListParticipants", json!({"room":room}))
            .await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        let value: Value = response.json().await.map_err(ApiError::internal)?;
        return Ok(value.get("code").and_then(Value::as_str) == Some("not_found"));
    }
    let participants: Participants = response
        .error_for_status()
        .map_err(ApiError::internal)?
        .json()
        .await
        .map_err(ApiError::internal)?;
    Ok(!participants
        .participants
        .iter()
        .any(|p| p.identity == user.to_string()))
}

async fn reap(state: &AppState) -> Result<(), ApiError> {
    let rows = sqlx::query("SELECT hm.hangout_id,hm.user_id,hm.joined_at,h.livekit_room FROM hangout_members hm JOIN hangouts h ON h.id=hm.hangout_id WHERE hm.left_at IS NULL AND h.ended_at IS NULL")
        .fetch_all(&state.pool).await.map_err(ApiError::internal)?;
    for row in rows {
        let user = parse_uuid(&row.get::<String, _>("user_id"))?;
        let disconnected = state
            .runtime
            .read()
            .await
            .users
            .get(&user)
            .filter(|u| u.connections == 0)
            .and_then(|u| u.disconnected_at);
        let Some(disconnected) = disconnected.filter(|at| at.elapsed() >= RECONNECT_GRACE) else {
            continue;
        };
        let joined: String = row.get("joined_at");
        let joined_at =
            chrono::DateTime::parse_from_rfc3339(&joined).map_err(ApiError::internal)?;
        if (Utc::now() - joined_at.with_timezone(&Utc))
            .to_std()
            .unwrap_or_default()
            < RECONNECT_GRACE
        {
            continue;
        }
        let room: String = row.get("livekit_room");
        // Relay outages and malformed/error replies must never evict a listener.
        if !absent_from_media(state, &room, user).await.unwrap_or(false) {
            continue;
        }
        let hangout = parse_uuid(&row.get::<String, _>("hangout_id"))?;
        // A reconnect, including reconnect-then-disconnect, invalidates this scan.
        // Hold the runtime read lock only through the short local transaction.
        let runtime = state.runtime.read().await;
        if !runtime
            .users
            .get(&user)
            .is_some_and(|u| u.connections == 0 && u.disconnected_at == Some(disconnected))
        {
            continue;
        }
        let mut tx = state
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(ApiError::internal)?;
        let removed = sqlx::query("UPDATE hangout_members SET left_at=? WHERE user_id=? AND hangout_id=? AND joined_at=? AND left_at IS NULL")
            .bind(Utc::now().to_rfc3339()).bind(user.to_string()).bind(hangout.to_string()).bind(joined)
            .execute(&mut *tx).await.map_err(ApiError::internal)?.rows_affected();
        if removed > 0 {
            end_if_empty(&mut tx, hangout).await?;
        }
        tx.commit().await.map_err(ApiError::internal)?;
        drop(runtime);
        if removed > 0 {
            state
                .emit("hangout_changed", json!({"hangout_id":hangout}))
                .await;
        }
    }
    Ok(())
}

impl AppState {
    pub async fn maintain_voice_presence(self) {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if reap(&self).await.is_err() {
                warn!("Voice presence cleanup deferred");
            }
        }
    }
}

#[cfg(test)]
mod tests;
