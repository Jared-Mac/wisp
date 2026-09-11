import QtQuick
import QtQuick.Controls

Grid {
  id: root

  required property var bridge
  required property var theme
  property bool muted: false
  property bool deafened: false
  property bool adaptive: false
  property real availableWidth: 100000
  property bool tooltipAbove: false
  readonly property bool compactSymbols: adaptive && availableWidth < theme.space(220)
  readonly property real buttonHeight: theme.space(theme.comfortable ? 36 : 32)
  readonly property bool stacked: adaptive && availableWidth < mutedIcon.width + deafenedIcon.width + spacing
  columns: stacked ? 1 : 2

  spacing: root.theme.spacing.sm
  height: root.stacked ? root.buttonHeight * 2 + spacing : root.buttonHeight

  Rectangle {
    id: mutedIcon
    objectName: "muteControl"
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.muted ? "Unmute microphone" : "Mute microphone"
    Keys.onSpacePressed: root.bridge.toggleMuted()
    Keys.onReturnPressed: root.bridge.toggleMuted()
    width: Math.min(root.availableWidth,root.theme.space(root.theme.comfortable && !root.compactSymbols ? 92 : 32))
    height: root.theme.space(root.theme.comfortable ? 36 : 32)
    radius: root.theme.cornerRadius
    color: root.theme.tui && !root.theme.comfortable ? "transparent" : root.muted
      ? root.theme.alpha(root.theme.warning, mutedMouse.containsMouse ? 0.3 : 0.18)
      : root.theme.alpha(root.theme.foreground, mutedMouse.containsMouse ? 0.12 : 0.055)
    border.color: activeFocus ? root.theme.focusBorder : root.muted ? root.theme.alpha(root.theme.warning, 0.72) : "transparent"
    border.width: root.theme.tui && !root.theme.comfortable && !activeFocus ? 0 : 1

    Image {
      visible: !root.theme.tui && !root.theme.comfortable && !root.theme.friendly && !root.compactSymbols
      anchors.centerIn: parent
      width: root.theme.space(20)
      height: width
      source: Qt.resolvedUrl(root.muted
        ? "../assets/microphone-muted.svg"
        : "../assets/microphone.svg")
      fillMode: Image.PreserveAspectFit
    }
    WispIcon { anchors.centerIn: parent; theme: root.theme; name: root.muted ? "microphone-off" : "microphone"; ink: root.muted ? root.theme.warning : root.theme.foreground; visible: root.theme.friendly || root.compactSymbols && !root.theme.tui }
    Text {
      anchors.centerIn: parent; visible: root.theme.tui || root.theme.comfortable && !root.compactSymbols
      text: root.theme.comfortable ? (root.muted ? "Unmute" : "Mute") : "[M]"; color: root.muted ? root.theme.warning : root.theme.foreground
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
      y: root.tooltipAbove ? -height-root.theme.spacing.sm : parent.height + root.theme.spacing.sm
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
    width: Math.min(root.availableWidth,root.theme.space(root.theme.comfortable && !root.compactSymbols ? 102 : 32))
    height: root.theme.space(root.theme.comfortable ? 36 : 32)
    radius: root.theme.cornerRadius
    color: root.theme.tui && !root.theme.comfortable ? "transparent" : root.deafened
      ? root.theme.alpha(root.theme.danger, deafenedMouse.containsMouse ? 0.32 : 0.2)
      : root.theme.alpha(root.theme.foreground, deafenedMouse.containsMouse ? 0.12 : 0.055)
    border.color: activeFocus ? root.theme.focusBorder : root.deafened ? root.theme.alpha(root.theme.danger, 0.72) : "transparent"
    border.width: root.theme.tui && !root.theme.comfortable && !activeFocus ? 0 : 1

    Image {
      visible: !root.theme.tui && !root.theme.comfortable && !root.theme.friendly && !root.compactSymbols
      anchors.centerIn: parent
      width: root.theme.space(20)
      height: width
      source: Qt.resolvedUrl(root.deafened
        ? "../assets/deafened.svg"
        : "../assets/headphones.svg")
      fillMode: Image.PreserveAspectFit
    }
    WispIcon { anchors.centerIn: parent; theme: root.theme; name: root.deafened ? "headphones-off" : "headphones"; ink: root.deafened ? root.theme.danger : root.theme.foreground; visible: root.theme.friendly || root.compactSymbols && !root.theme.tui }
    Text {
      anchors.centerIn: parent; visible: root.theme.tui || root.theme.comfortable && !root.compactSymbols
      text: root.theme.comfortable ? (root.deafened ? "Undeafen" : "Deafen") : "[D]"; color: root.deafened ? root.theme.danger : root.theme.foreground
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
      y: root.tooltipAbove ? -height-root.theme.spacing.sm : parent.height + root.theme.spacing.sm
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
