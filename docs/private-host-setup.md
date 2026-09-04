# Fresh private host — preparation, not an executed deployment

The user chose fresh encrypted history. Leave the old installation and its test
history untouched. Never copy its database, WAL, backups, host environment,
client `friend.env`, media key, or recovery files onto OVH.

## Purchase checkpoint

Do not place an order until the privacy rollout checks in `privacy-hardening.md`
are complete and a deployable Linux build is available. For four people, choose
Hillsboro/Oregon if offered at checkout. Compare the actual CPU/RAM/disk/bandwidth
specifications rather than assuming an old VPS model number means the same thing.
Use monthly billing initially; confirm renewal price, currency, tax and region.
Use Ubuntu 24.04 x86_64 to match the project's GitHub Actions build environment.
No database/control-panel/Windows add-ons are needed. No purchase is performed by
this document. A 75 GB disk is finite even with no per-file application cap;
monitor storage and expand it instead of silently deleting kept files.

## Access and services

1. Add an SSH public key at creation. Keep passwords/private keys out of chat.
   Patch the OS, enable automatic security updates, and use a non-root sudo
   account with key-only SSH. Verify a second SSH session and OVH console access
   before disabling password/root login or restricting the firewall.
2. Join the existing private Tailscale network. Restrict the service node's
   policy to the intended friends, and SSH to administrators only. Do not enable
   Tailscale Funnel or an exit node. Use a neutral hostname: certificate issuance
   can expose the hostname in certificate-transparency records.
3. Install a reviewed Ubuntu-compatible Wisp build and a pinned, verified
   LiveKit release. Do not copy CachyOS-built binaries to Ubuntu without checking
   ABI compatibility. Create separate non-login `wisp` and `wisp-media` users.
4. Fill the templates in `infra/private-host/`. Install the coordination config
   root:wisp 0640 and media config root:wisp-media 0640, under separate directories.
   Create independent random LiveKit service credentials and bootstrap token.
   These are server authentication secrets, NOT client media or recovery keys.
5. Start with a new `/var/lib/wisp/wisp.sqlite3`. Strict mode rejects legacy
   plaintext writes and refuses startup if active legacy content is present.
   Install the provided system services; inspect `systemd-analyze security`
   and runtime logs after startup. No release/client installation is automatic.

Serve only the two loopback HTTP backends over the private tailnet:

```sh
sudo tailscale serve --bg --https=443 http://127.0.0.1:8787
sudo tailscale serve --bg --https=8443 http://127.0.0.1:7880
```

The app URL is `https://NODE.TAILNET.ts.net`; signaling is
`wss://NODE.TAILNET.ts.net:8443`. Serve provisions HTTPS certificates and limits
access to the tailnet; it is distinct from public Funnel.
[Tailscale Serve reference](https://tailscale.com/docs/reference/tailscale-cli/serve).

Permit client access on `tailscale0` to TCP 443/8443/7881 and UDP 50000–50100,
with matching tailnet rules. Keep 8787/7880 and RTC ports closed on the public
interface. Allow the Tailscale UDP transport as required for direct connections;
test `tailscale ping` from each friend to avoid a slow relayed path. Do not flush
existing firewall rules or close the only working SSH path. LiveKit's configured
RTC range, interface filter and advertised node IP must match the actual node.
[LiveKit network configuration](https://github.com/livekit/livekit/blob/master/config-sample.yaml),
[ports reference](https://docs.livekit.io/transport/self-hosting/ports-firewall/).

## Enrollment and keys

- Bootstrap the existing circle administrator (currently Jared) over HTTPS from
  his client, not by leaving an administrator client/session on the VPS. Remove
  the bootstrap token from the server config after first enrollment and restart
  only when it is safe to do so. Preserve existing room ownership semantics.
- Issue one-use invites; enroll each client with the HTTPS origin. The launcher
  now saves that origin rather than forcing HTTP/8787. Initial HTTPS enrollment
  may omit the media key so encrypted chat can be set up first; voice/video stays
  blocked without a valid client-held media key.
- Each user selects Settings → Privacy → Set up encryption and saves their own
  recovery file somewhere private and separate from the VPS. Never share recovery
  files between friends. Configure every participant before sending to that chat.
- Generate a fresh media key on a trusted client. Distribute it only through
  already-configured encrypted DMs or another trusted private channel. Save it
  in each client's private configuration and restart while disconnected. Do not
  send it through the old plaintext test chat or place it on the media server.
- First public identities use TOFU. Later changes fail closed. A reinstall that
  loses trust pins reopens first-contact risk, even when its recovery identity is
  restored. This is archival E2EE, not forward secrecy or an independent audit.

## Validation before inviting everyone

Verify from two isolated clients: text, edits, images, files, recovery, room
admission, removal, expiry/keep, and reconnect. Inspect server storage for
ciphertext-only content. Confirm no development login, no secrets in URLs/logs,
and no media publication without a client key. Test voice/screenshare separately
with explicit user consent, then reboot and verify service/network persistence.

OVH can still observe traffic, identifiers, membership, room names, timestamps,
sizes and invite coordination metadata. It can disrupt or delete service data.
The client-only keys protect content, not availability or endpoint compromise.
Do not run a server-side recording/transcription service expecting E2EE to survive.

Back up the new encrypted database using SQLite's online backup operation; encrypt
the backup envelope on the client before keeping an off-host copy, since database
metadata is not E2EE. Keep the backup recipient's private key off the VPS. Test a
restore to a new isolated instance. Provider snapshots do not replace recovery
files and retain metadata/service credentials; account for that in retention.

## Rollback

Keep the old host and client backups until the new installation is verified.
Switch client endpoint/configuration back only after leaving voice; never autojoin.
Do not point the old client at encrypted history or reuse the new database with
an older schema. Preserve all new recovery files and trust pins during rollback.
