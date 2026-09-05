import QtQuick
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  signal createRoomRequested()
  readonly property bool collapsed: bridge.workspaceLayout.trayRoomsCollapsed
  spacing: theme.spacing.xs
  RoomsHeader {
    width: parent.width; bridge: root.bridge; theme: root.theme
    collapsible: true; collapsed: root.collapsed
    onToggled: root.bridge.workspaceLayout.trayRoomsCollapsed = !root.collapsed
    onCreateRequested: root.createRoomRequested()
  }
  SpotsView { width: parent.width; visible: !root.collapsed; bridge: root.bridge; theme: root.theme }
}
