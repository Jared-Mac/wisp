# Wisp

Wisp is a Quickshell-native social layer for a small group of friends. This
repository implements presence, ephemeral hangouts, SQLite persistence, pushed
events, persistent desktop state, LiveKit voice/screen/camera media, a
CLI/simulator, detachable native video surfaces, a standalone desktop window,
and an optional Omarchy bar integration. Milestones M0–M4 are implemented.

## Run it

```bash
./scripts/bootstrap.sh
just dev
```

In another terminal:

```bash
just app
just sim MemberA
cargo run -p wispctl -- status
cargo run -p wispctl -- join MemberA
```

`just app` syncs and opens the portable, resizable Quickshell application.
`just panel` syncs and opens its compact anchored presentation.
To install it as the named `wisp` Quickshell configuration, application
launcher, and `wisp-ui` command:

```bash
just app-sync
wisp
wisp-ui app open
wisp-ui app toggle
wisp-ui panel toggle
```

The app uses a single column at narrow sizes and a two-column layout when more
space is available. The standalone process stays connected to `wispd` when
both surfaces are hidden, so reopening is immediate and voice state remains
owned by the daemon. The legacy `wisp-ui open` and `wisp-ui toggle` forms remain
aliases for the full app.

Use `wisp` for a normal terminal launch. It starts the complete saved client or
reveals the already-running Wisp app. The searchable **Wisp** application menu
entry calls the same launcher, so both routes enforce one daemon, one tray icon,
and one UI process. The client runs as the per-user `wisp.service`, so the
terminal command returns after opening the app; `systemctl --user status
wisp.service` shows its state.

Launching **Wisp** from the desktop application menu starts the saved friend
client when necessary, or reopens the existing app when the daemon is already
running. After **Exit Wisp**, the same launcher starts the complete client again
rather than opening a disconnected UI. If the remote host is temporarily
offline, the daemon and tray remain available and retry until the host returns.
Diagnostic output is available with `journalctl --user -u wisp.service`; Wisp
does not maintain an unbounded private log file.

`wispd` also publishes a Wisp system-tray icon on desktops that support
StatusNotifierItem. Left-click the icon to toggle the Wisp panel beside the
tray. Right-click for panel Show/Hide, **Open Wisp app**, mute, deafen,
panel-anchor, and **Exit Wisp** actions. The panel also has an **Open app**
button. Muted and deafened states update the tray icon, tooltip, checked menu
items, and compact icons in the panel. Microphone mute is orange; deafen is red
and always forces microphone mute. Clicking the headset again leaves the
microphone muted; clicking the microphone while deafened clears both states.
The current-call area below the room list places **[d/c]** beside the room and
connection status, with sharing, camera, and invitations underneath. Participant
names and speaking/mute status remain visible below each room name.
The panel always opens on the operating system's primary display and remembers
its corner choice under **Settings → Desktop position**. Auto uses the tray
click's edge when the tray is on the primary display and otherwise falls back
to the primary display's bottom-right corner.

The default **Clear** audio preset publishes the microphone through the
DeepFilterNet3 neural denoiser (48 kHz, 10 ms frames, 30 ms algorithmic
latency) and transmits its output continuously. Its live 0–100 strength slider
maps to DeepFilterNet's 0–100 dB attenuation limit; zero bypasses noise
reduction and 100 allows full suppression. There is no secondary speech
detector or post-denoise gate. **Natural** keeps WebRTC echo cancellation and
lighter noise suppression, while **Studio** leaves the signal unprocessed. If
DeepFilterNet cannot initialize, Wisp falls back to RNNoise and reports that
backend under **Settings → Audio** and in `wispctl status`. The same view
reports processing time, deadline misses, and capture-queue depth for live
diagnosis.

During a hangout, **Share screen** opens the standard XDG desktop portal picker
for a monitor or individual window, while **Camera on** publishes the selected
camera as an independent track. Screen and camera can run together, and each
remote track gets its own **Watch** control and detachable Hyprland-managed
surface. Receiving video never opens a window or starts decoding until the
viewer asks to watch it. Closing a surface unmaps its native window and pauses
that track without interrupting voice; resizing selects an appropriate simulcast
layer.

