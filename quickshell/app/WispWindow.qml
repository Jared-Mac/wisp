import QtQuick
import Quickshell
import "components"

// Normal application host. Compact anchored surfaces live in
// WispPanelWindow.qml and the optional Omarchy Panel.qml adapter.
FloatingWindow {
  id: root

  required property var bridge
  required property var theme
  property bool localPreviewsPoppedOut: false
  readonly property bool chatVisible: visible && content.showingChats
  signal hideRequested()
  signal popOutLocalPreviewsRequested()

  title: "Wisp"
  implicitWidth: theme.space(1180)
  implicitHeight: theme.space(900)
  minimumSize: Qt.size(theme.space(360), theme.space(520))
  color: theme.background

  function reveal() {
    visible = true
    minimized = false
    Qt.callLater(function() {
      content.forceActiveFocus()
      if (root.contentItem && root.contentItem.Window.window)
        root.contentItem.Window.window.requestActivate()
    })
  }

  onVisibleChanged: {
    if (visible) Qt.callLater(function() { content.forceActiveFocus() })
    else content.resetNavigation()
  }
  onClosed: hideRequested()

  Rectangle {
    anchors.fill: parent
    color: root.theme.background
    SurfaceOutline { theme: root.theme; radius: 0 }

    WispContent {
      id: content
      anchors.fill: parent
      bridge: root.bridge
      theme: root.theme
      logoSource: Qt.resolvedUrl("assets/waveform.svg")
      presentation: "app"
      showCloseButton: true
      dismissOnNavigate: false
      localPreviewsPoppedOut: root.localPreviewsPoppedOut
      onPopOutLocalPreviewsRequested: root.popOutLocalPreviewsRequested()
      onCloseRequested: root.hideRequested()
      onAppRequested: { content.goHome(); root.reveal() }
    }
  }
}
