import QtQuick

Rectangle {
  id: root
  required property var hangout
  required property var bridge
  required property var theme
  signal joined()
  TapHandler { acceptedButtons: Qt.RightButton; onTapped: volumeMenu.open() }
  ParticipantVolumeMenu { id: volumeMenu; bridge: root.bridge; theme: root.theme; people: root.hangout.members || [] }

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
    visible: root.theme.terminal && root.bridge.selfState.hangout_id === root.hangout.id
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

  function isSelf(member) {
    return member && member.id === bridge.selfState.id
  }

  function memberMuted(member) {
    if (root.isSelf(member)) return !!root.bridge.effectiveMuted
    var muted = root.bridge.remoteMutedParticipants || []
    for (var i = 0; i < muted.length; i++)
      if (muted[i] === member.display_name) return true
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
          readonly property real iconSpace: (mutedIcon.visible ? mutedIcon.width + spacing : 0)
            + (deafenedIcon.visible ? deafenedIcon.width + spacing : 0)
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

          Image {
            id: mutedIcon
            visible: root.memberMuted(modelData)
            anchors.verticalCenter: parent.verticalCenter
            width: visible ? root.theme.space(14) : 0
            height: width
            source: Qt.resolvedUrl("../assets/microphone-muted.svg")
            fillMode: Image.PreserveAspectFit
          }

          Image {
            id: deafenedIcon
            visible: root.isSelf(modelData) && !!root.bridge.selfState.deafened
            anchors.verticalCenter: parent.verticalCenter
            width: visible ? root.theme.space(14) : 0
            height: width
            source: Qt.resolvedUrl("../assets/deafened.svg")
            fillMode: Image.PreserveAspectFit
          }
        }
      }
    }
    Text {
      width: Math.max(0, hangoutInfo.width - joinButton.width - chatButton.width - root.theme.spacing.sm * 2)
      height: Math.max(implicitHeight, joinButton.height)
      verticalAlignment: Text.AlignVCenter
      elide: Text.ElideRight
      text: (root.theme.tui ? "# " : "") + (root.hangout.label || "Room")
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
      text: root.theme.tui ? (root.bridge.selfState.hangout_id === root.hangout.id ? "[here]" : "[join]") : root.bridge.selfState.hangout_id === root.hangout.id ? "HERE" : "JOIN"
      color: root.theme.tui ? root.theme.accent : root.theme.accentText
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
      font.weight: Font.Bold
    }
    MouseArea {
      id: joinMouse
      anchors.fill: parent
      hoverEnabled: true
      enabled: root.bridge.selfState.hangout_id !== root.hangout.id
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
