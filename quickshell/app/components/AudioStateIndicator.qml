import QtQuick

Row {
  id: root

  required property var theme
  property bool muted: false
  property bool deafened: false

  visible: muted || deafened
  spacing: root.theme.spacing.sm
  height: visible ? root.theme.space(30) : 0

  Rectangle {
    visible: root.muted
    width: mutedLabel.implicitWidth + root.theme.spacing.lg * 2
    height: root.theme.space(30)
    radius: root.theme.cornerRadius
    color: root.theme.alpha(root.theme.danger, 0.22)
    border.color: root.theme.alpha(root.theme.danger, 0.72)
    border.width: 1

    Text {
      id: mutedLabel
      anchors.centerIn: parent
      text: "MICROPHONE MUTED"
      color: root.theme.danger
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
      font.weight: Font.Bold
    }
  }

  Rectangle {
    visible: root.deafened
    width: deafenedLabel.implicitWidth + root.theme.spacing.lg * 2
    height: root.theme.space(30)
    radius: root.theme.cornerRadius
    color: root.theme.alpha(root.theme.warning, 0.2)
    border.color: root.theme.alpha(root.theme.warning, 0.72)
    border.width: 1

    Text {
      id: deafenedLabel
      anchors.centerIn: parent
      text: "DEAFENED"
      color: root.theme.warning
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
      font.weight: Font.Bold
    }
  }
}
