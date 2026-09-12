//! Short-lived device activity leases route alerts, never message delivery.
use super::*;

const LEASE_SECONDS: u64 = 90;
const MAX_INSTANCES_PER_DEVICE: usize = 8;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Activity {
    Active,
    Idle,
    Unknown,
    Offline,
}

#[derive(Debug, Clone)]
pub(super) struct Lease {
    state: Activity,
    auto_away: bool,
    overridden: bool,
    sequence: u64,
    expires: Instant,
}

pub(super) type Leases = HashMap<(UserId, Uuid, Uuid), Lease>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Heartbeat {
    instance_id: Uuid,
    sequence: u64,
    state: Activity,
    auto_away: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct Policy {
    mobile_notifications: bool,
    active_desktops: usize,
    valid_for_seconds: u64,
    reason: &'static str,
}

fn live(leases: &Leases, user: UserId, now: Instant) -> impl Iterator<Item = &Lease> {
    leases.iter().filter_map(move |((owner, _, _), lease)| {
        (*owner == user && lease.expires > now && lease.state != Activity::Offline).then_some(lease)
    })
}

pub(super) fn effective_presence(
    leases: &Leases,
    user: UserId,
    manual: Presence,
    now: Instant,
) -> Presence {
    let mut idle_owner = false;
    for lease in live(leases, user, now) {
        // An active or unmeasurable desktop must not be shown as automatically away.
        if lease.state != Activity::Idle {
            return manual;
        }
        idle_owner |= lease.auto_away && !lease.overridden;
    }
    if idle_owner { Presence::Away } else { manual }
}

fn policy(leases: &Leases, user: UserId, manual: Presence, now: Instant) -> Policy {
    let active: Vec<_> = live(leases, user, now)
        .filter(|lease| lease.state == Activity::Active)
        .collect();
    let suppress = !active.is_empty() && manual != Presence::Away;
    Policy {
        mobile_notifications: !suppress,
        active_desktops: active.len(),
        // Clients measure elapsed time from request start, without synchronized clocks.
        valid_for_seconds: if suppress {
            active
                .iter()
                .map(|lease| lease.expires.duration_since(now).as_secs())
                .max()
                .unwrap_or(0)
                .min(LEASE_SECONDS)
        } else {
            0
        },
        reason: if manual == Presence::Away {
            "manual_away"
        } else if suppress {
            "desktop_active"
        } else {
            "no_active_desktop"
        },
    }
}

async fn identity(state: &AppState, headers: &HeaderMap) -> Result<(UserId, Uuid), ApiError> {
    let user = authenticate_headers(state, headers).await?;
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError::unauthorized("missing bearer token"))?;
    if state.config.allow_dev_sessions && token.starts_with("dev:") {
        return Ok((user, user));
    }
    let device: String = sqlx::query_scalar("SELECT s.device_id FROM sessions s JOIN devices d ON d.id=s.device_id WHERE s.token_hash=? AND s.expires_at>? AND s.revoked_at IS NULL AND d.revoked_at IS NULL AND d.user_id=?")
        .bind(token_hash(token)).bind(Utc::now().to_rfc3339()).bind(user.to_string())
        .fetch_optional(&state.pool).await.map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::unauthorized("session is invalid or expired"))?;
    Ok((user, parse_uuid(&device)?))
}

async fn manual_presence(state: &AppState, user: UserId) -> Result<Presence, ApiError> {
    let presence: String = sqlx::query_scalar("SELECT presence FROM users WHERE id=?")
        .bind(user.to_string())
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    presence.parse().map_err(ApiError::internal)
}

pub(super) async fn get_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Policy>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    let manual = manual_presence(&state, user).await?;
    Ok(Json(policy(
        &state.runtime.read().await.activity,
        user,
        manual,
        Instant::now(),
    )))
}

