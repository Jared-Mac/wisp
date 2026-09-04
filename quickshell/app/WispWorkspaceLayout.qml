import QtQuick
import Quickshell
import Quickshell.Io

Item {
  id: root
  visible: false
  property bool ready: false
  property alias dock: preferences.dock
  property alias activityRatio: preferences.activityRatio
  property alias activityCollapsed: preferences.activityCollapsed
  property alias roomsRatio: preferences.roomsRatio
  property alias trayRoomsCollapsed: preferences.trayRoomsCollapsed
  property alias chatTiles: preferences.chatTiles
  signal settingsSaved()
  signal settingsSaveFailed()
  signal resetRequested()
  property string error: ""
  function reset() {
    dock = "auto"; activityRatio = 0.25; activityCollapsed = false; roomsRatio = 0
    resetRequested()
  }
  function bounded(value, fallback) { return isFinite(value) ? Math.max(0.08, Math.min(0.85, value)) : fallback }
  Timer { id: saveDelay; interval: 200; onTriggered: settings.writeAdapter() }
  FileView {
    id: settings
    path: (Quickshell.env("XDG_CONFIG_HOME") || Quickshell.env("HOME") + "/.config") + "/wisp/workspace.json"
    blockLoading: true; blockWrites: true; atomicWrites: true; printErrors: false
    watchChanges: true; onFileChanged: reload()
    onLoaded: Qt.callLater(function() { root.ready = true })
    onLoadFailed: Qt.callLater(function() { root.ready = true })
    onAdapterUpdated: { root.error = ""; if (root.ready) saveDelay.restart() }
    onSaved: root.settingsSaved()
    onSaveFailed: { root.error = "Couldn't save the main window layout."; root.settingsSaveFailed() }
    JsonAdapter {
      id: preferences
      property string dock: "auto"
      property real activityRatio: 0.25
      property bool activityCollapsed: false
      property real roomsRatio: 0 // Fit the room list until its divider is moved.
      property bool trayRoomsCollapsed: false
      property string chatTiles: "" // Main-window split tree; no message content.
    }
  }
}
