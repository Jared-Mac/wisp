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
    id: knockInfo
    anchors.left: parent.left
    anchors.leftMargin: root.theme.spacing.lg
    anchors.verticalCenter: parent.verticalCenter
    spacing: root.theme.spacing.xs
    Binding on width { when: root.theme.terminal || root.theme.friendly; value: Math.max(0, knockActions.x - knockInfo.x - root.theme.spacing.lg); restoreMode: Binding.RestoreBindingOrValue }

    Text {
      Binding on width { when: root.theme.terminal || root.theme.friendly; value: knockInfo.width; restoreMode: Binding.RestoreBindingOrValue }
      elide: root.theme.terminal || root.theme.friendly ? Text.ElideRight : Text.ElideNone
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
      delegate: ChatButton {
        required property var modelData
        theme:root.theme;text:modelData.label;primary:modelData.primary
        onClicked:{root.bridge.respondKnock(root.knock.id,modelData.response);if(modelData.response==="accept")root.accepted()}
      }
    }
  }
}
