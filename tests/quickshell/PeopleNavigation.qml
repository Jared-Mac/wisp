import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp
import "app/views" as Views
import "app/ChatMarkup.js" as Markup
ShellRoot {
  id:test;property bool failed:false
  function check(ok,label){if(!ok){failed=true;console.error("PEOPLE_NAV_FAILED: "+label)}}
  function find(item,name,seen){if(!item)return null;seen=seen||[];if(seen.indexOf(item)>=0)return null;seen.push(item);if(item.objectName===name)return item;if(item.contentItem){var r=find(item.contentItem,name,seen);if(r)return r}var children=item.data||item.contentData||item.children||[];for(var i=0;i<children.length;i++){var r=find(children[i],name,seen);if(r)return r}return null}
  Wisp.WispTheme {id:theme;profile:Quickshell.env("WISP_TEST_THEME")||"soft_graphite"}
  Wisp.WispBridge {id:bridge;property var sent:[];function send(name,args){var id="test-"+(++requestId);sent.push({id:id,name:name,args:args});return id}}
  FloatingWindow {id:window;visible:true;implicitWidth:660;implicitHeight:760;color:theme.background
    Rectangle {id:canvas;anchors.fill:parent;color:theme.background
    Row {anchors.fill:parent;anchors.margins:16;spacing:16
      Views.PeopleView {id:app;width:300;bridge:bridge;theme:theme}
      Views.PeopleView {id:tray;width:300;bridge:bridge;theme:theme;presentation:"panel"}
    }
  }
  }
  TestCase {id:input;parent:window.contentItem;when:false}
  Component {id:preferences;Wisp.WispFriendPreferences {account:"self"}}
  Component.onCompleted:{
    var data=JSON.parse(JSON.stringify(bridge.snapshot));data.self.id="self";data.self.display_name="Morgan";
    var server={id:"local",name:"Community",connected:true};data.servers=[server];data.server_states=[{server:server,server_member:true,self:data.self,friends:[{id:"inside",display_name:"Alex",online:true,presence:"closed"},{id:"outside",display_name:"River",online:true,presence:"closed"}],conversations:[{id:"dm",kind:"direct",label:"River",members:[{id:"self"},{id:"outside"}],unread_count:1}],messages:[],spots:[],hangouts:[],knocks:[]}];bridge.applySnapshot(data)
  }
  Timer {running:true;interval:450;onTriggered:{
    bridge.friendships.put("local",{ready:true,people:[
      {id:"self",server_id:"local",display_name:"Morgan",relationship:"self",server_member:true},
      {id:"inside",server_id:"local",display_name:"Alex",relationship:"friend",server_member:true},
      {id:"outside",server_id:"local",display_name:"River",relationship:"friend",server_member:false},
      {id:"incoming",server_id:"local",display_name:"Sage",relationship:"incoming",server_member:false},
      {id:"outgoing",server_id:"local",display_name:"Birch",relationship:"outgoing",server_member:false},
      {id:"member",server_id:"local",display_name:"Rowan",relationship:"none",server_member:true}]})
    input.wait(60)
    test.check(!app.friendsMode && !tray.friendsMode,"server view remains the default with membership")
    test.check(test.find(app,"sidebarServerMembers").count===1,"nonmember requests stay out of server members")
    test.check(test.find(app,"serverPeopleTab").children[0].visibleFriends.length===1,"only actual server friends appear in server view")
    test.check(Markup.parts("wisp.you/tea123456781234").some(function(p){return p.href==="https://wisp.you/tea123456781234"}),"shortened invite is clickable without changing its label")
    var toggle=test.find(app,"toggleFriendsView"),inbox=test.find(app,"chatInbox")
    test.check(toggle.primary && toggle.text.indexOf("1")>=0,"incoming requests badge the Friends toggle")
    test.check(inbox.width<60 && toggle.x>=inbox.width,"Inbox and toggle share a compact row")
    toggle.clicked();input.wait(60)
    test.check(app.friendsMode && !tray.friendsMode,"app and tray Friends destinations are independent")
    test.check(app.allFriends.length===2 && !!test.find(app,"friendRequest-local-incoming") && !!test.find(app,"friendRequest-local-outgoing"),"Friends includes nonmember contacts and both request directions")
    var dm=test.find(app,"friendsDirect-local::dm");test.check(!!dm,"nonmember friend DM is available")
    dm.clicked();test.check(bridge.pendingConversationTiles.some(function(p){return p.id==="local::dm" && p.revealUnread}),"DM opens and highlights unread messages")
    var card=test.find(app,"friendRequest-local-incoming");test.find(card,"addFriend").clicked()
    var last=bridge.sent[bridge.sent.length-1];test.check(last.name==="accept_friend_request" && last.args.user_id==="incoming","request acceptance remains explicit and scoped")
    var saved=preferences.createObject(window.contentItem);input.wait(60);test.check(saved.friendsViewFor("app") && !saved.friendsViewFor("panel"),"view choice persists independently");saved.destroy()
    app.width=24;input.wait(30);test.check(toggle.width<=24 && inbox.width<=24 && toggle.y>=inbox.height,"narrow navigation wraps without clipping");app.width=300
    toggle.clicked();input.wait(30);test.check(!app.friendsMode,"toggle returns to server members")
    var data=JSON.parse(JSON.stringify(bridge.snapshot));data.server_states[0].server_member=false;data.servers=[];bridge.applySnapshot(data);input.wait(30)
    test.check(app.friendsMode && tray.friendsMode,"accounts without servers open Friends automatically")
    test.check(!bridge.sent.some(function(c){return c.name==="join" || c.name==="join_friend"}),"navigation never joins voice")
    if(Quickshell.env("WISP_PEOPLE_SCREENSHOT")) {
      bridge.friendPreferences.setFriendsView(true,"app")
      data=JSON.parse(JSON.stringify(bridge.snapshot));data.server_states[0].server_member=true;data.servers=[data.server_states[0].server];bridge.applySnapshot(data)
      Qt.callLater(function(){canvas.grabToImage(function(image){image.saveToFile(Quickshell.env("WISP_PEOPLE_SCREENSHOT"));console.log(test.failed ? "PEOPLE_NAV_FAILED" : "PEOPLE_NAV_OK");Qt.quit()})})
    } else {console.log(test.failed ? "PEOPLE_NAV_FAILED" : "PEOPLE_NAV_OK");Qt.quit()}
  }}
}
