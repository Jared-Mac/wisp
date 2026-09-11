import QtQuick
import "../components"

Flow {
  id: root
  required property var bridge
  required property var theme
  property bool adaptive: false
  readonly property bool narrow: adaptive && width < theme.space(140)
  readonly property bool tiny: adaptive && width < theme.space(56)
  property bool mainApp: false
  property bool showConnectedInvite: mainApp
  property bool horizontal: false
  readonly property int columns: horizontal ? Math.max(1, Math.floor((width + spacing) / (theme.space(250) + spacing))) : 1
  readonly property real cardWidth: (width - spacing * (columns - 1)) / columns
  width: parent ? parent.width : 0
  spacing: root.theme.spacing.xs
  Repeater {
    model: root.bridge.spots || []
    RoomCard { required property var modelData; width: root.cardWidth; room: modelData; bridge: root.bridge; theme: root.theme; mainApp: root.mainApp; showConnectedInvite:root.showConnectedInvite; adaptive: root.adaptive }
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap
    visible: !root.bridge.spots.length && !root.narrow
    text: root.theme.friendly ? "No rooms yet. Use + to create one." : "No rooms yet. Create one with [+]."
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
}
