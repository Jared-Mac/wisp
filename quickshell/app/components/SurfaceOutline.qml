import QtQuick

// A non-interactive frame; preserves the native surface, layout, and hit targets.
Rectangle {
  objectName: "wispSurfaceOutline"
  required property var theme
  anchors.fill: parent
  z: 1000
  visible: !theme.hostManaged
  color: "transparent"
  radius: theme.cornerRadius
  border.width: 1
  border.color: theme.surfaceBorder
}
