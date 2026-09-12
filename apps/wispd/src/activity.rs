use super::{Arc, AtomicU64, Daemon, Duration, Ordering, PathBuf, RwLock, Value, ensure, json};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Activity {
    Active,
    Idle,
    Unknown,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Preferences {
    pub auto_away: bool,
    pub idle_minutes: u16,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            auto_away: true,
            idle_minutes: 30,
        }
    }
}
impl Preferences {
    fn validate(self) -> anyhow::Result<Self> {
        ensure!(
            (1..=1440).contains(&self.idle_minutes),
            "Idle time must be between 1 and 1440 minutes"
        );
        Ok(self)
    }
}

pub(super) struct DesktopActivity {
    preferences: RwLock<Preferences>,
    current: RwLock<(Activity, &'static str)>,
    instance: uuid::Uuid,
    sequence: AtomicU64,
    wake: tokio::sync::Notify,
    sleeping: AtomicBool,
    power_transition: AtomicBool,
}
impl Default for DesktopActivity {
    fn default() -> Self {
        Self {
            preferences: RwLock::new(Preferences::default()),
            current: RwLock::new((Activity::Unknown, "starting")),
            instance: uuid::Uuid::new_v4(),
            sequence: AtomicU64::new(0),
            wake: tokio::sync::Notify::new(),
            sleeping: AtomicBool::new(false),
            power_transition: AtomicBool::new(false),
        }
    }
}
fn path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map_or_else(
            || PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"),
            PathBuf::from,
        )
        .join("wisp/activity.json")
}
impl DesktopActivity {
    async fn load(&self) {
        if let Ok(bytes) = tokio::fs::read(path()).await
            && let Ok(preferences) = serde_json::from_slice::<Preferences>(&bytes)
            && let Ok(preferences) = preferences.validate()
        {
            *self.preferences.write().await = preferences;
        }
    }
    pub async fn status(&self) -> Value {
        let preferences = *self.preferences.read().await;
        let (state, source) = *self.current.read().await;
        json!({"auto_away":preferences.auto_away,"idle_minutes":preferences.idle_minutes,"state":state,"source":source})
    }
    pub async fn configure(&self, args: &Value) -> anyhow::Result<Value> {
        self.configure_at(args, path()).await
    }
    async fn configure_at(&self, args: &Value, path: PathBuf) -> anyhow::Result<Value> {
        let preferences = serde_json::from_value::<Preferences>(args.clone())?.validate()?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .await?;
        drop(file);
        let result = async {
            tokio::fs::write(&temporary, serde_json::to_vec(&preferences)?).await?;
            tokio::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600)).await?;
            tokio::fs::rename(&temporary, path).await?;
            anyhow::Ok(())
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temporary).await;
        }
        result?;
        *self.preferences.write().await = preferences;
        self.wake.notify_one();
        Ok(self.status().await)
    }
}

impl Daemon {
    pub(super) async fn report_activity(&self, activity: Activity) {
        let preferences = *self.activity.preferences.read().await;
        let sequence = self.activity.sequence.fetch_add(1, Ordering::AcqRel) + 1;
        let mut apis = vec![(
            self.api.clone(),
            self.primary_connected.load(Ordering::Acquire),
        )];
        apis.extend(
            self.linked_servers
                .read()
                .await
                .values()
                .map(|server| (server.api.clone(), server.connected.load(Ordering::Acquire))),
        );
        futures_util::future::join_all(apis.into_iter().map(|(api, connected)| {
            let state = if connected || activity == Activity::Offline { activity } else { Activity::Unknown };
            let body = json!({"instance_id":self.activity.instance,"sequence":sequence,"state":state,"auto_away":preferences.auto_away});
            async move {
                // A failed/old server cannot leave a permanent suppression state:
                // leases expire server-side, and Android treats policy failures as notify.
                let _ = api.request(reqwest::Method::POST, "/v2/devices/me/activity")
                    .json(&body).timeout(Duration::from_secs(2)).send().await;
            }
        })).await;
    }
}

