import QtQuick
import Quickshell
import "app" as Wisp

ShellRoot {
  id: test
  property bool failed: false
  readonly property string mode: Quickshell.env("WISP_CHAT_FIXTURE_MODE")
  function check(condition, message) { if (!condition) { failed = true; console.error("CHAT_TEST_FAILED: " + message) } }
  function findFeed(item) { return findItem(item, "messageFeed") }
  function findItem(item, name) {
    if (item.objectName === name && item.visible) return item
    var children = item.children || []
    for (var i = 0; i < children.length; i++) {
      var result = findItem(children[i], name)
      if (result) return result
    }
    return null
  }
  Wisp.WispTheme { id: theme }
  Wisp.WispBridge {
    id: bridge
    property var sent: []
    function send(name, args) { sent.push({name:name,args:args}); requestId++; return "test-" + requestId }
  }
  Wisp.WispWindow { id: window; bridge: bridge; theme: theme; visible: test.mode !== "panel" }
  FloatingWindow {
    id: compact
    visible: test.mode === "panel"
    implicitWidth: 460; implicitHeight: 800
    Rectangle {
      id: compactSurface
      anchors.fill: parent
      color: theme.background
      Wisp.WispContent {
        id: compactContent
        anchors.fill: parent
        bridge: bridge; theme: theme; presentation: "panel"
        logoSource: Qt.resolvedUrl("app/assets/waveform.svg")
      }
    }
  }
  Component.onCompleted: {
    var data = JSON.parse(JSON.stringify(bridge.snapshot))
    data.self.id = "self"
    data.self.display_name = "Tyler"
    data.self.connection = "available"
    data.self.presence = "open"
    data.conversations = [
      {id:"porch",kind:"hangout",label:"Porch",spot_id:"porch",unread_count:0},
      {id:"dm",kind:"direct",label:"Jared",unread_count:2},
      {id:"friends",kind:"circle",label:"Friends",unread_count:0}
    ]
    data.friends = [{id:"jared",display_name:"Jared",online:true,presence:"open"}, {id:"charlie",display_name:"Charlie",online:false,presence:"away"}]
    data.messages = [
      {id:"1",conversation_id:"porch",sender:{id:"jared",display_name:"Jared"},created_at:"2026-09-03T17:00:00Z",content_type:"text/plain",payload:"The new chat has a lot more space. Can you send that screenshot here?"},
      {id:"2",conversation_id:"porch",sender:{id:"self",display_name:"Tyler"},created_at:"2026-09-03T17:01:00Z",edited_at:"2026-09-03T17:01:30Z",content_type:"text/plain",payload:"Yep — I can paste it without leaving Wisp. The conversation stays open while I switch tabs."},
      {id:"3",conversation_id:"porch",sender:{id:"jared",display_name:"Jared"},created_at:"2026-09-03T17:02:00Z",content_type:"text/plain",payload:"Perfect. We can keep Porch and our DMs side by side."}
    ]
    bridge.applySnapshot(data)
    bridge.selectConversation("porch")
    bridge.setDraft("porch", "Here's the latest version…")
    bridge.setDraft("dm", "A separate DM draft")
    test.check(bridge.draftFor("porch") !== bridge.draftFor("dm"), "drafts are independent")
    bridge.sendComposedMessage("dm")
    var id = "test-" + bridge.requestId
    bridge.finishRequest({id:id,ok:false,error:{message:"offline"}})
    test.check(bridge.draftFor("dm") === "A separate DM draft", "failed sends retain drafts")
    test.check(!bridge.sendingConversations.dm, "failed sends release busy state")
    // Stage a synthetic image; no operating-system clipboard is touched.
    bridge.pasteClipboard("dm")
    var pasteId = "test-" + bridge.requestId
    bridge.finishRequest({id:pasteId,ok:true,value:{token:"fixture",url:String(Qt.resolvedUrl("app/assets/waveform.svg")),file_name:"Screenshot.png",size:2000,is_image:true}})
    test.check(bridge.attachmentsFor("dm")[0].token === "fixture", "paste stages image")
    bridge.sendComposedMessage("dm")
    var imageSendId = "test-" + bridge.requestId
    bridge.finishRequest({id:imageSendId,ok:false})
    test.check(bridge.attachmentsFor("dm")[0].token === "fixture", "failed image send retains attachment")
    bridge.removeAttachment("dm", "fixture", false)
    test.check(bridge.attachmentsFor("dm").length === 0, "remove attachment clears preview")
    bridge.importChatFiles("dm", ["file:///tmp/one.txt", "file:///tmp/two.txt"])
    test.check(bridge.importingConversations.dm === 1, "file import disables send while staging")
    bridge.finishRequest({id:"test-" + bridge.requestId,ok:true,value:{attachments:[
      {token:"one",file_name:"one.txt",size:100,is_image:false},
      {token:"two",file_name:"two.txt",size:200,is_image:false}
    ]}})
    bridge.sendComposedMessage("dm")
    var firstUploadId = "test-" + bridge.requestId
    test.check(bridge.sent[bridge.sent.length-1].args.caption === "A separate DM draft", "caption accompanies first attachment")
    bridge.finishRequest({id:firstUploadId,ok:true})
    test.check(bridge.attachmentsFor("dm").length === 1 && bridge.sendingConversations.dm, "successful file advances queue")
    test.check(bridge.sent[bridge.sent.length-1].args.caption === "", "caption is not duplicated across attachments")
    bridge.finishRequest({id:"test-" + bridge.requestId,ok:false})
    test.check(bridge.attachmentsFor("dm")[0].token === "two" && !bridge.sendingConversations.dm, "partial failure retains only unsent file")
    bridge.sendComposedMessage("dm")
    test.check(bridge.sent[bridge.sent.length-1].args.token === "two", "retry never resends successful attachment")
    bridge.finishRequest({id:"test-" + bridge.requestId,ok:true})
    test.check(bridge.attachmentsFor("dm").length === 0, "successful queue clears attachments")
    if (test.mode === "image" || test.mode === "panel") {
      data = JSON.parse(JSON.stringify(data))
      data.messages.push({id:"image",conversation_id:"porch",sender:{id:"self",display_name:"Tyler"},created_at:"2026-09-03T17:03:00Z",content_type:"image/png",payload:{width:480,height:160,caption:"Pasted screenshot preview"}})
      bridge.chatImageUrls = {image:String(Qt.resolvedUrl("app/assets/waveform.svg"))}
      bridge.applySnapshot(data)
    }
    if (test.mode === "files" || test.mode === "panel") {
      data = JSON.parse(JSON.stringify(data))
      data.messages.push({id:"file",conversation_id:"porch",sender:{id:"jared",display_name:"Jared"},created_at:"2026-09-03T17:04:00Z",content_type:"application/octet-stream",payload:{file_name:"project-notes.txt",size:3400,caption:"Here are the notes."}})
      bridge.applySnapshot(data)
      bridge.pendingAttachments = {porch:[{token:"pending-file",file_name:"notes.txt",size:3000,is_image:false},{token:"pending-image",file_name:"Screenshot.png",size:8000,is_image:true,url:String(Qt.resolvedUrl("app/assets/waveform.svg"))}]}
      bridge.saveChatFile("file")
      test.check(bridge.savingFiles.file, "save disables duplicate download")
      bridge.finishRequest({id:"test-" + bridge.requestId,ok:true,value:{directory_url:"file:///tmp/test-saved",url:"file:///tmp/test-saved/notes.txt"}})
      test.check(!bridge.savingFiles.file && !!bridge.savedFiles.file, "successful save offers its directory without opening the file")
    }
    bridge.notificationMuted = true
    bridge.notificationVolume = 35
    bridge.notificationSoundPath = "file:///tmp/test-custom-sound.wav"
    if (test.mode === "settings") window.contentItem.children[0].children[0].settingsOpen = true
  }
  Timer {
    interval: 400; running: test.mode !== "settings"
    onTriggered: {
      var target = test.mode === "panel" ? compactSurface : window.contentItem
      var area = test.findItem(target, "chatDropArea")
      test.check(area && area.width > 0 && area.height > 0, "drop area covers active chat")
      if (!area) return
      var drop = {hasUrls:true,urls:["file:///tmp/fixture.txt"],supportedActions:Qt.CopyAction | Qt.MoveAction,accepted:false,action:Qt.IgnoreAction,
        accept: function(action) { this.action = action; this.accepted = true }}
      area.handleDrop(drop)
      test.check(drop.accepted && drop.action === Qt.CopyAction, "drop copies and never moves source files")
      test.check(bridge.sent[bridge.sent.length-1].name === "import_chat_files", "drop stages files without sending")
      bridge.finishRequest({id:"test-" + bridge.requestId,ok:false})
      test.check(!bridge.importingConversations.porch, "failed drop releases staging state")
    }
  }
  Timer {
    interval: 600; running: test.mode === "edit" || test.mode === "panel"
    onTriggered: {
      var target = test.mode === "panel" ? compactSurface : window.contentItem
      var feed = test.findFeed(target)
      test.check(!!feed, "shared message controls are available")
      if (!feed) return
      feed.beginEdit(bridge.snapshot.messages[1])
      test.check(feed.editingId === "2", "edit targets selected message")
      feed.savingEdit = bridge.editChatMessage("2", "Changed message")
      bridge.finishRequest({id:"test-" + bridge.requestId,ok:false,error:{message:"Test edit failed"}})
      test.check(!feed.savingEdit && feed.editError === "Test edit failed", "failed edits remain retryable")
      if (test.mode === "panel") {
        feed.savingEdit = bridge.editChatMessage("2", "Changed message")
        bridge.finishRequest({id:"test-" + bridge.requestId,ok:true})
        test.check(!feed.savingEdit, "successful edits release busy state")
      }
    }
  }
  Timer {
    interval: 1200; running: true
    onTriggered: {
      test.check(window.width >= 1100, "large app default width")
      test.check(window.height >= 850, "large app default height")
      var path = Quickshell.env("WISP_CHAT_SCREENSHOT")
      var target = test.mode === "panel" ? compactSurface : window.contentItem.children[0]
      if (test.mode === "edit") {
        var feed = test.findFeed(window.contentItem)
        test.check(feed && feed.editOpen, "edit dialog stays open after a failed save")
      }
      var started = target.grabToImage(function(result) {
        if (path) test.check(result.saveToFile(path), "save layout screenshot")
        console.log(test.failed ? "CHAT_TEST_FAILED" : "CHAT_WORKSPACE_OK")
        Qt.quit()
      })
      if (!started) { test.check(false, "grab layout"); Qt.quit() }
    }
  }
}
