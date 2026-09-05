import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Popup {
  id: root
  objectName: "participantVolumeMenu"
  required property var bridge
  required property var theme
  property var people: []
  property string roomConversationId: ""
  Loader {
    id: roomManager
    active: root.roomConversationId !== ""
    sourceComponent: RoomManager { objectName: "contextRoomManager"; bridge: root.bridge; theme: root.theme }
  }
  readonly property var participants: people.filter(function(p) { return p.id !== root.bridge.selfState.id })
  width: root.theme.space(300)
  padding: root.theme.spacing.lg
  closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
  background: Rectangle { color: root.theme.surface; radius: root.theme.cornerRadius; border.width: 1; border.color: root.theme.separator }
  contentItem: Column {
    spacing: root.theme.spacing.md
    ChatButton {
      objectName: "roomContextSettings"
      theme: root.theme; text: "room settings"
      width: parent.width
      visible: root.roomConversationId !== ""
      onClicked: { root.close(); roomManager.item.manage(root.roomConversationId) }
    }
    ChatButton {
      objectName: "roomContextNewPane"
      theme: root.theme; text: "open chat in new pane"; width: parent.width
      visible: root.roomConversationId !== ""
      onClicked: { root.close(); root.bridge.openChannel(root.roomConversationId, true) }
    }
    Text { text: "Volume · only for you"; color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true }
    ChatButton {
      theme: root.theme; text: "Invite to your call"
      width: parent.width
      visible: root.participants.length === 1 && !!root.bridge.currentVoiceRoom
        && String(root.participants[0].server_id || root.bridge.activeServer.id) === String(root.bridge.currentVoiceRoom.server_id || root.bridge.activeServer.id)
        && (!(root.bridge.currentVoiceRoom.members || []).some(function(p) { return p.id === root.participants[0].id })
          || (typeof root.bridge.needsEncryptedRoomAccess === "function" && root.bridge.needsEncryptedRoomAccess(root.participants[0])))
      onClicked: { root.bridge.inviteToRoom(root.participants[0]); root.close() }
    }
    Repeater {
      model: root.participants
      Column {
        required property var modelData
        width: parent.width
        RowLayout {
          width: parent.width
          Text {
            text: modelData.display_name; elide: Text.ElideRight; Layout.fillWidth: true
            color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
          }
          Text { text: Math.round(level.value) + "%"; color: root.theme.accent; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption }
          ChatButton { theme: root.theme; text: "↺"; Accessible.name: "Reset " + modelData.display_name + " to 100 percent"; onClicked: root.bridge.participantVolumes.setVolume(modelData, 100) }
        }
        Slider {
          id: level
          objectName: "participantVolume-" + modelData.id
          width: parent.width; from: 0; to: 200; stepSize: 1
          value: root.bridge.participantVolumes.volumeFor(modelData)
          Accessible.name: modelData.display_name + " local volume"
          onMoved: root.bridge.participantVolumes.setVolume(modelData, value)
          palette.highlight: root.theme.accent
        }
      }
    }
    Text {
      visible: root.participants.length === 0
      text: "No other participants in this room."
      color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    Text {
      width: parent.width; wrapMode: Text.Wrap
      visible: root.bridge.participantVolumes.error !== ""
      text: root.bridge.participantVolumes.error
      color: root.theme.danger; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
  }
}
