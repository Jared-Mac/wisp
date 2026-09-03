# Trusted Tailscale friend test

This mode is for a private alpha with people you trust. It is not a public
deployment. Name-only development login is disabled: each computer enrolls
with a one-use invite, receives its own revocable credential, and uses
short-lived sessions after that.

No Tailscale address or credential belongs in this repository. The host sends
friends its Tailscale DNS name and assigns each person one unique profile
(`Tyler`, `Jack`, or `Charlie`) separately.

## Host

Install and authenticate Tailscale, then start the isolated test mode:

```bash
sudo pacman -S --needed tailscale
sudo systemctl enable --now tailscaled
sudo tailscale up

./scripts/bootstrap.sh
just dev-tailscale
```

`dev-tailscale` derives the current `tailscale0` IPv4 address, generates a
random LiveKit server keypair for that run, creates persistent random bootstrap
and media E2EE secrets in `~/.config/wisp/host.env` (mode `0600`), enrolls the
host's administrator device, and binds coordination/media only to the tailnet
address. Normal `just dev` remains loopback-only.

In another terminal, print the host address:

```bash
just tailscale-info
```

Then create one invite for each assigned friend:

```bash
cargo run -q -p wispctl -- invite Tyler
cargo run -q -p wispctl -- invite Jack
```

Each invite is single-use and expires (30 minutes by default). Send each friend
three values privately: the host DNS name/IP, only their invite code, and the
`WISP_E2EE_KEY` value from `~/.config/wisp/host.env`. Do not post any of those
values in an issue, commit, or chat room you do not trust. Never send the
bootstrap token or the host's device credential.

Keep `just dev-tailscale` running during the test. Invite friends to the
tailnet, or share only this machine with them. If the tailnet uses restrictive
grants, allow the invited testers to reach this host on:

```text
TCP 8787  Wisp coordination and events
TCP 7880  LiveKit signaling
TCP 7881  LiveKit media fallback
UDP 7882  LiveKit primary media
```

Tailscale's default Linux netfilter mode accepts traffic arriving on
`tailscale0`. If the client can `tailscale ping` the host but the bootstrap's
HTTP check fails, inspect custom UFW/nftables rules on the host.

## Friend on CachyOS

Each friend needs Git, an authenticated Tailscale client, and a clone of this
repository:

```bash
git clone https://github.com/Jared-Mac/wisp.git
cd wisp
./scripts/friend-bootstrap-cachyos.sh
```

The bootstrap uses CachyOS/Arch packages, builds only the client binaries,
installs the standalone Quickshell UI plus the PipeWire/GStreamer portal
plugins and the detected KDE, Hyprland, or GNOME portal backend needed for
screen sharing, and starts `tailscaled`. Follow the Tailscale authentication
URL if prompted.

The host privately provides the host name, one unused profile, that profile's
one-use invite, and the shared media key. Enroll this computer once, entering
the two secrets at the hidden prompts, then start Wisp:

```bash
just friend-register <host>.ts.net Tyler
just friend
```

Use `Jack` or `Charlie` only when that is the profile assigned by the host.
The assignment, device credential, and media key are stored outside the Git
checkout in `~/.config/wisp/friend.env` with mode `0600`, so pulls and
reinstalls do not overwrite them. The invite cannot be replayed on a second
computer; ask the host for another invite for every additional device.
Keep the terminal open; `Ctrl+C` stops the local daemon and UI. The resizable
Wisp app opens automatically. Set presence to **Open**, then one person can
join another by name. The tray icon opens the same compact panel presentation
used by Wisp's optional Omarchy bar adapter; choose **Open app** to return to the
full layout.

The standalone UI works without Omarchy. The settings screen supports audio
device selection, processing presets, push-to-talk, mute, deafen, E2EE status,
and device management. Global
shortcut installation is currently Omarchy-specific, so CachyOS testers should
use click-and-hold **Talk** for this first test.

Use **Clear** for the default DeepFilterNet neural denoiser. While connected, **Share screen**
opens the desktop's trusted portal picker; select one monitor or window. The
cyan badge on both the tray and Wisp UI means the LiveKit share track is
actually active, not merely requested. Select a camera and publishing
quality/codec under **Settings → Video**. Camera and screen sharing can be on at
the same time; friends receive separate Watch buttons and neither surface opens
automatically.

## Test checklist

1. Confirm an invite cannot be reused and all enrolled testers show as available.
2. Exchange a direct message while one client is offline, reconnect it, and
   confirm the message and unread marker arrive.
3. Join **Porch** and confirm it appears in NOW only while occupied.
4. Join one friend and confirm two-way audio and that Settings reports E2EE.
5. Add the remaining friends and confirm active-speaker names.
6. Test mute, deafen, and push-to-talk.
7. Select **Clear**, compare keyboard/fan/air-conditioner noise with **Studio**,
   and confirm the settings page reports `deepfilternet` active (or `rnnoise`
   only if the fallback was needed).
8. Share one monitor, then one window; confirm both friends see a Watch control
   while no video window opens and no video is decoded automatically.
9. Turn the camera on while sharing. Each friend opens both Watch controls and
   confirms independent, correctly colored screen and camera windows. Tile,
   fullscreen, pin, hide, reopen, and close them while confirming voice stays
   connected.
10. Compare Balanced/H.264 with High/H.264. Try AV1 only when Settings reports a
   suitable hardware encoder, and record whether every participant negotiates
   it successfully.
11. Stop both publishers and verify the tray/bar badges and Watch controls clear.
12. Leave and rejoin once.
13. Unplug/reconnect one USB or Bluetooth audio device if convenient.
14. Record subjective notes about echo, keyboard noise, clipping, latency, and
   dropouts.

Do not forward these ports on the router, enable Tailscale Funnel, or expose
this development mode to the public internet. Tailscale encrypts transport and
LiveKit media has application-layer E2EE, but stored messages remain readable
to the Wisp server. Broader deployment still requires TLS, stronger automated
key distribution/rotation, and TURN.

References: [Tailscale Linux installation](https://tailscale.com/docs/install/linux),
[Tailscale machine sharing](https://tailscale.com/kb/1084/sharing), and
[LiveKit ports](https://docs.livekit.io/transport/self-hosting/ports-firewall/).
