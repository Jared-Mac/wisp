import QtQuick
import QtTest
import Quickshell
import "app" as Wisp
import "app/ChatTiles.js" as Tiles
import "app/SettingsSearch.js" as Search

ShellRoot {
  id: test
  property bool failed: false
  function check(ok, message) { if (!ok) { failed = true; console.error("STREAM_SEARCH_FAILED: " + message) } }
  function find(item, name) {
    if (!item) return null
    if (item.objectName === name) return item
    for (var child of item.children || []) { var found = find(child, name); if (found) return found }
    return null
  }
  function lastWatch() { return bridge.sent.filter(function(c) { return c.name === "watch_video" }).slice(-1)[0] }
  function videoLeaves(host) { return Tiles.leaves(host.tree).filter(function(n) { return !!host.videoFor(n.id) }) }
  function acknowledgeWatch(ok) {
    var command = lastWatch()
    if (ok) {
      var next = JSON.parse(JSON.stringify(bridge.snapshot))
      next.self.media.remote_videos.forEach(function(video) {
        if (video.participant === command.args.participant && video.source === command.args.source) {
          video.subscribed = command.args.open; video.surface_open = false
        }
      })
      bridge.applySnapshot(next)
    }
    bridge.finishRequest({id:command.id,ok:ok,value:{},error:ok ? null : {message:"Stream unavailable"}})
    input.wait(100)
  }
  function screenshot(name, item) {
    var path = Quickshell.env("WISP_STREAM_SCREENSHOT")
    if (path) { item.grabToImage(function(result) { result.saveToFile(path + "-" + name + ".png") }); input.wait(100) }
  }
  Wisp.WispAppearance { id: appearance; environment: "desktop" }
  Wisp.WispTheme { id: theme; profile: Quickshell.env("WISP_TEST_THEME") || "clean_tui"; appearanceController: appearance }
  Wisp.WispBridge {
    id: bridge; mainWindowOpen: true
    property var sent: []
    function send(name, args) { var id = "test-" + (++requestId); sent.push({id:id,name:name,args:args}); return id }
  }
  FloatingWindow {
    id: window; visible: true; implicitWidth: Number(Quickshell.env("WISP_TEST_WIDTH")) || 920; implicitHeight: 800
    color: theme.background
    Wisp.WispContent { id: page; anchors.fill: parent; bridge: bridge; theme: theme; presentation: "app" }
  }
  TestCase { id: input; parent: window.contentItem; when: false }
  Component.onCompleted: {
    var data = JSON.parse(JSON.stringify(bridge.snapshot))
    var people = [{id:"self",display_name:"Alex"},{id:"friend",display_name:"Riley"}]
    data.self.id = "self"; data.self.display_name = "Alex"; data.self.hangout_id = "active"; data.self.connection = "available"
    data.self.server_admin = true
    data.self.media.remote_videos = [{participant:"Riley",source:"screen_share",subscribed:false},{participant:"Riley",source:"camera",subscribed:false}]
    data.voice_server_id = "local"; data.selected_server_id = "local"
    data.servers = [{id:"local",name:"Home",connected:true},{id:"other",name:"Other",connected:true}]
    var state = {server:data.servers[0],self:data.self,spots:[{id:"lounge",name:"Lounge",active_hangout_id:"active",members:people}],
      hangouts:[{id:"active",label:"Lounge",members:people}],friends:[{id:"friend",display_name:"Riley",online:true,presence:"open"}],
      conversations:[{id:"spot:lounge",label:"Lounge",kind:"hangout",spot_id:"lounge",members:people}],
      messages:[],knocks:[],devices:[],room_invitations:[]}
    var other = JSON.parse(JSON.stringify(state)); other.server = data.servers[1]; other.self.server_admin = false
    data.server_states = [state,other]
    bridge.applySnapshot(data)
  }
  Timer {
    interval: 700; running: true
    onTriggered: {
      var host = test.find(page,"conversationPane")
      var room = test.find(page,"savedRoom-lounge")
      var buttonName = "participantStream-friend-screen_share"
      var watch = test.find(room,buttonName), camera = test.find(room,"participantStream-friend-camera")
      test.check(watch && watch.visible && watch.text === "watch" && camera && camera.visible,"voice list shows screen and camera watch controls")
      test.check(!bridge.workspaceLayout.streamsAsTiles && test.videoLeaves(host).length === 0 && !test.lastWatch(),"default is windows without auto-watching")
      if (!watch || !camera || !host) { Qt.quit(); return }
      test.check(watch.mapToItem(room,0,0).x + watch.width <= room.width && camera.mapToItem(room,0,0).x + camera.width <= room.width,"stream actions fit the narrow room rail")
      test.screenshot("room",room)
      input.mouseClick(watch,watch.width/2,watch.height/2)
      test.check(test.lastWatch().args.open && test.lastWatch().args.hosted && test.lastWatch().args.source === "screen_share","participant watch subscribes to that screen through the desktop host")
      test.acknowledgeWatch(true)
      var leaf = test.videoLeaves(host)[0]
      var tile = leaf ? test.find(page,"chatTileHost-" + leaf.key) : null
      test.check(tile && tile.detached && tile.popoutWindow.visible,"watch opens a separate window by default")
      test.check(test.find(room,buttonName).text === "leave" && test.find(room,"participantStream-friend-camera").text === "watch","only the watched source becomes leave")
      if (!tile) { Qt.quit(); return }
      input.wait(450)
      var renderer = test.find(tile.popoutWindow.contentItem,"remoteVideoRenderer")
      test.check(renderer && renderer.item && renderer.item.ready,"synthetic video renders in the new window: " + (renderer && renderer.item ? renderer.item.error + " " + renderer.item.socketPath : "no renderer"))
      test.screenshot("window",renderer.parent)
      var sentBefore = bridge.sent.length
      var dock = test.find(tile.popoutWindow.contentItem,"streamPlacementButton")
      test.check(dock && dock.text === "tile","window exposes an explicit tile button")
      dock.clicked(); input.wait(100)
      test.check(!tile.detached && !tile.popoutWindow.visible && bridge.sent.length === sentBefore,"tile button moves the stream without stopping it")
      test.find(tile,"streamPlacementButton").clicked(); input.wait(100)
      test.check(tile.detached,"window button pops the tile out again")
      tile.popoutWindow.contentItem.Window.window.close(); input.wait(100)
      test.check(!test.lastWatch().args.open && test.videoLeaves(host).length === 0,"OS close stops watching and removes the stream instead of docking")
      test.acknowledgeWatch(true)
      test.check(test.find(room,buttonName).text === "watch","window close restores watch beside the participant")
      bridge.workspaceLayout.setStreamsAsTiles(true)
      test.find(room,buttonName).clicked(); test.acknowledgeWatch(true)
      leaf = test.videoLeaves(host)[0]; tile = test.find(page,"chatTileHost-" + leaf.key)
      test.check(!tile.detached,"tile preference opens the next stream in the main app")
      test.find(room,buttonName).clicked()
      test.check(!test.lastWatch().args.open,"participant leave unsubscribes rather than leaving voice")
      test.acknowledgeWatch(true)
      test.check(test.videoLeaves(host).length === 0 && bridge.selfState.hangout_id === "active","participant leave closes its tile and keeps voice connected")
      test.find(room,buttonName).clicked(); test.acknowledgeWatch(false)
      test.check(test.videoLeaves(host).length === 0 && test.find(room,buttonName).text === "watch","failed subscription does not open a window or show leave")
      bridge.lastError = ""
      // An older Watch acknowledgement must not undo the newer Leave request.
      test.find(room,buttonName).clicked()
      var delayedWatch = test.lastWatch()
      bridge.watchVideo({participant:"Riley",source:"screen_share"},false)
      bridge.finishRequest({id:delayedWatch.id,ok:true,value:{}}); input.wait(80)
      test.check(test.videoLeaves(host).length === 0,"delayed watch reply cannot reopen a stream after Leave")
      test.acknowledgeWatch(true)
      // Screen and camera subscriptions remain independent when a publisher stops one.
      test.find(room,buttonName).clicked(); test.acknowledgeWatch(true)
      test.find(room,"participantStream-friend-camera").clicked(); test.acknowledgeWatch(true)
      test.check(test.videoLeaves(host).length === 2,"screen and camera can both be watched")
      var ended = JSON.parse(JSON.stringify(bridge.snapshot))
      ended.self.media.remote_videos = ended.self.media.remote_videos.filter(function(v) { return v.source === "camera" })
      bridge.applySnapshot(ended); input.wait(100)
      test.check(test.videoLeaves(host).length === 1 && !test.find(room,buttonName),"an ended screen disappears without closing the camera")
      test.find(room,"participantStream-friend-camera").clicked(); test.acknowledgeWatch(true)
      ended = JSON.parse(JSON.stringify(bridge.snapshot))
      ended.self.media.remote_videos.push({participant:"Riley",source:"screen_share",subscribed:false})
      bridge.applySnapshot(ended); input.wait(100)
      bridge.workspaceLayout.setStreamsAsTiles(false)
      test.find(room,"participantStream-friend-camera").clicked(); test.acknowledgeWatch(true)
      leaf = test.videoLeaves(host)[0]; tile = test.find(page,"chatTileHost-" + leaf.key)
      test.find(tile.popoutWindow.contentItem,"leaveStreamButton").clicked(); test.acknowledgeWatch(true)
      test.check(test.videoLeaves(host).length === 0,"stream window leave button stops its camera subscription")
      bridge.selectServer("other"); input.wait(100)
      test.check(!test.find(test.find(page,"savedRoom-lounge"),buttonName),"same participant name in another server does not inherit the live indicator")
      bridge.selectServer("local"); input.wait(100)
      room = test.find(page,"savedRoom-lounge")
      test.find(room,buttonName).clicked()
      var previousRoomWatch = test.lastWatch()
      var next = JSON.parse(JSON.stringify(bridge.snapshot))
      next.self.hangout_id = null; next.server_states[0].self.hangout_id = null
      bridge.applySnapshot(next)
      bridge.finishRequest({id:previousRoomWatch.id,ok:true,value:{}}); input.wait(100)
      test.check(test.videoLeaves(host).length === 0,"delayed watch reply cannot open a window after leaving voice")
      test.check(!test.find(test.find(page,"savedRoom-lounge"),buttonName),"leaving voice removes stale stream controls")
      test.check(bridge.workspaceLayout.chatTiles.indexOf("video:") < 0,"streams are never persisted for automatic watching")

      page.toggleSettings(); input.wait(100)
      var menu = test.find(page,"settingsMenu"), search = test.find(page,"settingsSearch")
      var beforeSearch = bridge.sent.length
      search.text = " STREAM default "; input.wait(100)
      test.check(menu.searchResults.length === 1 && menu.searchResults[0].target === "streamsAsTilesSetting","search matches option aliases, multiple words and case")
      test.check(bridge.sent.length === beforeSearch,"typing a settings search does not change settings or issue commands")
      test.screenshot("search",page)
      test.find(page,"settingsResult-streamsAsTilesSetting").clicked(); input.wait(120)
      test.check(menu.section === "video" && search.text === "" && menu.revealedTarget === "streamsAsTilesSetting","result navigates to stream display preference")
      var preference = test.find(page,"streamsAsTilesSetting")
      preference.checked = true; preference.toggled(); input.wait(100)
      test.check(bridge.workspaceLayout.streamsAsTiles,"stream preference is adjustable through Settings")
      search.text = "codec"; search.accepted(); input.wait(120)
      test.check(test.find(page,"advancedVideoSection").expanded && test.find(page,"videoCodecPicker").visible,"search expands advanced controls")
      search.text = "deepfilter"; search.accepted(); input.wait(120)
      var scroll = test.find(page,"dashboardScroll"), processing = test.find(page,"settingsProcessing")
      var position = processing.mapToItem(scroll,0,0)
      test.check(menu.revealedTarget === "settingsProcessing" && position.y >= 0 && position.y < scroll.height,"Enter jumps and scrolls to an individual audio setting")
      search.text = "no such setting"; input.wait(60)
      test.check(menu.searchResults.length === 0,"no matches has an empty result state")
      test.find(page,"clearSettingsSearch").clicked(); input.wait(60)
      test.check(!menu.searching && menu.section === "media","clearing search preserves current category")
      test.check(Search.search("server admin",false,false).length === 0 && Search.search("server admin",true,false).length >= 1,"server-only options respect permissions")
      Search.entries.forEach(function(entry) { test.check(!!menu.findSetting(menu,entry.target),"search target exists: " + entry.target) })
      test.check(!bridge.sent.some(function(c) { return /^(join|leave$|share$|camera$|knock|invite|send_message)/.test(c.name) }),"watching and searching never join voice or publish media")
      console.log(test.failed ? "STREAM_SEARCH_FAILED" : "STREAM_SEARCH_OK")
      Qt.quit()
    }
  }
}
