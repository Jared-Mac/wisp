import QtQuick

Column {
  id: root
  required property var bridge
  required property var theme
  signal leaveRequested()
  signal cameraRequested()
  spacing: root.theme.spacing.sm

  Flow {
    id: controls
    // Legacy Row allowed rounded children to overrun by a pixel. Leave a
    // non-rendering guard band so Flow never wraps that unchanged legacy row.
    width: parent.width + (root.theme.terminal ? 0 : root.theme.space(4))
    spacing: root.theme.spacing.sm

    Repeater {
      id: controlRepeater
      model: [
        { "label": root.bridge.shareStarting ? "Choosing…" : root.bridge.sharing ? "Stop share" : "Share screen", "action": "share" },
        { "label": root.bridge.cameraStarting ? "Starting…" : root.bridge.cameraActive ? "Camera off" : "Camera on", "action": "camera" },
        { "label": "Leave", "action": "leave" }
      ]
      delegate: Rectangle {
        required property var modelData
        objectName: "mediaAction-" + modelData.action
        readonly property bool controlEnabled: (modelData.action !== "share" || !root.bridge.shareStarting)
          && (modelData.action !== "camera" || (!root.bridge.cameraStarting
            && root.bridge.cameraState.devices.length > 0))
        width: root.theme.performative ? controlLabel.implicitWidth + root.theme.space(16) : Math.max(root.theme.terminal ? controlLabel.implicitWidth + root.theme.space(20) : 0, (root.width
          - controls.spacing * (controlRepeater.count - 1))
          / Math.max(1, controlRepeater.count))
        height: root.theme.space(root.theme.performative ? 28 : 34)
        radius: root.theme.cornerRadius
        color: root.theme.performative && !controlMouse.containsMouse ? "transparent" : controlMouse.containsMouse
          ? (modelData.action === "leave" ? root.theme.alpha(root.theme.danger, 0.28) : root.theme.alpha(root.theme.foreground, 0.12))
          : root.theme.alpha(root.theme.foreground, 0.065)
        opacity: controlEnabled ? 1 : 0.55

        Text {
          id: controlLabel
          anchors.centerIn: parent
          text: root.theme.performative ? "[" + modelData.label.toLowerCase() + "]" : modelData.label
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
            if (modelData.action === "share") root.bridge.toggleShare()
            else if (modelData.action === "camera") root.cameraRequested()
            else { root.bridge.leave(); root.leaveRequested() }
          }
        }
      }
    }
  }

  Repeater {
    model: root.bridge.remoteVideos

    delegate: Rectangle {
      required property var modelData
      readonly property bool watching: !!modelData.surface_open || !!modelData.subscribed
      width: root.width
      height: root.theme.space(42)
      radius: root.theme.cornerRadius
      color: root.theme.alpha(root.theme.accent, watching ? 0.15 : 0.08)
      border.width: 1
      border.color: root.theme.alpha(root.theme.accent, watching ? 0.55 : 0.24)

      Text {
        id: remoteVideoLabel
        anchors.left: parent.left
        anchors.leftMargin: root.theme.spacing.lg
        anchors.verticalCenter: parent.verticalCenter
        text: String(modelData.participant || "Friend")
          + (modelData.source === "camera" ? " camera" : " screen")
          + (modelData.requested_quality ? " · " + modelData.requested_quality : "")
        color: root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
        font.weight: Font.DemiBold
        Binding on width { when: root.theme.terminal; value: Math.max(0, watchButton.x - remoteVideoLabel.x - root.theme.spacing.md); restoreMode: Binding.RestoreBindingOrValue }
        elide: root.theme.terminal ? Text.ElideRight : Text.ElideNone
      }

      Rectangle {
        id: watchButton
        anchors.right: parent.right
        anchors.rightMargin: root.theme.spacing.sm
        anchors.verticalCenter: parent.verticalCenter
        width: watchLabel.implicitWidth + root.theme.spacing.lg * 2
        height: root.theme.space(30)
        radius: root.theme.cornerRadius
        color: watchMouse.containsMouse
          ? Qt.lighter(root.theme.accent, 1.12) : root.theme.accent

        Text {
          id: watchLabel
          anchors.centerIn: parent
          text: parent.parent.watching ? "Close" : "Watch"
          color: root.theme.accentText
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
          font.weight: Font.Bold
        }

        MouseArea {
          id: watchMouse
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: root.bridge.watchVideo(modelData, !parent.parent.watching)
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
