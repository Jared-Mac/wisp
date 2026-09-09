import QtQuick
import QtQuick.Controls

Rectangle {
  id: root; objectName: "currentCallBar"
  required property var bridge
  required property var theme
  property bool compact: false
  property real maximumHeight: theme.space(210)
  property bool roomInvitesInHeader: false
  readonly property bool inviteInRoomHeader: roomInvitesInHeader
    && bridge.voiceServerId === String(bridge.activeServer.id)
    && (bridge.spots || []).some(function(room) { return !!room.active_hangout_id && room.active_hangout_id === root.bridge.selfState.hangout_id })
  signal cameraRequested()
  readonly property bool inCall: !!bridge.currentVoiceRoom
  visible: inCall
  implicitHeight: inCall ? header.height + root.theme.spacing.md * 2 + (theme.friendly ? theme.space(4) : 0)
    + Math.min(controls.implicitHeight, Math.max(root.theme.space(40), maximumHeight - header.height - root.theme.spacing.md * 2 - (theme.friendly ? theme.space(4) : 0))) : 0
  color: theme.friendly ? theme.alpha(theme.accent,0.045) : theme.surface
  radius: theme.cornerRadius
  Rectangle { width: parent.width; height: 1; color: root.theme.separator }
  Item {
    id: header
    anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
    anchors.margins: root.theme.spacing.md; height: disconnect.height
    Item {
      anchors.left: parent.left; anchors.leftMargin: root.theme.space(8)
      anchors.right: disconnect.left; anchors.rightMargin: root.theme.spacing.sm
      height: parent.height
      Text {
        id: location
        objectName: "currentCallLocation"
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(1, Math.min(implicitWidth, parent.width - connection.implicitWidth - root.theme.spacing.sm))
        elide: Text.ElideRight
        text: root.bridge.currentVoiceRoom ? (root.bridge.voiceServerId === String(root.bridge.activeServer.id) ? "" : (root.bridge.currentVoiceRoom.server_name || "Wisp") + " / ") + root.bridge.currentVoiceLabel : ""
        color: root.theme.accent; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption; font.weight: root.theme.friendly ? Font.DemiBold : Font.Normal
        HoverHandler { id: locationHover }
        ToolTip.visible: locationHover.hovered && truncated; ToolTip.text: text
      }
      Text {
        id: connection; objectName: "currentCallConnection"
        anchors.left: location.right; anchors.leftMargin: root.theme.spacing.sm
        anchors.verticalCenter: parent.verticalCenter
        text: root.bridge.mediaState.livekit_connected ? "· connected" : "· connecting…"
        color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
      }
    }
    ChatButton {
      id: disconnect; objectName: "currentCallDisconnect"
      anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
      theme: root.theme; text: "d/c"; destructive: true
      iconName: "disconnect"; iconOnly: root.theme.friendly
      Binding on implicitWidth { when: root.theme.friendly; value: root.theme.space(root.compact ? 32 : 40); restoreMode: Binding.RestoreBindingOrValue }
      Binding on implicitHeight {when:root.theme.friendly;value:root.theme.space(root.compact ? 32 : 40);restoreMode:Binding.RestoreBindingOrValue}
      Accessible.name: "Disconnect from voice"
      ToolTip.visible: hovered; ToolTip.text: Accessible.name
      onClicked: root.bridge.leave()
    }
  }
  Flickable {
    anchors.left: parent.left; anchors.right: parent.right; anchors.top: header.bottom; anchors.bottom: parent.bottom
    anchors.leftMargin: root.theme.spacing.md; anchors.rightMargin: root.theme.spacing.md; anchors.bottomMargin: root.theme.spacing.md
    anchors.topMargin: root.theme.friendly ? root.theme.space(4) : 0
    contentWidth: width; contentHeight: controls.implicitHeight
    clip: true; boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar {}
    MediaControls {
      id: controls; width: parent.width; bridge: root.bridge; theme: root.theme; compact: root.compact; showLeave: false
      showInvite: !root.inviteInRoomHeader
      showRemoteStreams: !root.inviteInRoomHeader
      onCameraRequested: root.cameraRequested()
    }
  }
}
