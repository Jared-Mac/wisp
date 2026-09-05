import QtQuick
import QtQuick.Controls

Column {
  id: root
  required property var bridge
  required property var theme
  signal leaveRequested()
  signal cameraRequested()
  property bool showLeave: true
  spacing: root.theme.spacing.sm
  RoomInvitePicker { id: invitePicker; bridge: root.bridge; theme: root.theme }

  Flow {
    id: controls
    // Legacy Row allowed rounded children to overrun by a pixel. Leave a
    // non-rendering guard band so Flow never wraps that unchanged legacy row.
    width: parent.width + (root.theme.terminal ? 0 : root.theme.space(4))
    spacing: root.theme.spacing.sm

    Repeater {
      id: controlRepeater
      model: [
        { "label": root.bridge.sharing ? "Stop share" : root.bridge.shareStarting ? "Choosing…" : "Share", "action": "share" },
        { "label": root.bridge.cameraActive ? "Stop cam" : root.bridge.cameraStarting ? "Starting…" : "Camera", "action": "camera" },
        { "label": "Invite", "action": "invite" },
        { "label": "d/c", "action": "leave" }
      ].filter(function(action) { return root.showLeave || action.action !== "leave" })
      delegate: Rectangle {
        required property var modelData
        objectName: "mediaAction-" + modelData.action
        Accessible.name: modelData.action === "share" ? (publishing ? "Stop sharing screen" : "Share screen")
          : modelData.action === "camera" ? (publishing ? "Stop camera" : "Start camera") : modelData.action === "leave" ? "Disconnect from voice" : modelData.label
        ToolTip {
          visible: modelData.action === "invite" && controlMouse.containsMouse
          text: "Invite friends"
          y: parent.height + root.theme.spacing.sm
        }
        readonly property bool publishing: (modelData.action === "share" && root.bridge.sharing)
          || (modelData.action === "camera" && root.bridge.cameraActive)
        readonly property bool controlEnabled: publishing || ((modelData.action !== "share" || !root.bridge.shareStarting)
          && (modelData.action !== "camera" || (!root.bridge.cameraStarting
            && root.bridge.cameraState.devices.length > 0)))
        width: Math.min(root.width, root.theme.tui ? controlLabel.implicitWidth + root.theme.space(16) : Math.max(controlLabel.implicitWidth + root.theme.space(20), (root.width
          - controls.spacing * (controlRepeater.count - 1))
          / Math.max(1, controlRepeater.count)))
        height: Math.max(root.theme.space(root.theme.tui ? 28 : 34), controlLabel.implicitHeight + root.theme.space(10))
        radius: root.theme.cornerRadius
        border.width: publishing ? 1 : 0
        border.color: root.theme.danger
        color: publishing ? root.theme.alpha(root.theme.danger, controlMouse.pressed ? 0.42 : controlMouse.containsMouse ? 0.32 : 0.2)
          : root.theme.tui && !controlMouse.containsMouse ? "transparent" : controlMouse.containsMouse
          ? (modelData.action === "leave" ? root.theme.alpha(root.theme.danger, 0.28) : root.theme.alpha(root.theme.foreground, 0.12))
          : root.theme.alpha(root.theme.foreground, 0.065)
        opacity: controlEnabled ? 1 : 0.55

        Text {
          id: controlLabel
          anchors.centerIn: parent
          width: Math.min(implicitWidth, parent.width - root.theme.space(16))
          wrapMode: modelData.action === "invite" ? Text.NoWrap : Text.Wrap
          horizontalAlignment: Text.AlignHCenter
          text: root.theme.tui ? "[" + modelData.label.toLowerCase() + "]" : modelData.action === "leave" ? "Disconnect from voice" : modelData.label
          color: parent.publishing || modelData.action === "leave" ? root.theme.danger : root.theme.foreground
          font.weight: parent.publishing ? Font.Bold : Font.Normal
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
            else if (modelData.action === "invite") invitePicker.open()
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
