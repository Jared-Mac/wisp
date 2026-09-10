import QtQuick
import QtQuick.Controls

Popup {
  id: root
  objectName: "soundboardPopup"
  required property var bridge
  required property var theme
  readonly property string serverId: bridge.currentVoiceRoom ? bridge.voiceServerId : String((bridge.activeServer || {}).id || bridge.voiceServerId)
  readonly property string serverName: String((bridge.participantServer({server_id:serverId}).server || {}).name || "This server")
  property bool managing: false
  onOpened: managing = false
  onServerIdChanged: close()
  parent: Overlay.overlay
  width: Math.min(theme.space(root.managing ? 560 : 420), parent ? parent.width-theme.spacing.lg*2 : theme.space(420))
  height: Math.min(root.managing ? theme.space(700) : toolbar.height + theme.spacing.sm + library.implicitHeight + padding*2, parent ? parent.height-theme.spacing.lg*2 : theme.space(700))
  x: parent ? (parent.width-width)/2 : 0
  y: parent ? (parent.height-height)/2 : 0
  modal: true; padding: theme.spacing.lg
  closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
  readonly property string callKey: bridge.currentVoiceRoom ? bridge.voiceServerId + ":" + bridge.currentVoiceRoom.id : ""
  onCallKeyChanged: close()
  background: Rectangle {color:root.theme.surface;radius:root.theme.cornerRadius;border.width:1;border.color:root.theme.separator}
  contentItem: Column {
    spacing: root.theme.spacing.sm
    Row {
      id: toolbar
      width: parent.width; spacing: root.theme.spacing.sm
      Text {
        width: Math.max(0, parent.width-manage.width-closeButton.width-parent.spacing*2)
        anchors.verticalCenter: parent.verticalCenter; elide: Text.ElideRight
        text: "Soundboard · " + root.serverName; color: root.theme.foreground
        font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true
      }
      ChatButton {id:manage;objectName:"soundboardManageButton";theme:root.theme;text:root.managing ? "Back" : "Manage";iconName:root.managing ? "back" : "settings";onClicked:root.managing=!root.managing}
      ChatButton {id:closeButton;theme:root.theme;text:"Close";iconName:"close";iconOnly:true;forceIcon:true;Accessible.name:"Close soundboard";onClicked:root.close()}
    }
    Flickable {
      width: parent.width; height: parent.height-y; contentHeight: library.implicitHeight; clip: true
      ScrollBar.vertical: ScrollBar {}
      Loader {
        id: library; width: parent.width; active: root.visible
        sourceComponent: SoundboardView {
          bridge:root.bridge;theme:root.theme;serverId:root.serverId;playOnly:!root.managing
          maximumListHeight: Math.max(root.theme.space(72), Math.min(root.theme.space(288), (root.parent ? root.parent.height : 700)-root.theme.space(240)))
        }
      }
    }
  }
}
