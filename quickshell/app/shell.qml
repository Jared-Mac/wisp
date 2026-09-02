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
  function screenAt(globalX, globalY) {
    for (var index = 0; index < Quickshell.screens.length; index++) {
      var candidate = Quickshell.screens[index]
      if (globalX >= candidate.x && globalX < candidate.x + candidate.width
          && globalY >= candidate.y && globalY < candidate.y + candidate.height)
        return candidate
    }
    return Quickshell.screens.length > 0 ? Quickshell.screens[0] : null
  }
  function selectScreen(name) {
    if (name && name !== "auto") {
      for (var index = 0; index < Quickshell.screens.length; index++)
        if (Quickshell.screens[index].name === name) return Quickshell.screens[index]
    }
    if (transientScreenName) {
      for (var transientIndex = 0; transientIndex < Quickshell.screens.length; transientIndex++)
        if (Quickshell.screens[transientIndex].name === transientScreenName)
          return Quickshell.screens[transientIndex]
    }
    return Quickshell.screens.length > 0 ? Quickshell.screens[0] : null
  }
  function setAnchor(value) {
    if (!validAnchor(value)) return
    appSettings.anchor = value
    openWindow()
  }
  function setScreen(value) {
    appSettings.screen = value || "auto"
    transientScreenName = ""
    openWindow()
  }
  function activateFromTray(globalX, globalY) {
    var target = screenAt(globalX, globalY)
    if (target) transientScreenName = target.name
    var localX = target ? globalX - target.x : globalX
    var localY = target ? globalY - target.y : globalY
    var width = target ? target.width : 1
    var height = target ? target.height : 1
    trayRight = localX >= width / 2
    trayBottom = localY >= height / 2
    autoAnchor = (trayBottom ? "bottom-" : "top-") + (trayRight ? "right" : "left")
    trayVerticalInset = Math.max(appTheme.space(44),
      (trayBottom ? height - localY : localY) + appTheme.space(22))
    toggleWindow()
  }

  property string transientScreenName: ""
  property string autoAnchor: "bottom-right"
  property bool trayRight: true
  property bool trayBottom: true
  property int trayVerticalInset: appTheme.space(52)
  readonly property string resolvedAnchor: appSettings.anchor === "auto"
    ? autoAnchor : appSettings.anchor
  readonly property var selectedScreen: selectScreen(appSettings.screen)
  readonly property var anchorController: ({
    "anchor": appSettings.anchor,
    "screen": appSettings.screen,
    "screens": Quickshell.screens,
    "setAnchor": function(value) { app.setAnchor(value) },
    "setScreen": function(value) { app.setScreen(value) }
  })

  WispTheme { id: appTheme }

  FileView {
    id: settingsFile
    path: Quickshell.statePath("desktop.json")
    onAdapterUpdated: writeAdapter()

    JsonAdapter {
      id: appSettings
      property string anchor: "auto"
      property string screen: "auto"
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
