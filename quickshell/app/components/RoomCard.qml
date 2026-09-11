import QtQuick
import QtQuick.Controls

Column {
  id: root
  required property var room
  required property var bridge
  required property var theme
  property bool adaptive: false
  readonly property bool narrow: adaptive && width < theme.space(200)
  readonly property bool tiny: adaptive && width < theme.space(80)
  property bool mainApp: false
  property bool showConnectedInvite: mainApp
  objectName: "savedRoom-" + room.id
  readonly property var people: (room.members || []).map(function(person) { return root.bridge.scopedParticipant(Object.assign({},person,{server_id:String(root.room.server_id || root.bridge.activeServer.id)})) })
  readonly property string conversationId: bridge.roomConversationId(room, true)
  readonly property int pending: bridge.pendingCount(conversationId)
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
    padding: root.narrow ? 1 : root.theme.friendly ? root.theme.space(8) : root.theme.spacing.sm; leftPadding: root.narrow ? 1 : root.theme.spacing.md
    Accessible.name: "Open " + root.room.name + " chat; " + root.people.length + " in voice"
    ToolTip.visible: hovered; ToolTip.text: Accessible.name
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
        id: roomHeader; width: parent.width
        height: root.narrow ? root.theme.space(root.theme.friendly ? 36 : 28) + actions.height : Math.max(root.theme.space(root.theme.friendly ? 36 : 28),actions.height)
        WispIcon { id: roomIcon; theme: root.theme; name: "volume"; visible: root.theme.friendly && !root.narrow; anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter }
        Text {
          objectName: "roomName"
          anchors.left: roomIcon.visible ? roomIcon.right : parent.left; anchors.leftMargin: roomIcon.visible ? root.theme.space(8) : 0; anchors.right: root.narrow ? parent.right : actions.left; anchors.rightMargin: root.narrow ? 0 : root.theme.spacing.xs
          y: root.narrow ? root.theme.space(8) : (parent.height-height)/2; elide: Text.ElideRight
          horizontalAlignment: root.narrow ? Text.AlignHCenter : Text.AlignLeft
          text: root.tiny ? String(root.room.name).slice(0,1).toUpperCase() : (root.theme.friendly ? root.room.name + "  · " + root.people.length : "#" + root.room.name + " /" + root.people.length) + (root.pending>0 ? " · "+root.pending+" new" : "")
          color: root.pending>0 ? root.theme.accent : root.theme.foreground; font.family: root.theme.font.family
          font.pixelSize: root.theme.font.body; font.weight: Font.DemiBold
        }
        Item {
          id: actions
          x: root.narrow ? (parent.width-width)/2 : parent.width-width
          y: root.narrow ? root.theme.space(root.theme.friendly ? 36 : 28) : (parent.height-height)/2
          readonly property real spacing: root.theme.space(4)
          readonly property real rowWidth: (joinAction.visible ? joinAction.width + spacing : 0) + moreAction.width
          readonly property bool wrapActions: root.narrow && joinAction.visible && rowWidth > parent.width
          // Keep ordinary rows on one line; only the narrow rail stacks actions.
          width: wrapActions ? Math.max(joinAction.width, moreAction.width) : rowWidth
          height: wrapActions ? joinAction.height + spacing + moreAction.height : Math.max(joinAction.visible ? joinAction.height : 0, moreAction.height)
          ChatButton {
            id: joinAction; objectName: "joinRoom-" + root.room.id
            visible: !root.current || root.showConnectedInvite; enabled: root.bridge.activeServer.connected !== false
            theme: root.theme; text: root.current ? "inv" : "join"
            iconName: root.current ? "invite" : root.narrow ? "phone" : ""; iconOnly: root.current || root.narrow; forceIcon: root.current || root.narrow
            width: root.narrow ? Math.min(body.width,root.theme.space(28)) : root.current ? root.theme.space(32) : implicitWidth
            height: root.narrow ? root.theme.space(28) : implicitHeight
            primary: root.theme.friendly && !root.current
            Accessible.name: (root.current ? "Invite to " : "Join voice in ") + root.room.name
            ToolTip.visible: hovered; ToolTip.text: Accessible.name
            onClicked: if (root.current) invitePicker.open(); else root.bridge.joinConversationVoice(root.conversationId)
          }
          ChatButton {
            id: moreAction; objectName: "roomMoreButton"; theme: root.theme; text: "···"; iconName: "more"; iconOnly: root.theme.friendly || root.narrow; forceIcon: root.narrow; implicitWidth: root.narrow ? Math.min(body.width,root.theme.space(28)) : root.theme.space(30)
            x: !joinAction.visible || actions.wrapActions ? 0 : joinAction.width + actions.spacing
            y: actions.wrapActions ? joinAction.height + actions.spacing : 0
            height: root.narrow ? root.theme.space(28) : implicitHeight
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
            width: root.mainApp ? members.width : Math.min(members.width, participant.width + (unread.visible ? unread.width + line.spacing : 0) + (streams.visible ? streams.implicitWidth + line.spacing : 0))
            implicitHeight: line.implicitHeight
            Flow {
              id: line; width: root.tiny ? participant.width : parent.width; x: root.tiny ? (parent.width-width)/2 : 0; spacing: root.theme.spacing.xs
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
                HoverHandler { id: personHover }
                ToolTip.visible: personHover.hovered; ToolTip.text: modelData.display_name + (voiceStatus.description ? " · " + voiceStatus.description : "")
                Keys.onReturnPressed: participantMenu.showPerson(person, participant)
                Keys.onSpacePressed: participantMenu.showPerson(person, participant)
                TapHandler {
                  acceptedButtons: Qt.LeftButton | Qt.RightButton
                  onTapped: participantMenu.showPerson(participant.person, participant)
                }
                readonly property bool speaking: root.current && (root.bridge.activeSpeakers || []).indexOf(modelData.display_name) >= 0
                readonly property bool self: modelData.id === (root.bridge.participantServer(person).self || {}).id
                readonly property real iconSpace: (voiceStatus.visible ? voiceStatus.width + spacing : 0) + (avatar.visible ? avatar.width + spacing : 0)
                width: root.tiny && avatar.visible ? avatar.width : Math.min(members.width, name.implicitWidth + iconSpace)
                height: Math.max(root.theme.friendly ? root.theme.space(36) : 0, name.implicitHeight, voiceStatus.height, streams.visible ? root.theme.space(22) : 0)
                WispAvatar { id: avatar; bridge: root.bridge; userId: String(modelData.id); serverId: String(root.room.server_id || root.bridge.activeServer.id); theme: root.theme; name: modelData.display_name; speaking: parent.speaking; visible: root.theme.friendly && root.theme.showAvatars; width: Math.min(members.width,root.theme.space(root.narrow ? 20 : 28)); height: width; anchors.verticalCenter: parent.verticalCenter
                }
                Text {
                  id: name
                  anchors.verticalCenter: parent.verticalCenter
                  visible: !root.tiny || !avatar.visible
                  width: Math.max(1,parent.width-parent.iconSpace); wrapMode: root.narrow ? Text.NoWrap : Text.WrapAnywhere; elide: Text.ElideRight
                  text: root.tiny ? String(modelData.display_name).slice(0,1).toUpperCase() : (!root.mainApp && index > 0 ? "· " : "") + (parent.speaking ? "● " : "") + modelData.display_name
                  color: parent.speaking ? root.theme.accent : root.theme.friendly ? root.theme.foreground : root.theme.muted
                  font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
                }
                ParticipantVoiceStatus {
                  id: voiceStatus; theme: root.theme; visible: !root.tiny && description !== ""
                  anchors.verticalCenter: parent.verticalCenter
                  moderation: participant.moderation
                  muted: root.current && (participant.self ? root.bridge.effectiveMuted : (root.bridge.remoteMutedParticipants || []).indexOf(modelData.display_name) >= 0)
                  deafened: root.current && participant.self && root.bridge.selfState.deafened
                  localMuted: !participant.self && root.bridge.participantVolumes.isMuted(participant.person)
                }
              }
              ChatButton {
                id: unread; objectName: "participantUnread-"+modelData.id
                readonly property var conversation: root.bridge.directFor(participant.person)
                readonly property int pending: conversation ? root.bridge.pendingCount(conversation.id) : 0
                visible:pending>0;theme:root.theme;primary:true;text:String(pending);iconName:"chat"
                height:root.theme.space(24);width:Math.min(implicitWidth,members.width)
                Accessible.name:"Open "+modelData.display_name+" · "+pending+" unread messages"
                ToolTip.visible:hovered;ToolTip.text:Accessible.name
                onClicked:root.bridge.openPendingChat(conversation.id)
              }
              ParticipantStreams {
                id: streams; bridge: root.bridge; theme: root.theme; adaptive: root.adaptive; availableWidth: members.width
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
