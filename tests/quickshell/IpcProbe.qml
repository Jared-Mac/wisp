import QtQuick
import Quickshell
import Quickshell.Io

ShellRoot {
  id: root

  readonly property string socketPath: Quickshell.env("WISP_SOCKET")

  function handleLine(line) {
    try {
      var message = JSON.parse(line)
      if (message.type === "snapshot") {
        console.log("WISP_IPC_PROBE_OK")
        Qt.quit()
      }
    } catch (error) {
      console.error("WISP_IPC_PROBE_INVALID_JSON")
      Qt.quit()
    }
  }

  Socket {
    id: connection
    path: root.socketPath
    connected: true
    parser: SplitParser {
      splitMarker: "\n"
      onRead: function(line) { root.handleLine(line) }
    }
    onConnectionStateChanged: {
      if (!connected) return
      write(JSON.stringify({
        "v": 1,
        "id": "reliability-probe",
        "type": "command",
        "name": "hello",
        "args": { "client": "quickshell-reliability-probe" }
      }) + "\n")
      flush()
    }
  }

  Timer {
    interval: 5000
    running: true
    repeat: false
    onTriggered: {
      console.error("WISP_IPC_PROBE_TIMEOUT")
      Qt.quit()
    }
  }
}
