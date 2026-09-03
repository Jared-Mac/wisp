import QtQuick
import Quickshell

// Normal application host. Compact anchored surfaces live in
// WispPanelWindow.qml and the optional Omarchy Panel.qml adapter.
FloatingWindow {
  id: root

  required property var bridge
  required property var theme
  property bool localPreviewsPoppedOut: false
  signal hideRequested()
  signal popOutLocalPreviewsRequested()

  title: "Wisp"
  implicitWidth: theme.space(960)
  implicitHeight: theme.space(720)
  minimumSize: Qt.size(theme.space(420), theme.space(520))
  color: theme.background

  function reveal() {
    visible = true
    minimized = false
    Qt.callLater(function() {
      content.forceActiveFocus()
      if (root.contentItem && root.contentItem.window)
        root.contentItem.window.requestActivate()
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
    }
  }
}
