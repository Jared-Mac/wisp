import QtQuick

Rectangle {
  id: root
  required property var knock
  required property var bridge
  required property var theme
  signal accepted()

  implicitHeight: root.theme.space(76)
  radius: root.theme.cornerRadius
  color: root.theme.alpha(root.theme.warning, 0.10)
  border.width: 1
  border.color: root.theme.alpha(root.theme.warning, 0.30)

  Column {
    anchors.left: parent.left
    anchors.leftMargin: root.theme.spacing.lg
    anchors.verticalCenter: parent.verticalCenter
    spacing: root.theme.spacing.xs
    width: root.theme.terminal ? Math.max(0, knockActions.x - x - root.theme.spacing.lg) : implicitWidth

    Text {
      width: root.theme.terminal ? parent.width : implicitWidth
      elide: root.theme.terminal ? Text.ElideRight : Text.ElideNone
      text: String(root.knock.from.display_name || "A friend") + " wants to hang out"
      color: root.theme.foreground
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.body
      font.weight: Font.DemiBold
    }
    Text {
      text: "Knock expires soon"
      color: root.theme.muted
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
    }
  }

  Row {
    id: knockActions
    anchors.right: parent.right
    anchors.rightMargin: root.theme.spacing.md
    anchors.verticalCenter: parent.verticalCenter
    spacing: root.theme.spacing.sm

    Repeater {
      model: [
        { "label": "Later", "response": "later", "primary": false },
        { "label": "Join", "response": "accept", "primary": true }
      ]
      delegate: Rectangle {
        required property var modelData
        width: actionText.implicitWidth + root.theme.spacing.lg * 2
        height: root.theme.space(30)
        radius: root.theme.cornerRadius
        color: modelData.primary
          ? (actionMouse.containsMouse ? Qt.lighter(root.theme.accent, 1.12) : root.theme.accent)
          : root.theme.alpha(root.theme.foreground, actionMouse.containsMouse ? 0.14 : 0.08)

        Text {
          id: actionText
          anchors.centerIn: parent
          text: modelData.label
          color: modelData.primary ? "white" : root.theme.foreground
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
          font.weight: Font.Bold
        }
        MouseArea {
          id: actionMouse
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: {
            root.bridge.respondKnock(root.knock.id, modelData.response)
            if (modelData.response === "accept") root.accepted()
          }
        }
      }
    }
  }
}
