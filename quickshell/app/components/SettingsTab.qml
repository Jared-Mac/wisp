import QtQuick

ChatButton {
  id: root
  implicitHeight: theme.space(38)
  implicitWidth: tabLabel.implicitWidth + theme.space(theme.friendly ? 48 : 26)
  Accessible.name: text
  Accessible.description: primary ? "Selected settings tab" : "Settings tab"
  background: Rectangle {
    radius: root.theme.cornerRadius
    color: root.theme.alpha(root.theme.accent, root.primary ? 0.2 : root.down ? 0.18 : root.hovered ? 0.1 : 0.035)
    border.width: 1
    border.color: root.primary || root.visualFocus ? root.theme.accent : root.hovered ? root.theme.muted : root.theme.separator
    Rectangle {
      anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: parent.bottom
      anchors.margins: 1
      height: root.theme.space(3)
      visible: root.primary && !root.theme.friendly
      color: root.theme.accent
    }
  }
  contentItem: Item {
    WispIcon {theme:root.theme;name:root.iconName;ink:root.primary ? root.theme.accent : root.theme.muted;visible:root.theme.friendly;anchors.left:parent.left;anchors.verticalCenter:parent.verticalCenter}
    Text {
    id: tabLabel
    x: root.theme.friendly ? root.theme.space(24) : 0
    width: parent.width-x; height: parent.height
    text: root.theme.tui ? "[" + root.text + "]" : root.text
    color: root.primary ? root.theme.accent : root.theme.foreground
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.bold: root.primary
    horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter
    }
  }
}
