import QtQuick

Column {
  id: root
  required property var bridge
  required property var theme
  width: parent ? parent.width : 0
  spacing: root.theme.spacing.sm

  Text {
    text: "WHO MAY JOIN?"
    color: root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.weight: Font.Bold
  }

  Row {
    spacing: root.theme.spacing.sm
    Repeater {
      model: ["open", "knock", "closed", "away"]
      delegate: Rectangle {
        required property string modelData
        width: presenceText.implicitWidth + root.theme.spacing.lg * 2
        height: root.theme.space(30)
        radius: root.theme.cornerRadius
        color: root.bridge.selfState.presence === modelData
          ? root.theme.alpha(root.theme.accent, 0.38)
          : root.theme.alpha(root.theme.foreground, presenceMouse.containsMouse ? 0.12 : 0.055)
        Text {
          id: presenceText
          anchors.centerIn: parent
          text: modelData.charAt(0).toUpperCase() + modelData.slice(1)
          color: root.theme.foreground
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }
        MouseArea {
          id: presenceMouse
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: root.bridge.setPresence(modelData)
        }
      }
    }
  }
}
