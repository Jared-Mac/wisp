import QtQuick
import QtQuick.Controls

Rectangle {
  id: root
  required property var knock
  required property var bridge
  required property var theme
  property bool adaptive: false
  readonly property bool narrow: adaptive && width < theme.space(140)
  readonly property bool tiny: adaptive && width < theme.space(56)
  signal accepted()

  implicitHeight: root.narrow ? knockRail.implicitHeight + 4 : root.theme.space(76)
  radius: root.theme.cornerRadius
  color: root.theme.alpha(root.theme.warning, 0.10)
  border.width: 1
  border.color: root.theme.alpha(root.theme.warning, 0.30)

  Column {
    id: knockRail; width: parent.width; y: 2; spacing: 2; visible: root.narrow
    Text { width: parent.width; text: root.tiny ? String(root.knock.from.display_name).slice(0,1) : root.knock.from.display_name; elide: Text.ElideRight; color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption }
    Flow {
      width: parent.width
      Repeater {
        model: [{text:"Accept knock from ",action:"accept",icon:"phone"},{text:"Decline knock from ",action:"later",icon:"close"}]
        ChatButton {
          required property var modelData; theme: root.theme; width: Math.min(knockRail.width,root.theme.space(28)); height: root.theme.space(28)
          text: modelData.text + root.knock.from.display_name; iconName: modelData.icon; forceIcon: true; iconOnly: true
          onClicked: {root.bridge.respondKnock(root.knock.id,modelData.action);if(modelData.action==="accept")root.accepted()}
        }
      }
    }
  }
  Column {
    id: knockInfo; visible: !root.narrow
    anchors.left: parent.left
    anchors.leftMargin: root.theme.spacing.lg
    anchors.verticalCenter: parent.verticalCenter
    spacing: root.theme.spacing.xs
    Binding on width { when: root.theme.terminal || root.theme.friendly || root.theme.comfortable; value: Math.max(0, knockActions.x - knockInfo.x - root.theme.spacing.lg); restoreMode: Binding.RestoreBindingOrValue }

    Text {
      Binding on width { when: root.theme.terminal || root.theme.friendly || root.theme.comfortable; value: knockInfo.width; restoreMode: Binding.RestoreBindingOrValue }
      elide: root.theme.terminal || root.theme.friendly || root.theme.comfortable ? Text.ElideRight : Text.ElideNone
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
    id: knockActions; visible: !root.narrow
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
