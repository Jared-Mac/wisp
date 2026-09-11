import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components
import "app/views" as Views

ShellRoot {
  id: test
  property bool failed:false
  function check(ok,message) {if(!ok){failed=true;console.error("ROOM_ACTIVITY_FAILED: "+message)}}
  function find(item,name) {
    if(!item)return null
    if(item.objectName===name)return item
    for(var child of item.children || []){var result=find(child,name);if(result)return result}
    return null
  }
  function ids(){return bridge.servers.map(function(s){return s.id}).join(",")}
  function snapshot() {
    var data=JSON.parse(JSON.stringify(bridge.snapshot))
    data.self.id="me";data.self.hangout_id=null;data.self.connection="connected"
    data.servers=[{id:"z",name:"Zeta",connected:true},{id:"a",name:"Alpha",connected:true},{id:"b",name:"Beta",connected:true},{id:"c",name:"Cedar",connected:true}]
    data.selected_server_id="z";data.voice_server_id="z"
    data.server_states=data.servers.slice().sort(function(a,b){return a.name.localeCompare(b.name)}).map(function(server){
      return {server:server,self:data.self,friends:[{id:"friend",display_name:"Riley",online:true}],hangouts:[],
        spots:[{id:"lounge",name:"Lounge",active_hangout_id:null,members:[]}],
        conversations:[{id:"chat",kind:"hangout",label:"Lounge",spot_id:"lounge",members:[]}],messages:[],knocks:[],room_invitations:[]}
    })
    return data
  }
  Wisp.WispTheme {id:theme;profile:"soft_graphite"}
  Wisp.WispBridge {
    id:bridge
    property var sent:[]
    property var sounds:[]
    function send(name,args){sent.push({name:name,args:args});return "fixture-"+(++requestId)}
    function playNotificationSound(kind){sounds.push(kind)}
  }
  FloatingWindow {
    id:window;visible:true;implicitWidth:520;implicitHeight:900;color:theme.background
    Components.ServerSelector {id:selector;width:480;x:20;y:10;bridge:bridge;theme:theme;showInvite:false}
    ScrollView {x:20;y:70;width:480;height:800;clip:true
      Views.NotificationSettingsView {id:settings;width:480;bridge:bridge;theme:theme}
    }
  }
  TestCase {id:input;parent:window.contentItem;when:false}
  Component.onCompleted:bridge.applySnapshot(snapshot())
  Timer {
    interval:500;running:true
    onTriggered:{
      var prefs=bridge.serverPreferences
      if(Quickshell.env("WISP_ROOM_ACTIVITY_RELOAD")==="1") {
        test.check(test.ids()==="b,z,a,c","Custom home-group ordering survives a process restart")
        test.check(prefs.homeIds.slice().sort().join(",")==="b,z","Multiple homes survive a restart")
        test.check(bridge.friendRoomNotificationTiming==="always" && bridge.friendRoomCooldown===7 && bridge.friendRoomNotificationSound,"Alert preferences survive a restart")
        var another=test.snapshot()
        another.self.id="another-account"
        another.server_states.forEach(function(s){s.self.id="another-account"})
        bridge.applySnapshot(another);input.wait(150)
        test.check(test.ids()==="z,a,b,c" && prefs.homeIds.join(",")==="z","Another primary account gets its own defaults")
        bridge.applySnapshot(test.snapshot());input.wait(150)
        test.check(test.ids()==="b,z,a,c","Switching back retains the original account's order")
      } else {
        test.check(test.ids()==="z,a,b,c","Initial server order is order joined, not alphabetical state order")
        test.check(prefs.homeIds.join(",")==="z","First joined server is home by default")
        var combo=test.find(selector,"activeServerSelector")
        combo.popup.open();input.wait(150)
        var home=test.find(combo.popup.contentItem,"homeServer-b")
        test.check(!!home,"Dropdown has home controls")
        if(home)input.mouseClick(home,home.width/2,home.height/2)
        input.wait(150)
        test.check(test.ids()==="z,b,a,c","Additional home moves above ordinary servers")
        var up=test.find(combo.popup.contentItem,"moveServerUp-b")
        if(up)input.mouseClick(up,up.width/2,up.height/2)
        input.wait(100)
        test.check(test.ids()==="b,z,a,c","Move button reorders homes")
        test.check(bridge.activeServer.id==="z","Reordering does not switch the selected server")
        var grip=test.find(combo.popup.contentItem,"dragServer-a")
        test.check(!!grip,"Dropdown has drag handles")
        if(grip){input.mousePress(grip,grip.width/2,grip.height/2);input.mouseMove(grip,grip.width/2,grip.height/2+36,100);input.mouseRelease(grip,grip.width/2,grip.height/2+36)}
        input.wait(150)
        test.check(test.ids()==="b,z,c,a","Dragging reorders the non-home group")
        prefs.moveBy("a",-1);input.wait(80)
        test.check(test.ids()==="b,z,a,c","Movement never crosses the home boundary")
        combo.popup.close()
        test.find(settings,"roomActivitySection").expanded=true
        input.wait(100)
        test.check(test.find(settings,"friendRoomTimingSetting").count===3,"Notification timing choices are available")
        bridge.friendRoomNotificationTiming="always";bridge.friendRoomCooldown=7;bridge.friendRoomNotificationSound=true
        bridge.notificationSoundsEnabled=true
        var before=test.snapshot(),after=JSON.parse(JSON.stringify(before))
        var state=after.server_states.filter(function(s){return s.server.id==="b"})[0]
        state.spots[0].active_hangout_id="voice";state.spots[0].members=[{id:"friend",display_name:"Riley"}]
        bridge.applySnapshot(before)
        bridge.applySnapshot(after,"hangout_changed")
        for(var i=0;i<30 && !bridge.pendingConversationTiles.length;i++)input.wait(50)
        test.check(!bridge.roomActivity.error,"Desktop notification process succeeds")
        test.check(bridge.pendingConversationTiles.some(function(c){return c.id==="b::chat"}),"Clicking the desktop alert opens the room chat")
        test.check(bridge.sounds.filter(function(s){return s==="friend_room_join"}).length===1,"Optional room alert sound plays once")
        input.wait(100)
        test.check(bridge.roomActivity.pending.count===0,"Dismissed notifications release their process")
        bridge.applySnapshot(before,"hangout_changed");bridge.applySnapshot(after,"hangout_changed");input.wait(100)
        test.check(bridge.sounds.filter(function(s){return s==="friend_room_join"}).length===1,"Cooldown suppresses another banner and sound")
        test.check(!bridge.sent.some(function(c){return ["join_spot","join_hangout","share","camera"].indexOf(c.name)>=0}),"Notifications and ordering never join voice or publish")
        input.wait(200)
      }
      if(!test.failed)console.log("ROOM_ACTIVITY_OK")
      Qt.quit()
    }
  }
}
