# Message runs and screen-share sounds

Appearance → Chat layout offers three independent transcript styles, shared by
the main app and tray and saved across launches. Grouped is the default for new
and existing preferences that have no chat-layout selection.

- **Grouped:** one avatar and name/time header per sender run, on a continuous background.
- **Compact:** timestamps in a quiet left gutter and names inline with text, without
  chat avatars. Attachments and reply/forward context keep a compact name heading.
- **Soft groups:** one subtle rounded card per sender run, with a shared avatar/header.

A different sender/server, five-minute inactivity gap, calendar day, invitation,
or new-message divider starts a fresh run. Existing avatar visibility settings
still apply to Grouped and Soft groups. Compact does not change the avatar setting
for other lists or for switching back to the other chat styles.

Message actions and add-reaction controls appear on hover or keyboard focus in a
floating toolbar, without adding a row or moving messages. Continued-message
timestamps appear on hover; the action tooltip includes the full timestamp.
Existing reaction chips stay visible. Room invitations have no reaction controls,
and both the daemon and server reject attempts to add reactions to them. Replies,
forwards, pins, attachment previews, selectable text, unread navigation and live
typing remain available in all three layouts. Stable message delegates preserve
playing embeds across snapshots and style changes.

Screen-share start and stop cues follow changes in the current voice room's
sharing list, including your own stream. Joining, reconnecting, loading history,
camera activity, and other rooms do not replay cues. Notifications → Voice sounds
provides a separate enabled-by-default toggle and custom recordings for each cue;
the global notification mute and volume also apply. Only the standalone host
plays sounds, preventing duplicate playback from the tray adapter.

Focused validation: `scripts/test-message-runs.sh`, `scripts/test-chat-extras.sh`,
`scripts/test-unread-messages.sh`, `scripts/test-sound-playback.sh`,
`scripts/test-chat-logic.cjs`, and the server's `chat_extras_tests` module.
