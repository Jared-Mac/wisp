import QtQuick
import QtQuick.Controls

Popup {
  id: root
  objectName: "chatPinsPopup"
  required property var bridge
  required property var theme
  required property string conversationId
  readonly property var state: bridge.messageActions.pinsFor(conversationId)
  parent: Overlay.overlay
  width:Math.min(theme.space(420),parent ? parent.width-theme.space(24) : theme.space(420))
  height:Math.min(theme.space(440),parent ? parent.height-theme.space(24) : theme.space(440))
  x:parent ? (parent.width-width)/2 : 0;y:parent ? Math.max(theme.space(12),(parent.height-height)/3) : 0
  padding:theme.spacing.lg
  closePolicy:Popup.CloseOnEscape | Popup.CloseOnPressOutside
  onOpened:bridge.messageActions.loadPins(conversationId)
  onConversationIdChanged:if(opened) {close()}
  background:Rectangle {color:root.theme.surface;border.width:1;border.color:root.theme.separator;radius:root.theme.cornerRadius}
  Column {
    anchors.fill:parent;spacing:root.theme.spacing.md
    Row {
      width:parent.width;spacing:root.theme.spacing.sm
      WispIcon {theme:root.theme;name:"pin";anchors.verticalCenter:parent.verticalCenter}
      Text {width:parent.width-root.theme.space(64);anchors.verticalCenter:parent.verticalCenter;text:"Pinned messages";elide:Text.ElideRight;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.title}
      ChatButton {theme:root.theme;text:"×";width:root.theme.space(28);height:width;Accessible.name:"Close pins";onClicked:root.close()}
    }
    ListView {
      id: pins;objectName:"pinnedMessages";width:parent.width;height:Math.max(1,parent.height-y-status.height-parent.spacing)
      model:root.state.messages || [];clip:true;spacing:root.theme.spacing.sm;ScrollBar.vertical:ScrollBar {}
      delegate:Rectangle {
        id:pin;required property var modelData;objectName:"pinnedMessage-"+modelData.id
        width:pins.width;height:body.implicitHeight+root.theme.spacing.md*2
        color:root.theme.alpha(root.theme.foreground,0.04);border.width:1;border.color:root.theme.separator;radius:root.theme.cornerRadius
        Column {
          id:body;x:root.theme.spacing.md;y:x;width:parent.width-x*2;spacing:root.theme.spacing.sm
          Text {width:parent.width;textFormat:Text.PlainText;text:pin.modelData.sender.display_name+" · "+Qt.formatDateTime(new Date(pin.modelData.created_at),"MMM d, yyyy");elide:Text.ElideRight;color:root.theme.accent;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
          Text {width:parent.width;textFormat:Text.PlainText;wrapMode:Text.Wrap;maximumLineCount:5;elide:Text.ElideRight;text:root.bridge.messageActions.preview(pin.modelData);color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
          Row {
            width:parent.width;spacing:root.theme.spacing.sm
            ChatButton {objectName:"jumpToPin-"+pin.modelData.id;theme:root.theme;text:"View message";onClicked:{root.bridge.messageActions.locate(root.conversationId,pin.modelData.id);root.close()}}
            ChatButton {objectName:"unpinMessage-"+pin.modelData.id;visible:root.state.can_manage;theme:root.theme;text:"Unpin";onClicked:root.bridge.messageActions.setPin(root.conversationId,pin.modelData.id,false)}
          }
        }
      }
    }
    Text {id:status;width:parent.width;textFormat:Text.PlainText;wrapMode:Text.Wrap;text:root.state.error || (root.state.loading ? "Loading pins…" : pins.count ? "" : "No pinned messages yet");color:root.state.error ? root.theme.danger : root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
  }
}
