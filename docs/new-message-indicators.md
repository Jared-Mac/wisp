# New message indicators

Chats now retain a theme-colored “New messages” divider above the first incoming
message that arrived while that conversation was not being read. A “New” button
in the desktop/detached tile header jumps to that boundary. The same divider and
read handling apply to the tray transcript.

Returning to the app or selecting a conversation does not itself dismiss unread
messages. Interact with the chat and reach the latest messages; after a short
700 ms settling interval, the client acknowledges the conversation. The divider
stays visible for that visit and disappears when you leave the chat or window.
Incoming messages in a chat you are already reading at the bottom need no new
boundary. Scrolling up keeps subsequent arrivals unread.

Boundaries are shared across views of the same conversation, scoped by server,
and held only in client memory. On a fresh launch/reconnect, server unread counts
seed a boundary within available history. No new server data, message content
storage, or protocol changes are introduced. If the first marked message is
deleted, the divider advances to the next available incoming message.

Validation: `bash scripts/test-unread-messages.sh` exercises real QML feeds,
server-scoped snapshots, offline/background arrivals, own messages, focus-only
returns, read acknowledgments, scroll position, deletion, and multiple tiles.
`WISP_TEST_THEME` selects the appearance profile for the fixture.