Publishing defaults to H.264 at the High profile (screen up to 1080p60 at
8 Mbps; camera up to 720p30 at 2.5 Mbps). Settings also expose Balanced and
Ultra profiles plus VP8 and AV1. Wisp asks LiveKit for a detected hardware
encoder when one exists and reports the selected backend instead of assuming
GPU support. The tray and Omarchy center-bar icon show matching, independent
screen/camera badges. The Omarchy icon also keeps unread-message and
remote-video indicators visible while a call is active; stopping a share
revokes its portal session and unpublishes the track.

**Exit Wisp** closes the Quickshell UI and the tray-owning daemon. When Wisp
was started with `just dev`, that daemon exit also makes the development
supervisor stop its local server and LiveKit children, leaving no Wisp
background processes behind.

## Optional Omarchy integration

Omarchy users can additionally install the center-bar adapter:

```bash
just plugin-sync
omarchy-shell shell toggle dev.wisp
```

The Omarchy adapter and generic tray popup use the same compact `WispContent`
presentation; the adapter simply supplies Omarchy's native anchored popup host.
The adapter uses the newer numbered-section, bracketed-action, prompt-composer,
and live-status treatment while retaining Omarchy's active colors, font, scale,
corners, and popup geometry. It remains compact and does not inherit the
standalone window's selected palette.
Both offer **Open app** for the separate resizable layout. The adapter and the
standalone configuration have independent pushed connections to `wispd`; call
state remains daemon-owned. The adapter is not required on CachyOS or other
desktops.

## Accounts and public servers

Wisp connects directly to a self-hosted HTTPS server; clients do not need a VPN
or provider account. A fresh server has no built-in users or rooms. Its one-time
bootstrap creates the server owner account. Everyone else creates an account
from an encoded friend or room invitation, or signs in to an existing account.
Launching Wisp without saved account credentials opens the sign-in window.
Restarting clears stale voice-room membership before initializing media; it never
automatically rejoins a previous call. Failed media joins stop until an explicit
leave/rejoin instead of retrying on every chat or presence update.
Server events and voice signaling use bounded IPv6/IPv4 fallback, so an
unreachable address family cannot prevent using the reachable one. Failed
connections report their stage without exposing authentication tokens.
Opening the Omarchy popup also starts this account-aware flow when its daemon
is not running; its **Open app** action uses the same launcher. An already-running
client is left alone when opening the popup, including while its server is offline.
Choose **[create account]** there to enter an invitation and register, or
**[new server]** to bootstrap a server you own. Closing the prompt exits Wisp;
it does not reopen repeatedly. Saved credentials still sign in automatically.

Passwords are stored by the server only as Argon2id hashes. After a successful
login, Wisp saves a revocable device credential in
`~/.config/wisp/account.env` with user-only permissions and logs in automatically
on future starts. The account password is not saved locally. Chat recovery keys
and media keys remain client-side and are separate from account authentication.

Chat encryption sets itself up when an authenticated account connects. Wisp
creates and saves private keys on the device, then registers only the public
identity with the server; no setup button or backup file picker is required.
Chat stays blocked if setup fails. **Settings → Privacy → Back up recovery
file…** lets you save a separate recovery copy. Keep that copy somewhere safe
away from the server. A second or reinstalled device must restore the account's
existing recovery file; Wisp never silently replaces an existing identity.

**Settings → Profile** manages the selected server account's display name and
password. Display-name changes are signed by the account's existing encryption
identity; updated clients verify them before replacing a remembered name. Leave
voice on that account's devices before renaming. The sign-in username stays the
same. Password changes require the current password and keep existing device
credentials signed in; revoke unwanted devices under **Settings → Devices**.
Two-factor authentication is planned and is not available yet.

Right-click an active or empty saved room for **[room settings]** above the local
participant-volume sliders. Use the separate button beside the server selector for **[settings]**
(**[stngs]** when space is tight). It stays visible for server owners and
administrators and never overlaps the dropdown. Both the server name and dropdown arrow open the server picker.

