# Wisp plugins · API 1

The first plugin is **Valheim Modpacks**. It stores a draft per account and
conversation, accepts private mod ZIPs, resolves exact Thunderstore versions,
and shares an encrypted pack with a friend, group, or server channel. It creates
a separate local BepInEx profile. Sharing never installs anything on another member's computer.

This is the first usable platform release. An external marketplace, automatic update
feeds, background guest services, live collaborative documents, a Windows game
launcher, and remote game-server deployment are not implemented. Wisp servers
relay encrypted documents; they do not execute plugins or install Valheim mods.

## Try the Valheim plugin

1. On the main Wisp view, choose **Plugins**. A server owner or administrator
   selects **Add to server** beside **Valheim Modpacks** in the repository-backed
   catalog. Everyone in that server can now see its shortcut on the main view.
2. Click **Valheim Modpacks** (or **Open** on its catalog card). Wisp downloads
   and verifies the reviewed package from the selected server, then opens it.
   There is no plugin-folder picker. Opening does not install game mods, launch
   a game, join voice or share a document. Both client and server need the catalog
   release; unknown versions prompt a client update. **Developer tools** retains
   manual folder installation for local development only.
3. Choose **Personal draft** or a friend/group/server-channel conversation.
   Enter exact Thunderstore dependency strings, one per line, such as
   `denikson-BepInExPack_Valheim-5.4.2351`. Keep BepInEx among the dependencies.
   Save the draft. To include an unpublished mod, choose **Add private mod ZIP…**,
   select its ZIP, review its name/version, dependencies, size, and hash, then
   click **Add reviewed ZIP**. Select **Resolve dependencies** after adding mods.
   No Thunderstore account, team, or public listing is needed for a private ZIP.
4. Wisp recursively reads each downloaded package's `manifest.json`. Your root versions stay pinned.
   Dependency versions are minimums: Wisp selects the highest requirement it
   encounters, and refuses a root pin that is too old. Review the resulting package
   list and hashes.
5. **Share this revision** sends an ordinary, retained encrypted attachment to
   the selected conversation, including the actual bytes of any private mods.
   Members import it from **Shared in this
   conversation**, review it, then choose **Install local profile** themselves.
   Only loaded chat history is listed; older attachments remain normal files.
6. Choose an existing parent directory. A new `wisp-profile-<digest>` directory
   receives the verified, extracted pack. Your Valheim installation is untouched.
   Keep an earlier profile to return to that version. Modified profile files
   are not overwritten; choose a different parent folder for a fresh copy.

For native Linux Valheim, the BepInEx package supplies a launcher. Run it yourself
with the absolute game executable path, for example:

```sh
bash /path/to/profile/start_game_bepinex.sh /path/to/Valheim/valheim.x86_64
```

Wisp does not launch downloaded scripts or the game. BepInEx/mod code executes
with the game's permissions when you launch it; the Wisp plugin sandbox does
not sandbox game mods. Game compatibility and client-only/server-only mod
requirements still need to be checked for the chosen pack. Existing mod-manager
profiles and user-edited configuration are not imported or merged automatically.

## Valheim manager

Opening the Valheim shortcut displays its own mod manager. **Mods** lists public,
private and dependency packages with their exact versions; search or filter by
source and select a mod for details. **Add mod** accepts a pinned Thunderstore ID
and replaces an existing root version of that package. Private ZIPs retain their
review step. Removing mods edits the pack and keeps installed profiles.

Save draft changes, then **Resolve mods**, **Install profile**, or **Share pack**.
**Shared packs** lists importable revisions in the selected conversation.
**Installation** shows the saved local folder and launch guidance. “Resolved”
means dependency resolution succeeded; it does not mean everyone installed the
same pack. **All plugins** returns to server plugin administration. Exact package
list editing is available under Advanced; hashes and file paths do not crowd the
mod list. Other plugin IDs still use the generic plugin view.

## Private mod ZIPs

