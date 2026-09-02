import QtQuick
import Quickshell

FloatingWindow {
  id: root

  required property var bridge
  required property var theme
  signal hideRequested()

  title: "Wisp"
  implicitWidth: theme.space(460)
  implicitHeight: theme.space(700)
  minimumSize: Qt.size(theme.space(390), theme.space(480))
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
      showCloseButton: true
      dismissOnNavigate: false
      onCloseRequested: root.hideRequested()
    }
  }
}
