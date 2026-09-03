import QtQuick
import Quickshell
import "app" as Wisp
import "app/components" as Components

ShellRoot {
  id: test
  property bool failed: false
  readonly property string mode: Quickshell.env("WISP_CHAT_FIXTURE_MODE")
  readonly property bool compactMode: mode === "panel" || mode === "panelmedia" || mode === "panelsettings" || mode === "friends"
  property var popupTarget: null
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
  function findText(item, text) {
    if (item.text === text && item.visible) return item
    var children = item.children || []
    for (var i = 0; i < children.length; i++) {
      var result = findText(children[i], text)
      if (result) return result
    }
    return null
  }
  function findObject(item, name, visited) {
    if (!item || visited.indexOf(item) >= 0) return null
    visited.push(item)
    if (item.objectName === name) return item
    var children = item.data || item.children || []
    for (var i = 0; i < children.length; i++) {
      var result = findObject(children[i], name, visited)
      if (result) return result
    }
    return null
  }
  Wisp.WispAppearance {
    id: themeAppearance; environment: Quickshell.env("WISP_TEST_ADAPTER") === "omarchy" ? "omarchy" : "desktop"
    Component.onCompleted: if (Quickshell.env("WISP_TEST_PALETTE")) setPalette(Quickshell.env("WISP_TEST_PALETTE"))
  }
  Binding { target: theme; property: "profile"; value: themeAppearance.profile; when: test.mode === "themes" }
  Wisp.WispTheme {
    id: theme
    appearanceController: themeAppearance
    Component.onCompleted: {
      if ("profile" in theme) theme.profile = Quickshell.env("WISP_TEST_ADAPTER") === "omarchy" ? "legacy" : Quickshell.env("WISP_TEST_THEME") || "legacy"
      if (Quickshell.env("WISP_TEST_ADAPTER") === "omarchy") {
        // Representative host overrides, not Jared's settings or machine.
        foreground = "#d8dee9"; background = "#242933"; surface = "#242933"
        accent = "#81a1c1"; muted = "#a0a8b7"; danger = "#bf616a"
        cornerRadius = 7; fontFamily = "DejaVu Sans"; captionSize = 13; bodySize = 15; titleSize = 19; spacingScale = 1.1
        if ("profile" in theme) theme.profile = "legacy"
      }
    }
  }
  Wisp.WispBridge {
    id: bridge
    property var sent: []
    function send(name, args) { sent.push({name:name,args:args}); requestId++; return "test-" + requestId }
    function localPreviewUrl(stem, revision) { return String(Qt.resolvedUrl("app/assets/waveform.svg")) }
  }
  Wisp.WispWindow {
    id: window; bridge: bridge; theme: theme; visible: !test.compactMode && test.mode !== "preview"
    implicitWidth: theme.space(Quickshell.env("WISP_TEST_CONSTRAINED") === "1" ? 840 : 1180)
    implicitHeight: theme.space(Quickshell.env("WISP_TEST_CONSTRAINED") === "1" ? 700 : 900)
  }
  Wisp.WispPreviewWindow { id: preview; bridge: bridge; theme: theme; visible: test.mode === "preview" && bridge.sharing }
  Item {
    parent: window.contentItem
    Components.ClearHistoryDialog { id: clearDialog; bridge: bridge; theme: theme }
    Components.RoomManager { id: roomDialog; bridge: bridge; theme: theme }
  }
  FloatingWindow {
    id: compact
    visible: test.compactMode
    implicitWidth: Quickshell.env("WISP_TEST_CONSTRAINED") === "1" ? 400 : 460
    implicitHeight: Quickshell.env("WISP_TEST_CONSTRAINED") === "1" ? 700 : 800
    Rectangle {
      id: compactSurface
      anchors.fill: parent
      color: theme.background
      Wisp.WispContent {
        id: compactContent
        anchors.fill: parent
        bridge: bridge; theme: theme; presentation: "panel"
        logoSource: Qt.resolvedUrl("app/assets/waveform.svg")
        showAppButton: !!Quickshell.env("WISP_TEST_STRESS")
        showCloseButton: !!Quickshell.env("WISP_TEST_STRESS")
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
      {id:"porch",kind:"hangout",label:"Porch",spot_id:"porch",unread_count:0, self_role:"host", can_clear_for_everyone:true,
        members:[{id:"self",display_name:"Tyler"},{id:"jared",display_name:"Jared"}],member_roles:{self:"host",jared:"member"}},
      {id:"dm",kind:"direct",label:"Jared",unread_count:2},
      {id:"friends",kind:"circle",label:"Friends",unread_count:0}
    ]
    data.friends = [{id:"jared",display_name:"Jared",online:true,presence:"open"}, {id:"charlie",display_name:"Charlie",online:false,presence:"away"}]
    if (Quickshell.env("WISP_TEST_STRESS")) {
      data.self.display_name = "A very long display name"
      data.friends[0].display_name = "A friend with a long name"
      data.spots = [{id:"porch",name:"Porch with a long room name",members:[]}]
    }
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
    bridge.setAttachmentKeep("dm", "one", true)
    bridge.sendComposedMessage("dm")
    var firstUploadId = "test-" + bridge.requestId
    test.check(bridge.sent[bridge.sent.length-1].args.caption === "A separate DM draft", "caption accompanies first attachment")
    test.check(bridge.sent[bridge.sent.length-1].args.keep === true, "manual retention flag accompanies file")
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
    if (test.mode === "files" || test.mode === "panel" || test.mode === "transfer") {
      data = JSON.parse(JSON.stringify(data))
      data.messages.push({id:"file",conversation_id:"porch",sender:{id:"jared",display_name:"Jared"},created_at:"2026-09-03T17:04:00Z",content_type:"application/octet-stream",payload:{file_name:"project-footage.mp4",size:6000000000,expires_at:"2026-09-04T17:04:00Z",keep:false,caption:"Here is the footage."}})
      bridge.applySnapshot(data)
      bridge.pendingAttachments = {porch:[{token:"pending-file",file_name:"notes.txt",size:3000,is_image:false},{token:"pending-image",file_name:"Screenshot.png",size:8000,is_image:true,url:String(Qt.resolvedUrl("app/assets/waveform.svg"))}]}
      bridge.saveChatFile("file")
      test.check(bridge.savingFiles.file, "save disables duplicate download")
      bridge.finishRequest({id:"test-" + bridge.requestId,ok:true,value:{directory_url:"file:///tmp/test-saved",url:"file:///tmp/test-saved/notes.txt"}})
      test.check(!bridge.savingFiles.file && !!bridge.savedFiles.file, "successful save offers its directory without opening the file")
      if (test.mode === "transfer") {
        bridge.sendingConversations = {porch:true}
        bridge.transferProgress = {"upload:pending-file":{bytes:3000000000,total:6000000000}}
      }
    }
    if (["media", "panelmedia", "preview"].indexOf(test.mode) >= 0) {
      data = JSON.parse(JSON.stringify(data))
      data.self.hangout_id = "call"; data.self.muted = true
      data.hangouts = [{id:"call",label:"Porch",members:[{id:"self",display_name:"Tyler"},{id:"jared",display_name:"A very long friend name"}]}]
      data.self.media.camera.active = true; data.self.media.camera.viewers = ["Jared"]
      data.self.media.camera.devices = [{id:"fixture",label:"Fixture camera"}]
      data.self.media.screen_share.active = true
      data.self.media.remote_videos = [{participant:"A friend with a long name",source:"screen",requested_quality:"high"}]
      data.knocks = [{id:"knock",from:{id:"charlie",display_name:"Charlie with a long name"}}]
      bridge.applySnapshot(data)
    }
    if (test.mode === "empty") { data = JSON.parse(JSON.stringify(data)); data.conversations = []; data.messages = []; bridge.applySnapshot(data); bridge.closeConversation() }
    bridge.notificationMuted = true
    bridge.notificationVolume = 35
    bridge.notificationSoundPath = "file:///tmp/test-custom-sound.wav"
    if (test.mode === "settings" || test.mode === "themes") window.contentItem.children[0].children[0].settingsOpen = true
    if (test.mode === "panelsettings") compactContent.settingsOpen = true
  }
  Timer {
    interval: 600; running: test.mode === "themes"
    onTriggered: {
      var classic = test.findItem(window.contentItem, "theme-legacy")
      test.check(!!classic && classic.enabled, "Classic theme is available in Settings")
      var before = bridge.sent.length
      var draft = bridge.draftFor("porch")
      if (classic) classic.clicked()
      test.check(theme.profile === "legacy", "Settings selects Classic live")
      test.check(bridge.sent.length === before && bridge.draftFor("porch") === draft, "theme switching does not send commands or alter drafts")
    }
  }
  Timer {
    interval: 900; running: test.mode === "themes"
    onTriggered: {
      var terminal = test.findItem(window.contentItem, "theme-terminal")
      test.check(!!terminal && terminal.enabled, "Terminal remains available in Classic")
      if (terminal) terminal.clicked()
      test.check(theme.profile === "terminal", "Settings restores Terminal live")
      test.check(bridge.draftFor("dm") === "", "chat state remains unchanged")
    }
  }
  Timer {
    interval: 700; running: test.mode === "focus"
    onTriggered: {
      var button = test.findText(window.contentItem, "Chat options ▾")
      test.check(!!button && !!button.forceActiveFocus, "focusable chat button")
      if (button) button.forceActiveFocus(Qt.TabFocusReason)
    }
  }
  Timer {
    interval: 650; running: ["clearroom","cleardm","roomsettings","newroom"].indexOf(test.mode) >= 0
    onTriggered: {
      if (test.mode === "clearroom" || test.mode === "cleardm") {
        clearDialog.confirm(test.mode === "clearroom" ? "porch" : "dm")
        test.check(clearDialog.forEveryone === (test.mode === "clearroom"), "global clearing requires room permission")
        clearDialog.clearing = bridge.clearChatHistory(clearDialog.conversationId, clearDialog.forEveryone)
        test.check(bridge.sent[bridge.sent.length-1].args.for_everyone === clearDialog.forEveryone, "clear request explicitly scopes deletion")
        bridge.finishRequest({id:"test-" + bridge.requestId,ok:false,error:{message:"Test failure — history was not cleared."}})
        test.check(!clearDialog.clearing && clearDialog.error !== "", "failed clearing remains retryable")
      } else if (test.mode === "roomsettings") {
        roomDialog.manage("porch")
        test.check(roomDialog.owner && roomDialog.admin, "owner has administration controls")
        test.check(roomDialog.invitees.length === 1 && roomDialog.invitees[0].id === "charlie", "only non-members offered invitations")
      } else roomDialog.createRoom()
    }
  }
  Timer {
    interval: 400; running: ["settings","themes","panelsettings","preview","empty","transfer"].indexOf(test.mode) < 0
    onTriggered: {
      var target = test.compactMode ? compactSurface : window.contentItem
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
    interval: 650; running: test.mode === "menu"
    onTriggered: {
      var button = test.findText(window.contentItem, "Chat options ▾")
      test.check(!!button && !!button.clicked, "chat options button available")
      if (button && button.clicked) button.clicked()
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
    interval: 600; running: test.mode === "friends"
    onTriggered: {
      var star = test.findItem(compactSurface, "favorite-charlie")
      var before = bridge.sent.length
      test.check(!!star, "favorite action available in tray")
      if (star) star.clicked()
      test.check(bridge.sortedFriends[0].id === "charlie", "offline favorite precedes online non-favorite")
      var collapse = test.findItem(compactSurface, "friends-collapse")
      if (collapse) collapse.clicked()
      test.check(bridge.friendPreferences.collapsed && !test.findItem(compactSurface, "favorite-jared"), "collapse hides rows")
      if (collapse) collapse.clicked()
      test.check(!!test.findItem(compactSurface, "favorite-jared"), "expand restores rows")
      test.check(bridge.sent.length === before, "favorite and collapse do not join rooms or send commands")
      var scroll = test.findItem(compactSurface, "dashboardScroll")
      var audio = test.findItem(compactSurface, "globalAudioControls")
      if (scroll && audio) {
        var y = audio.mapToItem(compactSurface, 0, 0).y
        scroll.contentY = scroll.contentHeight
        test.check(audio.mapToItem(compactSurface, 0, 0).y === y, "audio controls stay pinned while scrolling")
        scroll.contentY = 0
      } else test.check(false, "scroll and global audio exist")
    }
  }
  Timer {
    interval: 1200; running: true
    onTriggered: {
      var surface = test.compactMode ? compactSurface : window.contentItem
      if (test.mode !== "preview") {
        var audio = test.findItem(surface, "globalAudioControls")
        test.check(!!audio && audio.width > 0, "mute/deafen available with or without a room and in settings")
        if (audio) {
          var position = audio.mapToItem(surface, 0, 0)
          test.check(position.y >= 0 && position.y + audio.height < surface.height, "audio controls inside window")
        }
      }
      test.check(window.width === theme.space(Quickshell.env("WISP_TEST_CONSTRAINED") === "1" ? 840 : 1180), "app width")
      test.check(window.height === theme.space(Quickshell.env("WISP_TEST_CONSTRAINED") === "1" ? 700 : 900), "app height")
      var path = Quickshell.env("WISP_CHAT_SCREENSHOT")
      var target = test.compactMode ? compactSurface : window.contentItem.children[0]
      if (test.mode === "preview") target = preview.contentItem.children[0]
      if (test.mode === "menu") {
        var menu = test.findObject(window.contentItem, "wispChatOptions", [])
        test.check(!!menu && menu.opened, "chat options menu opened")
        if (menu) target = menu.contentItem.parent
      }
      if (test.mode === "clearroom" || test.mode === "cleardm") target = clearDialog.contentItem.parent
      if (test.mode === "roomsettings" || test.mode === "newroom") target = roomDialog.contentItem.parent
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
