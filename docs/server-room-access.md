# Server room access

Ordinary rooms are visible to every account on the server, including people who
join through a server invitation. Selecting a room opens its chat; joining voice
still requires an explicit action.

Server owners and admins can create a room as **Private / invite-only**, or change
that setting in Room settings or Server settings. Private rooms are hidden from
people without room access. Changing a room to private preserves its current
members and explicit invitations, and stops automatic admissions for new members.

Public discovery does not bypass encrypted chat membership. Before a signed
admission finishes, a new member sees room metadata and can join voice, while the
chat composer shows that encrypted access is pending. No message history or
chat membership is exposed by this preview. An authorized member's client must
be online to sign encrypted admissions through the existing trust checks.

Migration 23 makes existing account-server rooms public, because earlier versions
marked every room private implicitly without offering an admin privacy setting.
It preserves signed rosters and message history. Subsequent restarts preserve
every explicit private/public choice. Direct messages and separate text-channel
membership are unchanged.
