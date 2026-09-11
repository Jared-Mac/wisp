import QtQuick
import QtQuick.Controls

ChatButton {
  id: root
  required property var bridge
  readonly property var pending: bridge.unreadConversations
  objectName: "chatInbox"
  text: "Inbox" + (pending.length ? " · " + pending.length : "")
  iconName: "inbox"; iconOnly: width < theme.space(90); forceIcon: iconOnly
  primary: pending.length > 0; quiet: !primary
  height: theme.space(32)
  Accessible.name: pending.length ? "Inbox · " + pending.length + " pending chats" : "Inbox · All caught up"
  ToolTip.visible: hovered || visualFocus; ToolTip.text: Accessible.name
  onClicked: inbox.open()
  Popup {
    id: inbox; objectName: "chatInboxPopup"
    parent: root.Overlay.overlay || root
    anchors.centerIn: parent
    width: Math.min(root.theme.space(400), parent.width-root.theme.space(24))
    height: Math.min(parent.height-root.theme.space(24),root.theme.space(64)+entries.contentHeight)
    padding: root.theme.space(12)
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    background: Rectangle {color:root.theme.surface;radius:root.theme.cornerRadius;border.width:1;border.color:root.theme.separator}
    contentItem: Column {
      spacing:root.theme.spacing.sm
      Text {
        text:root.pending.length ? "Pending chats" : "All caught up"
        color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body;font.bold:true
      }
      ListView {
        id:entries;width:parent.width;height:Math.min(contentHeight,inbox.availableHeight-root.theme.space(28))
        clip:true;model:root.pending;spacing:root.theme.spacing.xs
        ScrollBar.vertical:ScrollBar {}
        delegate:ChatButton {
          required property var modelData
          objectName:"inboxChat-"+modelData.id
          theme:root.theme;width:entries.width;height:root.theme.space(38)
          iconName:modelData.kind==="direct" ? "chat" : "hash"
          text:String(modelData.label || "Chat")+" · "+root.bridge.pendingCount(modelData.id)
          formatLabel:false;textAlignment:Text.AlignLeft
          onClicked:{root.bridge.openPendingChat(modelData.id);inbox.close()}
        }
      }
    }
  }
}
