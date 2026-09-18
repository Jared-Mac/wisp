import QtQuick
import Quickshell
import "app"

ShellRoot {
  id: test
  property int completions: 0
  WispSessionLauncher {
    id: launcher
    daemonConnected: true
    onCompleted: function(action, exitCode) {
      if (Quickshell.env("WISP_TEST_EXIT_CODE") === "127") {
        if (exitCode !== 127 || !clientMissing) { Qt.exit(1); return }
        console.log("SESSION_LAUNCHER_MISSING_OK")
        Qt.quit()
        return
      }
      if (exitCode !== 0 || clientMissing) Qt.exit(1)
      test.completions++
      if (test.completions === 1 && action === "ensure") {
        daemonConnected = true
        ensureRunning() // Connected popup must not launch anything.
        openApp()
        openApp() // Repeated actions must not spawn duplicate processes.
      } else if (test.completions === 2 && action === "app") {
        console.log("SESSION_LAUNCHER_OK")
        Qt.quit()
      } else Qt.exit(1)
    }
  }
  Component.onCompleted: {
    launcher.ensureRunning() // No process for an existing daemon.
    launcher.daemonConnected = false
    launcher.ensureRunning()
    launcher.ensureRunning()
  }
  Timer { interval: 5000; running: true; onTriggered: Qt.exit(1) }
}
