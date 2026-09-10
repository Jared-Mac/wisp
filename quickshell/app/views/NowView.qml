import QtQuick
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  property bool adaptive: false
  readonly property bool narrow: adaptive && width < theme.space(140)
  readonly property bool tiny: adaptive && width < theme.space(56)
  signal joined()
  signal roomLeft()
  signal cameraRequested()
  property bool showHeader: true
  readonly property var visibleHangouts: root.bridge.temporaryCalls || []
  width: parent ? parent.width : 0
  spacing: root.theme.spacing.xs
  visible: visibleHangouts.length > 0

  Item {
    visible: root.showHeader && !root.tiny
    width: parent.width; height: root.theme.space(20)
    Text {
    anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter
    text: root.theme.tui ? "/calls · " + root.visibleHangouts.length : root.theme.friendly ? "Calls" : "CALLS"
    color: root.theme.roomSectionColor
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.weight: Font.Bold
    font.letterSpacing: root.theme.terminal ? 1 : 0
    }
  }

  Repeater {
    model: root.visibleHangouts
    delegate: Column {
      id: hangoutEntry
      required property var modelData
      width: root.width
      spacing: root.theme.spacing.sm

      HangoutCard {
        width: parent.width
        hangout: hangoutEntry.modelData
        bridge: root.bridge
        theme: root.theme; adaptive: root.adaptive
        onJoined: root.joined()
      }

    }
  }
}
