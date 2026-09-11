# Message runs and screen-share sounds

Consecutive messages from the same person share one avatar, with tighter spacing
inside the run. Names and timestamps remain on every message. A different sender,
calendar day, or new-message divider starts a fresh run. Hiding avatars in
Appearance still removes the avatar gutter entirely.

The add-reaction button sits in the message heading and appears on hover or
keyboard focus. Its space is reserved so messages do not jump. Existing reaction
chips stay visible. Room invitations have no reaction controls, and both the
daemon and server reject attempts to add reactions to them.

Screen-share start and stop cues follow changes in the current voice room's
sharing list, including your own stream. Joining, reconnecting, loading history,
camera activity, and other rooms do not replay cues. Notifications → Voice sounds
provides a separate enabled-by-default toggle and custom recordings for each cue;
the global notification mute and volume also apply. Only the standalone host
plays sounds, preventing duplicate playback from the tray adapter.

Focused validation: `scripts/test-message-runs.sh`, `scripts/test-chat-extras.sh`,
`scripts/test-unread-messages.sh`, `scripts/test-sound-playback.sh`,
`scripts/test-chat-logic.cjs`, and the server's `chat_extras_tests` module.
