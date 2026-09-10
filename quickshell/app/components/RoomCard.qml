import QtQuick
import QtQuick.Controls

Column {
  id: root
  required property var room
  required property var bridge
  required property var theme
  property bool mainApp: false
  objectName: "savedRoom-" + room.id
  readonly property var people: (room.members || []).map(function(person) { return root.bridge.scopedParticipant(Object.assign({},person,{server_id:String(root.room.server_id || root.bridge.activeServer.id)})) })
  readonly property string conversationId: bridge.roomConversationId(room, true)
  readonly property bool current: String(room.server_id) === bridge.voiceServerId
    && !!room.active_hangout_id && room.active_hangout_id === bridge.selfState.hangout_id
  ParticipantMenu { id: participantMenu; bridge: root.bridge; theme: root.theme }
  RoomInvitePicker { id: invitePicker; objectName: "roomCallInvitePicker"; bridge: root.bridge; theme: root.theme }
  onCurrentChanged: if (!current) invitePicker.close()
  ParticipantVolumeMenu {
    id: menu; bridge: root.bridge; theme: root.theme; people: root.people
    roomConversationId: root.bridge.roomSettingsConversationId(root.room, true)
  }
  Button {
    id: openRoom; objectName: "openRoom-" + root.room.id
    width: parent.width
    implicitHeight: body.implicitHeight + topPadding + bottomPadding
    padding: root.theme.spacing.sm; leftPadding: root.theme.spacing.md
    Accessible.name: "Open " + root.room.name + " chat; " + root.people.length + " in voice"
    onClicked: root.bridge.openRoomChat(root.room, true)
    TapHandler { acceptedButtons: Qt.RightButton; onTapped: menu.open() }
    background: Rectangle {
      radius: root.theme.cornerRadius
      color: openRoom.hovered || root.bridge.activeConversationId === root.conversationId ? root.theme.alpha(root.theme.accent, 0.09) : "transparent"
      border.width: openRoom.visualFocus ? 1 : 0; border.color: root.theme.focusBorder
      Rectangle { width: root.theme.space(2); height: parent.height; visible: root.current; color: root.theme.accent }
    }
    contentItem: Column {
      id: body; spacing: root.theme.spacing.xs
      Item {
        width: parent.width; height: root.theme.space(root.theme.comfortable ? 38 : 28)
        Text {
          objectName: "roomName"
          anchors.left: parent.left; anchors.right: actions.left; anchors.rightMargin: root.theme.spacing.xs
          anchors.verticalCenter: parent.verticalCenter; elide: Text.ElideRight
          text: "#" + root.room.name + (root.theme.comfortable ? " · " + root.people.length + " in voice" : " /" + root.people.length)
          color: root.theme.foreground; font.family: root.theme.font.family
          font.pixelSize: root.theme.font.body; font.weight: Font.DemiBold
        }
        Row {
          id: actions; anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
          ChatButton {
            objectName: "joinRoom-" + root.room.id
            visible: !root.current || root.mainApp; enabled: root.bridge.activeServer.connected !== false
            theme: root.theme; text: root.theme.comfortable ? (root.current ? "Invite" : "Join") : root.current ? "inv" : "join"
            Accessible.name: (root.current ? "Invite to " : "Join voice in ") + root.room.name
            ToolTip.visible: hovered; ToolTip.text: Accessible.name
            onClicked: if (root.current) invitePicker.open(); else root.bridge.joinConversationVoice(root.conversationId)
          }
          ChatButton {
            objectName: "roomMoreButton"; theme: root.theme; text: "···"; implicitWidth: root.theme.space(30)
            Accessible.name: "Room settings and participant volumes"; onClicked: menu.open()
          }
        }
      }
      Flow {
        id: members; objectName: "roomParticipants"
        width: parent.width; spacing: root.theme.spacing.xs
        visible: root.people.length > 0
        Repeater {
          model: root.people
          delegate: Item {
            required property var modelData
            required property int index
            width: root.mainApp ? members.width : Math.min(members.width, participant.width + (streams.visible ? streams.implicitWidth + line.spacing : 0))
            implicitHeight: line.implicitHeight
            Flow {
              id: line; width: parent.width; spacing: root.theme.spacing.xs
              Row {
                objectName: "roomParticipant-" + modelData.id
                id: participant
                spacing: root.theme.spacing.xs
                readonly property var person: Object.assign({}, modelData, {server_id:String(root.room.server_id || root.bridge.activeServer.id)})
                readonly property var moderation: root.bridge.participantModeration(person)
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: modelData.display_name + " participant controls"
                Accessible.description: voiceStatus.description
                Keys.onReturnPressed: participantMenu.showPerson(person, participant)
                Keys.onSpacePressed: participantMenu.showPerson(person, participant)
                MouseArea {
                  parent: name
                  anchors.fill: parent; cursorShape: Qt.PointingHandCursor; acceptedButtons: Qt.LeftButton | Qt.RightButton
                  onClicked: participantMenu.showPerson(participant.person, participant)
                }
                readonly property bool speaking: root.current && (root.bridge.activeSpeakers || []).indexOf(modelData.display_name) >= 0
                readonly property bool self: modelData.id === (root.bridge.participantServer(person).self || {}).id
                readonly property real iconSpace: voiceStatus.visible ? voiceStatus.width + spacing : 0
                width: Math.min(members.width, name.implicitWidth + iconSpace)
                height: Math.max(implicitHeight, streams.visible ? root.theme.space(22) : 0)
                Text {
                  id: name
                  anchors.verticalCenter: parent.verticalCenter
                  width: Math.max(1,parent.width-parent.iconSpace); wrapMode: Text.WrapAnywhere
                  text: (!root.mainApp && index > 0 ? "· " : "") + (parent.speaking ? "● " : "") + modelData.display_name
                  color: parent.speaking ? root.theme.accent : root.theme.muted
                  font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
                }
                ParticipantVoiceStatus {
                  id: voiceStatus; theme: root.theme
                  anchors.verticalCenter: parent.verticalCenter
                  moderation: participant.moderation
                  muted: root.current && (participant.self ? root.bridge.effectiveMuted : (root.bridge.remoteMutedParticipants || []).indexOf(modelData.display_name) >= 0)
                  deafened: root.current && participant.self && root.bridge.selfState.deafened
                  localMuted: !participant.self && root.bridge.participantVolumes.isMuted(participant.person)
                }
              }
              ParticipantStreams {
                id: streams; bridge: root.bridge; theme: root.theme
                person: participant.person; current: root.current && !participant.self
                width: Math.min(members.width, implicitWidth)
              }
            }
          }
        }
      }
    }
  }
}
