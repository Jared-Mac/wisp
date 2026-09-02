import QtQuick
import Quickshell

PanelWindow {
  id: root

  required property var bridge
  required property var theme
  required property string anchorMode
  required property var anchorController
  property int verticalInset: theme.space(12)
  signal hideRequested()

  implicitWidth: theme.space(460)
  implicitHeight: theme.space(700)
  color: "transparent"
  focusable: true
  aboveWindows: true
  exclusiveZone: -1

  anchors {
    left: root.anchorMode.endsWith("left")
    right: root.anchorMode.endsWith("right")
    top: root.anchorMode.startsWith("top")
    bottom: root.anchorMode.startsWith("bottom")
  }

  margins {
    left: root.anchorMode.endsWith("left") ? root.theme.space(12) : 0
    right: root.anchorMode.endsWith("right") ? root.theme.space(12) : 0
    top: root.anchorMode.startsWith("top") ? root.verticalInset : 0
    bottom: root.anchorMode.startsWith("bottom") ? root.verticalInset : 0
  }

  function reveal() {
    visible = true
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
      anchorController: root.anchorController
      showCloseButton: true
      dismissOnNavigate: false
      onCloseRequested: root.hideRequested()
    }
  }
}
