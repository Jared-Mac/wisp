# Wisp plugins · API 1

The first plugin is **Valheim Modpacks**. It stores a draft per account and
conversation, resolves exact Thunderstore versions, shares an encrypted lockfile
with a friend, group, or server channel, and creates a separate local BepInEx
profile. Sharing never installs anything on another member's computer.

This is the first usable platform release. Package discovery, automatic update
feeds, background guest services, live collaborative documents, a Windows game
launcher, and remote game-server deployment are not implemented. Wisp servers
relay encrypted documents; they do not execute plugins or install Valheim mods.

## Try the Valheim plugin

1. Install the Rust `wasm32-unknown-unknown` target once, then run
   `bash scripts/build-valheim-plugin.sh`.
2. Open **Wisp → Plugins → Install plugin**. Choose
   `target/plugins/games.valheim`, review the identity, permissions, and digest,
   and install it.
3. Choose **Personal draft** or a friend/group/server-channel conversation.
   Enter exact Thunderstore dependency strings, one per line, such as
   `denikson-BepInExPack_Valheim-5.4.2351`. Keep BepInEx among the dependencies.
   Save the draft, then select **Resolve dependencies**.
4. Wisp recursively reads each downloaded package's `manifest.json`. Your root versions stay pinned.
   Dependency versions are minimums: Wisp selects the highest requirement it
   encounters, and refuses a root pin that is too old. Review the resulting package
   list and hashes.
5. **Share this revision** sends an ordinary, retained encrypted attachment to
   the selected conversation. Members import it from **Shared in this
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
permissions are `documents`, `thunderstore`, and `managed_files`. The native
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
and `resolved`. Each invocation starts a fresh instance; `state` is the only
persistent guest memory. The working example is in
`examples/plugins/shared-modpacks/src/lib.rs`.

A view declares `title`, `description`, `fields`, `actions`, `rows`, optional
`document`, `packages`, and `files`. Wisp renders plain text and native controls.
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
`apply`, `enable`, and `remove`. All scoped mutations take the generation from
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
server, and temporary profiles. It exercises encrypted pack sharing and import;
`--online` downloads pinned public BepInEx and Jötunn archives. It never launches Valheim
or touches real conversations.

Primary references: [Wasmi](https://github.com/wasmi-labs/wasmi),
[Thunderstore package format](https://wiki.thunderstore.io/mods/creating-a-package),
and [BepInExPack Valheim](https://thunderstore.io/c/valheim/p/denikson/BepInExPack_Valheim/).
