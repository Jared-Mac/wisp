import QtQuick
import QtQuick.Controls
import "../AvatarPresets.js" as Presets

Dialog {
  id: root
  objectName: "avatarPresetPicker"
  required property var theme
  property string selectedPath: ""
  property bool artworkRequested: false
  onAboutToShow: artworkRequested = true
  signal chosen(string path, string name)
  parent: Overlay.overlay
  anchors.centerIn: parent
  modal: true
  title: "Choose a Wisp avatar"
  width: Math.min(theme.space(570), parent ? parent.width-theme.space(24) : theme.space(570))
  height: Math.min(implicitHeight, parent ? parent.height-theme.space(24) : theme.space(580))
  padding: theme.space(16)
  closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
  ThemeControlStyle { theme: root.theme; control: root; outline: true }
  background: Rectangle { color: root.theme.surface; radius: root.theme.cornerRadius; border.width:1; border.color:root.theme.separator }
  header: Label {
    text: root.title; padding: root.theme.space(16)
    color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.title
  }
  footer: DialogButtonBox {
    background: Rectangle { color: root.theme.surface }
    ChatButton { theme: root.theme; text: "Cancel"; onClicked: root.close() }
  }
  contentItem: Flickable {
    implicitHeight: gallery.implicitHeight
    clip: true; contentWidth: width; contentHeight: gallery.implicitHeight
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar {}
    Grid {
      id: gallery
      objectName: "avatarPresetGrid"
      width: parent.width
      columns: Math.max(2, Math.min(5, Math.floor(width/root.theme.space(96))))
      spacing: root.theme.space(8)
      Repeater {
        model: Presets.choices
        delegate: Button {
          id: choice
          required property var modelData
          objectName: "avatarPreset-" + modelData.id
          readonly property url imagePath: Qt.resolvedUrl("../assets/avatars/"+modelData.id+".png")
          width: (gallery.width-gallery.spacing*(gallery.columns-1))/gallery.columns
          height: width + root.theme.space(24)
          padding: root.theme.space(5)
          enabled: artwork.status===Image.Ready
          Accessible.name: "Use " + modelData.name + " avatar, " + modelData.description
          onClicked: { root.chosen(imagePath.toString(),modelData.name); root.close() }
          background: Rectangle {
            color: choice.hovered ? root.theme.alpha(root.theme.accent,0.12) : "transparent"
            radius: root.theme.cornerRadius
            border.width: choice.visualFocus || root.selectedPath===choice.imagePath.toString() ? 2 : 1
            border.color: choice.visualFocus || root.selectedPath===choice.imagePath.toString() ? root.theme.accent : root.theme.separator
          }
          contentItem: Column {
            spacing: root.theme.space(5)
            Image {
              id: artwork; objectName:"avatarArtwork-"+choice.modelData.id
              width: parent.width; height: width
              source: root.artworkRequested ? choice.imagePath : ""; sourceSize.width:256; sourceSize.height:256
              asynchronous:true; fillMode:Image.PreserveAspectFit
            }
            Text {
              width: parent.width; horizontalAlignment: Text.AlignHCenter
              text: choice.modelData.name; textFormat:Text.PlainText; elide:Text.ElideRight
              color: root.theme.foreground; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption
            }
          }
        }
      }
    }
  }
}
