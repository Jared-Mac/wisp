import QtQuick

Rectangle {
  id: root
  required property var theme
  property string message: ""
  signal dismissed()
  visible: message !== ""
  implicitHeight: Math.max(label.implicitHeight + theme.spacing.md * 2, dismiss.height + theme.spacing.sm * 2)
  radius: theme.cornerRadius
  color: theme.alpha(theme.danger, 0.12)

  Text {
    id: label
    anchors.left: parent.left; anchors.right: dismiss.left; anchors.verticalCenter: parent.verticalCenter
    anchors.leftMargin: root.theme.spacing.md; anchors.rightMargin: root.theme.spacing.sm
    text: root.message; textFormat: Text.PlainText
    color: root.theme.danger; wrapMode: Text.Wrap
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  ChatButton {
    id: dismiss; objectName: "dismissError"
    anchors.right: parent.right; anchors.rightMargin: root.theme.spacing.sm; anchors.verticalCenter: parent.verticalCenter
    theme: root.theme; text: "Dismiss error"; iconName: "close"; iconOnly: true; forceIcon: true
    Accessible.name: "Dismiss error"
    quiet: true; destructive: true; width: root.theme.space(24); height: width
    onClicked: root.dismissed()
  }
}
