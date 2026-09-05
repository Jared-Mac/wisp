import QtQuick
import QtTest
import Quickshell
import "app" as Wisp

ShellRoot {
  id: test
  property bool failed: false
  readonly property bool compact: Quickshell.env("WISP_TEST_PRESENTATION") === "panel"
  function check(ok, message) { if (!ok) { failed = true; console.error("SETTINGS_ACCESS_FAILED: " + message) } }
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
    var path = Quickshell.env("WISP_SETTINGS_SCREENSHOT")
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
  }
  FloatingWindow {
    id: window
    visible: true
    implicitWidth: Number(Quickshell.env("WISP_TEST_WIDTH")) || (test.compact ? 420 : 840)
    implicitHeight: 760
    color: theme.background
    Wisp.WispContent { id: page; anchors.fill: parent; theme: theme; bridge: bridge; logoSource: Qt.resolvedUrl("app/assets/wisp-icon.svg"); presentation: test.compact ? "panel" : "app" }
  }
  TestCase { id: input; parent: window.contentItem; when: false }
  Component.onCompleted: {
    var data = JSON.parse(JSON.stringify(bridge.snapshot))
    var people = [{id:"self",display_name:"Me"},{id:"friend",display_name:"Friend"}]
    data.self.id = "self"; data.self.display_name = "Me"; data.self.connection = "available"; data.self.server_owner = true
    data.friends = [{id:"friend",display_name:"Friend",online:true,presence:"open"}]
    data.hangouts = [{id:"active",label:"Lounge",members:people}]
    data.spots = [{id:"lounge",name:"Lounge",active_hangout_id:"active",members:people},{id:"quiet",name:"Quiet",members:[]}]
    data.conversations = [
      {id:"spot:lounge",kind:"hangout",label:"Lounge",spot_id:"lounge",self_role:"host",members:people,member_roles:{self:"host",friend:"member"}},
      {id:"spot:quiet",kind:"hangout",label:"Quiet",spot_id:"quiet",self_role:"host",members:people,member_roles:{self:"host",friend:"member"}}
    ]
    var first = {id:"local",name:"Home",connected:true}, second = {id:"second",name:"Other",connected:true}
    data.servers = [first,second]; data.selected_server_id = "local"
    data.server_states = [Object.assign({},data,{server:first}),{server:second,self:Object.assign({},data.self,{id:"other-self"}),friends:[],hangouts:[],spots:[],conversations:[],messages:[],devices:[],knocks:[],room_invitations:[]}]
    bridge.applySnapshot(data)
  }
  Timer {
    interval: 550; running: true
    onTriggered: {
      var selector = test.find(page,"activeServerSelector")
      var arrow = test.find(selector,"serverDropdownArrow")
      input.mouseClick(arrow,arrow.width/2,arrow.height/2); input.wait(80)
      test.check(selector.popup.visible,"arrow opens server dropdown")
      input.mouseClick(arrow,arrow.width/2,arrow.height/2); input.wait(80)
      test.check(!selector.popup.visible,"arrow closes an open dropdown")
      input.mouseClick(selector,20,selector.height/2); input.wait(80)
      test.check(selector.popup.visible,"server name also opens dropdown")
      selector.popup.close(); input.wait(80)
      input.mouseMove(selector,30,selector.height/2); input.wait(50)
      var settings = test.find(page,"serverSettingsShortcut")
      test.check(settings.visible,"server settings stay visible")
      test.check(settings.text === "settings","short server name uses full settings label")
      var changed = JSON.parse(JSON.stringify(bridge.snapshot)); changed.servers[0].name = "An unusually long server name that needs the compact shortcut"; changed.server_states[0].server.name = changed.servers[0].name
      bridge.applySnapshot(changed); input.wait(100)
      test.check(settings.text === "stngs","long server name uses compact label")
      test.check(selector.x + selector.width < settings.x,"settings have a separate target outside the dropdown")
      var selectorWidth = selector.width, settingsX = settings.x
      input.mouseMove(page,page.width-2,page.height-2); input.wait(70)
      test.check(settings.visible && settings.x === settingsX && selector.width === selectorWidth,"hover never changes either click target")
      test.screenshot("server", page); input.wait(100)
      input.mouseClick(settings,settings.width/2,settings.height/2); input.wait(80)
      test.check(page.settingsOpen && test.find(page,"settingsMenu").section === "server","shortcut opens Server settings")
      var home = test.find(page,"headerHomeButton")
      test.check(home.contentItem.text === "[home]","home is bracketed exactly once")
      home.clicked(); input.wait(80)
      var room = test.find(page,"savedRoom-lounge")
      input.mouseClick(room,room.width/2,room.height/2,Qt.RightButton); input.wait(80)
      var popup = test.object(room,"participantVolumeMenu",[])
      test.check(popup && popup.visible,"right click opens room menu")
      var action = popup ? test.find(popup.contentItem,"roomContextSettings") : null
      test.check(action && action.visible,"room menu offers settings above volume controls")
      test.check(test.find(popup.contentItem,"participantVolume-friend") !== null,"room menu retains participant volume slider")
      test.screenshot("room-menu", popup.contentItem.parent); input.wait(100)
      var before = bridge.sent.length
      if (!action) { Qt.quit(); return }
      input.mouseClick(action,action.width/2,action.height/2); input.wait(80)
      var manager = test.object(popup,"contextRoomManager",[])
      // Popup objects are not Items; find their title through the overlay.
      test.check(manager && manager.opened && test.find(manager.contentItem,"roomSettingsMembers") !== null,"room settings dialog opens")
      test.check(bridge.sent.length === before,"opening room settings sends no room or media command")
      if (!manager) { Qt.quit(); return }
      manager.close(); input.wait(80)
      var empty = test.find(page,"savedRoom-quiet")
      input.mouseClick(empty,empty.width/2,empty.height/2,Qt.RightButton); input.wait(80)
      popup = test.object(empty,"participantVolumeMenu",[])
      action = popup ? test.find(popup.contentItem,"roomContextSettings") : null
      test.check(action && action.visible,"empty saved room has settings too")
      popup.close(); input.wait(80)
      page.toggleSettings(); input.wait(80)
      test.find(page,"settingsTab-profile").clicked(); input.wait(80)
      test.check(test.last().name === "account_profile" && test.last().args.server_id === "local","Profile loads selected account")
      test.reply({user_id:"self",username:"my-login",display_name:"Me",revision:0,password_available:true}); input.wait(80)
      var name = test.find(page,"profileDisplayName"), save = test.find(page,"profileSaveName")
      name.text = "New name"; save.clicked()
      test.check(test.last().name === "update_account_profile" && test.last().args.revision === 0,"name save includes current revision")
      test.reply({user_id:"self",username:"my-login",display_name:"New name",revision:1,password_available:true})
      var current = test.find(page,"profileCurrentPassword"), password = test.find(page,"profileNewPassword"), confirm = test.find(page,"profileConfirmPassword"), change = test.find(page,"profileSavePassword")
      current.text = "test current secret"; password.text = "test replacement secret"; confirm.text = "mismatch"
      test.check(!change.enabled,"password mismatch blocks submission")
      confirm.text = password.text; test.check(change.enabled,"matching valid password enables change")
      change.clicked()
      test.check(test.last().name === "change_account_password" && test.last().args.server_id === "local","password change targets current account")
      test.check(current.text === "" && password.text === "" && confirm.text === "","password fields clear immediately after submission")
      test.reply({ok:true})
      current.text = "discard this draft"; page.goHome(); input.wait(80)
      test.check(current.text === "","leaving settings clears password drafts")
      page.toggleSettings(); test.find(page,"settingsTab-profile").clicked(); input.wait(80)
      var stale = "test-" + bridge.requestId
      bridge.selectServer("second"); input.wait(80)
      test.check(!bridge.profileReady,"switching server clears loaded account")
      bridge.finishRequest({id:stale,ok:true,value:{display_name:"Stale response"}})
      test.check(!bridge.profileReady,"stale reply cannot populate another account")
      test.reply({user_id:"other-self",username:"other-login",display_name:"Other Me",revision:0,password_available:true})
      test.screenshot("profile", page); input.wait(120)
      test.check(bridge.sent.every(function(command) { return ["join_hangout","join_spot","leave","share","camera"].indexOf(command.name)<0 }),"settings navigation never changes media")
      console.log(test.failed ? "SETTINGS_ACCESS_FAILED" : "SETTINGS_ACCESS_OK")
      Qt.quit()
    }
  }
}
