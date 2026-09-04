import QtQuick
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  signal joined()
  signal roomLeft()
  signal cameraRequested()
  property bool showHeader: true
  readonly property var visibleHangouts: root.bridge.hangouts || []
  width: parent ? parent.width : 0
  spacing: root.theme.spacing.xs
  visible: true

  Item {
    visible: root.showHeader
    width: parent.width; height: root.theme.space(20)
    Text {
    anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter
    text: "ROOMS"
    color: root.theme.performative ? root.theme.warning : root.theme.muted
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
        theme: root.theme
        onJoined: root.joined()
      }

      MediaControls {
        visible: root.bridge.selfState.hangout_id === hangoutEntry.modelData.id
        width: parent.width
        bridge: root.bridge
        theme: root.theme
        onLeaveRequested: root.roomLeft()
        onCameraRequested: root.cameraRequested()
      }
    }
  }
}