The desktop daemon also maintains a private `~/.config/wisp/accounts.json`
registry. Each accepted invite or login adds or updates one independently
hosted server account; credentials are never shared between servers. The
active-server selector above Rooms changes the Rooms, Channels, Friends, and
administration context, while already-open chat tiles from other servers stay
open. Joining voice on another server stops old camera/screen publication
before switching the single active media context.
Server owners and administrators can assign a human-facing server name in
Server Settings. Selectors, chat pickers, and tile paths use that name instead
of exposing the connection hostname.

See [`docs/private-host-setup.md`](docs/private-host-setup.md) for Ubuntu server,
TLS, firewall, LiveKit, privacy, backup, and validation guidance.

## Messages, rooms, and devices

The app includes direct messages, the small-circle timeline, persistent Spot
chats, and the current ad-hoc room timeline. Messages are committed to SQLite
before acknowledgement, missed messages synchronize after reconnect, and
unread cursors are per user. Ad-hoc room messages expire after 24 hours.
Persistent rooms keep durable chat while their voice/video sessions exist only
while occupied. Any account can create a room; administration comes from the server role.
The server owner can grant persistent server-admin access from **Settings →
Server**. Server admins can rename and remove inactive rooms and organize
dedicated encrypted text channels into categories. Only the owner can grant or
revoke server admins; ownership, host credentials, secrets, service shutdown,
and VPS controls are deliberately not exposed through Wisp.

Friends can open a direct conversation from a friend row. Equivalent CLI
operations are useful for diagnostics:

```bash
wispctl dm MemberA "Are you free?"
wispctl status
wispctl devices
wispctl revoke-device <device-id>
```

The full app has a large, resizable tiled chat workspace. Switching chats, hiding
the window, or restarting Wisp does not close a conversation. In the main window,
**Chat options → Close tile** removes only the local tile; it never closes the
conversation on the server or deletes history. The tray's explicit **Close
conversation** action hides its tab without deleting messages. A new incoming DM
or opening that friend's DM restores it; the chat picker also lists closed chats.
**Clear Chat History…** is a
separate confirmed action. In DMs it clears your view first; messages and attached
files cleared by both participants are then removed from active server storage.
Newer messages not yet cleared by both remain. Server owners/admins clear room and channel history
for everyone, with an irreversible-action warning, red **Yes, clear**, and normal
**Cancel**. Other members can clear only their own view. These preferences are
saved per user on the host. Clearing overrides file Keep flags; it does not erase
existing backups or copies already saved outside Wisp.

**Settings → Appearance** selects **Performative**, **Clean TUI**, **Herdr**,
**Terminal Grid**, or **Classic** (the original appearance). All interface styles have exactly the same features,
chats, permissions, controls, and shortcuts. The choice applies immediately to
both the tray popup and full app, is saved locally in
`~/.config/wisp/appearance.json`, and survives restarts. Switching themes does
not reset drafts, change rooms, or activate media. Omarchy retains its existing
host-controlled appearance in the embedded bar adapter, while the standalone
resizable window remains Wisp-owned and user-selectable. Direct
launches with unknown integration metadata conservatively retain Classic unless
a theme has been explicitly chosen. Old `terminal-experimental` preferences
continue to work as Terminal.

Clean TUI is the restrained, chat-forward terminal option: it narrows the
Rooms/Friends activity rail, replaces colorful full-pane boxes with quiet rules,
uses neutral inactive chrome and subtle selections, and simplifies transcript
names and timestamps. It keeps chat tiling, pop-outs, the searchable chat picker,
keyboard controls, prompts, and the live status line. Interface style and color
palette are independent, so Clean TUI can use Ash & Olive, Solarized Japan, Wisp blue, or any other
palette without changing room, message, or media state.

