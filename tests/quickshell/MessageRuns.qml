import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components

ShellRoot {
  id:test
  property bool failed:false
  function check(ok,label){if(!ok){failed=true;console.error("MESSAGE_RUNS_FAILED: "+label)}}
  function find(item,name){if(!item)return null;if(item.objectName===name)return item;for(var c of item.children || []){var r=find(c,name);if(r)return r}return null}
  property var initial:null
  Wisp.WispAppearance {id:appearance;environment:"desktop"}
  Wisp.WispTheme {id:theme;appearanceController:appearance;profile:appearance.profile}
  Wisp.WispBridge {id:bridge;property var sent:[];function send(name,args){sent.push({name:name,args:args});return "fake-"+(++requestId)}}
  FloatingWindow {
    id:window;visible:true;implicitWidth:520;implicitHeight:760;color:theme.background
    Rectangle {
      id:canvas;anchors.fill:parent;color:theme.background
      Components.MessageFeed {id:feed;anchors.fill:parent;anchors.margins:12;theme:theme;bridge:bridge;conversationId:"local::chat"}
    }
  }
  TestCase {id:input;parent:window.contentItem;when:false}
  Component.onCompleted:{
    appearance.setProfile(Quickshell.env("WISP_TEST_THEME") || "soft_graphite")
    var data=JSON.parse(JSON.stringify(bridge.snapshot)),self={id:"self",display_name:"Morgan",hangout_id:null,media:{},presence:"open"}
    var server={id:"local",name:"Community",connected:true};data.servers=[server];data.selected_server_id="local";data.self=Object.assign(data.self,self)
    var messages=["self","self","river","self","river","river"].map(function(id,i){return {
      id:"m"+i,sender:{id:id,display_name:id==="self"?"Morgan":"River"},conversation_id:"chat",created_at:"2026-09-10T12:0"+i+":00Z",content_type:"text/plain",payload:["Anyone up for a game?","I can share my screen.","Sounds good!","Ready when you are.","One moment.","I'm here."][i]}})
    messages.push({id:"invite",sender:messages[5].sender,conversation_id:"chat",created_at:"2026-09-10T12:06:00Z",content_type:"application/vnd.wisp.room-invitation+json",payload:{id:"invite",state:"expired",room_name:"Lounge",expires_at:"2026-01-01T00:00:00Z"}})
    data.server_states=[{server:server,self:data.self,messages:messages,reactions:[{target_id:"invite",message:{id:"old-reaction",sender:self,content_type:"application/vnd.wisp.reaction+json",payload:{target:"invite",emoji:"😀"}}}],conversations:[{id:"chat",kind:"direct",label:"River",members:[self,messages[2].sender]}],friends:[],hangouts:[],spots:[],knocks:[],devices:[],room_invitations:[]}]
    data.server_states[0].reactions.push({target_id:"m0",message:{id:"normal-reaction",sender:self,content_type:"application/vnd.wisp.reaction+json",payload:{target:"m0",emoji:"😀"}}})
    initial=JSON.parse(JSON.stringify(data));bridge.applySnapshot(data)
  }
  Timer {running:true;interval:500;onTriggered:{
    input.mouseMove(window.contentItem,0,0);input.wait(40)
    for(var i=0;i<6;i++) {
      var avatar=test.find(feed,"messageAvatar-m"+i)
      test.check(avatar && avatar.visible===(theme.friendly && theme.showAvatars && [0,2,3,4].indexOf(i)>=0),"avatar starts the appropriate sender run "+i)
      test.check(test.find(feed,"messageAuthor-m"+i).visible,"every message keeps its author")
    }
    var action=test.find(feed,"addReaction-m1"), list=test.find(feed,"messageList"), height=list.contentHeight
    test.check(action && action.opacity===0,"empty reaction action is hidden at rest")
    test.check(test.find(feed,"reaction-m0-😀").visible,"existing reactions remain visible without hover")
    action.forceActiveFocus();input.wait(30);test.check(action.opacity===1,"keyboard focus reveals reaction action")
    window.contentItem.forceActiveFocus();input.mouseMove(action,2,2);input.wait(30)
    test.check(action.opacity===1,"message hover reveals reaction action")
    test.check(list.contentHeight===height,"revealing actions does not add a row or move messages")
    input.mouseMove(window.contentItem,0,0);input.wait(30);test.check(action.opacity===0,"reaction action hides again")
    test.check(!test.find(feed,"addReaction-invite").visible && !test.find(feed,"reaction-invite-😀"),"invites have no reaction action or historical chips")
    var count=bridge.sent.length;bridge.chatExtras.react("local","invite","😀");test.check(bridge.sent.length===count,"invite reactions cannot issue a client command")
    bridge.unreadMarkers.boundaries=({"local::chat":{firstId:"m1",createdAt:"2026-09-10T12:01:00Z",seen:false}});input.wait(30)
    test.check(test.find(feed,"messageAvatar-m1").visible===(theme.friendly && theme.showAvatars),"unread boundary begins a new avatar run")
    bridge.unreadMarkers.boundaries=({})
    var next=JSON.parse(JSON.stringify(test.initial));next.server_states[0].messages.shift();bridge.applySnapshot(next);input.wait(40)
    test.check(test.find(feed,"messageAvatar-m1").visible===(theme.friendly && theme.showAvatars),"deletion promotes the next message to run start")
    bridge.applySnapshot(test.initial);input.wait(50)
    var screenshot=Quickshell.env("WISP_MESSAGE_RUNS_SCREENSHOT")
    if(screenshot) canvas.grabToImage(function(im){im.saveToFile(screenshot);console.log(test.failed?"MESSAGE_RUNS_FAILED":"MESSAGE_RUNS_OK");Qt.quit()})
    else {console.log(test.failed?"MESSAGE_RUNS_FAILED":"MESSAGE_RUNS_OK");Qt.quit()}
  }}
}
