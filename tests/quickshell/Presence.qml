import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp
import "app/PresenceText.js" as PresenceText

ShellRoot {
  id: test
  property bool failed: false
  function check(ok, message) { if (!ok) { failed = true; console.error("PRESENCE_FAILED: " + message) } }
  function find(item, name) {
    if (!item) return null
    if (item.objectName === name) return item
    for (var child of item.children || []) { var found = find(child, name); if (found) return found }
    return null
  }
  Wisp.WispTheme { id: theme; profile: "performative" }
  Wisp.WispBridge {
    id: bridge
    property var sent: []
    function send(name, args) { sent.push({name:name,args:args}); requestId++; return "test-" + requestId }
  }
  FloatingWindow {
    id: window; visible: true; implicitWidth: 460; implicitHeight: 780; color: theme.background
    Wisp.WispContent {
      id: page; anchors.fill: parent; theme: theme; bridge: bridge
      logoSource: Qt.resolvedUrl("app/assets/wisp-icon.svg")
      presentation: Quickshell.env("WISP_TEST_PRESENTATION") || "app"
      dismissOnNavigate: true
      onCloseRequested: test.check(false, "sending a knock must keep its confirmation visible")
    }
  }
  TestCase { id: input; parent: window.contentItem; when: false }
  Component.onCompleted: {
    var data = JSON.parse(JSON.stringify(bridge.snapshot))
    data.self.id = "self"; data.self.display_name = "Me"; data.self.presence = "closed"
    data.self.hangout_id = null; data.self.connection = "available"
    data.friends = [{id:"friend",display_name:"A friend",online:true,presence:"knock"}]
    data.conversations = [{id:"direct:friend",kind:"direct",label:"A friend",members:[{id:"self",display_name:"Me"},{id:"friend",display_name:"A friend"}]}]
    var first = {id:"local",name:"First",connected:true}, second = {id:"second",name:"Second",connected:true}
    data.servers = [first,second]; data.selected_server_id = "local"
    data.server_states = [Object.assign({},data,{server:first}),Object.assign({},data,{server:second,self:Object.assign({},data.self,{presence:"away"})})]
    bridge.applySnapshot(data)
  }
  Timer {
    interval: 500; running: true
    onTriggered: {
      for (var mode of ["open","knock","closed","away"]) {
        var button = test.find(page,"presence-" + mode)
        test.check(!!button, "presence choice is visible")
        input.mouseMove(button,button.width/2,button.height/2); input.wait(550)
        test.check(button.ToolTip.visible && button.ToolTip.text === PresenceText.description(mode,true), "hover explains " + mode)
      }
      var icon = test.find(page,"friendPresence-friend")
      input.mouseMove(icon,icon.width/2,icon.height/2); input.wait(100)
      test.check(icon.ToolTip.visible && icon.ToolTip.text.indexOf("wait for them to accept") >= 0, "friend icon explains knocking")
      // Click the actual row, including a compact panel configured to dismiss on navigation.
      var name = test.find(page,"friendName")
      input.mouseClick(name,name.width/2,name.height/2)
      var sent = bridge.sent[bridge.sent.length-1]
      test.check(sent.name === "join_friend" && sent.args.server_id === "local", "friend click requests voice on its server")
      test.check(bridge.knockFeedback === "", "no success confirmation before server reply")
      bridge.handleLine(JSON.stringify({type:"result",id:"test-" + bridge.requestId,ok:true,value:{status:"knock_sent",knock_id:"fixture"}}))
      input.wait(60)
      var notice = test.find(page,"knockSentNotice")
      test.check(notice.visible && bridge.knockFeedback.indexOf("Knock sent to A friend") === 0, "successful knock shows recipient confirmation")
      test.check(notice.width <= page.width && notice.height > 0 && notice.y + notice.height < page.height, "confirmation fits a narrow window")
      // A DM call on another server uses the same acknowledgment path.
      bridge.callConversation(bridge.scopedConversationId("second","direct:friend"))
      sent = bridge.sent[bridge.sent.length-1]
      test.check(sent.name === "join_friend" && sent.args.server_id === "second" && sent.args.friend === "friend", "DM call scopes its knock correctly")
      bridge.handleLine(JSON.stringify({type:"result",id:"test-" + bridge.requestId,ok:true,value:{status:"knock_sent"}}))
      test.check(bridge.knockFeedback.indexOf("Knock sent to A friend") === 0, "DM knock also confirms")
      bridge.joinFriend("A friend")
      bridge.handleLine(JSON.stringify({type:"result",id:"test-" + bridge.requestId,ok:false,error:{message:"Friend is offline"}}))
      test.check(bridge.knockFeedback === "" && bridge.lastError === "Friend is offline", "failed knock shows error without success")
      bridge.joinFriend("A friend")
      bridge.handleLine(JSON.stringify({type:"result",id:"test-" + bridge.requestId,ok:true,value:{status:"joined"}}))
      test.check(bridge.knockFeedback === "", "direct join does not claim a knock was sent")
      bridge.selectServer("second")
      test.check(bridge.selfState.presence === "away", "selected server retains its own choice")
      test.check(bridge.sent.every(function(command) { return ["join_spot","join_hangout","share","camera"].indexOf(command.name)<0 }), "feedback and tooltips never start media")
      console.log(test.failed ? "PRESENCE_FAILED" : "PRESENCE_OK")
      Qt.quit()
    }
  }
}