**Performative** is the default for new desktop/CachyOS installations; saved
appearance choices, host-managed Omarchy adapters, and unknown environments are
preserved. A standalone window opened from Omarchy uses this same Wisp-owned
default and can be changed independently without restyling the bar popup.
Choose **Wisp blue**, **Graphite**, **Violet**, **Ember**, **Ash & Olive** (formerly
Performative colors), or **Solarized Japan** (formerly Herdr colors) independently
of the appearance. Palettes never change typography or geometry. Six independent
toggles control chat borders/rules, chat headings, room sections, friends sections,
online friend names, and message sender names. Clean TUI can use colored chat
rules while rooms/friends remain neutral. Color identity assignments survive
turning accents off and on. See [appearance and palettes](docs/appearance-and-palettes.md).

Presence choices, microphone mute, and deafen stay pinned in the header in
both windows, including outside a room and while scrolling. The current-call controls
sit directly below the room list, including while browsing another server. Its label and invitations always refer to the server hosting your call. Click the **Friends** header
to collapse/expand the tray list. Use the star beside a friend to favorite them.
Order is online favorites, offline favorites, online non-favorites, then offline
non-favorites, alphabetically within each group. Favorites apply in both windows;
favorites and tray collapse state are saved per account on this device in
`~/.config/wisp/friends.json` and are independent of the theme.

Click the top-left waveform/Wisp/profile area to open the account menu in either
window. **Settings** and **New Room** live here. A **[home]** action appears in the
header while in Settings, immediately left of **Open app** in the tray, and is
also the first account-menu item on non-home pages. It returns to chats without
closing the window or resetting drafts. Friend rows are compact, with the favorite star immediately after the
name. Stars appear only while hovering the friend row or focusing the star with
the keyboard, without shifting the name. Add Friend is deferred until the
friend-invitation workflow is agreed.

Friend access statuses use compact, theme-colored icons in both presentations:
open door (Open), bell (Knock), padlock (Closed), and moon (Away), with hover labels
and accessible names. A filled green connection dot means online; a hollow dot
means offline, without consuming name space with an additional text label.

We call the compact system-tray surface the **Tray popup**, and the separate
Open App surface the **Main window**. Right-clicking the system tray icon offers
**Open App** as its first action.

Only the Main window has resizable sections: drag the dividers between activity
and chat, or rooms and friends. Double-click a
divider to reset it; keyboard-focus a divider and use arrow keys for fine changes.
The **Layout** menu moves activity left/right/above/below chat or resets the layout.
Narrow windows stack activity and chat automatically. Preferences persist locally
in `~/.config/wisp/workspace.json`; switching layouts never changes conversations,
drafts, rooms, or media. A single arrow beside **Open** in the main header
collapses Activity completely, leaving no unused rail; expanding restores its previous size.
This collapsed state also persists. Chat uses a single compact tab/menu toolbar,
and the one-line composer grows with wrapped text up to a comfortable limit,
then scrolls. Shortening or sending a draft shrinks it again. The Tray popup
retains its single-column flow and existing composer height. Both composers place
the Send control inside the right edge (an up-arrow, or `[send]` in Performative), reserve text space around it, and
omit the keyboard hint (Enter sends; Shift+Enter still inserts a line break).
In the Tray popup, Rooms and Friends can be collapsed independently. The message
history fills the remaining vertical space and grows when either list is hidden;
the composer keeps its usual height. Room collapse is remembered locally and does
not leave the room or change the Main window's room list.

The Main window puts identity, access/audio controls, and window actions on one
row when space permits, wrapping at narrow sizes without hiding controls.

