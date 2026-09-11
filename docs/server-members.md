# Server members and friend requests

Friends is the primary list for accepted friends, with presence, favorites and
voice actions. The Other members section shows nonfriends and pending requests
from the active server, without repeating friends or your own account. It is
shown by default and remembers an explicit collapse independently. Its search
button opens the full server directory,
including friends and yourself, with a server selector and friend-request filter.
Both main and tray views use the same controls. Chat author names and room
participants also open a person menu.

Authenticated members can see account IDs and display names on their server.
The directory does not expose login names, credentials, private-room membership,
or nonfriends' presence. Being a server member does not create a friendship.

Requests require the recipient's acceptance. Duplicate or crossed sends leave
one pending request; retries are idempotent. Either participant can dismiss the
pending request, and only its recipient can accept it. Acceptance creates the
existing shared friendship and follows the client's normal encrypted contact
enrollment and pinned-key checks. No DM, voice connection or publication starts
until the user explicitly requests it.

Member catalogs refresh on connection, account profile changes, friendship
events and manual refresh. Ordinary presence snapshots do not fetch the
directory. Open member menus and keyboard focus survive request-state changes.

Validation:

- `cargo test -p wisp-server friendships --lib`
- `cargo test -p wispd member_directory_commands_use_the_selected_server_account`
- `cargo test -p wispd two_clients_encrypt_restore_and_admit_a_friend_without_manual_verification`
- `bash scripts/test-friendships-ui.sh`

The UI fixture covers main and tray member lists, narrow sidebars, server scope,
search, request states, retry, persisted preferences and chat-author actions in
Soft Graphite, Daylight, Hearth and Clean TUI. Tests use isolated accounts and
never send requests to production users.
