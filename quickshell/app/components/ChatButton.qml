import QtQuick
import QtQuick.Controls

Button {
  id: root
  required property var theme
  property bool primary: false
  property bool destructive: false
  implicitHeight: theme.space(34)
  implicitWidth: label.implicitWidth + theme.space(24)
  background: Rectangle {
    radius: root.theme.cornerRadius
    color: root.primary ? (root.destructive ? root.theme.danger : root.theme.accent) : root.theme.alpha(root.destructive
      ? root.theme.danger : root.theme.foreground, root.hovered ? 0.14 : 0.06)
    opacity: root.enabled ? 1 : 0.4
  }
  contentItem: Text {
    id: label
    text: root.text
    color: root.primary ? "white" : root.destructive ? root.theme.danger : root.theme.foreground
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    horizontalAlignment: Text.AlignHCenter
    verticalAlignment: Text.AlignVCenter
  }
}
