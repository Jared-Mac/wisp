import QtQuick
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  signal joined()
  signal roomLeft()
  width: parent ? parent.width : 0
  spacing: root.theme.spacing.sm

  Text {
    text: "NOW"
    color: root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.weight: Font.Bold
  }

  Text {
    visible: root.bridge.hangouts.length === 0
    text: "It’s quiet right now."
    color: root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.body
  }

  Repeater {
    model: root.bridge.hangouts
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
