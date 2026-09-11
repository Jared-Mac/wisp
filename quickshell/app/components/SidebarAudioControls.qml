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
  readonly property bool inlineControls: horizontal && width >= theme.space(480)
  readonly property real controlsWidth: Math.max(1, width-inset*2)
  implicitHeight: inlineControls ? Math.max(call.height, audioRow.height+inset*2)
    : (inCall ? call.height : 0) + audioRow.height + inset*2
  height: implicitHeight
  color: theme.friendly ? theme.alpha(theme.accent, 0.045) : theme.surface
  radius: theme.cornerRadius
  border.width: 1
  border.color: theme.separator

  CurrentCallBar {
    id: call
    visible: root.inCall
    width: root.inlineControls ? root.width-audioRow.width-root.inset*2 : root.width
    height: visible ? implicitHeight : 0
    bridge: root.bridge; theme: root.theme
    compact: true; adaptive: true; horizontal: root.inlineControls
    embedded: true; showAudio: false; showSoundboard: false
    maximumHeight: root.maximumCallHeight
    roomInvitesInHeader: root.roomInvitesInHeader
    onCameraRequested: root.cameraRequested()
  }

  Rectangle {
    visible: root.inCall && !root.inlineControls
    x: root.inset; y: call.height; width: root.controlsWidth; height: 1
    color: root.theme.separator
  }
  Row {
    id: audioRow
    x: root.inlineControls ? root.width-width-root.inset : root.inset
    y: root.inlineControls ? (root.height-height)/2 : call.height+root.inset
    spacing: root.theme.spacing.sm
    Text {
      visible: !root.inCall && root.controlsWidth >= controls.width+implicitWidth+audioRow.spacing
      width: visible ? root.controlsWidth-controls.width-audioRow.spacing : 0
      height: controls.height
      text: root.theme.tui ? "/audio" : "Audio"
      verticalAlignment: Text.AlignVCenter
      color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    Flow {
      id: controls
      readonly property real gap: root.theme.spacing.sm
      readonly property real cell: root.theme.space(32)
      width: Math.min(root.controlsWidth, cell*3+gap*2)
      spacing: gap
      AudioStateIndicator {
        id: audio; objectName: "globalAudioControls"
        bridge: root.bridge; theme: root.theme
        adaptive: true; availableWidth: controls.width
        tooltipAbove: true
        muted: !!root.bridge.selfState.muted || !!root.bridge.selfState.deafened
        deafened: !!root.bridge.selfState.deafened
      }
    }
  }
}