A private ZIP uses the usual mod-package layout: `manifest.json` at the archive
root with `name`, `version_number`, and `dependencies`, plus mod files such as
`BepInEx/plugins/Foundation.dll`. Public dependency strings in the manifest are
resolved from Thunderstore; the private package itself stays inside Wisp.
Wisp labels it `Local-Foundation-0.1.0` (using its manifest name and version).
Private mods can depend on another included `Local-Name-Version` package.
They cannot replace the official BepInEx loader.

Private ZIPs are limited to **128 MiB combined per draft/shared pack**, with a
1 GiB extracted profile limit. Inspection rejects unsafe paths, links, malformed
manifests, conflicting paths, and excessive expansion before adding the file.
Changing the ZIP after review requires another review. Removing a ZIP invalidates
the draft's lock without deleting existing profiles or earlier shared revisions.

The sender's ZIP path is never sent to peers. Private bytes are retained in a
separate cache and admitted to a host-owned registry for that account and
conversation. A plugin cannot share or install another draft's private files by
guessing a hash. Recipients import the complete encrypted bundle, review its
package list, and explicitly install their own profile. No mod code runs during
inspection, sharing, import, or installation. This does not alter a mod's own
solo/host/guest compatibility.

## Server catalog API

Schema 36 adds `server_plugins` (ID, approved digest, enabling admin and timestamp).
`GET /v1/server/plugins` returns `{catalog, can_manage}` to members only.
`PUT /v1/server/plugins/{id}` accepts `{enabled, digest}` from an admin/owner;
unknown IDs, stale approvals, outsiders and non-admin mutations are rejected.
`GET /v1/server/plugins/{id}/package` serves only an enabled package with
`X-Wisp-Plugin-Digest`. No private packs, local file paths or member install state
are stored in this registry. Removing a server plugin removes its shortcut and
future package access, preserving members' existing profiles and drafts.

The daemon commands are `plugin_server_catalog`, `plugin_server_enable`, and
`plugin_server_prepare`, scoped by `server_id`. The UI refreshes on server changes,
server plugin events and every 30 seconds while connected. Late replies cannot
populate another server's view. Prepare verifies the exact code against the
client's reviewed catalog before caching; it never accepts a server-supplied URL.
Catalog packaging/update instructions are in `plugins/catalog/README.md`.

A server plugin is shared availability, not a claim that members' installed game
mods are synchronized. Packs still belong to individual conversation drafts and
are exchanged as encrypted revisions. The page lists exact resolved mods, shared
revisions and local installation location. Live per-member sync status is not
implemented.

## Package and runtime

A package folder contains `plugin.json` and `plugin.wasm`. The manifest is strict:

```json
{
  "api": 1,
  "id": "example.tool",
  "name": "Example tool",
  "version": "1.0.0",
  "description": "A description of the tool.",
  "permissions": ["documents"]
}
```

IDs are namespaced lowercase identifiers. Versions use SemVer. Supported
permissions are `documents`, `thunderstore`, `managed_files`, and `local_archives`. The native
install review approves the digest of the parsed manifest plus module bytes;
a changed package requires a new review. Installed modules are verified on each
invocation. Disabling blocks subsequent invocations; lifecycle changes wait for
the current operation to finish. Removal keeps saved draft data,
previous package files, and independently owned game profiles.

The guest exports memory and two functions:

- `wisp_input() -> i32`: pointer to a writable 256 KiB input buffer.
- `wisp_handle(input_len: i32) -> i64`: output pointer in the high 32 bits,
  UTF-8 JSON length in the low 32 bits.

Request JSON contains `api`, `event`, `action`, `fields`, `state`, and `document`.
Response JSON contains `state` and `view`. Events are `open`, `action`, `import`,
`resolved`, `archive_added`, and `archive_removed`. Each invocation starts a fresh instance; `state` is the only
persistent guest memory. The working example is in
`examples/plugins/shared-modpacks/src/lib.rs`.

A view declares `title`, `description`, `fields`, `actions`, `rows`, optional
`document`, `packages`, `files`, and `archives`. Wisp renders plain text and native controls.
Guests cannot provide QML, HTML, JavaScript, external links, dialogs, or native
commands. The host alone renders installation and sharing controls. A view
request is not permission to perform a side effect.

