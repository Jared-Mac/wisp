import QtQuick

Column {
  id: root
  required property var bridge
  required property var theme
  signal leaveRequested()
  spacing: root.theme.spacing.sm

  Row {
    id: controls
    width: parent.width
    spacing: root.theme.spacing.sm

    Repeater {
      model: [
        { "label": root.bridge.selfState.muted ? "Unmute" : "Mute", "action": "mute", "active": !!root.bridge.selfState.muted },
        { "label": root.bridge.mediaState.surface_open ? "Close video" : "Open video", "action": "video", "active": false },
        { "label": root.bridge.selfState.sharing ? "Stop share" : "Share", "action": "share", "active": false },
        { "label": root.bridge.selfState.deafened ? "Undeafen" : "Deafen", "action": "deafen", "active": !!root.bridge.selfState.deafened },
        { "label": "Leave", "action": "leave", "active": false }
      ]
      delegate: Rectangle {
        required property var modelData
        width: (controls.width - controls.spacing * 4) / 5
        height: root.theme.space(34)
        radius: root.theme.cornerRadius
        color: modelData.active
          ? root.theme.alpha(modelData.action === "mute" ? root.theme.danger : root.theme.warning, controlMouse.containsMouse ? 0.36 : 0.24)
          : controlMouse.containsMouse
            ? (modelData.action === "leave" ? root.theme.alpha(root.theme.danger, 0.28) : root.theme.alpha(root.theme.foreground, 0.12))
            : root.theme.alpha(root.theme.foreground, 0.065)

        Text {
          anchors.centerIn: parent
          text: modelData.label
          color: modelData.action === "leave" || (modelData.action === "mute" && modelData.active)
            ? root.theme.danger
            : modelData.action === "deafen" && modelData.active
              ? root.theme.warning
              : root.theme.foreground
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }
        MouseArea {
          id: controlMouse
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: {
            if (modelData.action === "mute") root.bridge.toggleMuted()
            else if (modelData.action === "video") root.bridge.toggleSurface()
            else if (modelData.action === "deafen") root.bridge.toggleDeafened()
            else if (modelData.action === "share") root.bridge.toggleShare()
            else { root.bridge.leave(); root.leaveRequested() }
          }
        }
      }
    }
  }

  Rectangle {
    visible: root.bridge.pushToTalkState.enabled
    width: parent.width
    height: root.theme.space(40)
    radius: root.theme.cornerRadius
    color: root.bridge.pushToTalkState.active
      ? root.theme.alpha(root.theme.accent, 0.52)
      : root.bridge.selfState.muted
        ? root.theme.alpha(root.theme.danger, 0.16)
        : root.theme.alpha(root.theme.foreground, talkMouse.containsMouse ? 0.14 : 0.075)

    Text {
      anchors.centerIn: parent
      text: root.bridge.selfState.muted
        ? "Unmute before talking"
        : root.bridge.pushToTalkState.active ? "Talking — release to stop" : "Hold to talk"
      color: root.bridge.selfState.muted ? root.theme.danger : root.theme.foreground
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.body
      font.weight: Font.DemiBold
    }

    MouseArea {
      id: talkMouse
      anchors.fill: parent
      enabled: !root.bridge.selfState.muted
      hoverEnabled: true
      preventStealing: true
      cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
      onPressed: root.bridge.pushToTalkPress()
      onReleased: root.bridge.pushToTalkRelease()
      onCanceled: root.bridge.pushToTalkRelease()
    }
  }

  Timer {
    interval: 1000
    repeat: true
    running: talkMouse.pressed && root.bridge.pushToTalkState.enabled
    onTriggered: root.bridge.pushToTalkPress()
  }
}
