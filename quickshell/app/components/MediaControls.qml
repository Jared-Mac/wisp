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

    AudioStateIndicator {
      id: audioControls
      bridge: root.bridge
      theme: root.theme
      muted: !!root.bridge.selfState.muted || !!root.bridge.selfState.deafened
      deafened: !!root.bridge.selfState.deafened
    }

    Repeater {
      id: controlRepeater
      model: [
        { "label": root.bridge.mediaState.surface_open ? "Close video" : "Open video", "action": "video" },
        { "label": root.bridge.shareStarting ? "Choosing…" : root.bridge.sharing ? "Stop share" : "Share screen", "action": "share" },
        { "label": "Leave", "action": "leave" }
      ]
      delegate: Rectangle {
        required property var modelData
        readonly property bool controlEnabled: modelData.action !== "share" || !root.bridge.shareStarting
        width: (controls.width - audioControls.width
          - controls.spacing * controlRepeater.count)
          / Math.max(1, controlRepeater.count)
        height: root.theme.space(34)
        radius: root.theme.cornerRadius
        color: controlMouse.containsMouse
          ? (modelData.action === "leave" ? root.theme.alpha(root.theme.danger, 0.28) : root.theme.alpha(root.theme.foreground, 0.12))
          : root.theme.alpha(root.theme.foreground, 0.065)
        opacity: controlEnabled ? 1 : 0.55

        Text {
          anchors.centerIn: parent
          text: modelData.label
          color: modelData.action === "leave" ? root.theme.danger : root.theme.foreground
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }
        MouseArea {
          id: controlMouse
          anchors.fill: parent
          enabled: parent.controlEnabled
          hoverEnabled: true
          cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
          onClicked: {
            if (modelData.action === "video") root.bridge.toggleSurface()
            else if (modelData.action === "share") root.bridge.toggleShare()
            else { root.bridge.leave(); root.leaveRequested() }
          }
        }
      }
    }
  }

  Rectangle {
    visible: root.bridge.shareStarting || root.bridge.sharing
    width: parent.width
    height: root.theme.space(34)
    radius: root.theme.cornerRadius
    color: root.theme.alpha(root.theme.accent, 0.12)

    Text {
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.leftMargin: root.theme.spacing.lg
      anchors.rightMargin: root.theme.spacing.lg
      anchors.verticalCenter: parent.verticalCenter
      text: root.bridge.shareStarting
        ? "Choose a monitor or window in the system picker"
        : "Sharing " + String(root.bridge.screenShareState.source || "screen")
          + " · " + String(root.bridge.screenShareState.width || "?")
          + "×" + String(root.bridge.screenShareState.height || "?")
          + " @ " + String(root.bridge.screenShareState.fps || 30) + " fps"
      elide: Text.ElideRight
      color: root.theme.accent
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
      font.weight: Font.DemiBold
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
        ? root.theme.alpha(root.theme.warning, 0.16)
        : root.theme.alpha(root.theme.foreground, talkMouse.containsMouse ? 0.14 : 0.075)

    Text {
      anchors.centerIn: parent
      text: root.bridge.selfState.muted
        ? "Unmute before talking"
        : root.bridge.pushToTalkState.active ? "Talking — release to stop" : "Hold to talk"
      color: root.bridge.selfState.muted ? root.theme.warning : root.theme.foreground
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
