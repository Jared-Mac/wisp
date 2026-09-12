# PC activity and mobile notifications

Desktop activity is independent of Wisp focus, visible presence, and voice.
Automatic Away defaults to enabled after **30 minutes**, adjustable from 1 to
1440 minutes in Notifications → Presence and phone alerts. Preferences live in
the user's local `wisp/activity.json`. Disabling visible automatic Away does not
disable activity detection or mobile notification routing.

The Linux daemon uses compositor-wide input idle notifications (Wayland
`ext_idle_notifier_v1` version 2), GNOME's idle monitor, or X11's screen-saver
idle counter. XWayland is never used as a substitute for whole-Wayland activity.
Wayland idle inhibitors do not count as evidence of physical input. Optional
readable Linux joystick devices supplement keyboard/mouse idle detection for
controller-only play, with neutral calibration and a dead zone for drift.
Only current controller calibration/held state exists in process memory; no
key content, pointer coordinates, input history, foreground titles, or screen
content is collected or sent to the server.

The detector checks the user's graphical session lock/activity state and system
sleep state. A suspend notification immediately wakes the reporting task;
resume invalidates old observations. Linux boot time includes time asleep.
An unavailable or unconfirmed detector reports `unknown`, which cannot suppress
phone alerts. The compositor monitor starts conservatively until it observes
input or an idle transition. Polling is every five seconds; unchanged activity
is renewed every thirty seconds. No UI window has to stay open.

## Authenticated contract

All requests use the existing device session bearer token and account origin.
These additive endpoints do not require a protocol-version or schema bump.

`POST /v2/devices/me/activity`:

```json
{
  "instance_id": "process-specific UUID",
  "sequence": 1,
  "state": "active",
  "auto_away": true
}
```

States are `active`, `idle`, `unknown`, and `offline`. The server derives account
and device identity from authentication, rejects extra fields, and caps live
instances at eight per device. Sequence numbers increase within an instance;
late requests cannot replace a newer offline report. Leases expire after 90
seconds without a successfully authenticated renewal. Graceful daemon exit
reports offline; crashes, connectivity loss, and server restart fail toward
mobile alerts. Device revocation removes its leases immediately.

`GET /v2/accounts/me/notification-policy` and POST responses:

```json
{
  "mobile_notifications": false,
  "active_desktops": 1,
  "valid_for_seconds": 89,
  "reason": "desktop_active"
}
```

Any live active desktop suppresses mobile alerts, except when the user's saved
manual presence is Away. Allowed reasons are `desktop_active`, `manual_away`,
and `no_active_desktop`. An allow response has `valid_for_seconds: 0`.
Suppression TTL never exceeds 90 seconds. Measure freshness with monotonic
elapsed time from request start; no client/server clock agreement is needed.

The `notification_policy_changed` event carries these fields plus `user_id`.
Only sessions for that account receive it, including accounts with no server
membership. It is a policy refresh signal, never permission to replay an old
notification. Event and encrypted-message delivery remain unchanged for every
device. Mobile clients fetch fresh policy before a new alert batch, with a short
timeout, and notify on errors, unknown endpoints, malformed responses, or expiry.
Previously suppressed arrivals are consumed, not saved for later notification.

## Visible presence and voice

Automatic Away is an in-memory overlay; it never replaces the saved manual
presence. An active or unknown desktop prevents an automatic Away overlay.
Manual selection cancels the current idle lease's automatic ownership until
activity resumes. Returning from idle reveals the current saved choice rather
than writing an old status back over a newer choice.

Activity endpoints never change room membership or issue media tokens. A fresh
desktop starts without local voice ownership and masks inherited account voice
locally. Passive refreshes and desktop sign-in never leave or adopt a phone's
room. Only explicit voice actions grant local voice intent; unsuccessful joins
restore a previously disconnected local state. Duplicate-identity RTC termination
ends this desktop's local voice/recovery without an account-wide leave.

Current server voice membership and RTC identity are account-based. Simultaneous
independent voice sessions on two devices are not supported; an explicit second
join can transfer the shared RTC identity. There is no device-owned server voice
lease in this change. Clients must not infer cleanup authority from inherited
snapshots or persisted pending-leave flags. Remote/orphaned membership cleanup
requires an explicit user action.

Primary references: [Wayland idle monitor](https://quickshell.org/docs/v0.3.0/types/Quickshell.Wayland/IdleMonitor/)
and the `ext-idle-notify-v1.xml` shipped by `wayland-protocols`.