Use **+ add chat** in the main window's top bar, left of mute/deafen, to choose an existing conversation or
**New chat** and open it in a new tile, leaving the current tile intact. Wide tiles
split right; narrow tiles split below. The Activity toggle sits beside **Open** in
the main window header and never reserves an empty rail beside the chat.
**Shift+M** toggles mute and **Shift+D** toggles deafen when not typing in an
editor. Plain M/D no longer toggle audio; uppercase letters still type normally.
The chat pane's **⠿** handle opens **Split right**, **Split below**, and **Pop out
chat**. Up to eight independent panes can display different conversations at once.
Drag shared dividers to resize. Drag a pane's handle onto another pane: the center
swaps chats; an edge creates a split there, with a highlighted drop preview. This
preview shows the resulting layout after freeing the dragged pane's old slot.
The workspace's outer edges move a pane above/below/beside the entire remaining
group, including when dragging over its own old area. Top and bottom targets use
the nearest edge so tall panes remain easy to rearrange. Unchanged placements say
**Already here** and preserve existing divider sizes. Every main-window chat pane
and pop-out uses a current-conversation dropdown, with a searchable, scrollable
list grouped by server, then room category, dedicated channel category, and
friends/groups. Use the All/Rooms/Channels/Friends filters to jump between kinds.
Unread counts and closed conversations remain visible. Type to filter names;
Up/Down selects a result, Enter opens it, and Escape dismisses the picker.
The picker's **New chat** action opens a friend selector: start/reopen a DM, or
name a group and choose 2–31 friends (including offline friends). The creator is
included automatically. Group chats use the existing group-text wire type and
do not create/join a voice room. The creator can clear group history for everyone
through the existing explicit confirmation. Creation retries reuse an idempotency
token, so a lost response cannot create duplicate groups.

Custom group creation requires the server's new `POST /v1/conversations/group`
endpoint. No database migration or new conversation wire enum is needed. Older
servers return a clear update-required error and keep the form's selections;
existing DM creation still works. Updating the local client does not deploy the
remote server.

Group owners can add or remove friends from **Chat options → Group members**.
Encrypted membership changes publish a signed successor
roster, so added members cannot decrypt old history and removed members receive
no future message keys. A group remains hosted by exactly one server.
This follows the local Tile Flow split-tree interaction without changing KWin settings.
At very small window sizes, the workspace scrolls rather than crushing editors.
**Close pane** removes only that tile, never its conversation, draft, or history.
The split tree and chat destinations survive restart in the local workspace file.

Pop-outs are separate normal desktop windows, suitable for the desktop's own
tiling system. Click the **anchor icon** to return to the main window; closing the
pop-out does the same. Remaining docked panes expand while a chat is detached,
and reattaching restores its slot. Drafts, attachments, and the existing chat view
move with it. Pop-outs return to their saved main-window slots after app restart.
Notification sounds also stay quiet while a chat pop-out has focus.

A small down-arrow button appears at the lower-right of a chat feed when scrolled
above the latest messages. Click it to return to the bottom and resume following
new messages; it disappears at the bottom or when the chat fits without scrolling.
This local scrolling control works in the tray, main tiles, and pop-outs.

Camera On (including the C shortcut) opens a local-only preview with the destination
room and **Start Sharing Camera** / **Cancel**. Previewing uses Qt Multimedia
(`qt6-multimedia` on Arch-based systems) without an audio input or recorder. No
video is published until explicitly confirmed; missing/failed preview prevents
sharing. Closing the dialog/window or changing room/camera cancels it. The daemon
rechecks the confirmed room and camera while starting capture. Camera Off remains
immediate. Run `bash scripts/test-camera-confirmation.sh` for hardware-free checks.

Confirmed settings saves briefly show **Changes Saved** for 2.5 seconds. Activity
collapse, divider resizing, docking, and chat tiling save silently. Failed
saves retain their error feedback instead. The `[home]` action has no tooltip.

Wisp-managed windows, menus, dialogs, and local video previews use a shared thin
border for clear separation. Borders follow the selected color palette; Omarchy
continues using its host-provided frames.

Use **[+] beside Rooms** to create a private room managed by the server administrators. Saved rooms keep
their name and list position whether empty or occupied. Click a room to open
its chat; **[join]** on the room row or **Join voice** in the chat header
explicitly starts voice. Rooms are listed once, separately from dedicated text
channels. Temporary friend calls appear under **Calls** beside Friends. Room
headings show occupancy as **#name /2**, with participants in compact wrapping
rows below. Room context menus include **Open chat in new pane**. The chat header
places the alternate **Join voice** beside its options menu; DMs offer **Call**,
respecting the peer’s Open/Knock/Closed presence.

