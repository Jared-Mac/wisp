//@ pragma AppId dev.wisp

import QtQuick
import Quickshell
import Quickshell.Io

ShellRoot {
  id: app

  function openApp() {
    panelWindow.visible = false
    appWindow.reveal()
  }
  function closeApp() { appWindow.visible = false }
  function toggleApp() { appWindow.visible ? closeApp() : openApp() }

  function openPanel() { panelWindow.reveal() }
  function closePanel() { panelWindow.visible = false }
  function togglePanel() { panelWindow.visible ? closePanel() : openPanel() }
  function popOutLocalPreviews() {
    localPreviewsPoppedOut = true
    previewWindow.reveal()
  }
  function dockLocalPreviews() {
    previewWindow.visible = false
    localPreviewsPoppedOut = false
  }

  function validAnchor(value) {
    return value === "auto"
      || value === "bottom-right"
      || value === "bottom-left"
      || value === "top-right"
      || value === "top-left"
  }
  function screenNamed(name) {
    for (var index = 0; index < Quickshell.screens.length; index++)
      if (Quickshell.screens[index].name === name)
        return Quickshell.screens[index]
    return null
  }
  function primaryScreen() {
    return screenNamed(primaryScreenName)
      || (Quickshell.screens.length > 0 ? Quickshell.screens[0] : null)
  }
  function screenAt(globalX, globalY) {
    for (var index = 0; index < Quickshell.screens.length; index++) {
      var candidate = Quickshell.screens[index]
      if (globalX >= candidate.x && globalX < candidate.x + candidate.width
          && globalY >= candidate.y && globalY < candidate.y + candidate.height)
        return candidate
    }
    return null
  }
  function panelScreen() {
    return screenNamed(activePanelScreenName) || primaryScreen()
  }
  function setAnchor(value) {
    if (!validAnchor(value)) return
    appSettings.anchor = value
    openPanel()
  }
  function activateFromTray(globalX, globalY) {
    var target = primaryScreen()
    if (target) activePanelScreenName = target.name
    var localX = target ? globalX - target.x : globalX
    var localY = target ? globalY - target.y : globalY
    var width = target ? target.width : 1
    var height = target ? target.height : 1
    var clickIsOnTarget = localX >= 0 && localX < width
      && localY >= 0 && localY < height
    trayRight = clickIsOnTarget ? localX >= width / 2 : true
    trayBottom = clickIsOnTarget ? localY >= height / 2 : true
    autoAnchor = (trayBottom ? "bottom-" : "top-")
      + (trayRight ? "right" : "left")
    trayVerticalInset = clickIsOnTarget
      ? Math.max(appTheme.space(44),
          (trayBottom ? height - localY : localY) + appTheme.space(22))
      : appTheme.space(52)
    togglePanel()
  }
  function appDesktop() {
    return JSON.stringify({
      "visible": appWindow.visible,
      "width": appWindow.width,
      "height": appWindow.height,
      "wide_layout": appWindow.width >= appTheme.space(760)
    })
  }
  function panelDesktop() {
    return JSON.stringify({
      "visible": panelWindow.visible,
      "width": panelWindow.width,
      "height": panelWindow.height,
      "anchor": appSettings.anchor,
      "resolved_anchor": app.resolvedAnchor,
      "screen": app.selectedPanelScreen ? app.selectedPanelScreen.name : null,
      "vertical_inset": panelWindow.verticalInset
    })
  }

  readonly property string primaryScreenName: Quickshell.env("WISP_PRIMARY_SCREEN")
  property string activePanelScreenName: ""
  property string autoAnchor: "bottom-right"
  property bool trayRight: true
  property bool trayBottom: true
  property int trayVerticalInset: appTheme.space(52)
  property bool localPreviewsPoppedOut: false
  readonly property string resolvedAnchor: appSettings.anchor === "auto"
    ? autoAnchor : appSettings.anchor
  readonly property var selectedPanelScreen: panelScreen()
  readonly property var anchorController: ({
    "anchor": appSettings.anchor,
    "screen": app.selectedPanelScreen,
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
    clientName: "quickshell-desktop"
    notificationSoundsEnabled: true
    appFocused: (appWindow.visible && !!appWindow.contentItem.window && appWindow.contentItem.window.active)
      || (panelWindow.visible && !!panelWindow.contentItem.window && panelWindow.contentItem.window.active)
      || (previewWindow.visible && !!previewWindow.contentItem.window && previewWindow.contentItem.window.active)
    chatVisible: appWindow.chatVisible || panelWindow.chatVisible
  }

  // Compatibility endpoint: direct Wisp launches now mean the full app.
  IpcHandler {
    target: "dev.wisp"
    function open(): void { app.openApp() }
    function close(): void { app.closeApp() }
    function show(): void { app.openApp() }
    function hide(): void { app.closeApp() }
    function toggle(): void { app.toggleApp() }
    function activate(x: int, y: int): void { app.activateFromTray(x, y) }
    function anchor(position: string): void { app.setAnchor(position) }
    function desktop(): string { return app.appDesktop() }
    function quit(): void { Qt.quit() }
  }

  IpcHandler {
    target: "dev.wisp.app"
    function open(): void { app.openApp() }
    function close(): void { app.closeApp() }
    function show(): void { app.openApp() }
    function hide(): void { app.closeApp() }
    function toggle(): void { app.toggleApp() }
    function desktop(): string { return app.appDesktop() }
  }

  IpcHandler {
    target: "dev.wisp.panel"
    function open(): void { app.openPanel() }
    function close(): void { app.closePanel() }
    function show(): void { app.openPanel() }
    function hide(): void { app.closePanel() }
    function toggle(): void { app.togglePanel() }
    function activate(x: int, y: int): void { app.activateFromTray(x, y) }
    function anchor(position: string): void { app.setAnchor(position) }
    function desktop(): string { return app.panelDesktop() }
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

  WispPanelWindow {
    id: panelWindow
    visible: false
    bridge: bridge
    theme: appTheme
    screen: app.selectedPanelScreen
    anchorMode: app.resolvedAnchor
    verticalInset: {
      var sameEdge = anchorMode.indexOf("bottom-") === 0
        ? app.trayBottom : !app.trayBottom
      return sameEdge ? app.trayVerticalInset : appTheme.space(12)
    }
    anchorController: app.anchorController
    localPreviewsPoppedOut: app.localPreviewsPoppedOut
    onAppRequested: app.openApp()
    onPopOutLocalPreviewsRequested: app.popOutLocalPreviews()
    onHideRequested: app.closePanel()
  }

  WispWindow {
    id: appWindow
    visible: false
    bridge: bridge
    theme: appTheme
    localPreviewsPoppedOut: app.localPreviewsPoppedOut
    onPopOutLocalPreviewsRequested: app.popOutLocalPreviews()
    onHideRequested: app.closeApp()
  }

  WispPreviewWindow {
    id: previewWindow
    visible: false
    bridge: bridge
    theme: appTheme
    onDockRequested: app.dockLocalPreviews()
  }
}
