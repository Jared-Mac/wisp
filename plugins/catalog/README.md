# Wisp plugin catalog

This repository is the catalog for now. The matching server and desktop release
embed the reviewed `plugin.json` and `plugin.wasm` files. No GitHub token, live
GitHub API request, or Actions build is needed to enable a tool on a server.

Admins select **Plugins → Add to server**. Members see the tool on the main view;
**Open** retrieves and caches its package from their authenticated Wisp server.
Both ends pin the manifest + module digest. Clients reject unknown code, changed
permissions and incompatible catalog versions, and ask for a Wisp update. The
server stores only the registration; it does not execute a guest or install game
mods. Existing encrypted revision sharing and explicit local profile installation
remain unchanged.

To update Valheim Modpacks, edit `examples/plugins/shared-modpacks`, then run:

```sh
bash scripts/build-valheim-plugin.sh --catalog
```

Review both files, test the guest and host, and release client/server together.
The first catalog package is Valheim Modpacks 0.2.0 from source commit 5325a76.
If a package digest changes, an admin explicitly chooses **Update server plugin**;
existing registrations do not silently approve the new version. Keep old profiles
and drafts. Future online discovery can replace catalog delivery without making
arbitrary remote code trusted.