pub(super) async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Heartbeat>,
) -> Result<Json<Policy>, ApiError> {
    let (user, device) = identity(&state, &headers).await?;
    manual_presence(&state, user).await?;
    if request.instance_id.is_nil() {
        return Err(ApiError::bad_request(
            "invalid_instance",
            "An activity instance is required",
        ));
    }
    let now = Instant::now();
    let key = (user, device, request.instance_id);
    let mut runtime = state.runtime.write().await;
    // Expired entries cannot authorize suppression; the maintenance task publishes
    // transitions before reclaiming them. Bound allocations even without that task.
    let existing = runtime.activity.get(&key);
    if existing.is_none()
        && runtime
            .activity
            .iter()
            .filter(|((owner, id, _), lease)| {
                *owner == user && *id == device && lease.expires > now
            })
            .count()
            >= MAX_INSTANCES_PER_DEVICE
    {
        return Err(ApiError::bad_request(
            "activity_sessions_full",
            "Too many active device sessions",
        ));
    }
    let obsolete = existing.is_some_and(|old| request.sequence <= old.sequence);
    let changed = !obsolete
        && existing.is_none_or(|old| {
            old.state != request.state || old.auto_away != request.auto_away || old.expires <= now
        });
    if !obsolete {
        // An idle heartbeat cannot reclaim a status explicitly chosen by the user.
        let overridden = existing.is_some_and(|old| {
            old.overridden && old.state == Activity::Idle && request.state == Activity::Idle
        });
        runtime.activity.insert(
            key,
            Lease {
                state: request.state,
                auto_away: request.auto_away,
                overridden,
                sequence: request.sequence,
                expires: now + Duration::from_secs(LEASE_SECONDS),
            },
        );
    }
    drop(runtime);
    if changed {
        state.publish_activity(user).await?;
    }
    get_policy(State(state), headers).await
}

pub(super) fn cancel_auto_away(leases: &mut Leases, user: UserId) {
    for ((owner, _, _), lease) in leases.iter_mut() {
        if *owner == user && lease.state == Activity::Idle {
            lease.overridden = true;
        }
    }
}

pub(super) fn event_visible(event: &ServerEvent, user: UserId) -> bool {
    event.name != "notification_policy_changed"
        || event.payload.get("user_id").and_then(Value::as_str) == Some(user.to_string().as_str())
}

impl AppState {
    pub(super) async fn publish_activity(&self, user: UserId) -> Result<(), ApiError> {
        let manual = manual_presence(self, user).await?;
        let runtime = self.runtime.read().await;
        let now = Instant::now();
        let policy = policy(&runtime.activity, user, manual, now);
        let presence = effective_presence(&runtime.activity, user, manual, now);
        drop(runtime);
        self.emit(
            "presence_changed",
            json!({"user_id":user,"presence":presence}),
        )
        .await;
        let mut payload = serde_json::to_value(policy).map_err(ApiError::internal)?;
        payload["user_id"] = json!(user);
        self.emit("notification_policy_changed", payload).await;
        Ok(())
    }

    pub(super) async fn remove_device_activity(&self, user: UserId, device: Uuid) {
        self.runtime
            .write()
            .await
            .activity
            .retain(|(owner, id, _), _| *owner != user || *id != device);
        let _ = self.publish_activity(user).await;
    }

