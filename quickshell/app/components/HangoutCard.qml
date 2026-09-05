import QtQuick

Rectangle {
  id: root
  required property var hangout
  required property var bridge
  required property var theme
  readonly property bool current: root.bridge.selfState.hangout_id === root.hangout.id
    && (!root.hangout.server_id || String(root.hangout.server_id) === root.bridge.voiceServerId)
  signal joined()
  TapHandler { acceptedButtons: Qt.RightButton; onTapped: volumeMenu.open() }
  ParticipantMenu { id: participantMenu; bridge: root.bridge; theme: root.theme }
  ParticipantVolumeMenu {
    id: volumeMenu; bridge: root.bridge; theme: root.theme; people: (root.hangout.members || []).map(function(p) { return root.bridge.scopedParticipant(Object.assign({},p,{server_id:String(root.hangout.server_id || root.bridge.activeServer.id)})) })
    roomConversationId: root.bridge.roomSettingsConversationId(root.hangout, false)
  }

  objectName: "roomCard"
  implicitHeight: Math.max(root.theme.space(root.theme.tui ? 42 : 48), hangoutInfo.implicitHeight + root.theme.spacing.sm * 2)
  radius: root.theme.cornerRadius
  color: root.theme.tui ? root.theme.surface : root.theme.alpha(root.theme.foreground, 0.055)
  border.width: root.theme.tui ? 0 : 1
  border.color: root.hasActiveMember()
    ? root.theme.alpha(root.theme.accent, 0.65)
    : root.theme.roomBorder

  function memberSpeaking(name) {
    var active = bridge.activeSpeakers || []
    for (var i = 0; i < active.length; i++)
      if (active[i] === name) return true
    return false
  }

  Rectangle {
    visible: root.theme.terminal && root.current
    anchors { left: parent.left; top: parent.top; bottom: parent.bottom; margins: 1 }
    width: root.theme.space(2)
    color: root.theme.accent
  }

  function hasActiveMember() {
    var members = hangout.members || []
    for (var i = 0; i < members.length; i++)
      if (memberSpeaking(members[i].display_name)) return true
    return false
  }

  Column {
    id: hangoutInfo
    anchors.left: parent.left
    anchors.leftMargin: root.theme.spacing.lg
    anchors.verticalCenter: parent.verticalCenter
    spacing: root.theme.spacing.xs
    width: Math.max(0, root.width - root.theme.spacing.lg * 2)

    Flow {
      width: parent.width
      spacing: root.theme.spacing.xs

      Repeater {
        model: root.hangout.members || []

        delegate: Row {
          id: memberRow
          objectName: "roomMember-" + index
          required property var modelData
          required property int index
          spacing: root.theme.spacing.xs
          readonly property var person: root.bridge.scopedParticipant(Object.assign({},modelData,{server_id:String(root.hangout.server_id || root.bridge.activeServer.id)}))
          readonly property bool self: modelData.id === (root.bridge.participantServer(person).self || {}).id
          activeFocusOnTab: true
          Accessible.role: Accessible.Button
          Accessible.name: modelData.display_name + " participant controls"
          Accessible.description: voiceStatus.description
          Keys.onReturnPressed: participantMenu.showPerson(person,memberRow)
          Keys.onSpacePressed: participantMenu.showPerson(person,memberRow)
          MouseArea {
            parent: memberName; anchors.fill: parent; cursorShape: Qt.PointingHandCursor
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onClicked: participantMenu.showPerson(memberRow.person,memberRow)
          }
          readonly property real iconSpace: voiceStatus.visible ? voiceStatus.width + spacing : 0
          width: Math.min(hangoutInfo.width, memberName.implicitWidth + iconSpace)

          Text {
            id: memberName
            objectName: "roomMemberName-" + index
            width: Math.max(1, memberRow.width - memberRow.iconSpace)
            wrapMode: Text.WrapAnywhere
            anchors.verticalCenter: parent.verticalCenter
            text: (index > 0 ? " + " : "")
              + (root.memberSpeaking(modelData.display_name) ? "● " : "")
              + String(modelData.display_name || "")
            color: root.memberSpeaking(modelData.display_name)
              ? root.theme.accent : root.theme.foreground
            font.family: root.theme.font.family
            font.pixelSize: root.theme.font.body
            font.weight: Font.DemiBold
          }

          ParticipantVoiceStatus {
            id: voiceStatus; theme: root.theme
            anchors.verticalCenter: parent.verticalCenter
            moderation: root.bridge.participantModeration(memberRow.person)
            muted: root.current && (memberRow.self ? root.bridge.effectiveMuted : (root.bridge.remoteMutedParticipants || []).indexOf(modelData.display_name) >= 0)
            deafened: root.current && memberRow.self && root.bridge.selfState.deafened
            localMuted: !memberRow.self && root.bridge.participantVolumes.isMuted(memberRow.person)
          }
        }
      }
    }
    Text {
      width: Math.max(0, hangoutInfo.width - joinButton.width - chatButton.width - root.theme.spacing.sm * 2)
      height: Math.max(implicitHeight, joinButton.height)
      verticalAlignment: Text.AlignVCenter
      elide: Text.ElideRight
      text: "Call"
      color: root.theme.muted
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
    }
  }

  Rectangle {
    id: joinButton
    anchors.right: parent.right
    anchors.rightMargin: root.theme.spacing.md
    anchors.bottom: hangoutInfo.bottom
    width: joinText.implicitWidth + root.theme.spacing.lg * 2
    height: root.theme.space(30)
    radius: root.theme.cornerRadius
    color: root.theme.tui ? (joinMouse.containsMouse ? root.theme.alpha(root.theme.accent, 0.18) : "transparent") : joinMouse.containsMouse ? Qt.lighter(root.theme.accent, 1.12) : root.theme.accent

    Text {
      id: joinText
      anchors.centerIn: parent
      text: root.theme.tui ? (root.current ? "in call" : "[join]") : root.current ? "In call" : "Join"
      color: root.theme.tui ? root.theme.accent : root.theme.accentText
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
      font.weight: Font.Bold
    }
    MouseArea {
      id: joinMouse
      anchors.fill: parent
      hoverEnabled: true
      enabled: !root.current
      cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
      onClicked: {
        root.bridge.joinHangout(root.hangout.id)
        root.joined()
      }
    }
  }
  ChatButton {
    id: chatButton
    objectName: "openRoomChat"
    theme: root.theme; text: "Chat"
    anchors.right: joinButton.left; anchors.rightMargin: root.theme.spacing.sm
    anchors.verticalCenter: joinButton.verticalCenter
    Accessible.name: "Open " + (root.hangout.label || "room") + " text chat"
    onClicked: root.bridge.openRoomChat(root.hangout, false)
  }
}