**Invite to room** grants persistent chat/voice access; **Invite** sends
a voice invitation from your current call.

Room and channel clicks open new chat tiles by default. **Settings →
Notifications & Chat → Open channels and rooms in new tiles** can switch this
to reusing a channel or room-chat pane. DMs and private group chats stay in place;
without a channel pane, another tile opens. Already-open chats are focused without
duplicates.

The account menu also provides New Room. **Chat options → Room settings…** lets
server owners/admins invite friends. The server owner grants or revokes server
administration in **Settings → Server**; there are no separate room admins.
Private membership still controls access to encrypted chat and voice. Creating or
being invited to a room never automatically joins a voice/video call.

Both the full app and tray chat accept **Ctrl+V** screenshot pastes and local
file/image drops onto the conversation or composer. Attachments stay in a
removable preview strip until **Send** and may include a caption. Multiple files
send in order; a failed upload retains the unsent files without resending those
already acknowledged. **Enter** sends; **Shift+Enter** adds a line break.
While a send is pending, the composer is read-only and ignores repeated Enter
presses. Success clears the submitted draft before sending is enabled again;
failure keeps it available for editing and retry.
Inline image previews have a 12 MiB / 32 megapixel safety limit. PNG, JPEG, GIF,
and WebP drops within those limits are converted to a still PNG (animated formats
use their first frame); larger dropped images are sent as ordinary files.
Files have no application size cap: uploads resume in 4 MiB chunks, and uploads
and downloads stream without buffering the entire file. Available disk space
and the host/proxy configuration still constrain transfers. The legacy endpoint
for old clients retains its 25 MiB request limit. Up to eight attachments can be staged across
all conversations. Folders, symlinks, and web URLs are not imported.
Only conversation members can download attachments. Chat images never enlarge
beyond their native pixel size: previews shrink uniformly only to fit the feed's
width and height. Clicking opens a separate Wisp image window at **100%**, with
scrollbars for oversized originals and an optional **Fit** view. Preview sizing
does not resize the uploaded image. The viewer's copy icon places the original
image pixels on the clipboard, even in Fit mode, and briefly changes to a checkmark
after success. **Save file** downloads a generic attachment into a new
folder under Downloads/Wisp without opening it or giving it executable permissions.
Explicitly saved files remain yours even if the chat is later cleared/deleted.
Clipboard support includes Wayland data-control and X11/XWayland; nothing uploads
until Send. Drafts survive tab/window navigation, not a complete app restart.

Automatic file retention uses decimal GB, measured from completed upload:
files up to 1 GB have no automatic expiry; files over 1 GB through 5 GB expire
after 48 hours; files over 5 GB expire after 24 hours. **Keep on server** before
sending, or **Keep file** afterward, exempts a file from automatic expiry. Any
member with access to that message can change this flag. Removing Keep applies
the original upload deadline, so an older file may expire immediately. Expired
messages retain a marker but their server file bytes are removed. Cleanup runs
on startup and every minute, and expired downloads are denied immediately.
Abandoned incomplete uploads are removed after 24 hours of inactivity.
Chunks are stored in SQLite so database backups include the files. Deletion
releases database pages for reuse; it does not necessarily shrink the database
file or remove copies in older backups.

Every message has a **···** (**[:]** in TUI appearances) menu. **Copy** copies
plain text or an attachment caption locally; an uncaptioned image copies its
pixels. Editing and deletion remain available only on your own messages.
Use the menu on your own messages to edit text or an attachment caption, or
confirm **Delete message…** to remove it for everyone.
**Enter** saves an edit; **Shift+Enter** inserts a newline. Edits display a subtle
**edited** label without changing the original timestamp. Deleting an attachment
also removes its stored attachment; deletion does not erase existing backups
or copies someone has already saved outside Wisp.

Unread messages add a count badge to the system tray. Incoming messages play a
short sound only when Wisp is not focused. **Settings → Notifications** controls
mute, volume, and a custom local sound file; these settings live only on this
device in `~/.config/wisp/notifications.json`. Playback uses PipeWire's `pw-play`.
The explicit **Test sound** button works while Settings is focused.

