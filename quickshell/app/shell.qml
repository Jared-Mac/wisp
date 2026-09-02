//@ pragma AppId dev.wisp

import QtQuick
import Quickshell
import Quickshell.Io

ShellRoot {
  id: app

  function openWindow() { appWindow.reveal() }
  function closeWindow() { appWindow.visible = false }
  function toggleWindow() { appWindow.visible ? closeWindow() : openWindow() }

  WispTheme { id: appTheme }

  WispBridge {
    id: bridge
    clientName: "quickshell-app"
  }

  IpcHandler {
    target: "dev.wisp"

    function open(): void { app.openWindow() }
    function close(): void { app.closeWindow() }
    function show(): void { app.openWindow() }
    function hide(): void { app.closeWindow() }
    function toggle(): void { app.toggleWindow() }
    function quit(): void { Qt.quit() }
  }

  IpcHandler {
    target: "dev.wisp.bridge"

    function status(): string {
      return JSON.stringify({
        "connected": bridge.daemonConnected,
        "socket": bridge.socketPath,
        "error": bridge.lastError,
        "snapshot": bridge.snapshot
      })
    }
  }

  WispWindow {
    id: appWindow
    visible: true
    bridge: bridge
    theme: appTheme
    onHideRequested: app.closeWindow()
  }
}
