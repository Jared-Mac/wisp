import QtQuick
import Quickshell
import "app" as Wisp

ShellRoot {
  id: test
  function check(value, message) {
    if (!value) { console.error("PRIVACY_ROUTING_FAILED: " + message); Qt.exit(1) }
  }
  Wisp.WispBridge {
    id: bridge
    property var sent: []
    function send(name, args) {
      var id = "fixture-" + sent.length
      sent.push({id:id, name:name, args:args})
      return id
    }
  }
  property int step: 0
  property string firstRequest: ""
  Timer {
    interval: 100; running: true; repeat: true
    onTriggered: {
      if (!bridge.daemonConnected) return
      if (test.step === 0) {
        bridge.refreshPrivacy()
        test.check(!bridge.sent.some(function(v) { return v.name === "privacy_status" }),
          "A socket connection must not request privacy before its first snapshot")
        var snapshot = JSON.parse(JSON.stringify(bridge.snapshot))
        snapshot.self.id = "fixture-user"
        snapshot.self.connection = "available"
        snapshot.servers = [{id:"vps-a",name:"First",connected:true},{id:"vps-b",name:"Second",connected:true}]
        snapshot.selected_server_id = "vps-a"
        bridge.applySnapshot(snapshot)
      } else if (test.step === 1) {
        var privacy = bridge.sent.filter(function(v) { return v.name === "privacy_status" })
        test.check(privacy.length === 1 && privacy[0].args.server_id === "vps-a", "First privacy request targets the VPS, never local")
        test.firstRequest = privacy[0].id
        bridge.selectServer("vps-b")
      } else if (test.step === 2) {
        var privacy = bridge.sent.filter(function(v) { return v.name === "privacy_status" })
        test.check(privacy.length === 2 && privacy[1].args.server_id === "vps-b", "Changing servers refreshes account privacy")
        bridge.finishRequest({id:privacy[1].id,ok:true,value:{configured:true}})
        bridge.finishRequest({id:test.firstRequest,ok:false,error:{message:"Old account error"}})
        test.check(bridge.privacyStatus.configured && bridge.privacyFeedback === "", "Late responses from another account cannot overwrite current privacy")
        bridge.privacyFeedback = "Stale connection error"
        bridge.refreshPrivacy()
        var request = bridge.sent[bridge.sent.length - 1]
        bridge.finishRequest({id:request.id,ok:true,value:{configured:true}})
        test.check(bridge.privacyFeedback === "", "Successful status clears stale feedback")
        console.log("PRIVACY_ROUTING_OK")
        Qt.quit()
      }
      test.step++
    }
  }
  Timer { interval: 5000; running: true; onTriggered: { console.error("PRIVACY_ROUTING_FAILED: timeout"); Qt.exit(1) } }
}
