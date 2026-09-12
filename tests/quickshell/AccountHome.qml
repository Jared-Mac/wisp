import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp
import "app/views" as Views
import "app/components" as Components

ShellRoot {
  id:test
  property bool failed:false
  function check(ok,label) {if(!ok){failed=true;console.error("ACCOUNT_HOME_FAILED: "+label)}}
  function find(item,name,seen) {
    if(!item)return null;seen=seen || [];if(seen.indexOf(item)>=0)return null;seen.push(item)
    if(item.objectName===name)return item
    if(item.contentItem){var found=find(item.contentItem,name,seen);if(found)return found}
    for(var child of item.data || item.children || []){var found=find(child,name,seen);if(found)return found}
    return null
  }
  Wisp.WispTheme {id:theme;profile:"soft_graphite"}
  Wisp.WispBridge {id:bridge;property var sent:[];function send(name,args){var id="test-"+(++requestId);sent.push({id:id,name:name,args:args});return id}}
  Binding {target:bridge.friendships;property:"transportReady";value:true}
  FloatingWindow {
    id:window;visible:true;implicitWidth:720;implicitHeight:700;color:theme.background
    Row {
      anchors.fill:parent;anchors.margins:16;spacing:24
      Column {
        width:230;spacing:8
        Components.ServerSelector {id:selector;width:parent.width;bridge:bridge;theme:theme}
        Views.TrayRoomsView {id:rooms;width:parent.width;bridge:bridge;theme:theme}
        Views.ServerMembersView {id:members;width:parent.width;bridge:bridge;theme:theme}
        Views.FriendsView {id:friends;width:parent.width;bridge:bridge;theme:theme;collapsible:true}
      }
      ScrollView {width:400;height:parent.height;clip:true;Views.ProfileSettingsView {id:profile;width:400;bridge:bridge;theme:theme}}
    }
  }
  TestCase {id:input;parent:window.contentItem;when:false}
  Component.onCompleted:{
    var data=JSON.parse(JSON.stringify(bridge.snapshot));data.self.id="me";data.self.display_name="Example";data.self.server_owner=false;data.self.server_admin=false;data.server_member=false;data.servers=[];data.server_states=[{server:{id:"account",name:"Home",connected:true},server_member:false,self:data.self,friends:[],conversations:[],messages:[],spots:[],hangouts:[],knocks:[]}];bridge.applySnapshot(data)
  }
  Timer {interval:500;running:true;onTriggered:{
    test.check(bridge.servers.length===0,"account context is not a joined server")
    test.check(bridge.activeServer.id==="account" && bridge.selfState.id==="me","account context remains available for friends, DMs and settings")
    test.check(!rooms.visible && !members.visible,"no community rooms or directory without membership")
    test.check(!bridge.canManageServer && !bridge.serverMember,"no community permissions")
    test.check(test.find(selector,"serverInviteFriend").text==="Join a server","empty selector offers joining")
    bridge.friendships.put("account",{ready:true,loading:false,action:"",people:[{id:"incoming",display_name:"New friend",relationship:"incoming",server_id:"account"}]})
    var add=test.find(friends,"openAddFriend");test.check(add.pending===1 && add.primary,"friend requests remain discoverable without server")
    add.clicked();input.wait(80)
    var dialog=test.find(friends,"friendHandle");test.check(!!dialog,"add friend search is reachable")
    // Fulfil overview before another serialized account operation.
    for(var id of Object.keys(bridge.requests)){if(bridge.requests[id].kind==="membership")bridge.finishRequest({id:id,ok:true,value:{handle:null,blocked:[],server_member:false}})}
    dialog.text="@example";test.find(friends,"findFriend").clicked();input.wait(20)
    var last=bridge.sent[bridge.sent.length-1]
    test.check(last.name==="lookup_person" && last.args.server_id==="account","lookup stays on account service")
    bridge.finishRequest({id:last.id,ok:true,value:{person:{id:"target",display_name:"Example friend",handle:"example"}}});input.wait(30)
    var request=test.find(friends,"addFriend");request.clicked();input.wait(20)
    last=bridge.sent[bridge.sent.length-1];test.check(last.name==="send_friend_request" && last.args.user_id==="target","search requires explicit friend request")
    var data=JSON.parse(JSON.stringify(bridge.snapshot));data.server_states[0].server_member=true;data.server_states[0].server.name="Community";data.servers=[data.server_states[0].server];bridge.applySnapshot(data);input.wait(30)
    test.check(bridge.servers.length===1 && bridge.serverMember && members.visible,"accepting invite reveals community without restart")
    console.log(test.failed ? "ACCOUNT_HOME_FAILED" : "ACCOUNT_HOME_OK");Qt.quit()
  }}
}
