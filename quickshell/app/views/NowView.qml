import QtQuick
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  signal joined()
  signal roomLeft()
  readonly property var visibleHangouts: root.bridge.hangouts || []
  width: parent ? parent.width : 0
  spacing: root.theme.spacing.sm
  visible: visibleHangouts.length > 0

  Text {
    text: "HANGOUTS"
    color: root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.weight: Font.Bold
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
      }
    }
  }
}
