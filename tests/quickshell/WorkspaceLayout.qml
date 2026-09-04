import QtQuick
import Quickshell
import "app" as Wisp

ShellRoot {
  Wisp.WispWorkspaceLayout { id: layout }
  Timer {
    interval: 100; running: true
    onTriggered: {
      if (Quickshell.env("WISP_LAYOUT_RELOAD") !== "1") {
        layout.dock = "right"; layout.activityRatio = 0.35
        layout.roomsRatio = 0.6; layout.activityCollapsed = true
        layout.trayRoomsCollapsed = true
        layout.chatTiles = '{"key":"split","axis":"x","ratio":0.35,"a":{"key":"a","id":"room"},"b":{"key":"b","id":"dm"}}'
      }
    }
  }
  Timer {
    interval: 500; running: true
    onTriggered: {
      if (layout.dock !== "right" || Math.abs(layout.activityRatio - 0.35) > 0.001
          || Math.abs(layout.roomsRatio - 0.6) > 0.001 || !layout.activityCollapsed || !layout.trayRoomsCollapsed || JSON.parse(layout.chatTiles).b.id !== "dm" || layout.error)
        console.error("LAYOUT_FAILED persistence")
      else console.log("LAYOUT_OK")
      Qt.quit()
    }
  }
}