Invite friends into your current call with **Invite** below the room list or the
friend's right-click menu. An invitation card appears in your DM; **Accept & Join
Voice** joins the named room using the recipient's existing mute/deafen settings,
with camera and screen sharing off. Invites expire after five minutes or when the
sender leaves. A distinct sound (customizable in Notifications) and tray alert
make pending invites noticeable. Clicking a saved room opens its text chat
without joining voice.

The configured production VPS receives successful `main` builds automatically
from GitHub, waiting for voice rooms to empty before a server restart. See
[automatic server deployment](docs/automatic-server-deployment.md).

Deploy the updated host and clients together: database migrations 0004–0011 add
per-user conversation preferences, authenticated attachment storage, edit timestamps,
chunked uploads, persistent room membership/roles, and voice invitations. Existing
messages and conversations are preserved. Images, like text, are server-readable
in this alpha; media-call E2EE does not apply to chat attachments.

LiveKit audio and video use its built-in AES-GCM end-to-end encryption whenever
Wisp runs with device authentication. The media key is shared privately and is
never sent to `wisp-server`; the Settings device page reports whether E2EE is
active. Coordination data and messages remain server-readable in this private
alpha, so this is not yet an end-to-end-encrypted messenger.

For an always-on host installed from a release, copy and edit the generated
`~/.config/wisp/server.env`, configure LiveKit, then install the user units:

```bash
./scripts/install-host-services.sh
systemctl --user enable --now wisp-server.service
systemctl --user status wisp-server.service wisp-backup.timer
```

The daily backup timer keeps 14 verified SQLite backups under
`~/.local/share/wisp/server/backups`. Restore only while the server is stopped:

```bash
systemctl --user stop wisp-server.service
wisp-restore ~/.local/share/wisp/server/backups/<backup>.sqlite3
systemctl --user start wisp-server.service
```

To test voice and remote video without microphone feedback, start MemberA with
generated media:

```bash
just sim MemberA --publish-tone --publish-video --publish-camera
cargo run -p wispctl -- join MemberA
cargo run -p wispctl -- status
```

The status output reports the selected microphone/speaker and a growing
`received_audio_frames` counter. Synthetic screen and camera tracks appear as
separate available media. The UI's per-track **Watch** button, or
`wispctl watch MemberA screen_share` / `wispctl watch MemberA camera`, opens a
GPU-rendered native XWayland window with app ID `dev.wisp.surface`. Unwatching
either track hides its window and unsubscribes without leaving the LiveKit
room. `wispctl mute`, `unmute`, `deafen`, `undeafen`, and `leave` operate on the
LiveKit session.

Camera, codec, and publishing quality can also be controlled from the CLI:

```bash
cargo run -p wispctl -- video devices
cargo run -p wispctl -- video camera <device-id>
cargo run -p wispctl -- video quality high
cargo run -p wispctl -- video codec h264
cargo run -p wispctl -- camera on
cargo run -p wispctl -- camera off
```

Audio devices and processing are available in both Quickshell frontends and
from the CLI:

```bash
cargo run -p wispctl -- audio devices
cargo run -p wispctl -- audio input <device-id>
cargo run -p wispctl -- audio output <device-id>
cargo run -p wispctl -- audio preset natural  # or clear / studio
```

Device selection is applied without leaving an active room. If a preferred
headset disappears, Wisp uses an available fallback and restores the headset
when it returns.

Push-to-talk can be enabled under Wisp **Settings → Audio** or from the CLI:

```bash
cargo run -p wispctl -- ptt enable
cargo run -p wispctl -- ptt press
cargo run -p wispctl -- ptt release
cargo run -p wispctl -- ptt disable
```

The daemon automatically closes a press whose release is lost. On
Omarchy/Hyprland, install the CLI once with `just cli-install`, then click
**Set shortcut** in Wisp Settings and press the desired chord. Wisp writes a
small generated `~/.config/hypr/wisp.lua`, imports it from `bindings.lua`, and
reloads and validates Hyprland. **Clear** removes Wisp's binding. Other
compositors can bind the same press/release CLI commands manually.

