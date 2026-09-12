use super::activity::{Activity, Preferences};
use crate::activity_wayland::Monitor;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::atomic::Ordering,
};
use x11rb::{connection::Connection as _, protocol::screensaver::ConnectionExt as _};
use zbus::zvariant::OwnedObjectPath;

pub(super) fn boot_millis() -> u64 {
    let time = rustix::time::clock_gettime(rustix::time::ClockId::Boottime);
    u64::try_from(time.tv_sec).unwrap_or(0).saturating_mul(1000)
        + (u64::try_from(time.tv_nsec).unwrap_or(0) / 1_000_000)
}

#[derive(Default)]
struct PadState {
    axes: HashMap<u8, (i16, i16)>,
    buttons: HashMap<u8, bool>,
    observed_input: bool,
}
impl PadState {
    fn update(&mut self, event: &[u8]) {
        let value = i16::from_ne_bytes([event[4], event[5]]);
        let initialization = event[6] & 0x80 != 0;
        let kind = event[6] & 0x7f;
        let id = event[7];
        if kind == 2 {
            let axis = self.axes.entry(id).or_insert((value, value));
            if !initialization
                && ((i32::from(value) - i32::from(axis.0)).abs() > 8000
                    || (i32::from(axis.1) - i32::from(axis.0)).abs() > 8000)
            {
                self.observed_input = true;
            }
            axis.1 = value;
        } else if kind == 1 {
            self.buttons.insert(id, value != 0);
            if !initialization && value != 0 {
                self.observed_input = true;
            }
        }
    }
    fn held(&self) -> bool {
        self.observed_input
            && (self.buttons.values().any(|pressed| *pressed)
                || self
                    .axes
                    .values()
                    .any(|(base, now)| (i32::from(*now) - i32::from(*base)).abs() > 8000))
    }
}

#[derive(Default)]
struct Gamepads {
    devices: HashMap<PathBuf, (File, PadState)>,
    last_input: Option<u64>,
}
impl Gamepads {
    fn active(&mut self, threshold: u64, now: u64) -> bool {
        if let Ok(entries) = std::fs::read_dir("/dev/input") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if !name
                    .strip_prefix("js")
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                {
                    continue;
                }
                if let std::collections::hash_map::Entry::Vacant(slot) =
                    self.devices.entry(entry.path())
                {
                    // No keyboard/mouse devices, event payload logs, or input history.
                    if let Ok(file) = OpenOptions::new()
                        .read(true)
                        .custom_flags(libc::O_NONBLOCK)
                        .open(slot.key())
                    {
                        slot.insert((file, PadState::default()));
                    }
                }
            }
        }
        self.devices.retain(|path, (file, pad)| {
            if !path.exists() {
                return false;
            }
            let mut bytes = [0_u8; 4096];
            for _ in 0..4 {
                match file.read(&mut bytes) {
                    Ok(0) => return false,
                    Ok(length) => {
                        for event in bytes[..length].chunks_exact(8) {
                            let was = pad.observed_input;
                            pad.observed_input = false;
                            pad.update(event);
                            if pad.observed_input {
                                self.last_input = Some(now);
                            }
                            pad.observed_input |= was;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => return false,
                }
            }
            if pad.held() {
                self.last_input = Some(now);
            }
            true
        });
        self.last_input
            .is_some_and(|last| now.saturating_sub(last) < threshold)
    }
}

