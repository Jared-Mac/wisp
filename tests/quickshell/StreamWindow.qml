import QtQuick
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components
import "app/ChatTiles.js" as Tiles

ShellRoot {
  id: test
  property bool failed: false
  property var tile: null
  property string videoKey: ""
  property var volumePopup: null
  function findObject(item, name, visited) {
    if (!item || visited.indexOf(item)>=0) return null
    visited.push(item)
    if (item.objectName===name) return item
    for(var child of item.data || item.children || []) { var found=findObject(child,name,visited); if(found) return found }
    return null
  }
  function check(ok, message) { if (!ok) { failed=true; console.error("LOCAL_TEST_FAILED: " + message) } }
  function find(item, name) {
    if (!item) return null
    if (item.objectName===name) return item
    for(var child of item.children || []) { var found=find(child,name); if(found) return found }
    return null
  }
  function changeMedia(speakers,level) {
    var next=JSON.parse(JSON.stringify(bridge.snapshot))
    next.self.media.active_speakers=speakers; next.self.media.remote_audio_levels={Owner:level}
    bridge.snapshot=next
  }
  Wisp.WispAppearance { id: appearance; environment: "desktop"; Component.onCompleted: setPalette("performative") }
  Wisp.WispTheme { id: theme; profile: "performative"; appearanceController: appearance }
  Wisp.WispBridge {
    id: bridge
    mainWindowOpen: true
    property var sent: []
    property int soundCount: 0
    notificationSoundsEnabled: true
    function playNotificationSound() { soundCount++ }
    function send(name,args) { sent.push({name:name,args:args}); return "test-"+(++requestId) }
  }
  Wisp.WispWindow { id: window; bridge: bridge; theme: theme; visible: true; implicitWidth: Number(Quickshell.env("WISP_TEST_WIDTH")) || 1180; implicitHeight: 800 }
  FloatingWindow {
    id: panel; visible: true; implicitWidth: 460; implicitHeight: 800
    Wisp.WispContent { id: tray; anchors.fill: parent; bridge: bridge; theme: theme; presentation: "panel" }
  }
  TestCase { id: keys; parent: window.contentItem; when: false }
  Components.ParticipantVolumeMenu { id: roomVolumes; parent: window.contentItem; bridge: bridge; theme: theme; people: bridge.hangouts.length ? bridge.hangouts[0].members : [] }
  Component.onCompleted: {
    var data=JSON.parse(JSON.stringify(bridge.snapshot))
    data.self.id="self"; data.self.display_name="MemberA"; data.self.hangout_id="test_room"; data.self.muted=false
    data.self.media.remote_audio_participants=["Owner"]
    data.friends=[{id:"owner",display_name:"Owner",online:true,presence:"open"},{id:"member_c",display_name:"MemberC",online:false,presence:"away"}]
    data.hangouts=[{id:"test_room",label:"TestRoom",members:[{id:"self",display_name:"MemberA"},{id:"owner",display_name:"Owner"},{id:"member_c",display_name:"MemberC"}]}]
    data.conversations=[{id:"test_room",label:"TestRoom",kind:"spot",spot_id:"test_room",unread_count:0},{id:"dm",label:"Owner",kind:"direct",unread_count:12}]
    data.messages=[]
    data.self.media.remote_videos=[{participant:"Owner",source:"screen_share",subscribed:true}]
    bridge.applySnapshot(data); bridge.selectConversation("test_room")
  }
  Timer {
    interval: 300; running: true
    onTriggered: {
      test.tile=test.find(window.contentItem,"conversationPane")
      test.tile.openVideo({participant:"Owner",source:"screen_share",presentation:"window"})
      var videos=Tiles.leaves(test.tile.tree).filter(function(n){return !!test.tile.videoFor(n.id)})
      test.videoKey=videos[0].key
      test.check(test.tile.detachedKeys.indexOf(test.videoKey)>=0,"explicit watch opens dedicated window with main open")
      test.tile.openVideo({participant:"Owner",source:"screen_share",presentation:"window"})
      test.check(Tiles.leaves(test.tile.tree).filter(function(n){return !!test.tile.videoFor(n.id)}).length===1,"repeat watch reuses viewer")
      test.tile.openVideo({participant:"Owner",source:"screen_share",presentation:"tile"})
      test.check(test.tile.detachedKeys.indexOf(test.videoKey)<0,"explicit app choice docks viewer")
      test.tile.closePane(test.videoKey)
      bridge.mainWindowOpen=false
      test.tile.openVideo({participant:"Owner",source:"screen_share",presentation:"window"})
      test.videoKey=Tiles.leaves(test.tile.tree).filter(function(n){return !!test.tile.videoFor(n.id)})[0].key
      test.check(test.tile.detachedKeys.indexOf(test.videoKey)>=0,"dedicated viewer works with main closed")
      test.check(!bridge.mainWindowOpen,"dedicated watch does not reveal app")
      test.check(bridge.workspaceLayout.chatTiles.indexOf("video:")<0,"watch not persisted")
    }
  }
  Timer {
    interval: 900; running: true
    onTriggered: {
      test.check(test.tile.detachedKeys.indexOf(test.videoKey)>=0,"viewer stays detached")
      test.tile.closePane(test.videoKey)
      test.check(bridge.sent.some(function(c){return c.name==="watch_video" && c.args.open===false}),"close stops subscription")
      console.log(test.failed ? "STREAM_WINDOW_FAILED" : "STREAM_WINDOW_OK")
      Qt.quit()
    }
  }
}
