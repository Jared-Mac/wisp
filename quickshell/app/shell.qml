//@ pragma AppId dev.wisp

import QtQuick
import Quickshell
import Quickshell.Io

ShellRoot {
  id: app

  function openWindow() { appWindow.reveal() }
  function closeWindow() { appWindow.visible = false }
  function toggleWindow() { appWindow.visible ? closeWindow() : openWindow() }
  function validAnchor(value) {
    return value === "auto"
      || value === "bottom-right"
      || value === "bottom-left"
      || value === "top-right"
      || value === "top-left"
  }
  function primaryScreen() {
    for (var index = 0; index < Quickshell.screens.length; index++)
      if (Quickshell.screens[index].name === primaryScreenName)
        return Quickshell.screens[index]
    return Quickshell.screens.length > 0 ? Quickshell.screens[0] : null
  }
  function setAnchor(value) {
    if (!validAnchor(value)) return
    appSettings.anchor = value
    openWindow()
  }
  function activateFromTray(globalX, globalY) {
    var target = primaryScreen()
    var localX = target ? globalX - target.x : globalX
    var localY = target ? globalY - target.y : globalY
    var width = target ? target.width : 1
    var height = target ? target.height : 1
    var clickIsOnPrimary = localX >= 0 && localX < width && localY >= 0 && localY < height
    trayRight = clickIsOnPrimary ? localX >= width / 2 : true
    trayBottom = clickIsOnPrimary ? localY >= height / 2 : true
    autoAnchor = (trayBottom ? "bottom-" : "top-") + (trayRight ? "right" : "left")
    trayVerticalInset = clickIsOnPrimary
      ? Math.max(appTheme.space(44),
          (trayBottom ? height - localY : localY) + appTheme.space(22))
      : appTheme.space(52)
    toggleWindow()
  }

  readonly property string primaryScreenName: Quickshell.env("WISP_PRIMARY_SCREEN")
  property string autoAnchor: "bottom-right"
  property bool trayRight: true
  property bool trayBottom: true
  property int trayVerticalInset: appTheme.space(52)
  readonly property string resolvedAnchor: appSettings.anchor === "auto"
    ? autoAnchor : appSettings.anchor
  readonly property var selectedScreen: primaryScreen()
  readonly property var anchorController: ({
    "anchor": appSettings.anchor,
    "primaryScreen": app.selectedScreen,
    "setAnchor": function(value) { app.setAnchor(value) }
  })

  WispTheme { id: appTheme }

  FileView {
    id: settingsFile
    path: Quickshell.statePath("desktop.json")
    onAdapterUpdated: writeAdapter()

    JsonAdapter {
      id: appSettings
      property string anchor: "auto"
    }
  }

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
    function activate(x: int, y: int): void { app.activateFromTray(x, y) }
    function anchor(position: string): void { app.setAnchor(position) }
    function desktop(): string {
      return JSON.stringify({
        "visible": appWindow.visible,
        "anchor": appSettings.anchor,
        "resolved_anchor": app.resolvedAnchor,
        "screen": app.selectedScreen ? app.selectedScreen.name : null,
        "vertical_inset": appWindow.verticalInset
      })
    }
    function quit(): void { Qt.quit() }
  }

  IpcHandler {
    target: "dev.wisp.bridge"

    function status(): string {
      return JSON.stringify({
        "connected": bridge.daemonConnected,
        "socket": bridge.socketPath,
        "error": bridge.lastError,
        "self_status": bridge.selfStatusLabel,
        "snapshot": bridge.snapshot
      })
    }
  }

  WispWindow {
    id: appWindow
    visible: false
    bridge: bridge
    theme: appTheme
    screen: app.selectedScreen
    anchorMode: app.resolvedAnchor
    verticalInset: {
      var sameEdge = anchorMode.indexOf("bottom-") === 0
        ? app.trayBottom : !app.trayBottom
      return sameEdge ? app.trayVerticalInset : appTheme.space(12)
    }
    anchorController: app.anchorController
    onHideRequested: app.closeWindow()
  }
}