pub(super) struct Detector {
    wayland: Monitor,
    system: Option<zbus::Connection>,
    session: Option<zbus::Connection>,
    pads: Gamepads,
    previous: u64,
}
impl Detector {
    pub fn invalidate(&mut self) {
        self.wayland.reset.store(true, Ordering::Release);
        self.pads = Gamepads::default();
    }
    pub async fn new() -> Self {
        Self {
            wayland: Monitor::start(),
            system: zbus::Connection::system().await.ok(),
            session: zbus::Connection::session().await.ok(),
            pads: Gamepads::default(),
            previous: boot_millis(),
        }
    }
    pub async fn sample(&mut self, preferences: &Preferences) -> (Activity, &'static str) {
        let now = boot_millis();
        let interrupted = now.saturating_sub(self.previous) > 20_000;
        self.previous = now;
        if interrupted {
            self.invalidate();
        }
        self.wayland
            .seconds
            .store(u64::from(preferences.idle_minutes) * 60, Ordering::Release);
        let session = self.session_state().await;
        if session.is_none() {
            self.system = None;
        }
        if session == Some(false) {
            return (Activity::Idle, "locked_or_inactive");
        }
        if session.is_none() || interrupted {
            return (Activity::Unknown, "unavailable");
        }
        let threshold = u64::from(preferences.idle_minutes) * 60_000;
        if self.pads.active(threshold, now) {
            return (Activity::Active, "controller");
        }
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            let observation = self.wayland.observation();
            if observation.available {
                if observation.idle {
                    return (Activity::Idle, "wayland");
                }
                if observation.input_seen {
                    return (Activity::Active, "wayland");
                }
                return (Activity::Unknown, "waiting_for_input");
            }
            if let Some(idle) = self.gnome_idle().await {
                return (
                    if idle >= threshold {
                        Activity::Idle
                    } else {
                        Activity::Active
                    },
                    "gnome",
                );
            }
        } else if let Ok((connection, screen)) = x11rb::connect(None)
            && let Some(root) = connection.setup().roots.get(screen)
            && let Ok(request) = connection.screensaver_query_info(root.root)
            && let Ok(reply) = request.reply()
        {
            return (
                if u64::from(reply.ms_since_user_input) >= threshold {
                    Activity::Idle
                } else {
                    Activity::Active
                },
                "x11",
            );
        }
        (Activity::Unknown, "unavailable")
    }
    async fn session_state(&mut self) -> Option<bool> {
        if self.system.is_none() {
            self.system = zbus::Connection::system().await.ok();
        }
        let connection = self.system.as_ref()?;
        let manager = zbus::Proxy::new(
            connection,
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        )
        .await
        .ok()?;
        if manager
            .get_property::<bool>("PreparingForSleep")
            .await
            .ok()?
        {
            return Some(false);
        }
        // Restrict inspection to this Unix user's own graphical sessions.
        let uid = rustix::process::geteuid().as_raw();
        let user: OwnedObjectPath = manager.call("GetUser", &(uid,)).await.ok()?;
        let user = zbus::Proxy::new(
            connection,
            "org.freedesktop.login1",
            user,
            "org.freedesktop.login1.User",
        )
        .await
        .ok()?;
        let sessions: Vec<(String, OwnedObjectPath)> = user.get_property("Sessions").await.ok()?;
        let requested = std::env::var("XDG_SESSION_ID").ok();
        let mut graphical = false;
        for (id, path) in sessions {
            if requested.as_ref().is_some_and(|wanted| wanted != &id) {
                continue;
            }
            let session = zbus::Proxy::new(
                connection,
                "org.freedesktop.login1",
                path,
                "org.freedesktop.login1.Session",
            )
            .await
            .ok()?;
            let kind: String = session.get_property("Type").await.ok()?;
            if kind != "wayland" && kind != "x11" {
                continue;
            }
            graphical = true;
            if session.get_property::<bool>("Active").await.ok()? {
                return Some(!session.get_property::<bool>("LockedHint").await.ok()?);
            }
        }
        graphical.then_some(false)
    }
    async fn gnome_idle(&mut self) -> Option<u64> {
        if self.session.is_none() {
            self.session = zbus::Connection::session().await.ok();
        }
        let monitor = zbus::Proxy::new(
            self.session.as_ref()?,
            "org.gnome.Mutter.IdleMonitor",
            "/org/gnome/Mutter/IdleMonitor/Core",
            "org.gnome.Mutter.IdleMonitor",
        )
        .await
        .ok()?;
        monitor.call("GetIdletime", &()).await.ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "read-only probe of this PC's native desktop session"]
    async fn native_desktop_activity_probe() {
        let mut detector = Detector::new().await;
        for _ in 0..3 {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let sample = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                detector.sample(&Preferences::default()),
            )
            .await
            .unwrap();
            println!(
                "Native activity: {sample:?}; input idle protocol available: {}",
                detector.wayland.observation().available
            );
            assert_ne!(
                sample.1, "unavailable",
                "Native session detection failed on the testing PC"
            );
        }
    }
    fn event(value: i16, kind: u8, id: u8) -> [u8; 8] {
        let v = value.to_ne_bytes();
        [0, 0, 0, 0, v[0], v[1], kind, id]
    }
    #[test]
    fn controller_initial_values_and_small_drift_do_not_count_as_activity() {
        let mut pad = PadState::default();
        pad.update(&event(-32767, 0x82, 2));
        pad.update(&event(0, 0x82, 0));
        assert!(!pad.held());
        pad.update(&event(100, 2, 0));
        assert!(!pad.held());
        pad.update(&event(16000, 2, 0));
        assert!(pad.held());
        pad.update(&event(0, 2, 0));
        assert!(!pad.held());
        pad.update(&event(1, 1, 0));
        assert!(pad.held());
        pad.update(&event(0, 1, 0));
        assert!(!pad.held());
    }
}
