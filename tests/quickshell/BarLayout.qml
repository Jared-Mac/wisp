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
  Wisp.WispAppearance { id: appearance; environment: "omarchy"; managed: !Quickshell.env("WISP_TEST_THEME") }
  Wisp.WispTheme { id: theme; appearanceController: appearance; profile:Quickshell.env("WISP_TEST_THEME") || "legacy"; tuiTreatment:appearance.managed }
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
    Wisp.WispContent {
      id:desktop;visible:false;width:1100;height:800;bridge:bridge;theme:theme
      logoSource:Qt.resolvedUrl("app/assets/waveform.svg");presentation:"app"
    }
  }
  TestCase { id: input; parent:window.contentItem; when:false }
  Component.onCompleted: {
    bridge.workspaceLayout.trayChatFocused=false
    if(Quickshell.env("WISP_TEST_THEME"))appearance.setPalette(Quickshell.env("WISP_TEST_THEME"))
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
    conversations[0].unread_count=1; conversations[0].last_message=data.server_states[0].messages[1]
    conversations[2].unread_count=1; conversations[2].last_message=data.server_states[0].messages[2]
    bridge.applySnapshot(data)
    bridge.selectConversation("local::spot:lounge")
  }
  Timer {
    interval:500; running:true
    onTriggered: {
      page.appButtonText="Install Wisp"; input.wait(30)
      var appButton=test.find(page,"headerOpenAppButton")
      test.check(appButton && appButton.visible && !appButton.iconOnly && appButton.text==="Install Wisp","Missing client shows a labeled installation button")
      page.appButtonText="Open app"
      bridge.friendships.put("local",{ready:true,loading:false,people:bridge.friends.map(function(friend){return Object.assign({},friend,{relationship:"friend",server_member:true})})})
      input.wait(30)
      var unread=test.find(page,"unreadChat-local::dm:riley")
      test.check(!!unread && unread.visible,"Popup offers unread conversation navigation")
      if(unread)input.mouseClick(unread,unread.width/2,unread.height/2)
      input.wait(150)
      test.check(bridge.activeConversationId==="local::dm:riley","Unread button opens the requested conversation")
      test.check(bridge.sent.some(function(c){return c.name==="mark_conversation_read" && c.args.server_id==="local" && c.args.conversation_id==="dm:riley"}),"Popup unread button acknowledges the displayed messages")
      test.check(bridge.pendingCount("local::dm:riley")===0 && bridge.pendingCount("local::spot:lounge")===1,"Opening one unread chat clears only its own badge")
      bridge.selectConversation("local::spot:lounge"); input.wait(100)
      for (var size of [Qt.size(960,560),Qt.size(800,560),Qt.size(680,460)]) {
        test.fixtureWidth=size.width; test.fixtureHeight=size.height; input.wait(100)
        test.check(page.width===size.width,"Fixture resized to "+size.width)
        var workspace=test.find(page,"barWorkspace")
        var rooms=test.find(page,"barRoomsPane"), chat=test.find(page,"barChatPane"), friends=test.find(page,"barFriendsPane")
        test.check(!!workspace && page.landscapePanel,"Wide popup uses the horizontal workspace")
        test.check(rooms.y===chat.y && chat.y===friends.y && rooms.x+rooms.width<chat.x && chat.x+chat.width<friends.x,"Rooms, chat and friends occupy separate columns")
        test.check(chat.width>=size.width*0.42,"Chat receives the largest column")
        var footer=test.find(page,"trayAudioFooter"),footerAt=footer.mapToItem(workspace,0,0)
        test.check(footerAt.y>=rooms.height && footer.mapToItem(page,0,0).y+footer.height<=page.height,"Audio toolbar sits below every column")
        for (var name of ["muteControl","deafenControl","audioSoundboardButton","currentCallDisconnect","mediaAction-share","mediaAction-camera"]) {
          var button=test.find(footer,name), pos=button.mapToItem(footer,0,0)
          test.check(button.visible && pos.x>=0 && pos.y>=0 && pos.x+button.width<=footer.width+1 && pos.y+button.height<=footer.height+1,"Toolbar keeps "+name+" reachable at "+size.width)
        }
        test.check(!test.find(page,"headerSoundboardButton"),"Soundboard has one shortcut in the audio strip")
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
      test.fixtureWidth=960;test.fixtureHeight=560;input.wait(80)
      for(var compactSize of [Qt.size(460,800),Qt.size(320,600)]) {
        test.fixtureWidth=compactSize.width;test.fixtureHeight=compactSize.height;input.wait(100)
        var footer=test.find(page,"trayAudioFooter"),scroller=test.find(page,"dashboardScroll"),footerPosition=footer.mapToItem(page,0,0)
        test.check(footerPosition.y+footer.height<=page.height && footerPosition.y>page.height-theme.space(50),"default tray audio is anchored to the bottom")
        for(var controlName of ["muteControl","deafenControl","audioSoundboardButton","mediaAction-share","mediaAction-camera","currentCallDisconnect"]) {
          var control=test.find(footer,controlName),location=control.mapToItem(footer,0,0)
          test.check(location.y===0 && location.x+control.width<=footer.width+1,"default tray controls fit one row")
        }
        scroller.contentY=100;input.wait(20)
        test.check(footer.mapToItem(page,0,0).y===footerPosition.y,"tray audio does not scroll with the room list")
        scroller.contentY=0
        var screenshot=Quickshell.env("WISP_BAR_SCREENSHOTS")
        if(screenshot){page.grabToImage(function(result){result.saveToFile(screenshot+"/default-"+compactSize.width+".png")});input.wait(100)}
      }
      test.fixtureWidth=960;test.fixtureHeight=560;input.wait(80)
      var savedFriends=bridge.friendPreferences.trayCollapsed,savedMembers=bridge.friendPreferences.trayMembersCollapsed
      test.find(page,"trayChatFocusToggle").clicked();input.wait(120)
      test.check(page.trayChatFocused && !desktop.trayChatFocused && !test.find(desktop,"trayChatFocusToggle"),"chat focus is tray-only")
      test.check(!test.find(page,"barRoomsPane") && !test.find(page,"barFriendsPane"),"focus mode removes the surrounding columns")
      test.check(test.find(page,"trayComposerEditor").text==="Draft survives popup resizing","focus toggle preserves the chat draft")
      for(var focusSize of [Qt.size(960,560),Qt.size(460,700),Qt.size(320,600)]) {
        test.fixtureWidth=focusSize.width;test.fixtureHeight=focusSize.height;input.wait(120)
        var focused=test.find(page,"trayFocusedWorkspace"),focusChat=focused,audioRow=test.find(page,"trayAudioRow")
        test.check(focusChat.width===focused.width && focusChat.height>focused.height*0.85,"chat fills the focused tray at "+focusSize.width)
        for(var action of ["muteControl","deafenControl","audioSoundboardButton","mediaAction-share","mediaAction-camera","currentCallDisconnect"]) {
          var control=test.find(audioRow,action),at=control.mapToItem(audioRow,0,0)
          test.check(at.y===0 && at.x>=0 && at.x+control.width<=audioRow.width+1,"all audio buttons fit one row at "+focusSize.width)
        }
        var focusButton=test.find(page,"trayChatFocusToggle"),pin=test.find(page,"chatPinsButton")
        test.check(Math.abs(focusButton.mapToItem(pin.parent,0,0).x+focusButton.width+theme.spacing.sm-pin.x)<1,"focus toggle sits beside pins")
        var screenshot=Quickshell.env("WISP_BAR_SCREENSHOTS")
        if(screenshot){page.grabToImage(function(result){result.saveToFile(screenshot+"/focus-"+focusSize.width+".png")});input.wait(100)}
      }
      test.fixtureWidth=960;test.fixtureHeight=560;input.wait(100)
      for(var peekName of ["trayVoicePeek","trayFriendsPeek","trayRoomsPeek"]) {
        var trigger=test.find(page,peekName)
        input.mouseMove(trigger,trigger.width/2,trigger.height/2);input.wait(600)
        test.check(trigger.popup.opened && trigger.popup.height>40,"hover opens "+peekName)
        if(trigger.popup.opened) {
          if(peekName==="trayVoicePeek") {
            var room=test.find(trigger.popup.contentItem,"savedRoom-lounge")
            test.check(room && room.current && room.room.name==="Lounge" && room.people.length===2,"voice hover shows the connected room and its participants")
            test.check(test.find(room,"joinRoom-lounge").text==="inv","voice hover offers an invite instead of rejoining")
          }
          input.mouseMove(trigger.popup.contentItem,20,20);input.wait(300)
          test.check(trigger.popup.opened,"hover panel stays open while its contents are used")
          var screenshot=Quickshell.env("WISP_BAR_SCREENSHOTS")
          if(screenshot){trigger.popup.contentItem.grabToImage(function(result){result.saveToFile(screenshot+"/"+peekName+".png")});input.wait(100)}
        }
        trigger.popup.close()
      }
      // Layer-shell panels recreate their backing window when reopened.
      // Exercise clicks and loaded content, not only the popup's opened flag.
      for(var cycle=0;cycle<3;cycle++) {
        window.visible=false;input.wait(100);window.visible=true;input.wait(150)
        for(var menuName of ["trayRoomsPeek","trayFriendsPeek"]) {
          var menuButton=test.find(page,menuName)
          input.mouseClick(menuButton,menuButton.width/2,menuButton.height/2);input.wait(150)
          test.check(menuButton.popup.opened && menuButton.popup.height>40,"click opens "+menuName+" after reopening the tray")
          var expected=menuName==="trayRoomsPeek" ? "savedRoom-quiet" : "favorite-riley"
          var menuEntry=test.find(menuButton.popup.contentItem,expected)
          test.check(!!menuEntry && menuEntry.visible && menuEntry.width>0,"reopened menu lists "+expected)
          if(menuName==="trayRoomsPeek" && menuEntry) {
            var join=test.find(menuEntry,"joinRoom-quiet")
            test.check(join && join.visible && join.enabled,"room picker offers an available Join button")
          }
          menuButton.popup.close();input.mouseMove(window.contentItem,10,10);input.wait(50)
        }
      }
      test.find(page,"trayChatFocusToggle").clicked();input.wait(120)
      test.check(!!test.find(page,"barWorkspace") && test.find(page,"trayComposerEditor").text==="Draft survives popup resizing","leaving focus restores the layout and draft")
      test.check(bridge.friendPreferences.trayCollapsed===savedFriends && bridge.friendPreferences.trayMembersCollapsed===savedMembers,"focus mode preserves section choices")
      var before=bridge.sent.length
      var sound=test.find(test.find(page,"trayAudioFooter"),"audioSoundboardButton")
      input.mouseMove(sound,sound.width/2,sound.height/2); input.mouseClick(sound,sound.width/2,sound.height/2); input.wait(80)
      var menu=input.findChild(sound.parent,"soundboardPopup")
      test.check(menu && menu.opened,"Bottom shortcut opens the soundboard")
      if(menu)menu.close()
      test.check(bridge.sent.slice(before).every(function(c){return c.name==="soundboard_list" || c.name==="soundboard_status"}),"Opening the soundboard never joins or publishes")
      test.check(!bridge.sent.some(function(c){return ["join_spot","join_hangout","leave","share","camera"].indexOf(c.name)>=0}),"Layout changes preserve the voice session")
      test.fixtureWidth=960; input.wait(80)
      var disconnected=JSON.parse(JSON.stringify(bridge.snapshot))
      disconnected.self.hangout_id=null
      for(var server of disconnected.server_states || [])server.self.hangout_id=null
      bridge.applySnapshot(disconnected); input.wait(80)
      var idleFooter=test.find(page,"trayAudioFooter")
      test.check(!idleFooter.inVoice && test.find(idleFooter,"muteControl").visible && test.find(idleFooter,"audioSoundboardButton").visible,"Audio strip remains available outside a voice room")
      test.fixtureWidth=600; input.wait(100)
      test.check(!page.landscapePanel && !test.find(page,"barWorkspace"),"Small screens retain the compact layout")
      if(!test.failed)console.log("BAR_LAYOUT_OK")
      Qt.quit()
    }
  }
}