Wasmi runs without WASI or any host imports, in a separate worker with cleared
environment, 32 MiB guest memory, 30 million fuel units, and a 5-second wall
limit. The Linux worker additionally caps address space at 1 GiB and CPU at
4 seconds, including compilation, and disables core dumps. Modules are capped at 8 MiB and request/response JSON at 256 KiB. Guest
code never receives Wisp credentials, conversation messages, filesystem paths,
or arbitrary network access. The worker process is a failure/time boundary;
Wasm isolation, rather than an OS container, enforces guest capabilities.

## Host capabilities and shared documents

Thunderstore resolution accepts exact `Team-Package-Major.Minor.Patch` strings.
Downloads use only Thunderstore's HTTPS CDN and its approved CDN redirect.
The host reads archive manifests, resolves dependency minimums against explicit
root pins, limits graph size to 128 packages and a
profile to 1 GiB (with a 4 GiB download-cache quota), then records SHA-256 and byte size for each archive. Peer
installation checks all hashes and the complete dependency closure again.
A hash pins bytes; it does not establish that a mod author or mod is trustworthy.

The initial managed-files adapter handles BepInEx archive layouts: the official
Valheim loader, plugins, patchers, and config files. Unknown loose payloads go
under a package-specific plugins directory. Symlinks, special files, traversal,
case-insensitive collisions, oversized archives, and decompression limits are
checked before committing the staged directory. Packages needing custom
installers or other loaders require a future adapter. They are not executed.

Shared document envelopes contain `format`, `plugin`, `plugin_version`, `title`,
and `data`. They use the existing encrypted attachment protocol and signed
conversation roster, including linked Wisp accounts. Author and audience come
from the authenticated message, never the envelope. Import validates the
plugin ID and file size before passing only the document data to the guest.
Import does not install plugins, enable permissions, or execute game mods.

Public-only revisions keep the original `.wisp-plugin.json` envelope and Valheim
data format 1. Private revisions use `.wisp-plugin.zip`: a strict bundle-format-1
`bundle.json` contains the document and its private artifact list, alongside
`archives/<sha256>.zip` entries. Valheim data format 2 identifies a pack containing
private archives. Each local artifact adds `source: "local"`; omitting `source`
still means Thunderstore and preserves existing public-pack hashes. Import
validates the complete bundle, exact artifact identity, size/hash, and archive
structure before authorizing its private bytes. Extra/missing/duplicate files
are rejected. Older clients can download the encrypted attachment normally;
they need the updated plugin host to import it into the manager.

Documents are immutable revisions. Editing a received Valheim pack creates a
local draft for a subsequent revision; the document records its imported parent digest.
Any member who can send in the conversation can share a fork. There is no
special pack-owner role or automatic replacement of someone else's revision.
A group can select its agreed version by pinning the ordinary attachment in
chat. Legacy clients can download the attachment without understanding it.
No server schema or wire-message changes are required.

State is namespaced by plugin ID, server URL, account ID, and conversation ID.
Generation checks reject stale mutations across windows. One plugin operation
runs at a time; slow downloads and workers run outside the IPC command loop so
voice controls remain responsive. An operation interrupted before publication
or installation does not commit state. A completed share or profile is retained
even if a subsequent connection or disk failure prevents acknowledgement.

## CLI and validation

`wispctl plugin <operation> '<JSON arguments>'` exposes the same API. Operations:
`list`, `inspect`, `install`, `open`, `action`, `resolve`, `share`, `import`,
`apply`, `enable`, `remove`, `inspect_archive`, `add_archive`, and `remove_archive`. All scoped mutations take the generation from
`open`; `apply` also takes the displayed plan digest and local parent path.
Never put credentials in plugin arguments.

```sh
cargo test --locked -p wisp-plugins
cargo test --locked --manifest-path examples/plugins/shared-modpacks/Cargo.toml
bash scripts/test-plugins-ui.sh
# With release wispd/wisp-server and the guest already built:
python3 scripts/test-plugins.py --online
```

