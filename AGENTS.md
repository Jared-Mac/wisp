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
- Treat updates as coordinated client/server releases. When server code or
  migrations changed, back up and deploy the matching server build to the
  configured owner-managed production host, then verify schema, integrity,
  service state, and public health. Before restarting server media/control
  services, verify that no room is active. For client-only changes, verify the
  deployed server has no required delta and remains healthy. Never deploy to a
  friend's or otherwise unrelated host.
- Only push changes when the user requests a push.
