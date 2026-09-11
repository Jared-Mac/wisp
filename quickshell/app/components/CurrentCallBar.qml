import QtQuick
import QtQuick.Controls

Rectangle {
  id: root; objectName: "currentCallBar"
  required property var bridge
  required property var theme
  property bool compact: false
  property bool horizontal: false
  property bool adaptive: false
  property bool showAudio: true
  property bool showSoundboard: true
  property bool embedded: false
  readonly property real inset: adaptive ? Math.min(theme.spacing.md, Math.max(0,(width-theme.space(20))/8)) : theme.spacing.md
  readonly property bool narrow: adaptive && width < theme.space(140)
  property real maximumHeight: theme.space(210)
  property bool roomInvitesInHeader: false
  readonly property bool inviteInRoomHeader: roomInvitesInHeader
    && bridge.voiceServerId === String(bridge.activeServer.id)
    && (bridge.spots || []).some(function(room) { return !!room.active_hangout_id && room.active_hangout_id === root.bridge.selfState.hangout_id })
  signal cameraRequested()
  readonly property bool inCall: !!bridge.currentVoiceRoom
  visible: inCall
  implicitHeight: !inCall ? 0 : root.inset*2+header.height+(controls.implicitHeight>0 ? root.theme.space(4)+Math.min(controls.implicitHeight,Math.max(0,maximumHeight-header.height-root.inset*2)) : 0)
  color: embedded ? "transparent" : theme.friendly ? theme.alpha(theme.accent,0.045) : theme.surface
  radius: theme.cornerRadius
  Rectangle { visible: !root.embedded; width: parent.width; height: 1; color: root.theme.separator }
  Item {
    id: header
    x: root.inset; y: root.inset
    width: root.width - root.inset * 2
    height: headerActions.height
    Item {
      visible: !root.narrow
      anchors.left: parent.left; anchors.leftMargin: root.narrow ? 0 : root.theme.space(8)
      anchors.right: headerActions.left; anchors.rightMargin: root.theme.spacing.sm
      height: parent.height
      Text {
        id: location
        objectName: "currentCallLocation"
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(1, Math.min(implicitWidth, parent.width - (connection.visible ? connection.implicitWidth + root.theme.spacing.sm : 0)))
        elide: Text.ElideRight
        text: root.bridge.currentVoiceRoom ? (root.bridge.voiceServerId === String(root.bridge.activeServer.id) ? "" : (root.bridge.currentVoiceRoom.server_name || "Wisp") + " / ") + root.bridge.currentVoiceLabel : ""
        color: root.theme.accent; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption; font.weight: root.theme.friendly ? Font.DemiBold : Font.Normal
        HoverHandler { id: locationHover }
        ToolTip.visible: locationHover.hovered && truncated; ToolTip.text: text
      }
      Text {
        id: connection; visible: !root.narrow && header.width-headerActions.width >= root.theme.space(220); objectName: "currentCallConnection"
        anchors.left: location.right; anchors.leftMargin: root.theme.spacing.sm
        anchors.verticalCenter: parent.verticalCenter
        text: root.bridge.mediaState.livekit_connected ? "· connected" : "· connecting…"
        color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
      }
    }
    Flow {
      id:headerActions;anchors.right:parent.right
      width:Math.min(parent.width,root.theme.space(96)+root.theme.spacing.sm+root.theme.space(4));spacing:root.theme.space(4)
      MediaControls {
        width:Math.min(parent.width,root.theme.space(64)+root.theme.spacing.sm);bridge:root.bridge;theme:root.theme;compact:true;adaptive:true
        showAudio:false;showInvite:false;showLeave:false;showSoundboard:false;showRemoteStreams:false;showPushToTalk:false
        onCameraRequested:root.cameraRequested()
      }
    ChatButton {
      id: disconnect; objectName: "currentCallDisconnect"
      theme: root.theme; text: "d/c"; destructive: true
      iconName: "disconnect"; iconOnly: true; forceIcon: true
      Binding on implicitWidth { when: true; value: Math.min(headerActions.width,root.theme.space(32)); restoreMode: Binding.RestoreBindingOrValue }
      Binding on implicitHeight {when:true;value:root.theme.space(32);restoreMode:Binding.RestoreBindingOrValue}
      Accessible.name: "Disconnect from voice"
      ToolTip.visible: hovered; ToolTip.text: Accessible.name + (root.narrow ? " · " + root.bridge.currentVoiceLabel : "")
      onClicked: root.bridge.leave()
    }
    }

  }
  Flickable {
    anchors.left: parent.left; anchors.right: parent.right; anchors.top: header.bottom; anchors.bottom: parent.bottom
    anchors.leftMargin: root.inset; anchors.rightMargin: root.inset; anchors.bottomMargin: root.inset
    anchors.topMargin: controls.implicitHeight>0 ? root.theme.space(4) : 0
    contentWidth: width; contentHeight: controls.implicitHeight
    clip: true; boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar {}
    MediaControls {
      id: controls; width: parent.width; bridge: root.bridge; theme: root.theme; compact: root.compact; adaptive: root.adaptive; showLeave: false
      showInvite: !root.inviteInRoomHeader
      showAudio: root.showAudio; showPublishing:false; showSoundboard:root.showSoundboard && root.showAudio && (root.theme.friendly || small)
      showRemoteStreams: !root.inviteInRoomHeader
      onCameraRequested: root.cameraRequested()
    }
  }
}
