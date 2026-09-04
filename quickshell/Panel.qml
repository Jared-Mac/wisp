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

  WispAppearance { id: appearance; environment: "omarchy" }

  WispTheme {
    id: pluginTheme
    profile: "legacy"
    appearanceController: appearance
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
    delegateMediaToDesktop: true
    onDesktopWatchRequested: function(participant, source, open) {
      var launcher = mediaLauncher.createObject(root, {command:["env","WISP_INTEGRATION=omarchy","wisp-ui","media",open ? "watch" : "stop",participant,source]})
      launcher.running = true
    }
  }
  Component {
    id: mediaLauncher
    Process {
      onExited: function(code, status) {
        if (code !== 0) bridge.lastError = "Couldn't open the stream in the desktop workspace."
        destroy()
      }
    }
  }

  Process {
    id: appLauncher
    command: ["env", "WISP_INTEGRATION=omarchy", "wisp-ui", "app", "open"]
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
    text: ""
    labelVisible: false
    hasVisualContent: true
    fixedWidth: barVisual.implicitWidth + Style.space(14)
    active: bridge.hasError || bridge.knocks.length > 0
      || bridge.unreadMessages > 0 || bridge.sharing || bridge.cameraActive
    tooltipText: bridge.barTooltip
    onPressed: function(buttonCode) {
      if (buttonCode === Qt.RightButton) bridge.toggleMuted()
      else root.toggle()
    }

    Row {
      id: barVisual
      anchors.centerIn: parent
      spacing: Style.space(5)

      Item {
        width: Style.space(26)
        height: width

        Image {
          anchors.centerIn: parent
          width: Style.space(20)
          height: width
          source: Qt.resolvedUrl("app/assets/waveform.svg")
          fillMode: Image.PreserveAspectFit
          opacity: bridge.daemonConnected ? 1 : 0.42
        }

        Rectangle {
          id: screenBadge
          visible: bridge.sharing
          anchors.left: parent.left
          anchors.top: parent.top
          width: Style.space(9)
          height: width
          radius: width / 2
          color: "#32e6f4"
          border.width: 1
          border.color: Color.background

          Rectangle {
            anchors.centerIn: parent
            width: parent.width * 0.55
            height: parent.height * 0.38
            radius: 1
            color: "transparent"
            border.width: 1
            border.color: "#151821"
          }
        }

        Rectangle {
          id: cameraBadge
          visible: bridge.cameraActive
          anchors.right: parent.right
          anchors.top: parent.top
          width: Style.space(9)
          height: width
          radius: width / 2
          color: "#48dc96"
          border.width: 1
          border.color: Color.background

          Rectangle {
            anchors.centerIn: parent
            anchors.horizontalCenterOffset: -parent.width * 0.07
            width: parent.width * 0.52
            height: parent.height * 0.4
            radius: 1
            color: "#151821"
          }

          Rectangle {
            anchors.left: parent.horizontalCenter
            anchors.leftMargin: parent.width * 0.18
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width * 0.2
            height: parent.height * 0.28
            radius: 1
            color: "#151821"
          }
        }

        Rectangle {
          visible: bridge.remoteVideoAvailable && !bridge.sharing && !bridge.cameraActive
          anchors.right: parent.right
          anchors.top: parent.top
          width: Style.space(9)
          height: width
          radius: width / 2
          color: Color.accent
          border.width: 1
          border.color: Color.background

          Rectangle {
            anchors.centerIn: parent
            width: parent.width * 0.55
            height: parent.height * 0.38
            radius: 1
            color: "transparent"
            border.width: 1
            border.color: "#151821"
          }
        }

        Rectangle {
          visible: bridge.unreadMessages > 0
          anchors.left: parent.left
          anchors.bottom: parent.bottom
          width: Math.max(Style.space(9), unreadText.implicitWidth + Style.space(4))
          height: Style.space(9)
          radius: height / 2
          color: Color.accent
          border.width: 1
          border.color: Color.background

          Text {
            id: unreadText
            anchors.centerIn: parent
            text: bridge.unreadMessages > 99 ? "99+" : String(bridge.unreadMessages)
            color: "white"
            font.family: barButton.fontFamily
            font.pixelSize: Math.max(6, parent.height * 0.58)
            font.bold: true
            renderType: Text.NativeRendering
          }
        }

        Rectangle {
          visible: bridge.effectiveMuted
          anchors.right: parent.right
          anchors.bottom: parent.bottom
          width: Style.space(9)
          height: width
          radius: width / 2
          color: bridge.selfState.deafened ? "#ff5c6c" : "#f5b94c"
          border.width: 1
          border.color: Color.background

          Text {
            anchors.centerIn: parent
            text: bridge.selfState.deafened ? "×" : "/"
            color: "#151821"
            font.pixelSize: parent.height * 0.9
            font.bold: true
          }
        }
      }

      Text {
        visible: text.length > 0
        anchors.verticalCenter: parent.verticalCenter
        text: bridge.barLabel
        color: bridge.hasError ? Color.urgent : barButton.foreground
        font.family: barButton.fontFamily
        font.pixelSize: barButton.fontSize
        renderType: Text.NativeRendering
      }
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
    contentHeight: panel.fittedContentHeight(content.implicitHeight, Style.space(800))

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