Friends whose presence is `knock` receive expiring join requests. The recipient
gets Join/Later controls in the Wisp panel; the same flow is available from the
CLI:

```bash
cargo run -p wispctl -- join MemberA
cargo run -p wispctl -- knock <knock-id> join
# or: cargo run -p wispctl -- knock <knock-id> later
```

For unattended development, a simulator can respond automatically with
`just sim MemberA --presence knock --auto-respond-knocks join`.

To bind `Super+H`, copy the single reviewed line from
`infra/local/wisp-bindings.lua` into `~/.config/hypr/bindings.lua`, then run
`hyprctl reload` and `hyprctl configerrors`. The repository does not overwrite
personal Hyprland configuration automatically.

## Verification

```bash
just test
just test-integration
just test-media
just test-reliability
just test-knock
just test-ui
just test-private-alpha
just lint
```

`just test-ui` launches an isolated standalone Quickshell instance and checks
the full app and compact panel lifecycles, daemon status, anchoring, and clean
shutdown. `just bootstrap` installs
the pinned, checksum-verified LiveKit development
binary into `.tools/`, and `just dev` starts it on loopback. `just test-media`
tests one publisher with synthetic screen and camera tracks plus two viewers,
on-demand subscriptions, two simultaneous native surfaces, audio continuity,
surface close/reopen independence, missing-camera recovery, and SFU
reconnection. The same media gate runs headlessly in CI. `just test-reliability` runs
the shorter Voice MVP gate: four
users, repeated leave/rejoin, real Quickshell IPC process restarts, SFU failure
and recovery, clear connection errors, and bounded RSS growth. The cycle count
and memory allowance can be adjusted with `WISP_RELIABILITY_CYCLES` and
`WISP_RSS_GROWTH_LIMIT_KIB`; no one-hour soak is required. See
`docs/architecture.md`. `just test-knock` covers request deduplication, Later,
expiry, acceptance, both offline-user cases, and simulator auto-response.
`just test-private-alpha` covers one-use enrollment, short sessions, scoped
conversation access, offline delivery, unread cursors, TestRoom lifecycle,
retention, restart, online backup/restore, revocation, protocol rejection, and
the no-message-content logging rule.

## Automated builds and releases

GitHub Actions builds Wisp on every push to `main` and every pull request. The
CI workflow checks formatting and Clippy, runs the Rust and headless integration
tests, scans for obvious secrets, and produces a release-mode Linux x86_64
archive. Each successful `main` build also replaces the rolling `main`
pre-release, so enrolled Linux x86_64 clients can update without compiling Rust:

```bash
wisp-update
```

The updater works for every enrolled Linux x86_64 client; it contains no
profile-specific host, token, or media-key settings. It verifies the published
SHA-256 checksum, saves the prior binaries under
`~/.local/state/wisp/backups/`, and restarts an already-running client only when
it is not in a hangout or publishing video. It preserves each device's existing
enrollment and audio state, and clears any room or video state that unexpectedly
returns after restart. Archives from ordinary CI runs are available as workflow
artifacts for 14 days.

Pushing a version tag creates a GitHub release with the archive and its SHA-256
checksum:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The archive contains `wispd`, `wisp-account`, `wispctl`, `wisp-server`, the standalone
Quickshell app, the Omarchy adapter, and the runtime launch scripts. After
installing the desktop runtime dependencies, extract it and run:

```bash
./install.sh
wisp
```

Release installation also backs up and refreshes an already-installed Omarchy
adapter, without enabling a disabled adapter or adding one on other desktops.

The desktop systemd service has a 2 GiB memory pressure threshold, a 4 GiB hard
memory budget, and a 512 MiB swap budget to contain runaway media allocations.
These protect the desktop, not limit file-transfer sizes. Advanced users can
override them with `systemctl --user edit wisp.service`. Direct terminal launches
outside the service do not receive these cgroup limits.

The release job uses GitHub's short-lived repository token. It does not require
or receive deployment credentials, LiveKit secrets, or a personal access token.