    pub async fn maintain_device_activity(self) {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            let now = Instant::now();
            let mut users = std::collections::HashSet::new();
            self.runtime
                .write()
                .await
                .activity
                .retain(|(user, _, _), lease| {
                    if lease.expires <= now {
                        users.insert(*user);
                        false
                    } else {
                        true
                    }
                });
            for user in users {
                let _ = self.publish_activity(user).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{chat_headers, test_config};
    use crate::text_tests::{request, value};

    async fn beat(
        state: &AppState,
        id: Uuid,
        sequence: u64,
        activity: Activity,
        auto_away: bool,
    ) -> Policy {
        heartbeat(
            State(state.clone()),
            chat_headers(TEST_OWNER_ID),
            Json(Heartbeat {
                instance_id: id,
                sequence,
                state: activity,
                auto_away,
            }),
        )
        .await
        .unwrap()
        .0
    }
    async fn choose(state: &AppState, presence: Presence) {
        let _ = set_presence(
            State(state.clone()),
            chat_headers(TEST_OWNER_ID),
            Json(SetPresenceRequest { presence }),
        )
        .await
        .unwrap();
    }
    async fn visible(state: &AppState) -> Presence {
        state
            .snapshot(TEST_OWNER_ID.parse().unwrap())
            .await
            .unwrap()
            .self_state
            .presence
    }
    async fn current_policy(state: &AppState) -> Policy {
        get_policy(State(state.clone()), chat_headers(TEST_OWNER_ID))
            .await
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn activity_routes_alerts_independently_of_visible_away_without_changing_voice() {
        let state = AppState::new(test_config()).await.unwrap();
        let user = TEST_OWNER_ID.parse().unwrap();
        let id = Uuid::new_v4();
        choose(&state, Presence::Knock).await;
        assert!(current_policy(&state).await.mobile_notifications);
        let before = state.snapshot(user).await.unwrap();
        let active = beat(&state, id, 1, Activity::Active, false).await;
        assert!(!active.mobile_notifications);
        assert_eq!(active.active_desktops, 1);
        assert!((1..=90).contains(&active.valid_for_seconds));
        assert_eq!(visible(&state).await, Presence::Knock);
        assert!(
            get_policy(State(state.clone()), chat_headers(TEST_MEMBER_A_ID))
                .await
                .unwrap()
                .0
                .mobile_notifications
        );
        assert!(
            beat(&state, id, 2, Activity::Idle, false)
                .await
                .mobile_notifications
        );
        assert_eq!(visible(&state).await, Presence::Knock);
        beat(&state, id, 3, Activity::Idle, true).await;
        assert_eq!(visible(&state).await, Presence::Away);
        let stored: String = sqlx::query_scalar("SELECT presence FROM users WHERE id=?")
            .bind(user.to_string())
            .fetch_one(&state.pool)
            .await
            .unwrap();
        assert_eq!(stored, "knock");
        beat(&state, id, 4, Activity::Active, true).await;
        assert_eq!(visible(&state).await, Presence::Knock);
        let after = state.snapshot(user).await.unwrap();
        assert_eq!(before.self_state.hangout_id, after.self_state.hangout_id);
        assert_eq!(before.hangouts, after.hangouts);
        assert_eq!(before.knocks, after.knocks);
    }

    #[tokio::test]
    async fn manual_choices_cancel_current_idle_ownership_and_old_heartbeats_cannot_revive_close() {
        let state = AppState::new(test_config()).await.unwrap();
        let id = Uuid::new_v4();
        choose(&state, Presence::Knock).await;
        beat(&state, id, 1, Activity::Idle, true).await;
        assert_eq!(visible(&state).await, Presence::Away);
        choose(&state, Presence::Closed).await;
        beat(&state, id, 2, Activity::Idle, true).await;
        assert_eq!(visible(&state).await, Presence::Closed);
        beat(&state, id, 3, Activity::Active, true).await;
        assert_eq!(visible(&state).await, Presence::Closed);
        beat(&state, id, 4, Activity::Idle, true).await;
        assert_eq!(visible(&state).await, Presence::Away);
        choose(&state, Presence::Away).await;
        let policy = beat(&state, id, 5, Activity::Active, true).await;
        assert!(policy.mobile_notifications);
        assert_eq!(policy.reason, "manual_away");
        choose(&state, Presence::Open).await;
        assert!(
            beat(&state, id, 7, Activity::Offline, true)
                .await
                .mobile_notifications
        );
        assert!(
            beat(&state, id, 6, Activity::Active, true)
                .await
                .mobile_notifications
        );
        assert_eq!(visible(&state).await, Presence::Open);
    }

    #[tokio::test]
    async fn multiple_desktops_expiry_and_unknown_detection_fail_toward_notifications() {
        let state = AppState::new(test_config()).await.unwrap();
        let user = TEST_OWNER_ID.parse().unwrap();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        choose(&state, Presence::Knock).await;
        beat(&state, first, 1, Activity::Idle, true).await;
        assert!(
            !beat(&state, second, 1, Activity::Active, true)
                .await
                .mobile_notifications
        );
        assert_eq!(visible(&state).await, Presence::Knock);
        assert!(
            beat(&state, second, 2, Activity::Unknown, true)
                .await
                .mobile_notifications
        );
        assert_eq!(visible(&state).await, Presence::Knock);
        state
            .runtime
            .write()
            .await
            .activity
            .get_mut(&(user, user, second))
            .unwrap()
            .expires = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
        assert_eq!(visible(&state).await, Presence::Away);
        state
            .runtime
            .write()
            .await
            .activity
            .get_mut(&(user, user, first))
            .unwrap()
            .expires = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
        assert!(current_policy(&state).await.mobile_notifications);
        assert_eq!(visible(&state).await, Presence::Knock);
        // A server restart starts without any activity leases.
        let restarted = RuntimeState::default();
        assert!(
            policy(&restarted.activity, user, Presence::Knock, Instant::now()).mobile_notifications
        );
    }

    #[tokio::test]
    async fn authenticated_device_identity_revocation_and_private_events_are_enforced() {
        let state = AppState::new(test_config()).await.unwrap();
        let user = TEST_OWNER_ID.parse().unwrap();
        let mut tx = state.pool.begin().await.unwrap();
        let device = create_device(&mut tx, user, "Test PC").await.unwrap();
        tx.commit().await.unwrap();
        let session = device_session(
            State(state.clone()),
            Json(DeviceSessionRequest {
                device_id: device.device_id,
                device_token: device.device_token,
                protocol_version: PROTOCOL_VERSION,
            }),
        )
        .await
        .unwrap()
        .0;
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            format!("Bearer {}", session.token).parse().unwrap(),
        );
        let instance = Uuid::new_v4();
        let result = heartbeat(
            State(state.clone()),
            headers.clone(),
            Json(Heartbeat {
                instance_id: instance,
                sequence: 1,
                state: Activity::Active,
                auto_away: false,
            }),
        )
        .await
        .unwrap()
        .0;
        assert!(!result.mobile_notifications);
        assert!(state.runtime.read().await.activity.contains_key(&(
            user,
            device.device_id,
            instance
        )));
        let _ = revoke_device(
            State(state.clone()),
            headers.clone(),
            Path(device.device_id.to_string()),
        )
        .await
        .unwrap();
        assert!(current_policy(&state).await.mobile_notifications);
        assert!(
            heartbeat(
                State(state.clone()),
                headers,
                Json(Heartbeat {
                    instance_id: instance,
                    sequence: 2,
                    state: Activity::Active,
                    auto_away: false
                })
            )
            .await
            .is_err()
        );
        let app = router(state.clone());
        let invalid=request(&app,"POST","/v2/devices/me/activity",TEST_OWNER_ID,json!({"instance_id":instance,"sequence":3,"state":"active","auto_away":true,"user_id":TEST_MEMBER_A_ID})).await;
        assert!(!invalid.status().is_success());
        let own = value(
            request(
                &app,
                "GET",
                "/v2/accounts/me/notification-policy",
                TEST_OWNER_ID,
                json!({}),
            )
            .await,
        )
        .await;
        assert_eq!(own["mobile_notifications"], true);
        let event = ServerEvent {
            seq: 1,
            name: "notification_policy_changed".into(),
            occurred_at: Utc::now(),
            payload: json!({"user_id":user,"mobile_notifications":false}),
        };
        assert!(event_visible(&event, user));
        assert!(!event_visible(&event, TEST_MEMBER_A_ID.parse().unwrap()));
        for name in [
            "message_received",
            "message_sent",
            "knock_received",
            "snapshot",
        ] {
            assert!(event_visible(
                &ServerEvent {
                    name: name.into(),
                    ..event.clone()
                },
                TEST_MEMBER_A_ID.parse().unwrap()
            ));
        }
    }
}
