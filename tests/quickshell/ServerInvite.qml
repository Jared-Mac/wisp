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
    property var lastAccountInvite: null
    property string lastError: ""
    property int requests: 0
    property bool canManageServer: true
    function createAccountInvite(kind, conversation, minutes) {
      if (kind !== "friend" || conversation !== "" || minutes !== 30) throw new Error("wrong invitation request")
      requests++
    }
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
    if (!settings || !settings.visible || settings.y >= button.y) throw new Error("Server settings must remain above invite action")
    input.mouseClick(button)
    input.wait(50)
    var popup = find(selector, "serverInvitePopup")
    if (bridge.requests !== 1 || !popup || !popup.opened) throw new Error("Invite action must open the popup while creating the invitation")
    // Daemon snapshots replace the server object even when selection is unchanged.
    bridge.activeServer = {id:"test", name:"Friends", connected:true}
    input.wait(50)
    if (!popup.opened) throw new Error("A status refresh must not dismiss the pending invitation")
    bridge.lastAccountInvite = {uri:"wisp-invite:fixture"}
    bridge.activeServer = {id:"test", name:"Friends renamed", connected:true}
    input.wait(50)
    var link = find(popup.contentItem, "serverInviteLink")
    if (!popup.opened || !link.visible || link.text !== bridge.lastAccountInvite.uri) throw new Error("The completed invitation must remain available after a status refresh")
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
