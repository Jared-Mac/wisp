import QtQuick
import QtQuick.Controls
import "../ChatMarkup.js" as Markup

AnchoredPicker {
  id: root
  required property var bridge
  required property string serverId
  signal picked(string emoji)
  preferAbove: true
  padding: theme.space(10)
  closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
  onOpened: { reposition(); bridge.chatExtras.refresh(serverId,false); search.forceActiveFocus() }
  Connections {target:root.bridge.chatExtras;function onEpochChanged(){if(root.opened)Qt.callLater(function(){root.bridge.chatExtras.refresh(root.serverId,false)})}}
  background: Rectangle { color:root.theme.surface; border.color:root.theme.separator; border.width:1;radius:root.theme.cornerRadius }
  readonly property var options: {
    var items=Markup.wispNames.map(function(name){
      var everyday=Markup.wispEveryday.filter(function(item){return item.name===name})[0]
      var moment=Markup.wispMoments.filter(function(item){return item.name===name})[0]
      return {name:"wisp_"+name,emoji:":wisp_"+name+":",group:everyday?"Wisp Everyday":moment?"Wisp Moments":"Wisp",keywords:everyday?everyday.keywords:moment?moment.keywords:""}
    })
    items=items.concat(Markup.standard.map(function(emoji){return {name:emoji,emoji:emoji,group:"Standard"}}))
    items=items.concat(bridge.chatExtras.library(serverId,"").map(function(e){return {name:e.name,emoji:":e_"+e.id+":",group:e.scope==="server"?"Server":"Account"}}))
    var q=search.text.toLowerCase().trim()
    return items.filter(function(e){return !q || (e.name+" "+e.group+" "+(e.keywords || "")).toLowerCase().indexOf(q)>=0})
  }
  Column {
    anchors.fill:parent;spacing:root.theme.space(8)
    TextField {
      id:search;objectName:"emojiSearch";width:parent.width
      placeholderText:"Find emoji";color:root.theme.foreground
      font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption
      ThemeControlStyle {theme:root.theme;control:search}
      onAccepted: if(root.options.length){root.picked(root.options[0].emoji);root.close()}
    }
    GridView {
      width:parent.width;height:Math.max(0,parent.height-search.height-help.height-parent.spacing*2)
      model:root.options;clip:true;cellWidth:root.theme.space(42);cellHeight:root.theme.space(42)
      ScrollBar.vertical:ScrollBar {}
      delegate:ChatButton {
        id:choice;required property var modelData
        objectName:"emojiChoice-"+modelData.name
        width:root.theme.space(40);height:width;theme:root.theme
        Accessible.name:modelData.group+" · "+modelData.name
        ToolTip.visible:hovered;ToolTip.text:Accessible.name
        Component.onCompleted:root.bridge.chatExtras.load(root.serverId,modelData.emoji)
        onClicked:{root.picked(modelData.emoji);root.close()}
        contentItem: Item {
          Image { objectName:"emojiImage-"+choice.modelData.name;anchors.centerIn:parent;width:root.theme.space(28);height:width;source:root.bridge.chatExtras.url(root.serverId,choice.modelData.emoji);fillMode:Image.PreserveAspectFit;visible:source.toString()!=="" }
          Text {anchors.centerIn:parent;visible:!root.bridge.chatExtras.url(root.serverId,choice.modelData.emoji);text:choice.modelData.emoji[0]===":"?"…":choice.modelData.emoji;font.pixelSize:root.theme.space(23);color:root.theme.foreground}
        }
      }
    }
    Text {id:help;width:parent.width;wrapMode:Text.WordWrap;text:root.bridge.chatExtras.feedback[root.serverId] || "Custom emojis: Settings → Profile / Server";color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
  }
}
