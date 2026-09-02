import QtQuick
import Quickshell.Io
import qs.Commons
import qs.Ui
import "app"

// Optional Omarchy adapter. All application content below the bar/popup
// boundary is shared with the standalone Quickshell frontend.
Panel {
  id: root
  moduleName: "dev.wisp"
  ipcTarget: "dev.wisp"
  manageIpc: false

  visible: true
  implicitWidth: barButton.implicitWidth
  implicitHeight: barButton.implicitHeight

  onOpenedChanged: if (opened) {
    Qt.callLater(function() { content.forceActiveFocus() })
  } else content.resetNavigation()

  WispTheme {
    id: pluginTheme
    foreground: Color.foreground
    background: Color.popups.background
    surface: Color.popups.background
    accent: Color.accent
    muted: Color.muted
    danger: Color.urgent
    cornerRadius: Style.cornerRadius
    spacingScale: Style.spacing.scale
    fontFamily: Style.font.family
    captionSize: Style.font.caption
    bodySize: Style.font.body
    titleSize: Style.font.title
  }

  WispBridge {
    id: bridge
    clientName: "omarchy-plugin"
  }

  Process {
    id: appLauncher
    command: ["wisp-ui", "app", "open"]
  }

  IpcHandler {
    target: root.ipcTarget
    function open(): void { root.open() }
    function close(): void { root.close() }
    function show(): void { root.open() }
    function hide(): void { root.close() }
    function toggle(): void { root.toggle() }
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

  WidgetButton {
    id: barButton
    anchors.fill: parent
    bar: root.bar
    text: bridge.barText
    active: bridge.hasError
    tooltipText: bridge.hasError ? bridge.errorMessage : "Wisp — friends and hangouts"
    onPressed: function(buttonCode) {
      if (buttonCode === Qt.RightButton) bridge.toggleMuted()
      else root.toggle()
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: barButton
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: content
    contentWidth: panel.fittedContentWidth(content.implicitWidth)
    contentHeight: panel.fittedContentHeight(content.implicitHeight, Style.space(620))

    WispContent {
      id: content
      anchors.fill: parent
      bridge: bridge
      theme: pluginTheme
      logoSource: Qt.resolvedUrl("app/assets/waveform.svg")
      presentation: "panel"
      contentPadding: 0
      showAppButton: true
      showCloseButton: false
      dismissOnNavigate: true
      onAppRequested: {
        root.close()
        if (!appLauncher.running) appLauncher.running = true
      }
      onCloseRequested: root.close()
    }
  }
}
