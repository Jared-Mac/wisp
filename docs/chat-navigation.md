# Chat navigation

Incoming DMs open in their own tile by default without changing the active chat or raising the window. Turn this off in Settings → Notifications → Chat navigation. Existing tiles are reused; the eight-tile limit leaves additional arrivals in the inbox. Reconnecting and loading history never create extra tiles.

The inbox lists pending chats on the selected server. Friends, channel names, room names, and voice participants show pending-message indicators that open the matching conversation. Friends' names open text chat. Only the dedicated presence icon (or an explicit menu action) knocks or joins voice. Other members sit below the collapsible Friends list; friendship actions remain available there.

Focused chats become read after interaction at the latest message. Clicking New jumps to the unread boundary and acknowledges the chat immediately; replying also acknowledges it. Background tiles remain unread. Counts clear locally while the server confirms, and recover if that request fails.

Share, camera, and disconnect share the room-control row. The soundboard keeps the compact menu and server-sidebar shortcut from the soundboard release. Stream start/stop cues include your own screen; separate viewer cues indicate arrivals and departures from your stream. Each category can be muted or assigned custom sounds in Notifications.

Hovering the server selector measures an HTTP round trip to its health endpoint, cached for ten seconds while hovering. Nothing polls for ping while the pointer is elsewhere. Settings → Appearance → Show user avatars applies to the updated lists and defaults on.
