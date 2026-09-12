import QtQuick
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components

ShellRoot {
  QtObject { id: appearance; property string palette: "herdr"; property bool managed: false }
  Wisp.WispTheme { id: theme; profile: "clean_tui"; appearanceController: appearance }
  QtObject {
    id: bridge
    property var activeServer: ({id:"test", name:"Friends", connected:true})
    property var servers: [activeServer]
    property bool serverMember:true
    property var serverPreferences: ({homeIds:["test"],isHome:function(id){return true},canMove:function(id,dir){return false}})
    property var serverPings: ({})
    function refreshServerPing(id) {}
    property var accountActions: actions
    property var lastAccountInvite: null
    property string lastError: ""
    property int requests: 0
    property bool canManageServer: true

  }
  QtObject {
    id:actions
    property var invitation:null
    function state(id) {return {invitation:invitation,error:"",feedback:"",busy:false,invites:[]}}
    function act(command,args) {if(command!=="create_server_invite" || args.expires_in_minutes!==720)throw new Error("wrong invite request");bridge.requests++}
    function joinServer() {bridge.requests++}
  }
  FloatingWindow {
    id: window
    visible: true; implicitWidth: 360; implicitHeight: 450
    Components.ServerSelector { id: selector; width: 330; anchors.centerIn: parent; bridge: bridge; theme: theme }
  }
  TestCase { id: input; parent: window.contentItem; when: false }
  function find(item, name) {
    if (item.objectName === name) return item
    for (var child of item.data || item.children || []) { var match = find(child, name); if (match) return match }
    return null
  }
  Timer { interval: 300; running: true; onTriggered: {
    var button = find(selector, "serverInviteFriend")
    if (!button || !button.enabled) throw new Error("Invite action must be reachable")
    var settings = find(selector, "serverSettingsShortcut")
    if (!settings || !settings.visible || settings.y !== button.y || button.x + button.width > settings.x) throw new Error("Compact invite must sit beside server settings without overlap")
    if (button.width !== theme.space(36) || !button.iconOnly) throw new Error("Invite uses a compact icon button")
    var dropdown = find(selector, "activeServerSelector")
    var dropdownWidth = dropdown.width
    var expandedHeight = selector.implicitHeight
    selector.visible = false
    input.wait(50)
    if (selector.implicitHeight !== expandedHeight) throw new Error("Hiding the selector must not change its layout during window teardown")
    selector.visible = true
    selector.showInvite = false
    input.wait(50)
    if (button.visible || selector.implicitHeight !== expandedHeight || dropdown.width <= dropdownWidth) throw new Error("Hiding inline invite returns space to the server selector")
    var compactHeight = selector.implicitHeight
    selector.visible = false
    input.wait(50)
    if (selector.implicitHeight !== compactHeight) throw new Error("Compact selector height must remain stable while hidden")
    selector.visible = true
    selector.showInvite = true
    input.wait(50)
    input.mouseClick(button)
    input.wait(50)
    var popup = find(selector, "serverInvitePopup")
    if (bridge.requests !== 1 || !popup || !popup.opened) throw new Error("Invite action must open the popup while creating the invitation")
    // Daemon snapshots replace the server object even when selection is unchanged.
    bridge.activeServer = {id:"test", name:"Friends", connected:true}
    input.wait(50)
    if (!popup.opened) throw new Error("A status refresh must not dismiss the pending invitation")
    actions.invitation = {uri:"https://example.com/join/#v2.fixture",expires_at:"2030-01-01T00:00:00Z"}
    bridge.activeServer = {id:"test", name:"Friends renamed", connected:true}
    input.wait(50)
    var link = find(popup.contentItem, "serverInviteLink")
    if (!popup.opened || !link.visible || link.text !== actions.invitation.uri) throw new Error("The completed invitation must remain available after a status refresh")
    bridge.activeServer = {id:"test", name:"Friends", connected:false}
    input.wait(50)
    if (button.enabled) throw new Error("Offline invites must be disabled")
    if (!popup.opened) throw new Error("A connection update must not dismiss an existing invitation")
    bridge.activeServer = {id:"other", name:"Other server", connected:true}
    input.wait(50)
    if (popup.visible) throw new Error("Switching servers must dismiss the previous server's invitation")
    console.log("SERVER_INVITE_OK")
    Qt.quit()
  } }
}
