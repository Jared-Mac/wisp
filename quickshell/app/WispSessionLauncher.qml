import QtQuick
import Quickshell.Io

// Start the account-aware launcher, never an unauthenticated standalone UI.
Item {
  id: root
  property bool daemonConnected: false
  signal completed(string action, int exitCode)

  function ensureRunning() {
    if (!daemonConnected && !ensureProcess.running) ensureProcess.running = true
  }

  function openApp() {
    if (!appProcess.running) appProcess.running = true
  }

  Process {
    id: ensureProcess
    command: ["env", "WISP_INTEGRATION=omarchy", "wisp", "--ensure-running"]
    onExited: function(code, status) { root.completed("ensure", code) }
  }

  Process {
    id: appProcess
    command: ["env", "WISP_INTEGRATION=omarchy", "wisp"]
    onExited: function(code, status) { root.completed("app", code) }
  }
}