pub(super) async fn run(daemon: Arc<Daemon>) {
    daemon.activity.load().await;
    let mut detector = crate::activity_os::Detector::new().await;
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut sent = None;
    let mut last_report = 0;
    loop {
        tokio::select! { _ = interval.tick() => {}, () = daemon.activity.wake.notified() => {} }
        let preferences = *daemon.activity.preferences.read().await;
        let transition = daemon
            .activity
            .power_transition
            .swap(false, Ordering::AcqRel);
        if transition {
            detector.invalidate();
        }
        let (state, source) = if daemon.activity.sleeping.load(Ordering::Acquire) {
            (Activity::Idle, "suspended")
        } else if transition {
            (Activity::Unknown, "resuming")
        } else {
            tokio::time::timeout(Duration::from_secs(2), detector.sample(&preferences))
                .await
                .unwrap_or((Activity::Unknown, "unavailable"))
        };
        let previous = *daemon.activity.current.read().await;
        *daemon.activity.current.write().await = (state, source);
        if previous != (state, source) {
            daemon.emit(
                "desktop_activity_changed",
                daemon.activity.status().await,
                daemon.next_seq(0),
            );
        }
        let now = crate::activity_os::boot_millis();
        if sent != Some((state, preferences)) || now.saturating_sub(last_report) >= 30_000 {
            daemon.report_activity(state).await;
            sent = Some((state, preferences));
            last_report = now;
        }
    }
}

pub(super) async fn watch_sleep(daemon: Arc<Daemon>) {
    loop {
        let _ = receive_power_events(&daemon).await;
        // A lost bus connection may hide the resume signal. Recheck logind
        // instead of leaving a stale sleeping flag latched forever.
        if daemon.activity.sleeping.swap(false, Ordering::AcqRel) {
            daemon
                .activity
                .power_transition
                .store(true, Ordering::Release);
            daemon.activity.wake.notify_one();
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

async fn receive_power_events(daemon: &Daemon) -> zbus::Result<()> {
    let connection = zbus::Connection::system().await?;
    let manager = zbus::Proxy::new(
        &connection,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )
    .await?;
    let mut changes = manager.receive_signal("PrepareForSleep").await?;
    while let Some(message) = changes.next().await {
        if let Ok((sleeping,)) = message.body().deserialize::<(bool,)>() {
            daemon.activity.sleeping.store(sleeping, Ordering::Release);
            daemon
                .activity
                .power_transition
                .store(true, Ordering::Release);
            daemon.activity.wake.notify_one();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn preferences_save_privately_and_reject_invalid_updates_without_replacing_them() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("activity.json");
        let activity = DesktopActivity::default();
        activity
            .configure_at(&json!({"auto_away":false,"idle_minutes":45}), path.clone())
            .await
            .unwrap();
        let saved = tokio::fs::read(&path).await.unwrap();
        let preferences: Preferences = serde_json::from_slice(&saved).unwrap();
        assert!(!preferences.auto_away);
        assert_eq!(preferences.idle_minutes, 45);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(
            activity
                .configure_at(&json!({"auto_away":true,"idle_minutes":0}), path.clone())
                .await
                .is_err()
        );
        assert_eq!(tokio::fs::read(path).await.unwrap(), saved);
        assert_eq!(activity.status().await["idle_minutes"], 45);
    }
    #[test]
    fn visible_away_and_activity_threshold_are_independent_and_bounded() {
        let preferences = Preferences::default();
        assert!(preferences.auto_away);
        assert_eq!(preferences.idle_minutes, 30);
        let disabled =
            serde_json::from_value::<Preferences>(json!({"auto_away":false,"idle_minutes":30}))
                .unwrap()
                .validate()
                .unwrap();
        assert!(!disabled.auto_away);
        assert_eq!(disabled.idle_minutes, 30);
        assert!(
            Preferences {
                idle_minutes: 0,
                ..preferences
            }
            .validate()
            .is_err()
        );
        assert!(
            Preferences {
                idle_minutes: 1441,
                ..preferences
            }
            .validate()
            .is_err()
        );
        assert!(
            serde_json::from_value::<Preferences>(json!({"user_id":"no impersonation"})).is_err()
        );
    }
}
