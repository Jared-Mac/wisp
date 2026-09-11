import QtQuick
import QtQuick.Controls

Item {
  id: root
  required property var bridge
  required property var theme
  property bool adaptive: false
  readonly property bool narrow: adaptive && width < theme.space(140)
  readonly property bool tiny: adaptive && width < theme.space(80)
  property bool collapsible: false
  property bool collapsed: false
  property bool showSoundboard: false
  signal toggled()
  signal createRequested()
  implicitHeight: theme.space(showSoundboard && tiny ? 62 : 30)
  Button {
    id: toggle; objectName: "rooms-collapse"
    anchors.left: parent.left; anchors.right: root.showSoundboard ? soundboard.left : create.left; anchors.rightMargin: root.theme.spacing.xs
    visible: !root.tiny
    height: parent.height; enabled: root.collapsible
    Accessible.name: root.collapsed ? "Expand rooms" : "Collapse rooms"
    onClicked: root.toggled()
    background: Rectangle {
      color: toggle.hovered ? root.theme.alpha(root.theme.foreground, 0.06) : "transparent"
      border.width: toggle.visualFocus ? 1 : 0; border.color: root.theme.focusBorder
    }
    contentItem: Text {
      objectName: "roomsSectionHeader"; verticalAlignment: Text.AlignVCenter; elide: Text.ElideRight
      text: root.theme.friendly ? "Rooms" : (root.collapsible ? (root.collapsed ? "▸ " : "▾ ") : "") + (root.theme.comfortable ? "Rooms · " : root.theme.tui ? "/rooms · " : "ROOMS · ") + root.bridge.roomCount
      color: root.theme.roomSectionColor; font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption; font.bold: true
    }
  }
  Loader {
    id: soundboard
    active: root.showSoundboard; visible: active
    x: root.tiny ? (parent.width-width)/2 : create.x-width-root.theme.spacing.xs
    y: root.tiny ? 0 : (parent.height-height)/2
    width: Math.min(root.width,root.theme.space(28)); height: root.theme.space(28)
    sourceComponent: ChatButton {
      id: soundboardButton
      objectName: "serverSoundboardButton"
      theme: root.theme; text: "Soundboard"; iconName: "soundboard"; iconOnly: true; forceIcon: true
      Accessible.name: "Open soundboard"
      ToolTip.visible: hovered || visualFocus; ToolTip.text: "Soundboard"
      HoverHandler { id: pointer }
      onClicked: menu.openAt(soundboardButton, pointer.hovered ? pointer.point.position : Qt.point(width/2,height))
      SoundboardPopup { id: menu; bridge: root.bridge; theme: root.theme; hostItem: soundboardButton }
    }
  }
  ChatButton {
    id: create; objectName: "createRoomButton"
    x: root.tiny ? (parent.width-width)/2 : parent.width-width
    y: root.showSoundboard && root.tiny ? root.theme.space(34) : (parent.height-height)/2
    theme: root.theme; text: "+"; iconName: "add"; iconOnly: root.theme.friendly || root.tiny; forceIcon: root.tiny; implicitWidth: Math.min(root.width,root.theme.space(30))
    enabled: root.bridge.activeServer.connected !== false
    Accessible.name: "Create a room"; ToolTip.visible: hovered; ToolTip.text: "Create a room"
    onClicked: root.createRequested()
  }
}
