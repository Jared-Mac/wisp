# Unlisted Android test distribution

The owner-managed `https://wisp.you/android-test` page serves signed test APKs.
It is intentionally absent from navigation and sitemaps, with both a robots meta
tag and an `X-Robots-Tag` response header. The URL is unlisted, not access-controlled.
Android source, signing keys, passwords, and private build files stay off the host.

`site/` contains the static page. Import `wisp-android-test.caddy` at top level in
Caddy and `wisp_android_test` inside the existing owner-managed `wisp.you` site.
Keep all existing website and invitation routes. Validate Caddy before reloading;
this does not require a media-service restart.

For each release:

1. Confirm the private Android source commit is pushed and its CI succeeds. Use
   the Android task's signed distribution APK and accompanying manifest, not a
   CI debug APK. Independently check SHA-256, byte size, signing certificate,
   `apksigner verify`, `zipalign`, app ID, version, supported ABIs/minimum Android,
   and that the manifest is non-debuggable and built from clean source.
2. Build/test/deploy any required desktop and backend changes before making the
   download available. Observe the normal production backup and idle-room guard.
3. Back up current static files, release manifest, associations, and Caddy config.
   Copy the verified APK to `/var/www/wisp/android-test/downloads/` using a unique
   versioned filename. Never overwrite different bytes at a published filename;
   never remove older APKs during routine publication.
4. Install the page files, then atomically replace `release.json` with the verified
   distribution manifest **last**. The page validates the filename, checksum,
   version and release type before exposing the download button.
5. Publish the verified test-signing certificate association in
   `/var/www/wisp/android-links/.well-known/assetlinks.json`. Merge with any existing
   association entries; never replace unrelated apps. `assetlinks.json` here is
   the verified Wisp test certificate, not a private signing key. Recheck it for
   every signing-channel change.
6. Verify public HTTPS status, headers, page/version, and the complete downloaded
   APK checksum. Check association JSON is served without a redirect, and that
   `/android-test` remains a browser destination. Include appropriate Android
   changes and the test link in the coordinated Discord notes, following
   `docs/discord-patch-notes.md`.

The app's static invite path filters cover readable invite prefixes and `/join/`
without intercepting `/android-test` or APK downloads, including on Android 8–14.
Physical-phone installation, Samsung background restrictions, voice, and OS link
handling still require device testing; local build/CI checks do not substitute
for it. Keep emulators closed when the user has requested that.

Reference: [Android website associations](https://developer.android.com/training/app-links/configure-assetlinks).
