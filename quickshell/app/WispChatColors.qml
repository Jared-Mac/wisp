import QtQuick
import Quickshell
import Quickshell.Io
import "ChatColors.js" as ChatColors

Item {
  id: root
  visible: false
  property var conversations: []
  property bool ready: false
  property string error: ""
  readonly property var assignments: ChatColors.parse(preferences.assignments)
  function colorFor(id, fallback) { return assignments[String(id || "")] || fallback }
  function synchronize() {
    if (!ready || !conversations.length) return
    var encoded = ChatColors.assign(preferences.assignments, conversations.map(function(c) { return String(c.id || "") }))
    if (encoded === preferences.assignments) return
    preferences.assignments = encoded
    settings.writeAdapter()
  }
  onConversationsChanged: synchronize()
  FileView {
    id: settings
    path: (Quickshell.env("XDG_CONFIG_HOME") || Quickshell.env("HOME") + "/.config") + "/wisp/chat-colors.json"
    blockLoading: true; blockWrites: true; atomicWrites: true; printErrors: false
    watchChanges: true
    onFileChanged: reload()
    onLoaded: { root.ready = true; root.synchronize() }
    onLoadFailed: { root.ready = true; root.synchronize() }
    onSaved: root.error = ""
    onSaveFailed: root.error = "Couldn't save chat colors on this device."
    JsonAdapter { id: preferences; property string assignments: "{}" }
  }
}
