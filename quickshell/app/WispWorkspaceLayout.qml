import QtQuick
import Quickshell
import Quickshell.Io

Item {
  id: root
  visible: false
  property bool ready: false
  property alias dock: preferences.dock
  property alias activityWidth: preferences.activityWidth
  property alias activityHeight: preferences.activityHeight
  property alias activityColumnsRatio: preferences.activityColumnsRatio
  property alias activityRatio: preferences.activityRatio
  property alias activityCollapsed: preferences.activityCollapsed
  property alias roomsRatio: preferences.roomsRatio
  property alias trayRoomsCollapsed: preferences.trayRoomsCollapsed
  property alias trayChatFocused: preferences.trayChatFocused
  property alias chatTiles: preferences.chatTiles
  property alias streamsAsTiles: preferences.streamsAsTiles
  property alias channelsAsTiles: preferences.channelsAsTiles
  property alias incomingDmsAsTiles: preferences.incomingDmsAsTiles
  property alias selectedServerId: preferences.selectedServerId
  signal settingsSaved()
  signal settingsSaveFailed()
  signal streamPreferenceSaved()
  signal channelPreferenceSaved()
  property bool savingChannelPreference: false
  function setChannelsAsTiles(value) { savingChannelPreference = true; channelsAsTiles = value }
  property bool savingStreamPreference: false
  function setStreamsAsTiles(value) { savingStreamPreference = true; streamsAsTiles = value }
  signal resetRequested()
  property string error: ""
  function reset() {
    dock = "auto"; activityWidth = 0; activityHeight = 0; activityColumnsRatio = 0.58; activityRatio = 0.25; activityCollapsed = false; roomsRatio = 0
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
    onLoadFailed: function(error) {
      Qt.callLater(function() {
        var firstLoad = !root.ready
        root.ready = true
        // Only a missing first-run file gets the wider starting sidebar.
        // Existing automatic sizing and explicitly saved widths stay intact.
        if (firstLoad && error === FileViewError.FileNotFound && root.activityWidth === 0) {
          root.activityWidth = 280
          saveDelay.restart()
        }
      })
    }
    onAdapterUpdated: { root.error = ""; if (root.ready) saveDelay.restart() }
    onSaved: {
      root.settingsSaved()
      if (root.savingStreamPreference) { root.savingStreamPreference = false; root.streamPreferenceSaved() }
      if (root.savingChannelPreference) { root.savingChannelPreference = false; root.channelPreferenceSaved() }
    }
    onSaveFailed: { root.error = "Couldn't save the main window layout."; root.settingsSaveFailed() }
    JsonAdapter {
      id: preferences
      property string dock: "auto"
      property real activityWidth: 0 // Explicit sidebar width; zero keeps automatic sizing.
      property real activityHeight: 0 // Top/bottom dock height; independent of sidebar width.
      property real activityColumnsRatio: 0.58
      property real activityRatio: 0.25
      property bool activityCollapsed: false
      property real roomsRatio: 0 // Fit the room list until its divider is moved.
      property bool trayRoomsCollapsed: false
      property bool trayChatFocused: true
      property string chatTiles: "" // Main-window split tree; no message content.
      property bool streamsAsTiles: false
      property bool channelsAsTiles: true
      property bool incomingDmsAsTiles: true
      property string selectedServerId: ""
    }
  }
}
