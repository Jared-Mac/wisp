import QtQuick

Column {
  id: root
  required property var bridge
  required property var theme
  property string pendingRevokeId: ""
  width: parent ? parent.width : 0
  spacing: root.theme.spacing.lg

  Row {
    width: parent.width

    Column {
      width: parent.width - refreshButton.width

      Text {
        text: "Devices & privacy"
        color: root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.body
        font.weight: Font.DemiBold
      }

      Text {
        text: root.bridge.inHangout
          ? (root.bridge.mediaState.e2ee_enabled
            ? "Media is end-to-end encrypted" : "Development call · media encryption off")
          : "Each enrolled device can be revoked independently"
        color: root.bridge.inHangout && root.bridge.mediaState.e2ee_enabled
          ? root.theme.accent : root.theme.muted
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }
    }

    Rectangle {
      id: refreshButton
      width: refreshText.implicitWidth + root.theme.spacing.lg * 2
      height: root.theme.space(30)
      radius: root.theme.cornerRadius
      color: refreshMouse.containsMouse
        ? root.theme.alpha(root.theme.foreground, 0.12)
        : root.theme.alpha(root.theme.foreground, 0.055)

      Text {
        id: refreshText
        anchors.centerIn: parent
        text: "Refresh"
        color: root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }

      MouseArea {
        id: refreshMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.bridge.refreshDevices()
      }
    }
  }

  Repeater {
    model: root.bridge.devices
    delegate: Rectangle {
      required property var modelData
      width: root.width
      height: root.theme.space(42)
      radius: root.theme.cornerRadius
      color: root.theme.alpha(root.theme.foreground, 0.05)

      Text {
        anchors.left: parent.left
        anchors.leftMargin: root.theme.spacing.lg
        anchors.verticalCenter: parent.verticalCenter
        text: String(modelData.name || "Device")
        color: modelData.revoked ? root.theme.muted : root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }

      Text {
        anchors.right: parent.right
        anchors.rightMargin: root.theme.spacing.lg
        anchors.verticalCenter: parent.verticalCenter
        text: modelData.revoked ? "Revoked"
          : root.pendingRevokeId === String(modelData.id) ? "Confirm" : "Revoke"
        color: modelData.revoked ? root.theme.muted : root.theme.danger
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption

        MouseArea {
          anchors.fill: parent
          enabled: !modelData.revoked
          cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
          onClicked: {
            if (root.pendingRevokeId === String(modelData.id)) {
              root.bridge.revokeDevice(modelData.id)
              root.pendingRevokeId = ""
            } else {
              root.pendingRevokeId = String(modelData.id)
            }
          }
        }
      }
    }
  }

  Column {
    visible: String(root.bridge.selfState.display_name || "") === "Jared"
    width: parent.width
    spacing: root.theme.spacing.sm

    Text {
      text: "Invite a friend’s device"
      color: root.theme.foreground
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
      font.weight: Font.DemiBold
    }

    Flow {
      width: parent.width
      height: childrenRect.height
      spacing: root.theme.spacing.sm

      Repeater {
        model: root.bridge.friends
        delegate: Rectangle {
          required property var modelData
          width: inviteLabel.implicitWidth + root.theme.spacing.lg * 2
          height: root.theme.space(30)
          radius: root.theme.cornerRadius
          color: inviteMouse.containsMouse
            ? root.theme.alpha(root.theme.accent, 0.28)
            : root.theme.alpha(root.theme.foreground, 0.06)

          Text {
            id: inviteLabel
            anchors.centerIn: parent
            text: String(modelData.display_name || "Friend")
            color: root.theme.foreground
            font.family: root.theme.font.family
            font.pixelSize: root.theme.font.caption
          }

          MouseArea {
            id: inviteMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.bridge.createInvite(modelData.display_name, 30)
          }
        }
      }
    }

    Rectangle {
      visible: !!root.bridge.lastInvite
      width: parent.width
      height: visible ? root.theme.space(62) : 0
      radius: root.theme.cornerRadius
      color: root.theme.alpha(root.theme.accent, 0.12)

      Column {
        anchors.fill: parent
        anchors.margins: root.theme.spacing.md
        spacing: root.theme.spacing.xs

        Text {
          text: root.bridge.lastInvite
            ? "One-use invite for " + String(root.bridge.lastInvite.profile) : ""
          color: root.theme.muted
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }

        TextInput {
          width: parent.width
          readOnly: true
          selectByMouse: true
          text: root.bridge.lastInvite ? String(root.bridge.lastInvite.code) : ""
          color: root.theme.foreground
          selectionColor: root.theme.accent
          font.family: "monospace"
          font.pixelSize: root.theme.font.caption
        }
      }
    }
  }
}
