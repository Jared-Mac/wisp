# Trusted Tailscale friend test

This mode is for a short voice test with people you trust. It is not a public
deployment. Wisp's development login currently identifies a user by one of the
seeded profile names, so every tester who can reach the host must be trusted.

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
random in-memory LiveKit keypair for that run, and binds the coordination and
media servers to the tailnet address. Normal `just dev` remains loopback-only.

In another terminal, print the two pieces of information to send privately:

```bash
just tailscale-info
```

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
installs the standalone Quickshell UI, and starts `tailscaled`. Follow the
Tailscale authentication URL if prompted.

The host privately provides a host name and one unused profile. Start Wisp with:

```bash
just friend <host>.ts.net Tyler
```

Use `Jack` or `Charlie` only when that is the profile assigned by the host.
Keep the terminal open; `Ctrl+C` stops the local daemon and UI. The Wisp window
opens automatically. Set presence to **Open**, then one person can join another
by name.

The standalone UI works without Omarchy. The settings screen supports audio
device selection, processing presets, push-to-talk, mute, and deafen. Global
shortcut installation is currently Omarchy-specific, so CachyOS testers should
use click-and-hold **Talk** for this first test.

## Test checklist

1. Confirm all testers show as available.
2. Join one friend and confirm two-way audio.
3. Add the remaining friends and confirm active-speaker names.
4. Test mute, deafen, and push-to-talk.
5. Leave and rejoin once.
6. Unplug/reconnect one USB or Bluetooth audio device if convenient.
7. Record subjective notes about echo, keyboard noise, clipping, latency, and
   dropouts.

Do not forward these ports on the router, enable Tailscale Funnel, or expose
this development mode to the public internet. Production use still requires
real device authentication, TLS, durable secret management, and TURN.

References: [Tailscale Linux installation](https://tailscale.com/docs/install/linux),
[Tailscale machine sharing](https://tailscale.com/kb/1084/sharing), and
[LiveKit ports](https://docs.livekit.io/transport/self-hosting/ports-firewall/).
