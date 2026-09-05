import QtQuick
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
      lastAccountInvite = {uri:"wisp-invite:fixture"}
    }
  }
  FloatingWindow {
    visible: true; implicitWidth: 360; implicitHeight: 450
    Components.ServerSelector { id: selector; width: 330; anchors.centerIn: parent; bridge: bridge; theme: theme }
  }
  function find(item, name) {
    if (item.objectName === name) return item
    for (var child of item.children || []) { var match = find(child, name); if (match) return match }
    return null
  }
  Timer { interval: 300; running: true; onTriggered: {
    var button = find(selector, "serverInviteFriend")
    if (!button || !button.enabled) throw new Error("Invite action must be reachable")
    var settings = find(selector, "serverSettingsShortcut")
    if (!settings || !settings.visible || settings.y >= button.y) throw new Error("Server settings must remain above invite action")
    button.clicked()
    if (bridge.requests !== 1 || !bridge.lastAccountInvite) throw new Error("Invite action failed")
    bridge.activeServer = {id:"test", name:"Friends", connected:false}
    if (button.enabled) throw new Error("Offline invites must be disabled")
    console.log("SERVER_INVITE_OK")
    Qt.quit()
  } }
}
