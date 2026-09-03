import QtQuick
import QtQuick.Controls

Button {
  Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
  Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.caption; restoreMode: Binding.RestoreBindingOrValue }
  id: root
  required property var theme
  property bool primary: false
  property bool destructive: false
  implicitHeight: theme.space(34)
  implicitWidth: label.implicitWidth + theme.space(24)
  background: Rectangle {
    radius: root.theme.cornerRadius
    color: root.primary ? (root.destructive ? root.theme.danger : root.theme.terminal ? root.theme.alpha(root.theme.accent, root.hovered ? 0.24 : 0.12) : root.theme.accent) : root.theme.alpha(root.destructive
      ? root.theme.danger : root.theme.foreground, root.hovered ? 0.14 : 0.06)
    opacity: root.enabled ? 1 : 0.4
    border.width: root.theme.terminal ? 1 : 0
    border.color: root.visualFocus || root.primary ? root.theme.focusBorder : root.hovered ? root.theme.muted : root.theme.separator
    Rectangle {
      anchors.fill: parent; radius: parent.radius
      visible: root.theme.terminal && root.down
      color: root.theme.alpha(root.theme.foreground, 0.10)
    }
  }
  contentItem: Text {
    id: label
    text: root.text
    color: root.primary ? (root.destructive ? "#151821" : root.theme.terminal ? root.theme.accent : root.theme.accentText) : root.destructive ? root.theme.danger : root.theme.foreground
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    horizontalAlignment: Text.AlignHCenter
    verticalAlignment: Text.AlignVCenter
  }
}
