import QtQuick
import QtTest
import Quickshell
import "app" as Wisp
import "app/views" as Views

ShellRoot {
  id: test
  property bool failed: false
  property alias fixtureBridge: bridge
  property alias fixtureTheme: theme
  readonly property bool compact: Quickshell.env("WISP_TEST_PRESENTATION") === "panel"
  function check(ok, message) { if (!ok) { failed = true; console.error("ROOM_FLOW_FAILED: " + message) } }
  function find(item, name) {
    if (!item) return null
    if (item.objectName === name) return item
    for (var child of item.children || []) { var result = find(child,name); if (result) return result }
    return null
  }
  function object(item, name, visited) {
    if (!item || visited.indexOf(item) >= 0) return null
    visited.push(item)
    if (item.objectName === name) return item
    if (item.contentItem) { var content = object(item.contentItem,name,visited); if (content) return content }
    if (item.item) { var loaded = object(item.item,name,visited); if (loaded) return loaded }
    for (var child of item.data || item.contentData || item.children || []) { var found = object(child,name,visited); if (found) return found }
    return null
  }
  function last() { return bridge.sent[bridge.sent.length - 1] }
  function reply(value) { bridge.finishRequest({id:"test-" + bridge.requestId,ok:true,value:value}) }
  function screenshot(label, target) {
    var path = Quickshell.env("WISP_ROOM_FLOW_SCREENSHOT")
    if (path) target.grabToImage(function(result) { result.saveToFile(path + "-" + label + ".png") })
  }
  Wisp.WispAppearance { id: appearance; environment: Quickshell.env("WISP_TEST_ADAPTER") === "omarchy" ? "omarchy" : "desktop" }
  Wisp.WispTheme {
    id: theme; appearanceController: appearance
    profile: Quickshell.env("WISP_TEST_ADAPTER") === "omarchy" ? "legacy" : Quickshell.env("WISP_TEST_THEME") || "performative"
    Component.onCompleted: if (Quickshell.env("WISP_TEST_ADAPTER") === "omarchy") {
      tuiTreatment = true; foreground = "#d8dee9"; background = "#242933"; surface = "#242933"
      accent = "#81a1c1"; muted = "#a0a8b7"; cornerRadius = 7; fontFamily = "DejaVu Sans"
    }
  }
  Wisp.WispBridge {
    id: bridge
    property var sent: []
    function send(name,args) { sent.push({name:name,args:args}); requestId++; return "test-" + requestId }
    function localPreviewUrl(stem, revision) { return String(Qt.resolvedUrl("app/assets/waveform.svg")) }
  }
  FloatingWindow {
    id: window
    visible: true
    implicitWidth: Number(Quickshell.env("WISP_TEST_WIDTH")) || (test.compact ? 420 : 840)
    implicitHeight: 760
    color: theme.background
    Wisp.WispContent { id: page; anchors.fill: parent; theme: theme; bridge: bridge; logoSource: Qt.resolvedUrl("app/assets/wisp-icon.svg"); presentation: test.compact ? "panel" : "app" }
    // The real tray shares its bridge with a desktop tile host, even while hidden.
    Loader {
      active: test.compact; visible: false; width: 840; height: 760
      sourceComponent: Component { Views.TiledConversations { bridge: test.fixtureBridge; theme: test.fixtureTheme } }
    }
  }
  TestCase { id: input; parent: window.contentItem; when: false }
  function fixture() {
    var data = JSON.parse(JSON.stringify(bridge.snapshot))
    var people = [{id:"self",display_name:"Tyler"},{id:"friend",display_name:"Jared"}]
    data.self.server_admin=true;data.self.id="self"; data.self.display_name="Tyler"; data.self.hangout_id=null; data.self.connection="available"
    var first={id:"local",name:"Home",connected:true}, second={id:"second",name:"Other",connected:true}
    data.servers=[first,second]; data.selected_server_id="local"; data.voice_server_id="local"
    var rooms=[{id:"lounge",name:"Lounge",active_hangout_id:"active",members:people},{id:"quiet",name:"Quiet",members:[]}]
    var chats=[{id:"spot:lounge",kind:"hangout",label:"Lounge",spot_id:"lounge",self_role:"host",members:people},
      {id:"spot:quiet",kind:"hangout",label:"Quiet",spot_id:"quiet",self_role:"host",members:people},
      {id:"hangout:private",kind:"hangout",label:"Hangout",members:people},
      {id:"dm:friend",kind:"direct",label:"Jared",members:people}]
    var calls=[{id:"active",label:"Lounge",members:people},{id:"private",label:null,members:people}]
    var state={server:first,self:data.self,spots:rooms,hangouts:calls,conversations:chats,friends:[{id:"friend",display_name:"Jared",online:true,presence:"open"}],messages:[],knocks:[],devices:[],room_invitations:[]}
    var other=JSON.parse(JSON.stringify(state)); other.server=second; other.friends=[{id:"other",display_name:"Other friend",online:true,presence:"open"}]
    other.conversations[3].members=[{id:"other-self",display_name:"Other me"},{id:"other",display_name:"Other friend"}]
    other.conversations[3].label="Other friend"
    other.self.server_admin=false;other.self.id="other-self"; other.spots[0].name="Other lounge"; other.conversations[0].label="Other lounge"
    data.server_states=[state,other]
    return data
  }
  function visibleItems(item,name,result) {
    if (item.objectName===name && item.visible) result.push(item)
    for (var child of item.children || []) visibleItems(child,name,result)
    return result
  }
  function click(item) {
    if (!item) { check(false,"click target exists"); return }
    for(var p=item.parent;p;p=p.parent) {
      if(typeof p.contentY === "number" && p.contentHeight > p.height) {
        var pos=item.mapToItem(p,0,0)
        p.contentY=Math.max(0,Math.min(p.contentHeight-p.height,p.contentY+pos.y+item.height/2-p.height/2))
      }
    }
    input.wait(40)
    input.mouseClick(item,item.objectName.indexOf("openRoom-")===0 ? 12 : item.width/2,item.objectName.indexOf("openRoom-")===0 ? 12 : item.height/2)
  }
  Component.onCompleted: bridge.applySnapshot(fixture())
  Timer {
    interval:600; running:true
    onTriggered: {
      var lounge=test.find(page,"savedRoom-lounge"), quiet=test.find(page,"savedRoom-quiet")
      test.check(lounge && quiet,"occupied and empty rooms share a single list")
      test.check(bridge.roomCount===2 && bridge.temporaryCalls.length===1,"temporary calls do not inflate room count")
      test.check(!test.find(page,"serverChannel-spot:lounge"),"rooms are not duplicated as text channels")
      test.check(test.find(page,"friendCalls").visible,"temporary calls appear beside friends")
      test.check(test.find(lounge,"roomName").text==="#Lounge /2","room name leads the occupied row")
      var rowHeight=lounge.height
      var before=bridge.sent.length
      var open=test.find(page,"openRoom-quiet")
      test.click(open); input.wait(120)
      test.check(bridge.activeConversationId==="local::spot:quiet","room click opens its chat")
      test.check(!bridge.sent.slice(before).some(function(c){return c.name.indexOf("join_")===0}),"browsing does not join voice")
      var joins=test.visibleItems(page,"joinConversationVoice",[]).filter(function(button) { return button.conversationId==="local::spot:quiet" })
      test.check(joins.length===1,"selected room offers one explicit voice action")
      if(joins.length) {
        var workspace=joins[0]
        while (workspace.parent && typeof workspace.currentId === "undefined") workspace=workspace.parent
        var options=test.compact ? test.object(page,"trayChatOptions",[]) : test.find(workspace,"chatOptionsButton")
        if(options) test.check(Math.abs(joins[0].mapToItem(page,0,0).y-options.mapToItem(page,0,0).y)<theme.space(8),"alternate voice action sits beside chat options")
      }
      if(!joins.length) {console.log("ROOM_FLOW_FAILED");Qt.quit();return}
      test.click(joins[0]);input.wait(50)
      test.check(test.last().name==="join_spot" && test.last().args.spot_id==="quiet" && test.last().args.server_id==="local","explicit join targets selected room")
      var directJoin=test.find(page,"joinRoom-lounge"), selectedBefore=bridge.activeConversationId
      test.check(directJoin.visible,"join shortcut is visible on the room row")
      test.click(directJoin); input.wait(50)
      test.check(test.last().name==="join_spot" && test.last().args.spot_id==="lounge" && bridge.activeConversationId===selectedBefore,"room-row join works directly without switching chat")
      var firstPerson=test.find(lounge,"roomParticipant-self"), secondPerson=test.find(lounge,"roomParticipant-friend")
      test.check(firstPerson.visible && secondPerson.visible && firstPerson.y===secondPerson.y,"participants are visible together without expanding")
      var beforePerson = bridge.sent.length
      test.click(secondPerson); input.wait(80)
      var participantMenu=test.object(lounge,"participantMenu",[])
      test.check(participantMenu && participantMenu.visible,"clicking a participant opens their controls")
      test.check(bridge.sent.length===beforePerson,"participant click does not open room chat or join voice")
      var volume=test.find(participantMenu.contentItem,"participantMenuVolume")
      volume.value=145;volume.moved();input.wait(30)
      var person={id:"friend",server_id:"local",display_name:"Jared"}
      test.check(bridge.participantVolumes.volumeFor(person)===145,"participant slider saves local volume")
      test.find(participantMenu.contentItem,"participantLocalMute").clicked();input.wait(30)
      test.check(bridge.participantVolumes.isMuted(person) && bridge.participantVolumes.effectiveVolumeFor(person)===0,"local mute silences only this participant")
      test.check(test.find(secondPerson,"participantLocalMuteStatus").visible && !test.find(secondPerson,"participantMicrophoneStatus").visible,"local mute uses a speaker indicator without implying a muted microphone")
      test.find(participantMenu.contentItem,"participantLocalMute").clicked();input.wait(30)
      test.check(bridge.participantVolumes.volumeFor(person)===145 && bridge.participantVolumes.effectiveVolumeFor(person)===145,"unmute preserves previous volume")
      test.check(!bridge.participantVolumes.isMuted({id:"friend",server_id:"second"}),"local mute is scoped to server identity")
      test.check(test.find(participantMenu.contentItem,"participantServerMute").visible,"server admins receive moderation controls")
      test.find(participantMenu.contentItem,"participantServerMute").clicked()
      test.check(test.last().name==="moderate_voice" && test.last().args.server_id==="local" && test.last().args.user_id==="friend" && test.last().args.muted===true,"server mute targets the selected server and person")
      test.screenshot("participant-menu",participantMenu.contentItem.parent);input.wait(40)
      var messageAction=test.find(participantMenu.contentItem,"participantMessage")
      messageAction.clicked();input.wait(40)
      test.check(String(bridge.workspaceLayout.chatTiles).indexOf("dm:friend")>=0,"participant message opens a new DM tile by default")
      test.click(firstPerson);input.wait(40)
      test.check(test.find(participantMenu.contentItem,"participantSelfDeafen").visible && !test.find(participantMenu.contentItem,"participantLocalMute").visible,"self menu has deafen instead of per-person local mute")
      participantMenu.close()
      test.check(!test.find(lounge,"roomParticipantsToggle"),"room has no participant disclosure control")
      var data=fixture();data.self.hangout_id="active";data.self.media.livekit_connected=true;data.server_states[0].self=data.self
      bridge.applySnapshot(data);input.wait(120)
      lounge=test.find(page,"savedRoom-lounge");quiet=test.find(page,"savedRoom-quiet")
      test.check(test.find(lounge,"roomParticipant-friend").visible,"participants stay visible after snapshots")
      test.check(!test.find(lounge,"joinRoom-lounge").visible,"current room hides redundant join")
      test.check(lounge.y<quiet.y && test.find(lounge,"roomName").text==="#Lounge /2","joining preserves room order and name")
      var bar=test.find(page,"currentCallBar")
      var disconnect=test.find(bar,"currentCallDisconnect"), location=test.find(bar,"currentCallLocation"), connection=test.find(bar,"currentCallConnection")
      test.check(disconnect && Math.abs(disconnect.mapToItem(bar,0,disconnect.height/2).y-location.mapToItem(bar,0,location.height/2).y)<1,"disconnect aligns vertically with the room status")
      test.check(location.y===connection.y && connection.text==="· connected","room and connection status share one line")
      test.check(disconnect.text==="d/c" && !test.find(bar,"mediaAction-leave"),"disconnect appears once in the status row")
      test.check(bar.visible && test.find(bar,"currentCallLocation").text==="Lounge","call area identifies current room")
      test.check(test.visibleItems(page,"mediaAction-share",[]).length===1,"only one set of call controls")
      test.check(bar.mapToItem(page,0,0).y>=quiet.mapToItem(page,0,quiet.height).y,"call controls follow the saved room list")
      data=JSON.parse(JSON.stringify(data))
      data.server_states[0].voice_moderation={friend:{muted:true,deafened:true}}
      bridge.participantVolumes.setMuted(person,true)
      bridge.applySnapshot(data);input.wait(80)
      lounge=test.find(page,"savedRoom-lounge")
      secondPerson=test.find(lounge,"roomParticipant-friend")
      var micStatus=test.find(secondPerson,"participantMicrophoneStatus"), deafenStatus=test.find(secondPerson,"participantDeafenStatus")
      test.check(micStatus.visible && String(micStatus.source).endsWith("microphone-server-muted.svg"),"server mute uses a distinct shield microphone")
      test.check(deafenStatus.visible && String(deafenStatus.source).endsWith("server-deafened.svg"),"server deafen uses a distinct shield headphones icon")
      test.check(test.find(secondPerson,"participantLocalMuteStatus").visible,"local mute remains separately visible during server moderation")
      test.screenshot("moderation",page);input.wait(100)
      data=JSON.parse(JSON.stringify(data));data.server_states[0].voice_moderation={}
      bridge.applySnapshot(data);input.wait(60)
      secondPerson=test.find(test.find(page,"savedRoom-lounge"),"roomParticipant-friend")
      test.check(!test.find(secondPerson,"participantMicrophoneStatus").visible && !test.find(secondPerson,"participantDeafenStatus").visible && test.find(secondPerson,"participantLocalMuteStatus").visible,"server unmute clears server indicators without clearing local mute")
      bridge.participantVolumes.setMuted(person,false);input.wait(30)
      page.toggleSettings();input.wait(60)
      page.goHome();input.wait(60)
      test.check(bar.visible,"returning home restores the nearby call controls")
      test.screenshot(test.compact?"panel":"app",page);input.wait(100)
      data=JSON.parse(JSON.stringify(data))
      data.self.sharing=true; data.self.media.screen_share.active=true; data.self.media.camera.active=true
      bridge.applySnapshot(data);input.wait(100)
      var previews=test.find(page,"localBroadcastPreviews")
      test.check(previews.visible,"local publishing previews remain available")
      test.check(test.find(bar,"mediaAction-share").controlEnabled && test.find(bar,"mediaAction-camera").controlEnabled,"publishing stop actions stay available")
      data=JSON.parse(JSON.stringify(data))
      data.self.sharing=false; data.self.media.screen_share.active=false; data.self.media.camera.active=false
      bridge.applySnapshot(data);input.wait(50)
      bridge.selectServer("second");input.wait(100)
      lounge=test.find(page,"savedRoom-lounge")
      test.check(!lounge.current,"matching room IDs on another server do not show current voice")

      test.check(test.find(bar,"currentCallLocation").text==="Home / Lounge","browsing another server keeps original call location")
      open=test.find(page,"openRoom-lounge");test.click(open);input.wait(100)
      joins=test.visibleItems(page,"joinConversationVoice",[]).filter(function(button) { return button.conversationId==="second::spot:lounge" })
      test.check(joins.length>=1,"other server offers join even when call IDs match")
      if(joins.length) joins[0].clicked()
      test.check(test.last().name==="join_spot" && test.last().args.server_id==="second","chat voice join uses conversation server")
      bridge.selectConversation("second::dm:friend"); bridge.selectServer("local"); input.wait(100)
      var call=test.visibleItems(page,"callConversation",[]).filter(function(button) { return button.conversationId==="second::dm:friend" })[0]
      test.check(call && call.visible && call.enabled && call.text==="call","DM header offers a call action")
      if(call)test.click(call)
      test.check(test.last().name==="join_friend" && test.last().args.server_id==="second" && test.last().args.friend==="other","DM call targets its server and peer despite navigation selection")
      data=JSON.parse(JSON.stringify(data));data.server_states[1].friends[0].presence="closed"
      bridge.applySnapshot(data);input.wait(80)
      test.check(!test.visibleItems(page,"callConversation",[]).filter(function(button) { return button.conversationId==="second::dm:friend" })[0].enabled,"DM call respects closed presence")
      before=bridge.sent.length;bridge.callConversation("second::dm:friend")
      test.check(bridge.sent.length===before,"unavailable DM call sends no request")
      data=JSON.parse(JSON.stringify(data));data.server_states[1].friends[0].presence="knock"
      bridge.applySnapshot(data);input.wait(80)
      test.check(test.visibleItems(page,"callConversation",[]).filter(function(button) { return button.conversationId==="second::dm:friend" })[0].enabled,"knock presence allows an explicit call request")
      bridge.selectConversation("second::spot:lounge");bridge.selectServer("second");input.wait(80)
      var invite=test.find(bar,"mediaAction-invite")
      test.click(invite);input.wait(50)
      var popup=test.object(bar,"callInvitePicker",[])
      test.check(popup && popup.opened,"call invite picker opens from the room area")
      if(popup) {
        test.check(popup.y>=0 && popup.y+popup.height<=window.height,"invite menu stays inside the window")
        test.check(!!test.find(popup.contentItem,"callInvite-friend") && !test.find(popup.contentItem,"callInvite-other"),"call invites use voice server friends")
        popup.close()
      }
      test.click(test.find(bar,"currentCallDisconnect"))
      test.check(test.last().name==="leave","leave voice uses the current call command")
      data=JSON.parse(JSON.stringify(data))
      data.self.hangout_id=null;data.self.media.livekit_connected=false;data.server_states[0].self=data.self
      data.server_states[0].spots[0].members=[];data.server_states[0].spots[0].active_hangout_id=null
      data.server_states[0].hangouts=data.server_states[0].hangouts.filter(function(x){return x.id!=="active"})
      bridge.applySnapshot(data);bridge.selectServer("local");input.wait(100)
      lounge=test.find(page,"savedRoom-lounge")
      test.check(!bar.visible && bar.height===0,"call controls release space after leaving")
      test.check(lounge && test.find(lounge,"roomName").text==="#Lounge /0" && lounge.height<=rowHeight,"empty room keeps its name with a zero count")
      before=bridge.sent.length
      var create=test.find(page,"createRoomButton");test.click(create);input.wait(60)
      var manager=test.object(page,"identityRoomManager",[])
      test.check(manager && manager.opened && manager.creating,"plus next to Rooms opens creation")
      test.check(bridge.sent.length===before,"opening room creation does not join or publish")
      if(manager)manager.close()
      test.check(!bridge.sent.some(function(c){return ["share","camera","send_voice_invite"].indexOf(c.name)>=0}),"layout navigation never publishes or invites")
      console.log(test.failed?"ROOM_FLOW_FAILED":"ROOM_FLOW_OK")
      Qt.quit()
    }
  }
}
