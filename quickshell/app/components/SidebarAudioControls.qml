import QtQuick
import QtQuick.Controls

Rectangle {
  id: root
  objectName: "sidebarAudioFooter"
  required property var bridge
  required property var theme
  property bool horizontal: false
  property bool roomInvitesInHeader: false
  property real maximumCallHeight: theme.space(280)
  signal cameraRequested()
  readonly property bool inCall: !!bridge.currentVoiceRoom
  readonly property real inset: Math.min(theme.space(8), Math.max(0, (width-theme.space(20))/8))
  readonly property real controlsWidth: Math.max(1, width-inset*2)
  readonly property real groupWidth: Math.min(controlsWidth, theme.space(96)+theme.spacing.sm*2)
  readonly property bool inviteInRoomHeader: roomInvitesInHeader
    && bridge.voiceServerId === String(bridge.activeServer.id)
    && (bridge.spots || []).some(function(room) { return !!room.active_hangout_id && room.active_hangout_id === root.bridge.selfState.hangout_id })
  readonly property string roomName: inCall ? String(bridge.currentVoiceLabel || (bridge.currentVoiceRoom && bridge.currentVoiceRoom.label) || "Voice") : ""
  implicitHeight: content.implicitHeight+inset*2
  height: implicitHeight
  color: theme.friendly ? theme.alpha(theme.accent, 0.045) : theme.surface
  radius: theme.cornerRadius
  border.width: 1
  border.color: theme.separator

  Column {
    id: content; objectName: "currentCallBar"
    x: root.inset; y: root.inset; width: root.controlsWidth
    spacing: root.theme.space(4)
    Row {
      id: status
      visible: root.controlsWidth >= root.theme.space(80)
      width: parent.width; spacing: root.theme.spacing.xs
      Text {
        id: location; objectName: "currentCallLocation"
        width: Math.min(implicitWidth, Math.max(1,status.width-(connection.visible ? connection.width+status.spacing : 0)))
        text: root.inCall ? root.roomName : "Audio"
        elide: Text.ElideRight
        color: root.inCall ? root.theme.accent : root.theme.muted
        font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
      }
      Text {
        id: connection; objectName: "currentCallConnection"
        visible: root.inCall
        text: root.bridge.mediaState.livekit_connected ? "- connected" : "- connecting…"
        color: root.theme.muted
        font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
      }
      HoverHandler { id: statusHover }
      ToolTip.visible: statusHover.hovered
      ToolTip.text: root.inCall ? root.roomName + " " + connection.text : "Audio"
    }
    Flow {
      width: parent.width; spacing: root.theme.spacing.sm
      AudioStateIndicator {
        id: audio; objectName: "globalAudioControls"
        bridge: root.bridge; theme: root.theme
        adaptive: true; availableWidth: root.groupWidth
        tooltipAbove: true
        muted: !!root.bridge.selfState.muted || !!root.bridge.selfState.deafened
        deafened: !!root.bridge.selfState.deafened
      }
      MediaControls {
        visible: root.inCall; width: root.groupWidth
        bridge: root.bridge; theme: root.theme; compact: true; adaptive: true
        leaveObjectName: "currentCallDisconnect"
        showAudio: false; showSoundboard: false; showInvite: false
        showRemoteStreams: false; showPushToTalk: false
        onCameraRequested: root.cameraRequested()
      }
    }
    MediaControls {
      width: parent.width
      bridge: root.bridge; theme: root.theme; compact: true; adaptive: true
      visible: implicitHeight > 0
      showAudio: false; showPublishing: false; showSoundboard: false; showLeave: false
      showInvite: root.inCall && !root.inviteInRoomHeader
      showRemoteStreams: root.inCall && !root.inviteInRoomHeader
      showPushToTalk: root.inCall
    }
  }
}
