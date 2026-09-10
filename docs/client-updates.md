# Client updates

Every updater-enabled Linux x86_64 client follows successful GitHub `main`
releases, regardless of who pushed the change. Existing users install this build
once (or run `wisp-update` after it is published) to enable in-app updates.
Windows distribution is still planned; this does not replace Quickshell.

Settings → Updates provides four independent preferences:

- **Install updates automatically**: enabled by default. Install after all Wisp
  surfaces have been idle and unfocused for a minute.
- **Check when Wisp starts**: enabled by default.
- **Check while Wisp is running**: enabled by default.
- **Check interval**: 5 minutes by default; adjustable from 1 to 1,440 minutes.

**Check for updates** works with all automatic controls disabled. Available
updates show a notice in the main app and tray panel; **Update now** installs
manually once voice is disconnected and drafts/transfers are finished. Turning
off automatic installation still permits notices and manual installation.

Checks fetch only a small JSON release manifest, not the archive. At five minutes,
there are at most 12 scheduled checks per hour per running installation. A local
lock and last-check timestamp deduplicate checks from multiple UI surfaces. No
account details, messages, or enrollment credentials are sent to GitHub.

The installer downloads on demand, bounds archive size, checks SHA-256 and version,
rejects unsafe archive paths/links, and preflights the client executables. The
manifest is published after the archive. Local source builds with a newer commit
timestamp are not automatically downgraded to an older published release.

Before installation, every live UI must acknowledge a new preparation request.
Voice, streams, unfinished messages, attachments, transfers and audio tests defer
installation. Prepared surfaces temporarily block new input. Activity and the
automatic-install preference are checked again after downloading and backing up.
The installer runs in a separate per-user systemd unit, so stopping Wisp does not
kill the update operation. It installs only client binaries, refreshes the UI and
an already-installed Omarchy adapter, verifies files and the running daemon,
preserves mute/deafen, and keeps voice/camera/screen sharing disconnected.

Preferences live in `$XDG_CONFIG_HOME/wisp/updates.json`; release metadata is in
`installed-release.json`. State and recoverable backups are under
`$XDG_STATE_HOME/wisp/`. Failed automatic installs require a manual retry or a
newer release instead of repeatedly downloading the same failing package.
Installation failures attempt to restore the previous binaries/UI. The user's
source checkout is never pulled or changed by the client updater.
