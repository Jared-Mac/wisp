import QtQuick
import QtQuick.Controls

Popup {
  id: root
  objectName: "participantMenu"
  required property var bridge
  required property var theme
  property var person: ({})
  readonly property var serverState: bridge.participantServer(person)
  readonly property bool self: String(person.id) === String((serverState.self || {}).id)
  readonly property var moderation: bridge.participantModeration(person)
  readonly property bool localMuted: bridge.participantVolumes.isMuted(person)
  readonly property bool canMessage: !self && (serverState.friends || []).some(function(p) { return String(p.id) === String(root.person.id) })
  parent: Overlay.overlay
  width: Math.min(theme.space(280), parent ? parent.width - 16 : 280)
  padding: theme.spacing.md
  closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
  function showPerson(value, anchor) {
    person = bridge.scopedParticipant(value)
    var point = anchor.mapToItem(parent, 0, anchor.height)
    x = Math.max(8, Math.min(point.x, parent.width - width - 8))
    y = Math.max(8, Math.min(point.y, parent.height - implicitHeight - 8))
    open()
  }
  background: Rectangle { color: root.theme.surface; border.color: root.theme.separator; border.width: 1; radius: root.theme.cornerRadius }
  contentItem: Column {
    spacing: root.theme.spacing.sm
    Text {
      width: parent.width; text: root.person.display_name || "Participant"; elide: Text.ElideRight
      color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
    }
    ChatButton {
      objectName: "participantMessage"; width: parent.width; theme: root.theme
      visible: !root.self; enabled: root.canMessage; text: "message"
      Accessible.name: "Open direct message in a new tile"
      ToolTip.visible: hovered; ToolTip.text: root.canMessage ? Accessible.name : "Add as a friend to message"
      onClicked: { root.bridge.openParticipantDirect(root.person); root.close() }
    }
    Text {
      visible: !root.self; text: "Volume · only for you · " + Math.round(level.value) + "%"
      color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    Slider {
      id: level; objectName: "participantMenuVolume"; width: parent.width; visible: !root.self
      from: 0; to: 200; stepSize: 1; value: root.bridge.participantVolumes.volumeFor(root.person)
      Accessible.name: "Local volume for " + root.person.display_name
      palette.highlight: root.theme.accent
      onMoved: root.bridge.participantVolumes.setVolume(root.person, value)
    }
    ChatButton {
      objectName: "participantLocalMute"; width: parent.width; theme: root.theme; visible: !root.self
      text: root.localMuted ? "unmute for me" : "mute for me"
      onClicked: root.bridge.participantVolumes.setMuted(root.person, !root.localMuted)
    }
    ChatButton {
      objectName: "participantSelfMute"; width: parent.width; theme: root.theme; visible: root.self
      text: root.bridge.selfState.muted ? "unmute" : "mute"; onClicked: root.bridge.toggleMuted()
    }
    ChatButton {
      objectName: "participantSelfDeafen"; width: parent.width; theme: root.theme; visible: root.self
      text: root.bridge.selfState.deafened ? "undeafen" : "deafen"; onClicked: root.bridge.toggleDeafened()
    }
    Rectangle { width: parent.width; height: 1; color: root.theme.separator; visible: root.bridge.canModerateParticipant(root.person) }
    ChatButton {
      objectName: "participantServerMute"; width: parent.width; theme: root.theme
      visible: root.bridge.canModerateParticipant(root.person)
      text: root.moderation.muted ? "remove server mute" : "server mute"
      onClicked: root.bridge.moderateParticipant(root.person, {muted:!root.moderation.muted})
    }
    ChatButton {
      objectName: "participantServerDeafen"; width: parent.width; theme: root.theme
      visible: root.bridge.canModerateParticipant(root.person)
      text: root.moderation.deafened ? "remove server deafen" : "server deafen"
      onClicked: root.bridge.moderateParticipant(root.person, {deafened:!root.moderation.deafened})
    }
    Text {
      width: parent.width; wrapMode: Text.Wrap; visible: root.moderation.muted || root.moderation.deafened
      text: root.moderation.deafened ? "Server deafened" : "Server muted"
      color: root.theme.danger; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
  }
}
