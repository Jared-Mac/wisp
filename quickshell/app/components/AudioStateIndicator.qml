import QtQuick
import QtQuick.Controls

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
    objectName: "muteControl"
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.muted ? "Unmute microphone" : "Mute microphone"
    Keys.onSpacePressed: root.bridge.toggleMuted()
    Keys.onReturnPressed: root.bridge.toggleMuted()
    width: root.theme.space(32)
    height: width
    radius: root.theme.cornerRadius
    color: root.theme.tui ? "transparent" : root.muted
      ? root.theme.alpha(root.theme.warning, mutedMouse.containsMouse ? 0.3 : 0.18)
      : root.theme.alpha(root.theme.foreground, mutedMouse.containsMouse ? 0.12 : 0.055)
    border.color: activeFocus ? root.theme.focusBorder : root.muted ? root.theme.alpha(root.theme.warning, 0.72) : "transparent"
    border.width: root.theme.tui && !activeFocus ? 0 : 1

    Image {
      visible: !root.theme.tui
      anchors.centerIn: parent
      width: root.theme.space(20)
      height: width
      source: Qt.resolvedUrl(root.muted
        ? "../assets/microphone-muted.svg"
        : "../assets/microphone.svg")
      fillMode: Image.PreserveAspectFit
    }
    Text {
      anchors.centerIn: parent; visible: root.theme.tui
      text: "[M]"; color: root.muted ? root.theme.warning : root.theme.foreground
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }

    MouseArea {
      id: mutedMouse
      anchors.fill: parent
      acceptedButtons: Qt.LeftButton
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: root.bridge.toggleMuted()
    }

    ToolTip {
      id: muteTooltip
      objectName: "muteTooltip"
      visible: mutedMouse.containsMouse
      x: (parent.width - width) / 2
      y: parent.height + root.theme.spacing.sm
      margins: root.theme.spacing.sm
      padding: root.theme.spacing.sm
      width: Math.min(mutedTip.implicitWidth + padding * 2, root.Window.window ? root.Window.window.width - margins * 2 : root.theme.space(360))
      background: Rectangle {
        radius: root.theme.cornerRadius
        color: root.theme.surface
        border.color: root.theme.alpha(root.theme.warning, 0.72)
      }
      contentItem: Text {
        id: mutedTip
        wrapMode: Text.Wrap
        text: (root.deafened
          ? "Unmute microphone and undeafen"
          : root.muted ? "Unmute microphone" : "Mute microphone") + " · Shift+M"
        color: root.muted ? root.theme.warning : root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }
    }
  }

  Rectangle {
    id: deafenedIcon
    objectName: "deafenControl"
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.deafened ? "Undeafen" : "Deafen"
    Keys.onSpacePressed: root.bridge.toggleDeafened()
    Keys.onReturnPressed: root.bridge.toggleDeafened()
    width: root.theme.space(32)
    height: width
    radius: root.theme.cornerRadius
    color: root.theme.tui ? "transparent" : root.deafened
      ? root.theme.alpha(root.theme.danger, deafenedMouse.containsMouse ? 0.32 : 0.2)
      : root.theme.alpha(root.theme.foreground, deafenedMouse.containsMouse ? 0.12 : 0.055)
    border.color: activeFocus ? root.theme.focusBorder : root.deafened ? root.theme.alpha(root.theme.danger, 0.72) : "transparent"
    border.width: root.theme.tui && !activeFocus ? 0 : 1

    Image {
      visible: !root.theme.tui
      anchors.centerIn: parent
      width: root.theme.space(20)
      height: width
      source: Qt.resolvedUrl(root.deafened
        ? "../assets/deafened.svg"
        : "../assets/headphones.svg")
      fillMode: Image.PreserveAspectFit
    }
    Text {
      anchors.centerIn: parent; visible: root.theme.tui
      text: "[D]"; color: root.deafened ? root.theme.danger : root.theme.foreground
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }

    MouseArea {
      id: deafenedMouse
      anchors.fill: parent
      acceptedButtons: Qt.LeftButton
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: root.bridge.toggleDeafened()
    }

    ToolTip {
      id: deafenTooltip
      objectName: "deafenTooltip"
      visible: deafenedMouse.containsMouse
      x: (parent.width - width) / 2
      y: parent.height + root.theme.spacing.sm
      margins: root.theme.spacing.sm
      padding: root.theme.spacing.sm
      width: Math.min(deafenedTip.implicitWidth + padding * 2, root.Window.window ? root.Window.window.width - margins * 2 : root.theme.space(360))
      background: Rectangle {
        radius: root.theme.cornerRadius
        color: root.theme.surface
        border.color: root.theme.alpha(root.theme.danger, 0.72)
      }
      contentItem: Text {
        id: deafenedTip
        wrapMode: Text.Wrap
        text: (root.deafened ? "Undeafen · keep microphone muted" : "Deafen and mute microphone") + " · Shift+D"
        color: root.deafened ? root.theme.danger : root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }
    }
  }
}
