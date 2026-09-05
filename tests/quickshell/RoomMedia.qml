import QtQuick
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components

ShellRoot {
  id: test
  property bool failed: false
  function check(ok, message) { if (!ok) { failed = true; console.error("ROOM_MEDIA_FAILED " + message) } }
  function find(item, name) {
    if (item.objectName === name) return item
    for (var child of item.children || []) { var result = find(child, name); if (result) return result }
    return null
  }
  QtObject { id: appearance; property string palette: "ash_olive"; property bool managed: false }
  QtObject {
    id: testBridge
    function roomSettingsConversationId(room, persistent) { return "" }
    property var activeServer: ({id:"local",connected:true})
    function scopedParticipant(person) { return Object.assign({},person,{server_id:"local",account_id:"self"}) }
    function participantServer(person) { return {server:activeServer,self:selfState,friends:[]} }
    function participantModeration(person) { return person.id === "owner" ? {muted:true,deafened:true} : {muted:false,deafened:false} }
    function canModerateParticipant(person) { return false }
    property var friends: []
    property var participantVolumes: QtObject {
      property string error: ""
      function volumeFor(person) { return 100 }
      function isMuted(person) { return person.id === "owner" }
    }
    property var selfState: ({id:"self",hangout_id:"test_room",deafened:true,muted:true})
    property bool effectiveMuted: true
    property var activeSpeakers: ["MemberC"]
    property var remoteMutedParticipants: ["Owner"]
    property var remoteVideos: []
    property var pushToTalkState: ({enabled:false})
    property bool sharing: true
    property bool cameraActive: true
    property bool shareStarting: false
    property bool cameraStarting: false
    // Stopping must stay available even if camera enumeration disappears.
    property var cameraState: ({devices:[]})
    property int shareClicks: 0
    property int cameraClicks: 0
    function toggleShare() { shareClicks++ }
  }
  FloatingWindow {
    id: window
    visible: true; implicitWidth: 1000; implicitHeight: 420; color: "#151821"
    Row {
      id: samples
      anchors.fill: parent; anchors.margins: 12; spacing: 16
      Repeater {
        model: ["clean_tui", "performative", "legacy", "clean_tui"]
        delegate: Column {
          required property string modelData
          required property int index
          width: index === 3 ? 260 : 220; spacing: 12
          Wisp.WispTheme { id: sampleTheme; profile: modelData; appearanceController: appearance }
          Text { text: modelData; color: sampleTheme.foreground }
          Components.HangoutCard {
            width: parent.width; theme: sampleTheme; bridge: testBridge
            hangout: ({id:"test_room",label:"TestRoom",members:[
              {id:"member_c",display_name:"MemberC"},{id:"self",display_name:"MemberA"},
              {id:"owner",display_name:"Owner"},{id:"long",display_name:"AnUnusuallyLongFriendNameThatMustWrap"}]})
          }
          Components.MediaControls {
            width: parent.width; theme: sampleTheme; bridge: testBridge
            onCameraRequested: testBridge.cameraClicks++
          }
        }
      }
    }
  }
  TestCase { id: clicks; parent: window.contentItem; when: false }
  Timer {
    interval: 400; running: true
    onTriggered: {
      for (var column of samples.children) {
        var room = test.find(column, "roomCard")
        if (!room) continue
        for (var i = 0; i < 4; i++) {
          var name = test.find(room, "roomMemberName-" + i)
          var row = test.find(room, "roomMember-" + i)
          test.check(name && !name.truncated && name.width > 40, "name is readable: " + i)
          var pos = row.mapToItem(room, 0, 0)
          test.check(pos.x >= 0 && pos.y >= 0 && pos.x + row.width <= room.width && pos.y + row.height <= room.height, "member fits card")
        }
        var selfRow = test.find(room, "roomMember-1"), moderatedRow = test.find(room, "roomMember-2")
        test.check(String(test.find(selfRow,"participantMicrophoneStatus").source).endsWith("/microphone-muted.svg") && String(test.find(selfRow,"participantDeafenStatus").source).endsWith("/deafened.svg"),"self mute and deafen retain the ordinary icons")
        test.check(test.find(moderatedRow,"participantMicrophoneStatus").visible && String(test.find(moderatedRow,"participantMicrophoneStatus").source).endsWith("/microphone-server-muted.svg"),"calls show server mute with a shield")
        test.check(test.find(moderatedRow,"participantDeafenStatus").visible && String(test.find(moderatedRow,"participantDeafenStatus").source).endsWith("/server-deafened.svg") && test.find(moderatedRow,"participantLocalMuteStatus").visible,"calls distinguish server deafen and local mute together")
        var share = test.find(column, "mediaAction-share")
        var camera = test.find(column, "mediaAction-camera")
        var invite = test.find(column, "mediaAction-invite")
        test.check(!!invite && invite.modelData.label === "Invite", "compact invite action exists")
        if (invite) {
          var inviteLabel = invite.children.find(function(child) { return child.text === "[invite]" || child.text === "Invite" })
          test.check(!!inviteLabel && inviteLabel.width >= inviteLabel.implicitWidth && inviteLabel.lineCount === 1, "invite brackets never wrap")
          test.check(invite.parent.children.indexOf(invite) > invite.parent.children.indexOf(camera), "invite follows camera")
          test.check(invite.parent.children.indexOf(invite) < invite.parent.children.indexOf(test.find(column, "mediaAction-leave")), "invite precedes leave")
        }
        test.check(share.publishing && camera.publishing && share.border.width === 1 && camera.border.width === 1, "both live controls highlighted")
        test.check(share.controlEnabled && camera.controlEnabled, "stop actions stay enabled")
        test.check(share.modelData.label === "Stop share" && camera.modelData.label === "Stop cam", "explicit stop labels")
        test.check(share.width <= column.width && camera.width <= column.width, "controls fit narrow rail")
        clicks.mouseClick(share, share.width / 2, share.height / 2)
        clicks.mouseClick(camera, camera.width / 2, camera.height / 2)
      }
      test.check(testBridge.shareClicks === 4 && testBridge.cameraClicks === 4, "stop clicks route correctly across styles")
      var path = Quickshell.env("WISP_ROOM_SCREENSHOT")
      if (path) samples.grabToImage(function(result) { result.saveToFile(path) })
    }
  }
  Timer {
    interval: 650; running: true
    onTriggered: { testBridge.sharing = false; testBridge.cameraActive = false }
  }
  Timer {
    interval: 850; running: true
    onTriggered: {
      for (var column of samples.children) {
        var share = test.find(column, "mediaAction-share")
        if (!share) continue
        var camera = test.find(column, "mediaAction-camera")
        test.check(!share.publishing && !camera.publishing && share.border.width === 0 && camera.border.width === 0, "idle controls not highlighted")
        test.check(camera.modelData.label === "Camera" && !camera.controlEnabled, "idle camera still needs device")
      }
      if (!test.failed) console.log("ROOM_MEDIA_OK")
      Qt.quit()
    }
  }
}
