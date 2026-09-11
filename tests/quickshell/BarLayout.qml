import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp

ShellRoot {
  id: test
  property bool failed: false
  property real fixtureWidth:960
  property real fixtureHeight:560
  function check(ok, message) { if (!ok) { failed=true; console.error("BAR_LAYOUT_FAILED: "+message) } }
  function find(item, name) {
    if (!item) return null
    if (item.objectName===name) return item
    for (var child of item.children || []) { var found=find(child,name); if(found) return found }
    return null
  }
  Wisp.WispAppearance { id: appearance; environment: "omarchy"; managed: true }
  Wisp.WispTheme { id: theme; appearanceController: appearance; profile:"legacy"; tuiTreatment:true
    foreground:"#c2c5de"; background:"#0d1021"; surface:"#0d1021"; accent:"#aaa0f4"; muted:"#929bb9"; danger:"#f7768e"; cornerRadius:8 }
  Wisp.WispBridge {
    id: bridge
    property var sent: []
    function send(name,args) { sent.push({name:name,args:args}); return "fixture-"+(++requestId) }
  }
  FloatingWindow {
    id: window; visible:true; implicitWidth:960; implicitHeight:560
    color: theme.background
    Wisp.WispContent {
      id: page; width:test.fixtureWidth; height:test.fixtureHeight; bridge:bridge; theme:theme
      logoSource:Qt.resolvedUrl("app/assets/waveform.svg")
      presentation:"panel"; horizontalPanel:true; contentPadding:10; showAppButton:true
    }
  }
  TestCase { id: input; parent:window.contentItem; when:false }
  Component.onCompleted: {
    var data = JSON.parse(JSON.stringify(bridge.snapshot))
    var people = [{id:"self",display_name:"Ash"},{id:"riley",display_name:"Riley"}]
    data.self.id = "self"; data.self.display_name = "Ash"; data.self.connection = "connected"; data.self.presence = "open"
    data.self.hangout_id = "voice"; data.self.server_admin = true; data.self.muted = true; data.self.deafened = false
    data.self.media.livekit_connected = true; data.self.media.remote_videos = [{participant:"Riley",source:"screen_share",subscribed:false}]
    data.self.media.audio.input_devices = [{id:"mic",name:"USB microphone"}]; data.self.media.audio.selected_input_id = "mic"
    data.self.media.audio.output_devices = [{id:"headphones",name:"Headphones"}]; data.self.media.audio.selected_output_id = "headphones"
    var server = {id:"local",name:"Moonlight Club",connected:true}
    data.servers = [server]; data.selected_server_id = "local"; data.voice_server_id = "local"
    var conversations = [{id:"spot:lounge",kind:"hangout",label:"Lounge",spot_id:"lounge",members:people},
      {id:"spot:quiet",kind:"hangout",label:"Quiet corner",spot_id:"quiet",members:[]},
      {id:"dm:riley",kind:"direct",label:"Riley",members:people}]
    data.server_states = [{server:server,self:data.self,
      spots:[{id:"lounge",name:"Lounge",active_hangout_id:"voice",members:people},{id:"quiet",name:"Quiet corner",members:[]}],
      hangouts:[{id:"voice",label:"Lounge",members:people}],conversations:conversations,
      friends:[{id:"riley",display_name:"Riley",online:true,presence:"open"},{id:"sam",display_name:"Sam",online:true,presence:"knock"},{id:"june",display_name:"June",online:false,presence:"away"}],
      messages:[{id:"one",conversation_id:"spot:lounge",sender:people[1],created_at:"2026-09-09T20:10:00Z",content_type:"text/plain",payload:"Anyone up for a quiet game night?"},
        {id:"two",conversation_id:"spot:lounge",sender:people[0],created_at:"2026-09-09T20:11:00Z",content_type:"text/plain",payload:"I'm in! Give me a minute to grab some tea."},
        {id:"three",conversation_id:"dm:riley",sender:people[1],created_at:"2026-09-09T20:12:00Z",content_type:"text/plain",payload:"I saved you a spot. Come say hello whenever you're ready."}],
      knocks:[],devices:[],room_invitations:[]}]
    bridge.applySnapshot(data)
    bridge.selectConversation("local::spot:lounge")
  }
  Timer {
    interval:500; running:true
    onTriggered: {
      for (var size of [Qt.size(960,560),Qt.size(800,560),Qt.size(680,460)]) {
        test.fixtureWidth=size.width; test.fixtureHeight=size.height; input.wait(100)
        test.check(page.width===size.width,"Fixture resized to "+size.width)
        var workspace=test.find(page,"barWorkspace")
        var rooms=test.find(page,"barRoomsPane"), chat=test.find(page,"barChatPane"), friends=test.find(page,"barFriendsPane")
        test.check(!!workspace && page.landscapePanel,"Wide popup uses the horizontal workspace")
        test.check(rooms.y===chat.y && chat.y===friends.y && rooms.x+rooms.width<chat.x && chat.x+chat.width<friends.x,"Rooms, chat and friends occupy separate columns")
        test.check(chat.width>=size.width*0.42,"Chat receives the largest column")
        var footer=test.find(workspace,"sidebarAudioFooter")
        test.check(footer.y>=rooms.height && footer.y+footer.height<=workspace.height+1,"Audio toolbar sits below every column")
        for (var name of ["muteControl","deafenControl","serverSoundboardButton","currentCallDisconnect","mediaAction-share","mediaAction-camera"]) {
          var button=test.find(footer,name), pos=button.mapToItem(footer,0,0)
          test.check(button.visible && pos.x>=0 && pos.y>=0 && pos.x+button.width<=footer.width+1 && pos.y+button.height<=footer.height+1,"Toolbar keeps "+name+" reachable at "+size.width)
        }
        test.check(!test.find(page,"headerSoundboardButton").visible,"Soundboard has one shortcut in the audio strip")
        var editor=test.find(page,"trayComposerEditor")
        test.check(!!editor,"Chat composer remains available")
        if(editor) {
          if(size.width===960) editor.text="Draft survives popup resizing"
          else test.check(editor.text==="Draft survives popup resizing","Resizing preserves the chat draft")
          var box=test.find(chat,"composerMessageBox"), ep=box.mapToItem(chat,0,0)
          test.check(ep.y>=0 && ep.y+box.height<=chat.height+1,"Composer and send button fit in the chat column at "+size.width)
        }
        var screenshot=Quickshell.env("WISP_BAR_SCREENSHOTS")
        if(screenshot) { page.grabToImage(function(result){result.saveToFile(screenshot+"/bar-"+size.width+".png")}); input.wait(100) }
      }
      var before=bridge.sent.length
      var sound=test.find(page,"serverSoundboardButton")
      input.mouseMove(sound,sound.width/2,sound.height/2); input.mouseClick(sound,sound.width/2,sound.height/2); input.wait(80)
      var menu=input.findChild(sound,"soundboardPopup")
      test.check(menu && menu.opened,"Bottom shortcut opens the soundboard")
      if(menu)menu.close()
      test.check(bridge.sent.slice(before).every(function(c){return c.name==="soundboard_list" || c.name==="soundboard_status"}),"Opening the soundboard never joins or publishes")
      test.check(!bridge.sent.some(function(c){return ["join_spot","join_hangout","leave","share","camera"].indexOf(c.name)>=0}),"Layout changes preserve the voice session")
      test.fixtureWidth=960; input.wait(80)
      var disconnected=JSON.parse(JSON.stringify(bridge.snapshot))
      disconnected.self.hangout_id=null
      for(var server of disconnected.server_states || [])server.self.hangout_id=null
      bridge.applySnapshot(disconnected); input.wait(80)
      var idleFooter=test.find(page,"sidebarAudioFooter")
      test.check(!test.find(idleFooter,"currentCallBar").visible && test.find(idleFooter,"muteControl").visible && test.find(idleFooter,"serverSoundboardButton").visible,"Audio strip remains available outside a voice room")
      test.fixtureWidth=600; input.wait(100)
      test.check(!page.landscapePanel && !test.find(page,"barWorkspace"),"Small screens retain the compact layout")
      if(!test.failed)console.log("BAR_LAYOUT_OK")
      Qt.quit()
    }
  }
}
