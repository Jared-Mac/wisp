import QtQuick

Rectangle {
  id: root
  required property var knock
  required property var bridge
  required property var theme
  signal accepted()

  readonly property bool stacked: root.theme.comfortable && width < root.theme.space(360)
  implicitHeight: stacked ? knockInfo.implicitHeight + knockActions.height + root.theme.space(24) : root.theme.space(76)
  radius: root.theme.cornerRadius
  color: root.theme.alpha(root.theme.warning, 0.10)
  border.width: 1
  border.color: root.theme.alpha(root.theme.warning, 0.30)

  Column {
    id: knockInfo
    anchors.left: parent.left
    anchors.leftMargin: root.theme.spacing.lg
    anchors.verticalCenter: root.stacked ? undefined : parent.verticalCenter
    anchors.top: root.stacked ? parent.top : undefined
    anchors.topMargin: root.theme.spacing.lg
    spacing: root.theme.spacing.xs
    Binding on width { when: root.theme.terminal; value: Math.max(0, root.stacked ? root.width - root.theme.spacing.lg * 2 : knockActions.x - knockInfo.x - root.theme.spacing.lg); restoreMode: Binding.RestoreBindingOrValue }

    Text {
      Binding on width { when: root.theme.terminal; value: knockInfo.width; restoreMode: Binding.RestoreBindingOrValue }
      elide: root.theme.terminal ? Text.ElideRight : Text.ElideNone
      wrapMode: root.stacked ? Text.Wrap : Text.NoWrap
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
    anchors.verticalCenter: root.stacked ? undefined : parent.verticalCenter
    anchors.bottom: root.stacked ? parent.bottom : undefined
    anchors.bottomMargin: root.theme.spacing.lg
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
          color: modelData.primary ? root.theme.accentText : root.theme.foreground
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
