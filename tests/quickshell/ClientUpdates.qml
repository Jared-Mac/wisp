import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp
import "app/views" as Views
import "app/SettingsSearch.js" as Search

ShellRoot {
  id:test
  property bool failed:false
  function check(ok,label) { if(!ok){failed=true;console.error("UPDATES_FAILED: "+label)} }
  function find(item,name) { if(!item)return null;if(item.objectName===name)return item;var children=item.children || [];for(var i=0;i<children.length;i++){var result=find(children[i],name);if(result)return result}return null }
  Wisp.WispTheme {id:theme;profile:Quickshell.env("WISP_TEST_THEME") || "soft_graphite"}
  QtObject {
    id:bridge
    property var updates:updater
    property bool daemonConnected:true
    property string clientName:"updates-fixture"
    property bool appFocused:true
    property var selfState:({media:{}})
    property var voiceRecovery:({pending:false,shuttingDown:false})
    property var watchedMedia:({})
    property var mediaWatchRequests:({})
    property var drafts:({})
    property var pendingAttachments:({})
    property var sendingConversations:({})
    property var importingConversations:({})
    property var savingFiles:({})
    property var messageActions:({replies:{}})
    property bool privacyBusy:false
    property bool profileBusy:false
    property bool serverSettingsBusy:false
    property bool audioTestBusy:false
    property var audioTestState:({phase:"idle"})
    property var soundboard:({pending:false,playing:false,previewing:false,busy:{}})
  }
  Wisp.WispUpdates {id:updater;bridge:bridge;initialized:true;ready:true;leasePath:Quickshell.env("WISP_TEST_LEASE");processStart:"fixture"}
  Wisp.WispBridge {id:realBridge}
  FloatingWindow {
    id:window;visible:true;implicitWidth:440;implicitHeight:760;color:theme.background
    Views.UpdatesSettingsView {id:settings;anchors.left:parent.left;anchors.right:parent.right;anchors.top:parent.top;anchors.margins:20;bridge:bridge;theme:theme}
  }
  TestCase {id:input;when:false;parent:window.contentItem}
  Timer {
    interval:600;running:true
    onTriggered:{
      test.check(updater.preferences.automatic && updater.preferences.background_checks && updater.preferences.check_on_launch && updater.preferences.interval_minutes===5,"defaults")
      test.check(test.find(settings,"checkForUpdates").enabled,"manual check is available")
      test.find(settings,"checkForUpdates").clicked(); input.wait(700)
      test.check(updater.available && !updater.checking,"manual check fetches isolated manifest")
      test.check(test.find(settings,"installUpdate").visible && test.find(settings,"installUpdate").enabled,"manual Update button")
      bridge.drafts={chat:"unfinished"};input.wait(30)
      test.check(!updater.safe && !test.find(settings,"installUpdate").enabled,"draft blocks install")
      bridge.drafts={};bridge.selfState={hangout_id:"room",media:{livekit_connected:true}};input.wait(30)
      test.check(!updater.safe,"voice blocks install")
      bridge.selfState={media:{}};bridge.messageActions={replies:{chat:{message_id:"reply"}}};input.wait(30)
      test.check(!updater.safe,"reply-only draft blocks install")
      bridge.messageActions={replies:{}};input.wait(30)
      test.check(updater.safe,"idle client can install")
      updater.setPreference("automatic",false);updater.setPreference("background_checks",false);updater.setPreference("check_on_launch",false);updater.setPreference("interval_minutes",30);input.wait(1200)
      test.check(!updater.preferences.automatic && !updater.preferences.background_checks && !updater.preferences.check_on_launch && updater.preferences.interval_minutes===30,"all preferences save independently")
      test.check(test.find(settings,"checkForUpdates").enabled,"manual check stays available with all automatic controls off")
      test.check(Search.search("update interval",false,false).some(function(v){return v.target==="updateInterval"}),"search finds update interval")
      var state=JSON.parse(JSON.stringify(realBridge.snapshot));state.servers=[{id:"local",name:"Fixture",connected:true}];state.selected_server_id="local";state.voice_server_id="local"
      state.self=Object.assign({},state.self,{connection:"connected",hangout_id:"room",media:{livekit_connected:true}})
      state.server_states=[{server:state.servers[0],self:Object.assign({},state.self,{connection:"joining",media:{}}),friends:[],hangouts:[],spots:[],messages:[],conversations:[]}]
      realBridge.applySnapshot(state);input.wait(30)
      test.check(realBridge.selfState.connection==="connected","live media connection overrides stale scoped Joining")
      state=JSON.parse(JSON.stringify(state));state.self=Object.assign({},state.self,{connection:"reconnecting",media:{livekit_connected:false}});realBridge.applySnapshot(state);input.wait(30)
      test.check(realBridge.selfState.connection==="reconnecting","recovery state remains visible")
      if(test.failed){Qt.quit();return}
      if(Quickshell.env("WISP_UPDATES_SCREENSHOT")) settings.grabToImage(function(im){im.saveToFile(Quickshell.env("WISP_UPDATES_SCREENSHOT"));console.log("UPDATES_OK");Qt.quit()})
      else {console.log("UPDATES_OK");Qt.quit()}
    }
  }
}
