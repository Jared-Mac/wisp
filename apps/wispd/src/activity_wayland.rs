//! Compositor idle notifications; no keyboard or pointer content is read.
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, delegate_noop,
    protocol::{wl_registry, wl_seat},
};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1 as notification, ext_idle_notifier_v1 as notifier,
};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Observation {
    pub available: bool,
    pub idle: bool,
    pub input_seen: bool,
}

pub(super) struct Monitor {
    pub seconds: Arc<AtomicU64>,
    pub reset: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    latest: Arc<Mutex<Observation>>,
}

impl Monitor {
    pub fn start() -> Self {
        let seconds = Arc::new(AtomicU64::new(1800));
        let reset = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let latest = Arc::new(Mutex::new(Observation::default()));
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            let (timeout, invalidate, stopped, result) =
                (seconds.clone(), reset.clone(), stop.clone(), latest.clone());
            std::thread::spawn(move || {
                while !stopped.load(Ordering::Acquire) {
                    let _ = observe(&timeout, &invalidate, &stopped, &result);
                    *result
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) =
                        Observation::default();
                    for _ in 0..5 {
                        if stopped.load(Ordering::Acquire) {
                            return;
                        }
                        std::thread::sleep(Duration::from_secs(1));
                    }
                }
            });
        }
        Self {
            seconds,
            reset,
            stop,
            latest,
        }
    }
    pub fn observation(&self) -> Observation {
        *self
            .latest
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

#[derive(Default)]
struct Seat {
    proxy: Option<wl_seat::WlSeat>,
    timers: Vec<notification::ExtIdleNotificationV1>,
    idle: bool,
    input_seen: bool,
}
#[derive(Default)]
struct State {
    manager: Option<(u32, notifier::ExtIdleNotifierV1)>,
    seats: HashMap<u32, Seat>,
    rebuild: bool,
}
impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                if interface == "ext_idle_notifier_v1" {
                    state.manager = Some((name, registry.bind(name, version.min(2), qh, ())));
                    state.rebuild = true;
                } else if interface == "wl_seat" {
                    state.seats.insert(
                        name,
                        Seat {
                            proxy: Some(registry.bind(name, version.min(7), qh, ())),
                            ..Seat::default()
                        },
                    );
                    state.rebuild = true;
                }
            }
            wl_registry::Event::GlobalRemove { name } => {
                if let Some(seat) = state.seats.remove(&name) {
                    for timer in seat.timers {
                        timer.destroy();
                    }
                    if let Some(proxy) = seat.proxy
                        && proxy.version() >= 5
                    {
                        proxy.release();
                    }
                }
                if state.manager.as_ref().is_some_and(|(id, _)| *id == name) {
                    if let Some((_, manager)) = state.manager.take() {
                        manager.destroy();
                    }
                    state.rebuild = true;
                }
            }
            _ => {}
        }
    }
}
impl Dispatch<notification::ExtIdleNotificationV1, (u32, bool)> for State {
    fn event(
        state: &mut Self,
        _: &notification::ExtIdleNotificationV1,
        event: notification::Event,
        &(id, long): &(u32, bool),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let Some(seat) = state.seats.get_mut(&id) {
            match event {
                notification::Event::Idled if long => seat.idle = true,
                notification::Event::Resumed => {
                    seat.input_seen = true;
                    if long {
                        seat.idle = false;
                    }
                }
                _ => {}
            }
        }
    }
}
delegate_noop!(State: ignore wl_seat::WlSeat);
delegate_noop!(State: ignore notifier::ExtIdleNotifierV1);

fn observe(
    seconds: &AtomicU64,
    reset: &AtomicBool,
    stop: &AtomicBool,
    latest: &Mutex<Observation>,
) -> anyhow::Result<()> {
    let connection = Connection::connect_to_env()?;
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    connection.display().get_registry(&qh, ());
    let mut state = State::default();
    queue.roundtrip(&mut state)?;
    let mut previous = 0;
    while !stop.load(Ordering::Acquire) {
        queue.dispatch_pending(&mut state)?;
        let timeout = seconds.load(Ordering::Acquire).clamp(1, 86400);
        if state.rebuild || previous != timeout || reset.swap(false, Ordering::AcqRel) {
            for (&id, seat) in &mut state.seats {
                for timer in seat.timers.drain(..) {
                    timer.destroy();
                }
                seat.idle = false;
                seat.input_seen = false;
                if let (Some((_, manager)), Some(proxy)) = (&state.manager, &seat.proxy) {
                    // Screen-blanking inhibitors are not proof of user activity.
                    // Version 2 reports real input even while a game inhibits idle.
                    let long = if manager.version() >= 2 {
                        manager.get_input_idle_notification(
                            u32::try_from(timeout * 1000)?,
                            proxy,
                            &qh,
                            (id, true),
                        )
                    } else {
                        manager.get_idle_notification(
                            u32::try_from(timeout * 1000)?,
                            proxy,
                            &qh,
                            (id, true),
                        )
                    };
                    seat.timers.push(long);
                    let short = if manager.version() >= 2 {
                        manager.get_input_idle_notification(1000, proxy, &qh, (id, false))
                    } else {
                        manager.get_idle_notification(1000, proxy, &qh, (id, false))
                    };
                    seat.timers.push(short);
                }
            }
            state.rebuild = false;
            previous = timeout;
        }
        *latest
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Observation {
            available: state
                .manager
                .as_ref()
                .is_some_and(|(_, manager)| manager.version() >= 2)
                && !state.seats.is_empty(),
            idle: !state.seats.is_empty() && state.seats.values().all(|seat| seat.idle),
            input_seen: state.seats.values().any(|seat| seat.input_seen),
        };
        connection.flush()?;
        if let Some(read) = connection.prepare_read() {
            let backend = connection.backend();
            let descriptor = backend.poll_fd();
            let mut fd = [rustix::event::PollFd::new(
                &descriptor,
                rustix::event::PollFlags::IN,
            )];
            match rustix::event::poll(
                &mut fd,
                Some(&rustix::time::Timespec {
                    tv_sec: 1,
                    tv_nsec: 0,
                }),
            ) {
                Ok(ready) if ready > 0 => {
                    read.read()?;
                }
                Err(error) if error != rustix::io::Errno::INTR => return Err(error.into()),
                _ => {}
            }
        }
    }
    Ok(())
}
