import QtQuick

Row {
  id: root

  required property var bridge
  required property var theme
  property bool muted: false
  property bool deafened: false

  spacing: root.theme.spacing.sm
  height: root.theme.space(32)

  Rectangle {
    id: mutedIcon
    width: root.theme.space(32)
    height: width
    radius: root.theme.cornerRadius
    color: root.muted
      ? root.theme.alpha(root.theme.warning, mutedMouse.containsMouse ? 0.3 : 0.18)
      : root.theme.alpha(root.theme.foreground, mutedMouse.containsMouse ? 0.12 : 0.055)
    border.color: root.muted ? root.theme.alpha(root.theme.warning, 0.72) : "transparent"
    border.width: 1

    Image {
      anchors.centerIn: parent
      width: root.theme.space(20)
      height: width
      source: Qt.resolvedUrl(root.muted
        ? "../assets/microphone-muted.svg"
        : "../assets/microphone.svg")
      fillMode: Image.PreserveAspectFit
    }

    MouseArea {
      id: mutedMouse
      anchors.fill: parent
      acceptedButtons: Qt.LeftButton
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: root.bridge.toggleMuted()
    }

    Rectangle {
      visible: mutedMouse.containsMouse
      z: 10
      anchors.left: parent.left
      anchors.bottom: parent.top
      anchors.bottomMargin: root.theme.spacing.sm
      width: mutedTip.implicitWidth + root.theme.spacing.lg * 2
      height: root.theme.space(28)
      radius: root.theme.cornerRadius
      color: root.theme.surface
      border.color: root.theme.alpha(root.theme.warning, 0.72)

      Text {
        id: mutedTip
        anchors.centerIn: parent
        text: root.deafened
          ? "Unmute microphone and undeafen"
          : root.muted ? "Unmute microphone" : "Mute microphone"
        color: root.muted ? root.theme.warning : root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }
    }
  }

  Rectangle {
    id: deafenedIcon
    width: root.theme.space(32)
    height: width
    radius: root.theme.cornerRadius
    color: root.deafened
      ? root.theme.alpha(root.theme.danger, deafenedMouse.containsMouse ? 0.32 : 0.2)
      : root.theme.alpha(root.theme.foreground, deafenedMouse.containsMouse ? 0.12 : 0.055)
    border.color: root.deafened ? root.theme.alpha(root.theme.danger, 0.72) : "transparent"
    border.width: 1

    Image {
      anchors.centerIn: parent
      width: root.theme.space(20)
      height: width
      source: Qt.resolvedUrl(root.deafened
        ? "../assets/deafened.svg"
        : "../assets/headphones.svg")
      fillMode: Image.PreserveAspectFit
    }

    MouseArea {
      id: deafenedMouse
      anchors.fill: parent
      acceptedButtons: Qt.LeftButton
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: root.bridge.toggleDeafened()
    }

    Rectangle {
      visible: deafenedMouse.containsMouse
      z: 10
      anchors.left: parent.left
      anchors.bottom: parent.top
      anchors.bottomMargin: root.theme.spacing.sm
      width: deafenedTip.implicitWidth + root.theme.spacing.lg * 2
      height: root.theme.space(28)
      radius: root.theme.cornerRadius
      color: root.theme.surface
      border.color: root.theme.alpha(root.theme.danger, 0.72)

      Text {
        id: deafenedTip
        anchors.centerIn: parent
        text: root.deafened ? "Undeafen · keep microphone muted" : "Deafen and mute microphone"
        color: root.deafened ? root.theme.danger : root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }
    }
  }
}