The integration fixture creates two isolated accounts, an ephemeral loopback
server, and temporary profiles. Its default offline run inspects a private ZIP,
shares/imports its encrypted bundle, installs identical peer files, checks draft
isolation, rejects changed inputs, and restores a corrupted cache by reimport;
`--online` downloads pinned public BepInEx and Jötunn archives. It never launches Valheim
or touches real conversations.

Primary references: [Wasmi](https://github.com/wasmi-labs/wasmi),
[Thunderstore package format](https://wiki.thunderstore.io/mods/creating-a-package),
and [BepInExPack Valheim](https://thunderstore.io/c/valheim/p/denikson/BepInExPack_Valheim/).


### Server-wide packs

Opening a server's Valheim plugin selects **Server pack · all members** by default.
Admins edit a local server draft, resolve it and **Publish to server**. Members see
the current published revision and may install it on their own PC. **Use personal
draft** copies an existing saved personal draft into an empty server draft; the
original remains available. The scope selector retains personal and conversation
packs for optional targeted sharing. Publishing never installs mods or launches
games on another member's machine.

The transport is an internal encrypted server channel, hidden from chat navigation
and legacy snapshots. Admin publishing is enforced by the server. Saved unpublished
drafts survive refresh and a newer published revision is not silently overwritten.
New members may require an admin to republish for their expanded signed roster.

### Installed server-pack status and sync

The manager's **This PC** panel compares its managed profile with the published
server pack. It distinguishes **Up to date**, **Update available**, **Local
changes**, **Profile missing**, **Not installed**, and unavailable/unpublished
states. Matching is based on the locked artifact plan and the receipt's actual
file hashes, including detecting missing and added files. It does not claim that
a running game loaded that profile or that other players are synced.

Opening the manager and pressing **Check** verifies local files. A 30-second timer
while the manager is visible checks the latest server revision; unchanged revisions
reuse the timestamped local check without scanning files again. New revisions are
resolved independently of the editable draft, so polling cannot replace unsaved
or unpublished edits. This is advisory verification of a user-owned profile, not
an anti-cheat or attestation mechanism.

**Install server pack** asks for a parent folder once. **Sync / update** reuses it,
downloads/verifies the published artifacts and creates or reuses a matching
profile. Changed profiles remain intact; repair creates a verified sibling. Old
profiles remain available for rollback. Clear or corrupt private-archive caches
are recovered from the authenticated encrypted publication. Restart the game
using the new profile after syncing. Wisp never launches the game automatically.

IPC adds `plugin_sync_status` (`server_pack:true`, optional `verify:true`) and
`plugin_sync` (`generation`, `sync_revision`, `sync_digest`, optional `path` for
first installation). Sync rejects a stale publication before installing and checks
again afterward. It updates only the saved folder/generation, preserving draft
state. The host stores a bounded `sync.json` sidecar next to the scoped instance;
existing instance, receipt, guest, encrypted document and server schemas are
unchanged. Tests cover real encrypted revisions, actual file changes, cached-check
behavior, stale sync requests, cache restoration, safe repair and draft preservation.

### Server member status

The **Members** tab shows the last check reported by each member's supported
clients. Each PC has its own status; a synced Linux installation cannot hide an
outdated Windows installation. Only members whose reporting PCs all match are
counted as fully synced. Unreported clients remain unknown. Old-revision reports
say **Pack changed · recheck needed**, since a new publication may contain unchanged
mod files. A confirmed file-plan mismatch says **Needs update**. Checks expire to
stale after 24 hours. These statuses describe managed profiles, not running games.

Schema 39 adds bounded device reports. GET/PUT `/v1/server/plugins/{id}/sync` is
server-member-only and requires an enabled plugin. PUT binds identity to the bearer
session, validates the latest encrypted publication and recipient access, and stores
only its revision, status, OS and check timestamp. Cached polls preserve the original
check time; older reports cannot overwrite newer checks. Device names, local paths,
file lists and mod contents never leave the PC. Revocation filters devices from the
roster; membership removal and plugin removal clear their reports. Up to eight
reporting devices are retained per member/plugin. Names resolve from current user
records, so public-name changes do not break the reports. Installation and drafts
remain usable when an older server does not support reporting.
