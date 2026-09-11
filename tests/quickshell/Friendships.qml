import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components
import "app/views" as Views

ShellRoot {
  id:test
  property bool failed:false
  function check(ok,label){if(!ok){failed=true;console.error("FRIENDSHIPS_FAILED: "+label)}}
  function find(item,name,seen){
    if(!item)return null;seen=seen || [];if(seen.indexOf(item)>=0)return null;seen.push(item)
    if(item.objectName===name)return item
    if(item.contentItem){var r=find(item.contentItem,name,seen);if(r)return r}
    if(item.footer){var r=find(item.footer,name,seen);if(r)return r}
    var children=item.data || item.contentData || item.children || [];for(var i=0;i<children.length;i++){var r=find(children[i],name,seen);if(r)return r}return null
  }
  property var catalog:[{id:"self",display_name:"Morgan",relationship:"self"},{id:"river",display_name:"River",relationship:"none"},{id:"sage",display_name:"Sage",relationship:"incoming"},{id:"friend",display_name:"Alex",relationship:"friend"}]
  function ack(id,ok,people){bridge.finishRequest({id:id,ok:ok,value:{people:people || catalog},error:ok ? null : {message:"Temporary failure. Try again."}})}
  function ackLists(){Object.keys(bridge.requests).forEach(function(id){var r=bridge.requests[id];if(r.kind==="friendship" && r.action==="list")test.ack(id,true,r.server_id==="other" ? [{id:"otherSelf",display_name:"Me",relationship:"self"},{id:"river",display_name:"River elsewhere",relationship:"none"}] : test.catalog)})}
  function last(){return bridge.sent[bridge.sent.length-1]}
  Wisp.WispTheme {id:theme;profile:Quickshell.env("WISP_TEST_THEME") || "soft_graphite"}
  Wisp.WispBridge {id:bridge;property var sent:[];function send(name,args){var id="fake-"+(++requestId);sent.push({id:id,name:name,args:args});return id}}
  Component {id:savedPreferences;Wisp.WispFriendPreferences {account:"self"}}
  FloatingWindow {
    id:window;visible:true;implicitWidth:760;implicitHeight:700;color:theme.background
    Row {id:scene;anchors.fill:parent;anchors.margins:16;spacing:16
      Column {id:leftSidebar;width:230;spacing:8
        Views.ServerMembersView {id:serverPeople;width:parent.width;bridge:bridge;theme:theme}
        Views.FriendsView {id:friends;width:parent.width;bridge:bridge;theme:theme;collapsible:true;adaptive:true}
      }
      Column {width:parent.width-leftSidebar.width-parent.spacing;spacing:16
        Views.ServerMembersView {id:trayPeople;width:parent.width;bridge:bridge;theme:theme}
        Views.FriendsView {id:tray;width:parent.width;bridge:bridge;theme:theme;collapsible:true}
        Components.MessageFeed {id:feed;width:parent.width;height:220;bridge:bridge;theme:theme;conversationId:"local::chat"}
      }
    }
  }
  TestCase {id:input;parent:window.contentItem;when:false}
  Component.onCompleted:{
    var data=JSON.parse(JSON.stringify(bridge.snapshot)),self={id:"self",display_name:"Morgan",hangout_id:null,media:{},presence:"open",server_admin:false}
    var server={id:"local",name:"Community",connected:true},other={id:"other",name:"Other community",connected:true};data.servers=[server,other];data.selected_server_id="local";data.self=Object.assign(data.self,self)
    data.server_states=[{server:server,self:data.self,messages:[{id:"message",sender:{id:"river",display_name:"River"},conversation_id:"chat",created_at:"2026-09-10T10:00:00Z",content_type:"text/plain",payload:"See you in the lounge!",encryption_version:1}],conversations:[{id:"chat",kind:"hangout",spot_id:"room",label:"Lounge",members:[self]}],friends:[{id:"friend",display_name:"Alex",online:true,presence:"open"}],hangouts:[],spots:[],knocks:[],devices:[],room_invitations:[]},
      {server:other,self:Object.assign({},data.self,{id:"otherSelf"}),messages:[],conversations:[],friends:[],hangouts:[],spots:[],knocks:[],devices:[],room_invitations:[]}]
    bridge.applySnapshot(data)
  }
  Timer {running:true;interval:500;onTriggered:{
    test.ackLists();input.wait(40)
    test.check(bridge.friendships.state("local").people.length===4,"directory includes nonfriends and self")
    var before=bridge.sent.length;bridge.friendships.sync("presence_changed");test.check(bridge.sent.length===before,"ordinary presence updates do not refetch directory")
    test.check(test.find(serverPeople,"sidebarServerMembers").visible && test.find(trayPeople,"sidebarServerMembers").visible,"People lists are visible in server sections")
    test.find(serverPeople,"browseMembers").clicked();input.wait(30);test.ackLists();input.wait(20)
    var dialog=test.find(serverPeople,"serverPeopleDialog"),list=test.find(dialog,"serverPeopleList"),search=test.find(dialog,"memberSearch")
    test.check(dialog.opened && list.count===4,"server directory accessible from People")
    search.text="riv";input.wait(30);test.check(list.count===1,"search finds nonfriend")
    var add=test.find(dialog,"serverMemberAction-river");add.clicked()
    test.check(test.last().name==="send_friend_request" && test.last().args.server_id==="local" && test.last().args.user_id==="river","quick add uses member ID and server scope")
    test.ack(test.last().id,false);input.wait(20);test.check(!!bridge.friendships.state("local").error && add.enabled,"failed request permits retry")
    add.clicked();test.catalog=test.catalog.map(function(p){return Object.assign({},p,p.id==="river" ? {relationship:"outgoing"} : {})});test.ack(test.last().id,true);input.wait(20)
    test.check(bridge.friendships.state("local").feedback==="Friend request sent" && add.text==="Request sent","sent confirmation replaces Add friend")
    add.clicked();input.wait(20);test.ackLists();input.wait(20)
    var memberMenu=test.find(test.find(dialog,"serverMember-river").parent,"participantMenu")
    test.check(memberMenu.opened && !test.find(memberMenu,"participantMenuVolume").visible,"member menu has no voice-only controls")
    test.find(memberMenu,"dismissFriendRequest").clicked();test.catalog=test.catalog.map(function(p){return Object.assign({},p,p.id==="river" ? {relationship:"none"} : {})});test.ack(test.last().id,true);memberMenu.close()
    search.text="";input.wait(20)
    var only=test.find(dialog,"friendRequestsOnly");only.checked=true;input.wait(20);test.check(list.count===1,"incoming requests remain easy to find")
    test.find(dialog,"serverMemberAction-sage").clicked();input.wait(20);test.ackLists();input.wait(20)
    var incomingMenu=test.find(test.find(dialog,"serverMember-sage").parent,"participantMenu");test.find(incomingMenu,"addFriend").clicked()
    test.check(test.last().name==="accept_friend_request","incoming request requires acceptance")
    test.catalog=test.catalog.map(function(p){return Object.assign({},p,p.id==="sage" ? {relationship:"friend"} : {})});test.ack(test.last().id,true);incomingMenu.close();only.checked=false
    dialog.close();input.wait(30)
    bridge.friendPreferences.toggleCollapsed();input.wait(20)
    test.check(test.find(serverPeople,"sidebarServerMembers").visible,"collapsing Friends does not hide People")
    test.find(serverPeople,"members-collapse").clicked();input.wait(20)
    var restored=savedPreferences.createObject(window.contentItem);input.wait(80);test.check(restored.membersCollapsed && restored.collapsed,"People and Friends collapse preferences persist independently");restored.destroy()
    test.find(serverPeople,"members-collapse").clicked();bridge.friendPreferences.toggleCollapsed()
    leftSidebar.width=24;input.wait(20)
    test.check(test.find(serverPeople,"browseMembers").width<=24,"People search fits minimum server sidebar")
    leftSidebar.width=230
    test.find(feed,"messageAuthor-message").clicked();input.wait(20);test.ackLists();input.wait(20)
    var authorMenu=test.find(feed,"participantMenu");test.check(authorMenu.opened && test.find(authorMenu,"addFriend").visible && !test.find(authorMenu,"participantMenuVolume").visible,"chat author opens Add friend without voice controls")
    authorMenu.close()
    bridge.friendships.act({id:"river",server_id:"other"},"send");test.check(test.last().args.server_id==="other","same account ID on another server stays scoped");test.ack(test.last().id,true,[{id:"river",display_name:"River elsewhere",relationship:"outgoing"}])
    test.check(bridge.friendships.relationship({id:"river",server_id:"local"})==="none","other server request does not change local relationship")
    before=bridge.sent.length;bridge.friendships.act({id:"self",server_id:"local"},"send");bridge.friendships.act({id:"friend",server_id:"local"},"send");test.check(bridge.sent.length===before,"self and existing friends cannot receive requests")
    bridge.friendships.sync("friend_requests_changed");test.check(bridge.sent.length===before+2,"request events refresh connected servers");test.ackLists()
    if(test.failed){Qt.quit();return}
    if(Quickshell.env("WISP_FRIENDSHIPS_SCREENSHOT")){test.find(serverPeople,"browseMembers").clicked();test.ackLists();input.wait(60);dialog.contentItem.parent.grabToImage(function(im){im.saveToFile(Quickshell.env("WISP_FRIENDSHIPS_SCREENSHOT"));console.log("FRIENDSHIPS_OK");Qt.quit()})}
    else {console.log("FRIENDSHIPS_OK");Qt.quit()}
  }}
}
