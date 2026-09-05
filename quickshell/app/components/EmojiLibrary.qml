import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs

Column {
  id: root
  required property var bridge
  required property var theme
  required property string serverId
  required property string scope
  property string selectedPath: ""
  property string filter: ""
  spacing: theme.space(8)
  onVisibleChanged:if(visible)bridge.chatExtras.refresh(serverId,false)
  onServerIdChanged:if(visible)bridge.chatExtras.refresh(serverId,false)
  Component.onCompleted:if(visible)bridge.chatExtras.refresh(serverId,false)
  Connections {target:root.bridge.chatExtras;function onEpochChanged(){if(root.visible)Qt.callLater(function(){root.bridge.chatExtras.refresh(root.serverId,false)})}}
  Text {text:root.scope==="server"?"Server emojis":"My emojis";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body;font.bold:true}
  Text {width:parent.width;wrapMode:Text.WordWrap;text:root.scope==="server"?"Available to everyone on this server.":"Your emoji library for this account.";color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
  Flow {
    width:parent.width;spacing:root.theme.space(6)
    TextField {
      id:name;objectName:"newEmojiName-"+root.scope
      width:Math.min(root.width,root.theme.space(180));maximumLength:32;placeholderText:"emoji_name"
      color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption
      ThemeControlStyle {theme:root.theme;control:name}
    }
    ChatButton {theme:root.theme;text:root.selectedPath?"change image":"choose image";onClicked:filePicker.open()}
    ChatButton {objectName:"uploadEmoji-"+root.scope;theme:root.theme;text:"add";enabled:!!root.selectedPath && /^[a-zA-Z0-9_]{2,32}$/.test(name.text);onClicked:{root.bridge.chatExtras.upload(root.serverId,root.scope,name.text,root.selectedPath);root.selectedPath="";name.text=""}}
  }
  Text {width:parent.width;wrapMode:Text.WordWrap;text:"No emoji count limit. Images up to 2 MB; stored as static emojis.";color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
  Text {width:parent.width;wrapMode:Text.WordWrap;visible:text!=="";text:root.bridge.chatExtras.feedback[root.serverId] || "";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
  TextField {id:search;width:parent.width;placeholderText:"Search this library";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption;ThemeControlStyle {theme:root.theme;control:search}}
  ListView {
    width:parent.width;height:Math.min(contentHeight,root.theme.space(260));clip:true
    model:root.bridge.chatExtras.library(root.serverId,root.scope).filter(function(e){return e.name.indexOf(search.text.toLowerCase().trim())>=0})
    ScrollBar.vertical:ScrollBar {}
    delegate:Row {
      id:entry;required property var modelData
      width:ListView.view.width;height:root.theme.space(40);spacing:root.theme.space(8)
      Component.onCompleted:root.bridge.chatExtras.load(root.serverId,":e_"+modelData.id+":")
      Image {anchors.verticalCenter:parent.verticalCenter;width:root.theme.space(28);height:width;source:root.bridge.chatExtras.url(root.serverId,":e_"+entry.modelData.id+":");fillMode:Image.PreserveAspectFit}
      Text {anchors.verticalCenter:parent.verticalCenter;width:Math.max(0,parent.width-root.theme.space(130));text:entry.modelData.name;elide:Text.ElideRight;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
      ChatButton {theme:root.theme;text:"remove";onClicked:root.bridge.chatExtras.remove(root.serverId,entry.modelData.id)}
    }
  }
  FileDialog {id:filePicker;title:"Choose an emoji";nameFilters:["Images (*.png *.jpg *.jpeg *.webp *.gif)"];onAccepted:root.selectedPath=selectedFile.toString()}
}
