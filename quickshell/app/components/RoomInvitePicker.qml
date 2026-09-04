import QtQuick
import QtQuick.Controls

Popup {
  id: root
  required property var bridge
  required property var theme
  width: Math.min(root.theme.space(360), parent ? parent.width : root.theme.space(360))
  height: Math.min(root.theme.space(420), body.implicitHeight + padding * 2)
  padding: theme.spacing.lg
  closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
  background: Rectangle { color: root.theme.surface; radius: root.theme.cornerRadius; border.width: 1; border.color: root.theme.separator }
  contentItem: Flickable {
    contentHeight: body.implicitHeight; clip: true
    Column {
      id: body; width: parent.width; spacing: root.theme.spacing.sm
      Text {
        width: parent.width; wrapMode: Text.Wrap; textFormat: Text.PlainText
        text: "Invite to voice · " + (root.bridge.currentVoiceRoom ? root.bridge.currentVoiceRoom.label || "Room" : "Not connected")
        color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
      }
      Repeater {
        model: root.bridge.sortedFriends || []
        ChatButton {
          required property var modelData
          theme: root.theme; width: parent.width
          text: modelData.display_name
          enabled: !!root.bridge.currentVoiceRoom && !(root.bridge.currentVoiceRoom.members || []).some(function(p) { return p.id === modelData.id }) && !(root.bridge.invitationRequests || {})[modelData.id]
          onClicked: { root.bridge.inviteToRoom(modelData); root.close() }
        }
      }
    }
  }
}
