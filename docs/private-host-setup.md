# Fresh private host setup

The user chose fresh encrypted history. Leave the old installation and its test
history untouched. Never copy its database, WAL, backups, host environment,
client `friend.env`, media key, or recovery files onto OVH.

## Capacity planning

Complete the privacy rollout checks in `privacy-hardening.md` before enrolling
real accounts. For a small west-coast group, choose a nearby region when offered.
Compare the actual CPU/RAM/disk/bandwidth
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
2. Point a hostname at the server's public IPv4/IPv6 addresses. Wisp has no VPN,
   overlay-network, or hosted coordination dependency. Keep SSH public-key only;
   a private administration network is optional and is never required by clients.
   Certificate issuance exposes the hostname in certificate-transparency records.
   Publish an AAAA record only after IPv6 is configured and reachable on the VPS.
   Provider-assigned IPv6 addresses are not proof that the guest OS has configured
   them. Check `ip -6 address`, the IPv6 route, and HTTPS on both 443 and 8443
   from an external IPv6 client. A working IPv4 `curl` check alone can hide a
   blackholed AAAA record. Wisp staggers IPv6/IPv4 connection attempts, but hosts
   should still provide working records. For OVH, retrieve the assigned gateway
   from its control panel and follow its [IPv6 setup guide](https://support.us.ovhcloud.com/hc/en-us/articles/4406638994067-Configuring-IPv6-on-a-VPS);
   preserve IPv4/SSH configuration and back up networking files first.
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

Terminate HTTPS locally with Caddy (or an equivalent reviewed reverse proxy).
The supplied Caddy template exposes Wisp on 443 and LiveKit signaling on 8443,
while their HTTP listeners remain inaccessible through the firewall:

```sh
sudo install -m 0644 infra/private-host/Caddyfile.example /etc/caddy/Caddyfile
sudoedit /etc/caddy/Caddyfile
sudo caddy validate --config /etc/caddy/Caddyfile
sudo systemctl reload caddy
```

The app URL is `https://wisp.example.com`; signaling is
`wss://wisp.example.com:8443`. Clients need only that hostname or an encoded
Wisp invitation. They do not need SSH, provider credentials, or a VPN account.

Permit public TCP 80/443/8443/7881 and UDP 7882–7885. LiveKit 1.13.6 uses
`rtc.udp_port: 7882-7885` to multiplex connections across four shared UDP sockets,
matching the four-vCPU host. Omit `rtc.port_range_start` and `rtc.port_range_end`
when using multiplexing. For larger hosts, size the mux range to at least the
vCPU count and update the firewall to match. TCP 7881 remains the fallback.
Keep 8787/7880 blocked publicly; Caddy reaches them through loopback. Port 80
exists only for certificate validation and HTTPS redirection. Do not flush
existing firewall rules or close the only working SSH path. LiveKit's configured
RTC ports and advertised public node IP must match the actual server. When
migrating an existing host, back up the config and firewall rules, open the new
ports first, restart LiveKit only with no active media participants, verify its
listeners and health, then remove the old UDP range. Clients discover these
ports through signaling; no client configuration or database migration is needed.
[LiveKit network configuration](https://github.com/livekit/livekit/blob/master/config-sample.yaml),
[ports reference](https://docs.livekit.io/transport/self-hosting/ports-firewall/).

## Enrollment and keys

- Bootstrap the first account over HTTPS from the Wisp onboarding window. The
  first account becomes server owner. Remove the bootstrap token from the server
  config after enrollment and restart only when it is safe to do so.
- A fresh server contains no default users or rooms. Create rooms normally after
  sign-in. Friend and room invitation URIs encode the public server address and a
  one-use token, so invitees can paste one value into onboarding. Passwords are
  stored only as Argon2id hashes. Successful login installs a revocable local
  device credential for automatic login; the password is not saved locally.
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
