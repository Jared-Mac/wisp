import QtQuick
import QtQuick.Controls

Popup {
  id: root
  objectName: "callInvitePicker"
  required property var bridge
  required property var theme
  parent: Overlay.overlay
  width: Math.min(root.theme.space(360), parent ? parent.width - root.theme.spacing.lg * 2 : root.theme.space(360))
  height: Math.min(root.theme.space(420), parent ? parent.height - root.theme.spacing.lg * 2 : root.theme.space(420), body.implicitHeight + padding * 2)
  x: parent ? (parent.width - width) / 2 : 0
  y: parent ? Math.max(root.theme.spacing.lg, parent.height - height - root.theme.space(110)) : 0
  readonly property string callKey: root.bridge.currentVoiceRoom ? String(root.bridge.currentVoiceRoom.server_id || "") + ":" + root.bridge.currentVoiceRoom.id : ""
  onCallKeyChanged: close()
  padding: theme.spacing.lg
  closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
  background: Rectangle { color: root.theme.surface; radius: root.theme.cornerRadius; border.width: 1; border.color: root.theme.separator }
  contentItem: Flickable {
    contentHeight: body.implicitHeight; clip: true
    Column {
      id: body; width: parent.width; spacing: root.theme.spacing.sm
      Text {
        width: parent.width; wrapMode: Text.Wrap; textFormat: Text.PlainText
        text: "Invite friends · " + (root.bridge.currentVoiceRoom ? root.bridge.currentVoiceLabel || root.bridge.currentVoiceRoom.label || "Call" : "Not connected")
        color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
      }
      Repeater {
        model: root.bridge.voiceFriends || root.bridge.sortedFriends || []
        ChatButton {
          required property var modelData
          objectName: "callInvite-" + modelData.id
          theme: root.theme; width: parent.width
          text: modelData.display_name
          enabled: !!root.bridge.currentVoiceRoom && (!(root.bridge.currentVoiceRoom.members || []).some(function(p) { return p.id === modelData.id })
            || (typeof root.bridge.needsEncryptedRoomAccess === "function" && root.bridge.needsEncryptedRoomAccess(modelData))) && !(root.bridge.invitationRequests || {})[modelData.id]
          onClicked: { root.bridge.inviteToRoom(modelData); root.close() }
        }
      }
    }
  }
}
