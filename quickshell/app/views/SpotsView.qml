import QtQuick
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  width: parent ? parent.width : 0
  spacing: root.theme.spacing.xs
  Repeater {
    model: root.bridge.spots || []
    RoomCard { required property var modelData; width: root.width; room: modelData; bridge: root.bridge; theme: root.theme }
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap
    visible: !root.bridge.spots.length
    text: "No rooms yet. Create one with [+]."
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
}
