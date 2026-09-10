import QtQuick
import QtQuick.Controls

Dialog {
  id: root
  objectName: "forwardMessageDialog"
  required property var bridge
  required property var theme
  property var sourceMessage: null
  property var destination: null
  property string requestId: ""
  property string error: ""
  readonly property bool busy: requestId !== ""
  readonly property var destinations: {
    var result=[], conversations=bridge.conversations, query=search.text.trim().toLocaleLowerCase()
    bridge.serverStates.forEach(function(state) {
      if (state.server.connected === false) return
      var serverId=String(state.server.id), existing={}
      conversations.filter(function(c) { return String(c.server_id)===serverId && !c.pending_access }).forEach(function(c) {
        if (c.kind==="direct") (c.members || []).forEach(function(m) { existing[String(m.id)]=true })
        result.push({key:String(c.id),label:String(c.label || "Chat"),server:String(state.server.name || "Server"),args:root.bridge.withConversationScope(c.id)})
      })
      ;(state.friends || []).forEach(function(f) {
        if (String(f.id)===String(state.self.id) || existing[String(f.id)]) return
        result.push({key:serverId+"/friend/"+f.id,label:String(f.display_name),server:String(state.server.name || "Server"),args:{server_id:serverId,friend:String(f.id)}})
      })
    })
    return result.filter(function(d) {return !query || (d.label+" "+d.server).toLocaleLowerCase().indexOf(query)>=0})
      .sort(function(a,b) {return a.label.localeCompare(b.label) || a.server.localeCompare(b.server)})
  }
  function showMessage(message) { sourceMessage=message; destination=null; error=""; requestId=""; search.text=""; open() }
  function submit() {
    if (busy || !destination || !sourceMessage) return
    if (!destinations.some(function(d) {return d.key===root.destination.key})) {error="This chat is no longer available.";return}
    error=""; requestId=bridge.messageActions.forward(sourceMessage,destination.args)
    if (!requestId) error="Wisp is disconnected. Try again after reconnecting."
  }
  parent: Overlay.overlay
  width: Math.min(theme.space(460),parent ? parent.width-theme.space(24) : theme.space(460))
  height: Math.min(theme.space(540),parent ? parent.height-theme.space(24) : theme.space(540))
  x: parent ? (parent.width-width)/2 : 0; y: parent ? (parent.height-height)/2 : 0
  modal: true
  closePolicy: busy ? Popup.NoAutoClose : Popup.CloseOnEscape | Popup.CloseOnPressOutside
  onOpened: search.forceActiveFocus()
  background: Rectangle {color:root.theme.surface;radius:root.theme.cornerRadius;border.width:1;border.color:root.theme.separator}
  header: Label {text:"Forward message";padding:root.theme.spacing.lg;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.title}
  contentItem: Column {
    spacing: root.theme.spacing.md
    Rectangle {
      width: parent.width; height: quote.implicitHeight+root.theme.spacing.md*2
      color: root.theme.alpha(root.theme.foreground,0.04); radius: root.theme.cornerRadius
      Column {
        id: quote; anchors.left:parent.left;anchors.right:parent.right;anchors.margins:root.theme.spacing.md;y:root.theme.spacing.md
        Text {width:parent.width;textFormat:Text.PlainText;elide:Text.ElideRight;text:String(((root.sourceMessage || {}).sender || {}).display_name || "");color:root.theme.accent;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
        Text {width:parent.width;textFormat:Text.PlainText;wrapMode:Text.Wrap;maximumLineCount:3;elide:Text.ElideRight;text:root.bridge.messageActions.preview(root.sourceMessage);color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
      }
    }
    TextField {id:search;objectName:"forwardDestinationSearch";width:parent.width;enabled:!root.busy;placeholderText:"Find a chat or friend";ThemeControlStyle {theme:root.theme;control:search}}
    ListView {
      id: choices; objectName:"forwardDestinations"; width:parent.width
      height:Math.max(root.theme.space(40),parent.height-quote.parent.height-search.height-status.height-parent.spacing*3)
      model:root.destinations;clip:true;spacing:root.theme.spacing.xs;ScrollBar.vertical:ScrollBar {}
      delegate: ItemDelegate {
        id: choice;required property var modelData;objectName:"forwardDestination-"+modelData.key
        width:choices.width;height:root.theme.space(52);enabled:!root.busy
        onClicked:root.destination=modelData
        Accessible.name:modelData.label+" · "+modelData.server
        background:Rectangle {radius:root.theme.cornerRadius;color:root.theme.alpha(root.theme.accent,root.destination && root.destination.key===choice.modelData.key ? 0.18 : choice.hovered ? 0.08 : 0)}
        contentItem:Column {
          Text {width:parent.width;textFormat:Text.PlainText;text:choice.modelData.label;elide:Text.ElideRight;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
          Text {width:parent.width;textFormat:Text.PlainText;text:choice.modelData.server;elide:Text.ElideRight;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
        }
      }
      Text {anchors.centerIn:parent;visible:choices.count===0;text:"No matching chats";color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
    }
    Text {id:status;width:parent.width;textFormat:Text.PlainText;wrapMode:Text.Wrap;text:root.error || (root.busy ? "Forwarding…" : root.destination ? "To: "+root.destination.label : "Choose a destination");color:root.error ? root.theme.danger : root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
  }
  footer: DialogButtonBox {
    background:Rectangle {color:root.theme.surface}
    ChatButton {theme:root.theme;text:"Cancel";enabled:!root.busy;onClicked:root.close()}
    ChatButton {objectName:"confirmForward";theme:root.theme;text:"Forward";primary:true;enabled:!!root.destination && !root.busy;onClicked:root.submit()}
  }
  Connections {
    target:root.bridge.messageActions
    function onForwardFinished(id,ok,error) {if(id!==root.requestId)return;root.requestId="";if(ok)root.close();else root.error=error || "Forwarding failed. Try again."}
  }
}
