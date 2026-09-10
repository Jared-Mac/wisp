import QtQuick
import QtQuick.Controls

Popup {
  id: root
  objectName: "soundboardPopup"
  required property var bridge
  required property var theme
  readonly property string serverId: bridge.currentVoiceRoom ? bridge.voiceServerId : String((bridge.activeServer || {}).id || bridge.voiceServerId)
  property bool managing: false
  onOpened: managing = false
  onServerIdChanged: close()
  parent: Overlay.overlay
  width: Math.min(theme.space(560), parent ? parent.width-theme.spacing.lg*2 : theme.space(560))
  height: Math.min(theme.space(root.managing ? 700 : 560), parent ? parent.height-theme.spacing.lg*2 : theme.space(root.managing ? 700 : 560))
  x: parent ? (parent.width-width)/2 : 0
  y: parent ? (parent.height-height)/2 : 0
  modal: true; padding: theme.spacing.lg
  closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
  readonly property string callKey: bridge.currentVoiceRoom ? bridge.voiceServerId + ":" + bridge.currentVoiceRoom.id : ""
  onCallKeyChanged: close()
  background: Rectangle {color:root.theme.surface;radius:root.theme.cornerRadius;border.width:1;border.color:root.theme.separator}
  contentItem: Column {
    spacing: root.theme.spacing.sm
    Flow {
      width: parent.width; spacing: root.theme.spacing.sm
      ChatButton {objectName:"soundboardManageButton";theme:root.theme;text:root.managing ? "Back to sounds" : "Manage sounds";onClicked:root.managing=!root.managing}
      ChatButton {theme:root.theme;text:"Close";onClicked:root.close()}
    }
    Flickable {
      width: parent.width; height: parent.height-y; contentHeight: library.implicitHeight; clip: true
      ScrollBar.vertical: ScrollBar {}
      Loader {
        id: library; width: parent.width; active: root.visible
        sourceComponent: SoundboardView {bridge:root.bridge;theme:root.theme;serverId:root.serverId;playOnly:!root.managing}
      }
    }
  }
}
