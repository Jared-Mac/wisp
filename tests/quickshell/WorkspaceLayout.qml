import QtQuick
import Quickshell
import "app" as Wisp

ShellRoot {
  Wisp.WispWorkspaceLayout { id: layout }
  Timer {
    interval: 100; running: true
    onTriggered: {
      if (Quickshell.env("WISP_LAYOUT_RELOAD") !== "1") {
        if (layout.activityWidth !== 280) console.error("LAYOUT_FAILED first launch sidebar width")
        if (layout.streamsAsTiles) console.error("LAYOUT_FAILED streams should open windows by default")
        if (!layout.trayChatFocused) console.error("LAYOUT_FAILED tray should focus chat by default")
        layout.setStreamsAsTiles(true)
        if (!layout.channelsAsTiles) console.error("LAYOUT_FAILED channels should open new tiles by default")
        layout.dock = "right"; layout.activityWidth = 24; layout.activityRatio = 0.35
        layout.activityHeight = 270; layout.activityColumnsRatio = 0.64
        layout.roomsRatio = 0.6; layout.activityCollapsed = true
        layout.trayRoomsCollapsed = true
        layout.trayChatFocused = false
        layout.setChannelsAsTiles(false)
        layout.chatTiles = '{"key":"split","axis":"x","ratio":0.35,"a":{"key":"a","id":"room"},"b":{"key":"b","id":"dm"}}'
      }
    }
  }
  Timer {
    interval: 500; running: true
    onTriggered: {
      if (layout.activityWidth !== 24 || layout.dock !== "right" || Math.abs(layout.activityRatio - 0.35) > 0.001
          || layout.activityHeight !== 270 || Math.abs(layout.activityColumnsRatio - 0.64) > 0.001
          || Math.abs(layout.roomsRatio - 0.6) > 0.001 || !layout.activityCollapsed || !layout.trayRoomsCollapsed || layout.trayChatFocused || JSON.parse(layout.chatTiles).b.id !== "dm" || layout.channelsAsTiles || !layout.streamsAsTiles || layout.error)
        console.error("LAYOUT_FAILED persistence")
      else console.log("LAYOUT_OK")
      Qt.quit()
    }
  }
}
