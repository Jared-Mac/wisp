import QtQuick
import QtQuick.Controls

Flow {
  id:root
  required property var bridge
  required property var theme
  required property string serverId
  required property string messageId
  property Item actionHost: null
  property bool revealActions: true
  property bool allowed: true
  readonly property bool actionEngaged: reactionAction.activeFocus || picker.opened
  visible: allowed
  property var reactions:allowed ? bridge.chatExtras.groups(serverId,messageId) : []
  spacing:theme.space(5)
  Repeater {
    model:root.reactions
    ChatButton {
      id:reaction;required property var modelData;theme:root.theme
      objectName:"reaction-"+root.messageId+"-"+modelData.emoji
      primary:!!modelData.own
      implicitWidth:root.theme.space(50);implicitHeight:root.theme.space(30)
      enabled:!root.bridge.chatExtras.pendingReactions[root.bridge.chatExtras.key(root.serverId,root.messageId)]
      Accessible.name:(modelData.own?"Remove ":"Add ")+modelData.emoji+" reaction · "+modelData.users.length
      ToolTip.visible:hovered;ToolTip.text:modelData.users.map(function(u){return u.display_name}).join(", ")
      Component.onCompleted:root.bridge.chatExtras.load(root.serverId,modelData.emoji)
      onClicked:root.bridge.chatExtras.react(root.serverId,root.messageId,modelData.emoji)
      contentItem:Row {
        spacing:root.theme.space(4)
        Image {width:root.theme.space(22);height:width;source:root.bridge.chatExtras.url(root.serverId,reaction.modelData.emoji);visible:source.toString()!=="";fillMode:Image.PreserveAspectFit}
        Text {visible:!root.bridge.chatExtras.url(root.serverId,reaction.modelData.emoji);text:reaction.modelData.emoji[0]===":"?"◇":reaction.modelData.emoji;color:root.theme.foreground;font.pixelSize:root.theme.space(18)}
        Text {anchors.verticalCenter:parent.verticalCenter;text:reaction.modelData.users.length;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
      }
    }
  }
  ChatButton {
    id: reactionAction
    parent: root.actionHost || root
    visible:root.allowed
    quiet: true
    opacity: root.revealActions || activeFocus || picker.opened ? 1 : 0
    objectName:"addReaction-"+root.messageId
    theme:root.theme;text:"+☺";iconName:"emoji";iconOnly:root.theme.friendly;implicitWidth:root.theme.space(root.actionHost ? 26 : 42);implicitHeight:root.theme.space(root.actionHost ? 20 : 28)
    Accessible.name:"Add reaction"
    onClicked:picker.showAt(reactionAction)
    EmojiPicker {id:picker;objectName:"reactionPicker-"+root.messageId;bridge:root.bridge;theme:root.theme;serverId:root.serverId;onPicked:emoji=>root.bridge.chatExtras.react(root.serverId,root.messageId,emoji)}
  }
}
