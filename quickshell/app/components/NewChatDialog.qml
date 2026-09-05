import QtQuick
import QtQuick.Controls

Dialog {
  id: root
  required property var bridge
  required property var theme
  signal created(string conversationId)
  property bool group: false
  property var selectedIds: []
  property string requestId: ""
  property string groupRequestId: ""
  property string submittedSignature: ""
  property string selectedServerId: ""
  property string error: ""
  readonly property bool busy: requestId!==""
  readonly property var selectedServerState: bridge.serverStates.filter(function(state) {
    return String(state.server.id)===String(root.selectedServerId)
  })[0] || ({friends:[]})
  readonly property var availableFriends: (selectedServerState.friends || []).filter(function(f) {
    return String(f.id)!==String(root.bridge.selfState.id) && String(f.display_name).toLocaleLowerCase().indexOf(search.text.trim().toLocaleLowerCase())>=0
  }).sort(function(a,b) { return String(a.display_name).localeCompare(String(b.display_name)) })
  readonly property bool canSubmit: selectedIds.length >= (group?2:1) && selectedIds.length <= (group?31:1) && (!group || !!nameField.text.trim())
  function begin() { group=false; selectedIds=[]; requestId=""; groupRequestId=""; submittedSignature=""; error=""; selectedServerId=String(bridge.activeServer.id || ""); nameField.text=""; search.text=""; open() }
  function toggleFriend(id) {
    if (busy) return
    selectedIds=selectedIds.indexOf(id)>=0 ? selectedIds.filter(function(v) { return v!==id }) : group ? selectedIds.concat([id]) : [id]
  }
  function submit() {
    if (busy || !canSubmit) return
    error=""
    var args
    if (group) {
      var signature=JSON.stringify([nameField.text.trim(),selectedIds.slice().sort()])
      if (signature!==submittedSignature) {
        // Uniqueness token for retry safety, not a secret or authorization token.
        groupRequestId="xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g,function(c) { var r=Math.floor(Math.random()*16); return (c==="x"?r:(r&3)|8).toString(16) })
        submittedSignature=signature
      }
      args={server_id:selectedServerId,name:nameField.text.trim(),members:selectedIds,request_id:groupRequestId}
    } else {
      var friend=bridge.friends.filter(function(f) { return String(f.id)===selectedIds[0] })[0]
      if (!friend) { error="This friend is no longer available."; return }
      args={server_id:selectedServerId,friend:String(friend.display_name)}
    }
    requestId=bridge.createChat(group,args)
    if (!requestId) error="Wisp is disconnected. Try again after reconnecting."
  }
  parent: Overlay.overlay
  width: Math.min(theme.space(440), parent ? parent.width-theme.space(24) : theme.space(440))
  height: Math.min(theme.space(520), parent ? parent.height-theme.space(24) : theme.space(520))
  x: parent ? (parent.width-width)/2 : 0
  y: parent ? (parent.height-height)/2 : 0
  modal: true
  title: "New chat"
  closePolicy: busy ? Popup.NoAutoClose : Popup.CloseOnEscape | Popup.CloseOnPressOutside
  font.family: theme.font.family; font.pixelSize: theme.font.caption
  palette.windowText: theme.foreground; palette.text: theme.foreground
  background: Rectangle { color: root.theme.surface; radius: root.theme.cornerRadius; border.width: 1; border.color: root.theme.muted }
  header:Label { text:root.title; padding:root.theme.spacing.lg; color:root.theme.foreground; font.family:root.theme.font.family; font.pixelSize:root.theme.font.title; background:Rectangle { color:root.theme.surface } }
  contentItem: ScrollView {
    id:contentScroll
    contentWidth:availableWidth
    Item {
    width:contentScroll.availableWidth
    implicitHeight:Math.max(contentScroll.availableHeight,root.theme.space(320))
    height:implicitHeight
    Row {
      id: modes; anchors.top: parent.top; width: parent.width; spacing: root.theme.spacing.sm
      ChatButton { theme: root.theme; text:"Direct message"; width:(parent.width-parent.spacing)/2; primary:!root.group; enabled:!root.busy; onClicked:{root.group=false;root.selectedIds=[]} }
      ChatButton { theme: root.theme; text:"Group chat"; width:(parent.width-parent.spacing)/2; primary:root.group; enabled:!root.busy; onClicked:{root.group=true;root.selectedIds=[]} }
    }
    ComboBox {
      id:serverPicker
      anchors.top:modes.bottom; anchors.topMargin:root.theme.spacing.sm; width:parent.width
      height:root.theme.space(34); enabled:!root.busy
      model:root.bridge.servers; textRole:"name"; valueRole:"id"
      currentIndex:{ for(var i=0;i<root.bridge.servers.length;i++) if(String(root.bridge.servers[i].id)===root.selectedServerId)return i; return 0 }
      onActivated:function(index){var server=root.bridge.servers[index];if(server){root.selectedServerId=String(server.id);root.selectedIds=[]}}
      Accessible.name:"Server for new chat"
      font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption
      background:Rectangle { color:root.theme.background; border.width:1; border.color:serverPicker.activeFocus?root.theme.accent:root.theme.separator }
      delegate:ItemDelegate { required property var modelData; width:serverPicker.width; text:String(modelData.name); font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption; ThemeControlStyle { theme:root.theme; control:parent } }
      popup.background:Rectangle { color:root.theme.surface; border.width:1; border.color:root.theme.muted; radius:root.theme.cornerRadius }
    }
    TextField {
      id: nameField; objectName:"newChatName"; property bool wispTextEditor:true
      anchors.top:serverPicker.bottom; anchors.topMargin:root.theme.spacing.sm; width:parent.width
      visible:root.group; height:visible?root.theme.space(34):0; enabled:!root.busy
      maximumLength:60; placeholderText:"Group name"; color:root.theme.foreground; placeholderTextColor:root.theme.muted
      font.family:root.theme.font.family; font.pixelSize:root.theme.font.body
      background:Rectangle { color:root.theme.background; border.width:1; border.color:nameField.activeFocus?root.theme.accent:root.theme.separator }
    }
    TextField {
      id:search; objectName:"newChatFriendSearch"; property bool wispTextEditor:true
      anchors.top:nameField.bottom; anchors.topMargin:root.theme.spacing.sm; width:parent.width; height:root.theme.space(34); enabled:!root.busy
      placeholderText:"Search friends…"; color:root.theme.foreground; placeholderTextColor:root.theme.muted
      font.family:root.theme.font.family; font.pixelSize:root.theme.font.body
      background:Rectangle { color:root.theme.background; border.width:1; border.color:search.activeFocus?root.theme.accent:root.theme.separator }
    }
    ListView {
      anchors.top:search.bottom; anchors.topMargin:root.theme.spacing.sm; anchors.bottom:summary.top; anchors.bottomMargin:root.theme.spacing.sm
      width:parent.width; clip:true; model:root.availableFriends
      ScrollBar.vertical:ScrollBar {}
      delegate:CheckDelegate {
        id:friendChoice; required property var modelData
        width:ListView.view.width; height:root.theme.space(38)
        text:String(modelData.display_name) + (modelData.online ? "" : " · offline")
        checked:root.selectedIds.indexOf(String(modelData.id))>=0; enabled:!root.busy
        onClicked:root.toggleFriend(String(modelData.id))
        font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption
        ThemeControlStyle { theme:root.theme; control:friendChoice }
      }
      Text { anchors.centerIn:parent; visible:root.availableFriends.length===0; text:"No matching friends"; color:root.theme.muted; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption }
    }
    Text {
      id:summary; anchors.bottom:errorText.top; width:parent.width; wrapMode:Text.Wrap
      text:root.group ? root.selectedIds.length+" selected · Choose 2–31 friends. You are included. No call will start." : "Choose one friend. Existing DMs reopen with their history."
      color:root.theme.muted; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption
    }
    Text { id:errorText; anchors.bottom:parent.bottom; width:parent.width; visible:!!root.error; text:root.error; wrapMode:Text.Wrap; color:root.theme.danger; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption }
    }
  }
  footer:Row {
    padding:root.theme.spacing.sm; spacing:root.theme.spacing.sm; layoutDirection:Qt.RightToLeft
    ChatButton { objectName:"newChatSubmit"; theme:root.theme; text:root.busy?"Working…":root.group?"Create group":"Open DM"; primary:true; enabled:root.canSubmit&&!root.busy; onClicked:root.submit() }
    ChatButton { theme:root.theme; text:"Cancel"; enabled:!root.busy; onClicked:root.close() }
  }
  Connections {
    target:root.bridge
    function onChatCreationFinished(id,success,conversationId,error) {
      if (id!==root.requestId) return
      root.requestId=""
      if (success) { root.close(); root.created(conversationId) } else root.error=error
    }
    function onDaemonConnectedChanged() {
      if (root.busy && !root.bridge.daemonConnected) { root.requestId=""; root.error="Connection lost. Retry to finish opening the same chat." }
    }
  }
}
