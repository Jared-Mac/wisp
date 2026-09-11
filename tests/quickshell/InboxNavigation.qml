import QtQuick
import QtTest
import Quickshell
import "app" as Wisp
import "app/views" as Views
import "app/ChatTiles.js" as Tiles

ShellRoot {
  id:test
  property bool failed:false
  property int sequence:0
  function check(ok,label){if(!ok){failed=true;console.error("INBOX_FAILED: "+label)}}
  function find(item,name){if(!item)return null;if(item.objectName===name)return item;var children=item.children || [];for(var i=0;i<children.length;i++){var found=find(children[i],name);if(found)return found}if(item.contentItem && children.indexOf(item.contentItem)<0)return find(item.contentItem,name);return null}
  function incoming(id,event){
    var data=JSON.parse(JSON.stringify(bridge.snapshot)),state=data.server_states[0]
    var c=state.conversations.filter(function(c){return c.id===id})[0]
    if(!c){c={id:id,kind:"direct",label:id,members:[{id:"self",display_name:"Rowan"},{id:id,display_name:id}],unread_count:0};state.conversations.push(c)}
    var message={id:"new-"+(++sequence),conversation_id:id,sender:{id:id,display_name:id},created_at:new Date(1789041600000+sequence*1000).toISOString(),content_type:"text/plain",payload:"Hello from "+id}
    c.last_message=message;c.unread_count++;state.messages.push(message)
    bridge.applySnapshot(data,event || "message_created")
  }
  Wisp.WispAppearance{id:appearance;Component.onCompleted:setProfile(Quickshell.env("WISP_TEST_THEME") || "soft_graphite")}
  Wisp.WispTheme{id:theme;appearanceController:appearance;profile:appearance.profile}
  Wisp.WispBridge{id:bridge;property var sent:[];function send(name,args){sent.push({name:name,args:args});return "fixture-"+(++requestId)}}
  FloatingWindow{id:window;visible:true;implicitWidth:1100;implicitHeight:720;color:theme.background
    Rectangle{id:canvas;anchors.fill:parent;color:theme.background
      Views.MainWorkspace{id:workspace;anchors.fill:parent;anchors.margins:10;bridge:bridge;theme:theme}
    }
  }
  TestCase{id:input;parent:window.contentItem;when:false}
  Component.onCompleted:{
    bridge.workspaceLayout.chatTiles=JSON.stringify({key:"room",id:"local::room"})
    var data=JSON.parse(JSON.stringify(bridge.snapshot)),server={id:"local",name:"Community",connected:true}
    var self=Object.assign({},data.self,{id:"self",display_name:"Rowan",connection:"connected",hangout_id:"voice",media:Object.assign({},data.self.media,{livekit_connected:true})})
    var friend={id:"mira",display_name:"Mira",presence:"knock",online:true}
    data.servers=[server];data.selected_server_id="local";data.voice_server_id="local";data.self=self
    data.server_states=[{server:server,self:self,conversations:[{id:"room",kind:"circle",label:"Lounge",server_channel:true,members:[self,friend]}],messages:[],friends:[friend],hangouts:[{id:"voice",label:"Lounge",members:[self,friend],sharing:[]}],spots:[{id:"lounge",name:"Lounge",active_hangout_id:"voice",members:[self,friend]}],knocks:[],devices:[],room_invitations:[]}]
    bridge.applySnapshot(data);bridge.activeConversationId="local::room"
  }
  Timer{running:true;interval:500;onTriggered:{
    var tiles=test.find(workspace,"conversationPane"),friend=test.find(workspace,"friendName"),voice=test.find(workspace,"friendPresence-mira")
    bridge.friendships.put("local",{people:[{id:"river",display_name:"River",server_id:"local",relationship:"none"}],ready:true,loading:false})
    input.wait(50)
    test.check(bridge.workspaceLayout.incomingDmsAsTiles,"incoming DMs default on")
    test.check(bridge.workspaceLayout.activityWidth===280,"first launch chooses a sidebar wide enough for six controls")
    test.check(test.find(workspace,"serverPeopleSection").mapToItem(canvas,0,0).y>test.find(workspace,"friends-collapse").mapToItem(canvas,0,0).y,"other members below friends")
    var start=bridge.sent.length;input.mouseClick(friend,2,friend.height/2);input.wait(40)
    test.check(bridge.sent.slice(start).some(function(c){return c.name==="open_direct"}) && !bridge.sent.slice(start).some(function(c){return c.name==="join_friend"}),"friend name opens text only")
    input.mouseClick(voice,voice.width/2,voice.height/2);input.wait(30)
    test.check(bridge.sent.some(function(c){return c.name==="join_friend"}),"voice icon explicitly requests voice")
    var active=tiles.activeKey,current=bridge.activeConversationId
    test.incoming("mira");input.wait(120)
    test.check(Tiles.leaves(tiles.tree).some(function(n){return n.id==="local::mira"}),"incoming DM opens a tile")
    test.check(tiles.activeKey===active && bridge.activeConversationId===current,"incoming tile preserves current chat")
    test.check(bridge.unreadConversations.length===1 && bridge.pendingCount("local::mira")===1,"inbox retains unread arrival")
    test.check(!test.find(workspace,"participantUnread-self").visible && test.find(workspace,"participantUnread-mira").visible,"DM badge appears on the peer only, never yourself")
    var count=tiles.paneCount;test.incoming("mira");input.wait(70);test.check(tiles.paneCount===count,"second message never duplicates tile")
    bridge.workspaceLayout.incomingDmsAsTiles=false;test.incoming("river");input.wait(70)
    test.check(tiles.paneCount===count && bridge.unreadConversations.length===2,"disabled automatic tiles keep inbox notifications")
    bridge.workspaceLayout.incomingDmsAsTiles=true;test.incoming("history","server_reconnected");input.wait(70)
    test.check(tiles.paneCount===count,"reconnect history never opens tiles")
    var firstUnread=bridge.unreadMarkers.boundary("local::mira").firstId
    var badge=test.find(workspace,"participantUnread-mira")
    input.mouseClick(badge,badge.width/2,badge.height/2);input.wait(90)
    test.check(bridge.activeConversationId==="local::mira","participant badge activates the matching tile")
    var focused=Tiles.leaves(tiles.tree).filter(function(n){return n.id==="local::mira"})[0]
    var activeFeed=test.find(test.find(workspace,"chatTile-"+focused.key),"messageFeed")
    test.check(activeFeed.highlightedId===firstUnread && bridge.pendingCount("local::mira")===0 && !badge.visible,"participant badge highlights first unread and clears the notification")
    firstUnread=bridge.unreadMarkers.boundary("local::river").firstId
    var inboxButton=test.find(workspace,"chatInbox")
    inboxButton.clicked();input.wait(50)
    var inboxPopup=input.findChild(inboxButton,"chatInboxPopup")
    var inboxEntry=inboxPopup ? test.find(inboxPopup.contentItem,"inboxChat-local::river") : null
    test.check(!!inboxEntry,"inbox entry is available")
    if(inboxEntry)inboxEntry.clicked();input.wait(90)
    focused=Tiles.leaves(tiles.tree).filter(function(n){return n.id==="local::river"})[0]
    activeFeed=focused ? test.find(test.find(workspace,"chatTile-"+focused.key),"messageFeed") : null
    test.check(activeFeed && activeFeed.highlightedId===firstUnread && bridge.pendingCount("local::river")===0 && tiles.paneCount===count+1,"inbox opens a missing tile, highlights unread and clears its notification")
    test.incoming("mira");bridge.openChannel("local::mira",true);input.wait(60)
    // Ordinary chat navigation leaves New available; it uses the same action.
    var pendingButtons=[]
    function collect(item){if(item.objectName==="chatNewMessagesButton" && item.visible)pendingButtons.push(item);(item.children || []).forEach(collect)}
    collect(workspace);test.check(pendingButtons.length>0,"new message control is available")
    if(pendingButtons.length)pendingButtons[0].clicked()
    input.wait(60);test.check(bridge.pendingCount("local::mira")===0 && !bridge.unreadMarkers.pending("local::mira"),"New clears marker and count immediately")
    bridge.applySnapshot(JSON.parse(JSON.stringify(bridge.snapshot)),"snapshot");input.wait(30)
    test.check(!bridge.unreadMarkers.pending("local::mira"),"stale server count does not resurrect marker")
    test.incoming("mira");input.wait(40);test.check(bridge.pendingCount("local::mira")>0,"later messages can become unread again")
    firstUnread=bridge.unreadMarkers.boundary("local::mira").firstId
    test.find(workspace,"messageFriend-mira").clicked();input.wait(70)
    test.check(bridge.pendingCount("local::mira")===0,"friend unread badge clears the same notification")
    var selector=test.find(workspace,"activeServerSelector");start=bridge.sent.length
    input.mouseMove(selector,8,8);input.wait(30);bridge.refreshServerPing("local")
    var pings=bridge.sent.slice(start).filter(function(c){return c.name==="server_ping"});test.check(pings.length===1,"hover ping is scoped and cached")
    var soundboard=test.find(workspace,"audioSoundboardButton"),mute=test.find(workspace,"muteControl")
    test.check(soundboard!==null && Math.abs(soundboard.mapToItem(canvas,0,0).y-mute.mapToItem(canvas,0,0).y)<2,"soundboard sits beside mute and deafen")
    var disconnect=test.find(workspace,"currentCallDisconnect"),share=test.find(workspace,"mediaAction-share"),camera=test.find(workspace,"mediaAction-camera")
    test.check(Math.abs(disconnect.mapToItem(canvas,0,0).y-share.mapToItem(canvas,0,0).y)<2 && Math.abs(camera.mapToItem(canvas,0,0).y-share.mapToItem(canvas,0,0).y)<2,"share camera and disconnect align on one row")
    test.check(Math.abs(mute.mapToItem(canvas,0,0).y-share.mapToItem(canvas,0,0).y)<2,"all six audio controls share one compact row")
    test.check(test.find(workspace,"currentCallLocation").text==="Lounge" && test.find(workspace,"currentCallConnection").text==="- connected","compact status identifies the connected room")
    appearance.setShowAvatars(false);input.wait(40);test.check(!test.find(workspace,"friendAvatar").visible,"hide avatars applies to friends");appearance.setShowAvatars(true)
    input.wait(40)
    var screenshot=Quickshell.env("WISP_INBOX_SCREENSHOT")
    if(screenshot)canvas.grabToImage(function(im){im.saveToFile(screenshot);console.log(test.failed?"INBOX_FAILED":"INBOX_OK");Qt.quit()})
    else{console.log(test.failed?"INBOX_FAILED":"INBOX_OK");Qt.quit()}
  }}
}
