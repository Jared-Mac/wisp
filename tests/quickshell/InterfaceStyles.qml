import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp

ShellRoot {
  id: test
  property bool failed: false
  function check(ok, description) { if (!ok) { failed = true; console.error("INTERFACE_FAILED " + description) } }
  function find(item, name) {
    if (!item) return null
    if (item.objectName === name) return item
    for (var child of item.children || []) { var result = find(child, name); if (result) return result }
    return null
  }
  function capture(name) {
    var directory = Quickshell.env("WISP_INTERFACE_SCREENSHOTS")
    if (directory) canvas.grabToImage(function(result) { result.saveToFile(directory + "/" + name + ".png") })
    input.wait(100)
  }
  Wisp.WispAppearance { id: appearance; environment: "desktop" }
  Wisp.WispTheme { id: theme; appearanceController: appearance; profile: appearance.profile }
  Wisp.WispBridge {
    id: bridge
    property var sent: []
    function send(name, args) { sent.push({name:name,args:args}); return "fixture-" + (++requestId) }
  }
  FloatingWindow {
    id: window; visible: true; implicitWidth: Number(Quickshell.env("WISP_TEST_WIDTH")) || 1180; implicitHeight: 800; color: theme.background
    Rectangle {
      id: canvas; anchors.fill: parent; color: theme.background
      Wisp.WispContent { id: page; anchors.fill: parent; theme: theme; bridge: bridge; presentation: "app" }
    }
  }
  TestCase { id: input; parent: window.contentItem; when: false }
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
    interval: 450; running: true
    onTriggered: {
      test.check(appearance.profile === "soft_graphite" && appearance.palette === "soft_graphite", "fresh desktop defaults to Soft Graphite")
      var pane = test.find(page,"conversationPane")
      pane.addConversation(pane.activeKey,"local::dm:riley"); input.wait(100)
      var before = bridge.sent.length
      for (var style of ["soft_graphite","daylight","hearth"]) {
        appearance.setProfile(style); input.wait(80)
        test.check(test.find(page,"wispWordmark").visible, "original Wisp branding remains visible in " + style)
        for (var w of [Number(Quickshell.env("WISP_TEST_WIDTH")) || 1180]) {
          input.wait(100)
          test.check(canvas.width === w, "fixture resized to " + w)
          var bar = test.find(page,"currentCallBar"), rooms = test.find(page,"roomsPane"), friends = test.find(page,"friendsPane")
          var pos = bar.mapToItem(page,0,0), friendPos = friends.mapToItem(page,0,0)
          test.check(bar.visible && pos.y + bar.height <= friendPos.y, "voice controls remain above friends at " + style + "/" + w)
          var mute = test.find(bar,"mediaAction-mute"), camera = test.find(bar,"mediaAction-camera")
          test.check(mute.visible && mute.width >= 32 && camera.mapToItem(bar,0,0).y + camera.height <= bar.height, "all voice controls fit at " + style + "/" + w)
          var watch = test.find(page,"roomParticipants")
          test.check(watch.visible && watch.height > 40, "participants stay visible on separate rows")
          test.check(pane.paneCount === 2, "both chats survive appearance changes")
          test.capture(style + "-" + w)
        }
        if (Quickshell.env("WISP_TEST_ADAPTIVE") === "1") {
          var rail = test.find(page,"activityPane"), divider = test.find(page,"activityResizeHandle")
          for (var dock of ["left","right"]) {
            bridge.workspaceLayout.dock = dock
            for (var railWidth of [24,48,88,140,200,360]) {
              bridge.workspaceLayout.activityWidth = railWidth; input.wait(80)
              test.check(Math.abs(rail.width-theme.space(railWidth)) < 1,"sidebar honors width " + railWidth)
              var controls = test.find(page,"currentCallBar")
              for (var action of ["mute","deafen","share","camera"]) {
                var button = test.find(controls,"mediaAction-"+action), p = button.mapToItem(rail,0,0)
                test.check(button.visible && button.width >= theme.space(18) && p.x >= 0 && p.x+button.width <= rail.width+1,"reachable "+action+" at "+railWidth)
              }
              var drop = test.find(page,"activeServerSelector"), gear = test.find(page,"serverSettingsShortcut")
              var friend = test.find(page,"friendName")
              test.check(!friend.visible || friend.mapToItem(rail,0,0).x + friend.width <= rail.width + 1,"friend names fit at " + railWidth)
              test.check(drop.width >= theme.space(18) && gear.width <= rail.width,"server controls fit at "+railWidth)
              drop.popup.open(); input.wait(30)
              test.check(drop.popup.width >= theme.space(220),"server menu remains readable")
              drop.popup.close()
              if (dock === "left" && [24,140,200].indexOf(railWidth)>=0) test.capture(style+"-rail-"+railWidth)
            }
            divider.moved(dock === "right" ? 10000 : -10000); input.wait(30)
            test.check(rail.width === theme.space(24) && !bridge.workspaceLayout.activityCollapsed,"drag clamps to visible minimum")
            bridge.workspaceLayout.activityCollapsed = true; input.wait(20)
            test.check(!rail.visible,"explicit collapse hides sidebar")
            bridge.workspaceLayout.activityCollapsed = false; input.wait(20)
            test.check(rail.visible && rail.width === theme.space(24),"expand restores saved rail")
          }
          appearance.setShowAvatars(false); input.wait(50)
          test.check(!theme.showAvatars,"avatar visibility preference applies")
          appearance.setShowAvatars(true)
          bridge.workspaceLayout.dock = "left"; bridge.workspaceLayout.activityWidth = 0; input.wait(60)
        }
        page.toggleSettings(); input.wait(40)
        for (var tab of ["profile","media","video","appearance","notifications","privacy","devices"]) {
          test.find(page,"settingsTab-" + tab).clicked(); input.wait(40)
          if (tab === "appearance") test.capture(style + "-appearance")
          test.check(test.find(page,"settingsTab-" + tab).primary, "settings category remains reachable: " + tab)
        }
        test.find(page,"settingsTab-profile").clicked(); input.wait(30)
        var passwordSection = test.find(page,"profilePasswordSection")
        if (!passwordSection.expanded) {
          var header = passwordSection.children.find(function(item) { return typeof item.clicked === "function" })
          header.forceActiveFocus(); input.keyClick(Qt.Key_Space); input.wait(30)
          test.check(passwordSection.expanded, "advanced sections open with the keyboard")
        }
        page.goHome(); input.wait(50)
      }
      test.check(!bridge.sent.slice(before).some(function(command) { return /^(join|leave|share|camera|send_message|knock)/.test(command.name) }), "presentation changes never publish or change voice membership")
      console.log(test.failed ? "INTERFACE_FAILED" : "INTERFACE_OK")
      Qt.quit()
    }
  }
}
