import QtQuick
import Quickshell
import Quickshell.Io

Item {
  id: root
  property string path
  property bool connected: false
  property bool rejected: false
  signal read(string line)
  signal unsafeResponse()
  function write(data) { if (connected) relay.write(data) }
  function flush() {} // Process.write delivers commands directly to its pipe.
  function reject() {
    if (rejected) return
    rejected = true
    connected = false
    relay.running = false
    unsafeResponse()
  }
  Process {
    id: relay
    command: ["python3", "-I", "-u", decodeURIComponent(String(Qt.resolvedUrl("ipc-transport.py")).replace(/^file:\/\//, "")), root.path]
    running: true
    stdinEnabled: true
    // Only the local relay writes here. It bounds and validates each frame
    // before forwarding, then waits for @ack before reading another frame.
    stdout: SplitParser {
      onRead: function(line) {
        if (root.rejected) return
        if (line === "@connected") { root.connected = true; return }
        if (line === "@rejected" || line.length > 8 * 1024 * 1024) { root.reject(); return }
        root.read(line)
        if (!root.rejected) relay.write("@ack\n")
      }
    }
    onExited: function(code) {
      root.connected = false
      if (code === 2) root.reject()
    }
  }
}
