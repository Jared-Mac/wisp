use super::*;
use crate::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Notify;

#[derive(Default)]
struct Relay {
    mode: AtomicUsize,
    reached: Notify,
    resume: Notify,
}

async fn setup() -> (
    AppState,
    Arc<Relay>,
    tokio::task::JoinHandle<()>,
    Uuid,
    Uuid,
) {
    let relay = Arc::new(Relay::default());
    let handler = relay.clone();
    let app = Router::new().route(
        "/twirp/livekit.RoomService/ListParticipants",
        post(move || {
            let handler = handler.clone();
            async move {
                let mode = handler.mode.load(Ordering::Relaxed);
                if mode == 6 {
                    handler.reached.notify_one();
                    handler.resume.notified().await;
                }
                let (status, body) = match mode {
                    1 => (
                        StatusCode::OK,
                        json!({"participants":[{"identity":TEST_MEMBER_A_ID}]}),
                    ),
                    2 => (
                        StatusCode::SERVICE_UNAVAILABLE,
                        json!({"code":"unavailable"}),
                    ),
                    3 => (
                        StatusCode::NOT_FOUND,
                        json!({"error":"proxy route missing"}),
                    ),
                    4 => (StatusCode::NOT_FOUND, json!({"code":"not_found"})),
                    5 => (StatusCode::OK, json!({"participants":"invalid"})),
                    _ => (StatusCode::OK, json!({})),
                };
                (status, Json(body))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut state = AppState::new(crate::tests::test_config()).await.unwrap();
    Arc::make_mut(&mut state.config).livekit_url = origin;
    let user = TEST_MEMBER_A_ID.parse().unwrap();
    let hangout = Uuid::new_v4();
    sqlx::query("INSERT INTO hangouts(id,livekit_room,created_at) VALUES(?,?,?)")
        .bind(hangout.to_string())
        .bind(format!("wisp-{hangout}"))
        .bind((Utc::now() - ChronoDuration::seconds(60)).to_rfc3339())
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO hangout_members(hangout_id,user_id,joined_at) VALUES(?,?,?)")
        .bind(hangout.to_string())
        .bind(TEST_MEMBER_A_ID)
        .bind((Utc::now() - ChronoDuration::seconds(60)).to_rfc3339())
        .execute(&state.pool)
        .await
        .unwrap();
    (state, relay, server, user, hangout)
}

async fn active(state: &AppState) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM hangout_members WHERE left_at IS NULL")
        .fetch_one(&state.pool)
        .await
        .unwrap()
}

async fn old_disconnect(state: &AppState, user: Uuid) {
    state.set_connected(user, true).await;
    state.set_connected(user, false).await;
    state
        .runtime
        .write()
        .await
        .users
        .get_mut(&user)
        .unwrap()
        .disconnected_at = Some(
        Instant::now()
            .checked_sub(RECONNECT_GRACE + Duration::from_secs(1))
            .unwrap(),
    );
}

#[tokio::test]
async fn grace_media_and_errors_preserve_members_but_confirmed_absence_retires_them() {
    let (state, relay, server, user, hangout) = setup().await;
    state.set_connected(user, true).await;
    state.set_connected(user, false).await;
    reap(&state).await.unwrap();
    assert_eq!(
        active(&state).await,
        1,
        "short outages retain room membership"
    );
    old_disconnect(&state, user).await;
    for mode in [1, 2, 3, 5] {
        relay.mode.store(mode, Ordering::Relaxed);
        reap(&state).await.unwrap();
        assert_eq!(
            active(&state).await,
            1,
            "media presence or untrusted absence must preserve membership"
        );
    }
    let mut events = state.events.subscribe();
    relay.mode.store(0, Ordering::Relaxed);
    reap(&state).await.unwrap();
    assert_eq!(active(&state).await, 0);
    assert_eq!(events.recv().await.unwrap().name, "hangout_changed");
    let ended: Option<String> = sqlx::query_scalar("SELECT ended_at FROM hangouts WHERE id=?")
        .bind(hangout.to_string())
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert!(ended.is_some());
    server.abort();
}

#[tokio::test]
async fn another_device_and_reconnection_invalidate_disconnection_cleanup() {
    let (state, relay, server, user, _) = setup().await;
    let mut events = state.events.subscribe();
    state.set_connected(user, true).await;
    state.set_connected(user, true).await;
    state.set_connected(user, false).await;
    for _ in 0..3 {
        assert_eq!(events.recv().await.unwrap().payload["online"], true);
    }
    reap(&state).await.unwrap();
    assert_eq!(active(&state).await, 1);
    state.set_connected(user, false).await;
    old_disconnect(&state, user).await;
    relay.mode.store(6, Ordering::Relaxed);
    let scanning = {
        let state = state.clone();
        tokio::spawn(async move {
            reap(&state).await.unwrap();
        })
    };
    relay.reached.notified().await;
    state.set_connected(user, true).await;
    state.set_connected(user, false).await;
    relay.resume.notify_one();
    scanning.await.unwrap();
    assert_eq!(
        active(&state).await,
        1,
        "a new disconnect gets its own grace period"
    );
    old_disconnect(&state, user).await;
    relay.mode.store(4, Ordering::Relaxed);
    reap(&state).await.unwrap();
    assert_eq!(
        active(&state).await,
        0,
        "a confirmed missing LiveKit room is absent"
    );
    server.abort();
}

#[tokio::test]
async fn a_new_membership_cannot_be_removed_by_an_older_media_probe() {
    let (state, relay, server, user, hangout) = setup().await;
    old_disconnect(&state, user).await;
    relay.mode.store(6, Ordering::Relaxed);
    let scanning = {
        let state = state.clone();
        tokio::spawn(async move {
            reap(&state).await.unwrap();
        })
    };
    relay.reached.notified().await;
    sqlx::query("UPDATE hangout_members SET joined_at=? WHERE user_id=? AND hangout_id=?")
        .bind((Utc::now() + ChronoDuration::seconds(1)).to_rfc3339())
        .bind(user.to_string())
        .bind(hangout.to_string())
        .execute(&state.pool)
        .await
        .unwrap();
    relay.resume.notify_one();
    scanning.await.unwrap();
    assert_eq!(active(&state).await, 1);
    server.abort();
}

#[tokio::test]
async fn heartbeat_expires_an_unresponsive_socket_but_keeps_a_responsive_client() {
    let state = AppState::new(crate::tests::test_config()).await.unwrap();
    let user = TEST_MEMBER_A_ID.parse().unwrap();
    let handler = state.clone();
    let app = Router::new().route(
        "/",
        get(move |ws: WebSocketUpgrade| {
            let handler = handler.clone();
            async move {
                ws.on_upgrade(move |socket| {
                    event_socket(handler, user, socket, false, Duration::from_millis(40))
                })
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("ws://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let (silent, _) = tokio_tungstenite::connect_async(&address).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if state
                .runtime
                .read()
                .await
                .users
                .get(&user)
                .is_some_and(|u| u.connections == 0 && u.disconnected_at.is_some())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("silent TCP peers must not remain online indefinitely");
    drop(silent);
    let (mut responsive, _) = tokio_tungstenite::connect_async(&address).await.unwrap();
    let alive = tokio::time::timeout(Duration::from_millis(300), async {
        while let Some(message) = responsive.next().await {
            let message = message.unwrap();
            assert!(!message.is_close());
            // The desktop uses this same transport's automatic Pong handling.
            // Reading normally must keep it alive without application changes.
        }
    })
    .await;
    assert!(alive.is_err(), "responsive connection should remain open");
    assert_eq!(state.runtime.read().await.users[&user].connections, 1);
    server.abort();
}
