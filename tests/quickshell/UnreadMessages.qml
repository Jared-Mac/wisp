import QtQuick
import QtTest
import Quickshell
import "app" as Wisp
import "app/views" as Views

ShellRoot {
  id: test
  property bool failed: false
  property string focused: ""
  property int sequence: 0
  function check(ok, label) { if (!ok) { failed=true; console.error("UNREAD_FAILED: " + label) } }
  function find(item,name) {
    if (!item) return null
    if (item.objectName===name) return item
    var children=item.children || []
    for (var i=0;i<children.length;i++) { var found=find(children[i],name); if (found) return found }
    if (item.contentItem && children.indexOf(item.contentItem)<0) return find(item.contentItem,name)
    return null
  }
  function message(id, chat, own) {
    return {id:id,conversation_id:chat,sender:{id:own ? "self" : "friend",display_name:own ? "Rowan" : "Mira"},created_at:new Date(1789041600000+(++sequence)*1000).toISOString(),content_type:"text/plain",payload:"Fictional message " + id}
  }
  function append(chat, id, own) {
    var s=JSON.parse(JSON.stringify(bridge.snapshot)), state=s.server_states[0], m=message(id,chat,own)
    state.messages.push(m)
    state.conversations.forEach(function(c) { if(c.id===chat) { c.last_message=m; c.unread_count=Number(c.unread_count || 0)+(own ? 0 : 1) } })
    bridge.applySnapshot(s,"message_created")
  }
  function ackRead(chat) {
    var s=JSON.parse(JSON.stringify(bridge.snapshot))
    s.server_states[0].conversations.forEach(function(c) { if(c.id===chat)c.unread_count=0 })
    bridge.applySnapshot(s,"conversation_read")
  }
  function reads() { return bridge.sent.filter(function(c) { return c.name==="mark_conversation_read" }) }
  Wisp.WispTheme { id:theme; profile:Quickshell.env("WISP_TEST_THEME") || "soft_graphite" }
  Wisp.WispBridge { id:bridge; property var sent:[]; function send(name,args) { sent.push({name:name,args:args}); return "fixture-"+(++requestId) } }
  FloatingWindow {
    id:window; visible:true; implicitWidth:1000; implicitHeight:640; color:theme.background
    Row {
      id: scene
      anchors.fill:parent; spacing:12
      Views.ConversationWorkspace {id:first;width:480;height:620;bridge:bridge;theme:theme;tiled:true;selectedId:"local::chat";paneActive:test.focused==="chat"}
      Views.ConversationWorkspace {id:second;width:480;height:620;bridge:bridge;theme:theme;tiled:true;selectedId:"local::other";paneActive:test.focused==="other"}
    }
  }
  TestCase { id:input; parent:window.contentItem; when:false }
  Component.onCompleted: {
    var s=JSON.parse(JSON.stringify(bridge.snapshot)), server={id:"local",name:"Lantern club",connected:true}, me={id:"self",display_name:"Rowan",media:{},presence:"open"}
    var old=message("old","chat",false), unread=message("offline","chat",false)
    s.self=Object.assign(s.self,me);s.servers=[server];s.selected_server_id="local"
    s.server_states=[{server:server,self:s.self,conversations:[{id:"chat",kind:"direct",label:"Mira",unread_count:1,last_message:unread},{id:"other",kind:"direct",label:"Jules",unread_count:0}],messages:[old,unread],friends:[],hangouts:[],spots:[],knocks:[],devices:[],room_invitations:[]}]
    bridge.applySnapshot(s)
  }
  Timer {
    running:true; interval:500
    onTriggered: {
      var feed=test.find(first,"messageFeed"), other=test.find(second,"messageFeed")
      test.check(!!feed && !!other,"two real chat tiles render"); if(!feed || !other){Qt.quit();return}
      // Window managers differ in offscreen mode. Control foreground state,
      // while using the real feed timers, list geometry and bridge snapshots.
      feed.readerFocused=Qt.binding(function(){return test.focused==="chat"})
      other.readerFocused=Qt.binding(function(){return test.focused==="other"})
      test.check(bridge.unreadMarkers.boundary("local::chat").firstId==="offline","offline unread boundary seeded")
      test.focused="chat";input.wait(850)
      test.check(test.reads().length===0 && bridge.unreadMarkers.pending("local::chat"),"alt-tab alone does not acknowledge")
      test.check(test.find(first,"newMessagesDivider-offline").visible && test.find(first,"chatNewMessagesButton").visible,"divider and tile indicator visible")
      feed.engageReader();input.wait(850)
      test.check(test.reads().length===1 && test.reads()[0].args.server_id==="local" && test.reads()[0].args.conversation_id==="chat","focused reader at latest acknowledges scoped conversation")
      test.ackRead("chat");input.wait(80)
      test.check(!!bridge.unreadMarkers.boundary("local::chat"),"read snapshot preserves divider during visit")
      test.focused="other";input.wait(80)
      test.check(!bridge.unreadMarkers.boundary("local::chat"),"divider clears after leaving read chat")
      test.append("chat","background",false);input.wait(100)
      test.check(bridge.unreadMarkers.boundary("local::chat").firstId==="background","inactive visible tile gets boundary")
      test.append("chat","second-new",false);input.wait(100)
      test.check(bridge.unreadMarkers.boundary("local::chat").firstId==="background","multiple arrivals preserve first boundary")
      var count=test.reads().length
      test.focused="chat";input.wait(850)
      test.check(test.reads().length===count,"returning focus still requires engagement")
      feed.engageReader();input.wait(850);test.ackRead("chat")
      test.focused="";input.wait(60)
      test.check(!bridge.unreadMarkers.boundary("local::chat"),"leaving window clears already read marker")
      test.append("other","self-message",true);input.wait(80)
      test.check(!bridge.unreadMarkers.boundary("local::other"),"own send never creates marker")
      test.focused="chat";feed.engageReader();input.wait(850)
      test.append("chat","watching",false);input.wait(850)
      test.check(!bridge.unreadMarkers.boundary("local::chat"),"live chat being read at bottom stays clear")
      test.ackRead("chat")
      for(var n=0;n<35;n++)test.append("chat","long-"+n,true)
      input.wait(150)
      feed.revealMessage("old");input.wait(100)
      test.check(feed.awayFromLatest,"fixture is scrolled away from latest")
      count=test.reads().length;test.append("chat","below-fold",false);input.wait(850)
      test.check(bridge.unreadMarkers.pending("local::chat") && test.reads().length===count,"scrolled-up focused tile retains unread")
      feed.scrollToLatest();input.wait(850)
      test.check(bridge.unreadMarkers.boundary("local::chat").seen,"reaching latest acknowledges but retains divider")
      test.ackRead("chat");test.focused="";input.wait(60)
      test.append("chat","deleted-first",false);test.append("chat","remaining",false);input.wait(60)
      var changed=JSON.parse(JSON.stringify(bridge.snapshot));changed.server_states[0].messages=changed.server_states[0].messages.filter(function(m){return m.id!=="deleted-first"});bridge.applySnapshot(changed,"message_deleted");input.wait(60)
      test.check(bridge.unreadMarkers.boundary("local::chat").firstId==="remaining","deleted first unread advances divider")
      changed=JSON.parse(JSON.stringify(bridge.snapshot));var duplicate={id:"remote",name:"Other server",connected:true};changed.servers.push(duplicate)
      var foreign=test.message("remote-own","chat",true);foreign.sender.id="remote-self"
      changed.server_states.push({server:duplicate,self:{id:"remote-self"},messages:[foreign],conversations:[{id:"chat",kind:"direct",label:"Other Mira",unread_count:0,last_message:foreign}],friends:[],hangouts:[],spots:[],knocks:[],devices:[],room_invitations:[]});bridge.applySnapshot(changed,"message_created");input.wait(60)
      test.check(!bridge.unreadMarkers.boundary("remote::chat") && bridge.unreadMarkers.pending("local::chat"),"server identities and chat boundaries stay separate")
      test.check(!bridge.sent.some(function(c){return /^(join|share|camera|watch_video)/.test(c.name)}),"no media starts")
      var screenshot=Quickshell.env("WISP_UNREAD_SCREENSHOT")
      if(screenshot)scene.grabToImage(function(im){im.saveToFile(screenshot);console.log(test.failed?"UNREAD_FAILED":"UNREAD_OK");Qt.quit()})
      else {console.log(test.failed?"UNREAD_FAILED":"UNREAD_OK");Qt.quit()}
    }
  }
}
