# Wisp avatar collection

Fifteen original generated pixel-art profile-picture choices, created with the
built-in image-generation tool. The finished PNG assets live in
`quickshell/app/assets/avatars/` and ship with the client. `generation.json` records
the exact prompt for each image, dimensions, relative asset path, and SHA-256.
The images retain the generator's original file metadata.

In Settings → Profile → Wisp avatars, choose a design to preview it, then use
Save picture to apply it to the account on the selected server. Custom uploads
and removal remain available. The existing avatar endpoint normalizes the saved
picture; no new server API or external image service is needed.

The collection uses cyan, blue, lavender and occasional coral or mint accents on
soft graphite. Designs: Drift, Glitch, Luna, Echo, Orbit, Fern, Nova, Tide, Comet,
Pebble, Pulse, Bloom, Aurora, Relay, and Halo.

Validation: `bash scripts/test-avatar-presets.sh` covers asset loading, selection,
explicit save, failed-save retry, cancel, switching servers, keyboard selection,
and narrow layouts. The gallery was checked in Soft Graphite, Daylight, Hearth,
and Clean TUI. All assets fit the existing 2 MB / 4096-pixel upload limits.
