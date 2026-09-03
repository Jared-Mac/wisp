# Wisp local workflow

- After completing requested updates or pulling changes, build the current client,
  sync its installed UI, and relaunch the local running app unless the user says
  otherwise. Verify the running binary and installed UI match this checkout.
- Never automatically join or rejoin a room as part of an update/relaunch.
  Leave any active room before restarting, preserve mute/deafen settings, and
  keep the camera, screen sharing, and other media publishing off until the user
  explicitly starts them.
- Preserve existing work and credentials. Do not print device tokens, invites,
  or media keys. Keep a recoverable copy of the previous installed client.
- Updating the local client does not update Jared's host. Call out required
  server migrations/API updates; do not restart or deploy that host implicitly.
- Only push changes when the user requests a push.
