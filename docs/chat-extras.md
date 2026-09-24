# Links, emojis, reactions and video

Web links in message text and attachment captions open in the default browser.
Chat remains plain text on the wire; clients escape markup before displaying
links and emoji images. Pasted HTML never becomes executable content.

## Mentions

Type `@` in any chat composer to search that conversation's members. Use Up/Down
and Enter or Tab to select a person, or click a suggestion. Escape dismisses the
suggestions without changing the draft. Selecting a mention does not send it.
Names with spaces or other special characters use quotes, such as `@"Mira Moon"`.
Known mentions appear in the chat's accent color.

Mentions match the recipient's current display name, case-insensitively, within
the receiving server. Display names are unique per server. They remain ordinary
message text inside the existing encrypted payload, including attachment captions;
there is no new server-visible mention metadata. Older clients can read the text.
Renaming an account changes which name should be used for future mentions.

In Settings → Notifications, **Only messages that @mention me** filters message
sounds on this device. Existing focus rules and mute settings still apply; other
messages retain unread badges. Voice sounds keep their separate settings. Own
messages, forwarded quotations, emails, web links, and history/reconnect loads do
not trigger mention sounds. There are no `@everyone` or `@here` broadcast mentions.

Run `scripts/test-mentions.sh` for autocomplete, rendering, and notification checks.

## Emoji and reactions

The composer’s smile button opens a searchable picker containing standard
Unicode emojis, thirty bundled Wisp emojis, the account library and server
emojis. Click the reaction button below a message to add a reaction; click a
selected reaction again to remove yours. Counts are per person, and hovering
shows who reacted. Reactions are encrypted and signed with the same account
keys as chat, bound to the original message and its original audience (restricted
to people who still belong to the conversation). The server sees routing
metadata, not which emoji was chosen, for encrypted reactions.

The original twelve mint ghosts are joined by **Wisp Everyday**, twelve lavender
ghosts: wink, cool, think, blush, sweat, facepalm, peek, party, coffee, popcorn,
salute and shrug. Search `everyday` to see the whole new set, or try terms such as
`cozy`, `celebrate` or `thinking`. Their shortcodes are `:wisp_coffee:` and so on;
they work in both messages and reactions. All artwork is bundled SVG, with no
download or upload required. Older clients without the artwork display the
readable shortcode. No server change or migration is needed.

Six warm gold **Wisp Moments** ghosts add hug, music, gaming, bonk, melting and
comfy. Search `moments` to see this set. The picker stays within the app or tray
window, including when opened from a narrow chat tile at the right edge.

Manage personal emojis in Settings → Profile → My emojis. Server administrators
manage shared emojis in Settings → Server → Server emojis. Both libraries have
no count limit and use paged API retrieval. Names contain 2–32 letters, digits
or underscores. PNG, JPEG, GIF and WebP inputs up to 2 MB are normalized into
static 128-pixel PNGs. Removing an emoji hides it from the library; its existing
uses in chat remain readable. Emoji artwork is account/server asset data, not
encrypted chat content. Schema migration 22 adds the asset and reaction tables.

YouTube watch, short-link, Shorts, live and embed URLs expose a **play here**
button. Nothing contacts YouTube until it is clicked. A private, off-the-record
Qt WebEngine view hosts the official YouTube player inside the chat. Audio,
camera, location and file permission requests are refused. Close video stops
playback; opening the original link in a browser remains available if YouTube
disallows embedded playback. The application identifier `dev.wisp`, rather than
the user’s server address, identifies the player’s origin and referrer, following
[YouTube’s embedded-player guidance](https://developers.google.com/youtube/terms/required-minimum-functionality#embedded-player-api-client-identity).

## Linux web runtime

Inline video needs Qt WebEngine 6.8 or newer. Quickshell 0.3.1 constructs its Qt
application with `argc=0`, which crashes Chromium initialization. Wisp builds a
private copy with `argc=1` and `AA_ShareOpenGLContexts` enabled before creating
the application. It does not replace the desktop’s Quickshell. Source, checksum,
build flags and installation logic are in `scripts/build-web-runtime.sh`.

On Arch/Omarchy, the additional build dependencies are `cmake`, `ninja`, `gcc`,
`cli11`, `wayland-protocols` and `qt6-webengine`. UI sync builds the runtime once,
then reuses it until Qt or the runtime version changes. The same builder is
installed as `wisp-web-runtime` for adding missing dependencies later. If those
dependencies are unavailable, links still open in the browser and **play here**
explains the missing runtime. The rest of Wisp does not depend on WebEngine.

Run `scripts/test-chat-extras.sh` for offline UI checks, `cargo test -p wisp-server
chat_extras_tests` for permissions/storage, and the daemon’s encrypted two-client
integration test for reaction authenticity and original-audience handling.
