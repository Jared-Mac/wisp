import QtQuick
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components
import "app/ChatTiles.js" as Tiles

ShellRoot {
  id: test
  property bool failed: false
  readonly property string mode: Quickshell.env("WISP_CHAT_FIXTURE_MODE")
  readonly property real testWidth: Number(Quickshell.env("WISP_TEST_WIDTH")) || (Quickshell.env("WISP_TEST_CONSTRAINED") === "1" ? 840 : 1180)
  readonly property real testHeight: Number(Quickshell.env("WISP_TEST_HEIGHT")) || (Quickshell.env("WISP_TEST_CONSTRAINED") === "1" ? 700 : 900)
  readonly property bool compactMode: mode === "panelimagegeometry" || mode === "panellatest" || mode === "panelaudiotooltips" || mode === "panelpresence" || mode === "traycollapse" || mode === "panel" || mode === "panelmedia" || mode === "panelsettings" || mode === "friends" || mode === "panelidentity" || mode === "panelidentityactions"
  function setImageFixture(w,h) {
    var data=JSON.parse(JSON.stringify(bridge.snapshot))
    bridge.chatImageUrls={"geometry":"data:image/svg+xml,"+encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="'+w+'" height="'+h+'"><rect width="'+w+'" height="'+h+'" fill="#254261"/><circle cx="40" cy="40" r="25" fill="#e6b75b"/><path d="M0 0L'+w+' '+h+'" stroke="#74c9d6" stroke-width="5"/></svg>')}
    data.messages=[{id:"geometry",conversation_id:"porch",sender:{id:"jared",display_name:"Jared"},created_at:"2026-09-03T17:04:00Z",content_type:"image/png",payload:{width:w,height:h,caption:"Original "+w+" × "+h}}]
    bridge.snapshot=data
  }
  property real readingPosition: 0
  function appendScrollFixture() {
    var data=JSON.parse(JSON.stringify(bridge.snapshot))
    data.messages.push({id:"scroll-"+data.messages.length,conversation_id:"porch",sender:{id:"jared",display_name:"Jared"},created_at:"2026-09-03T17:04:00Z",content_type:"text/plain",payload:"Another message in the conversation"})
    bridge.snapshot=data
  }
  property real trayInitialFeed: 0
  property real trayFriendsFeed: 0
  property int trayCommandCount: 0
  property var popupTarget: null
  property var tileKeys: []
  property int tileCommands: 0
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
      if (test.mode === "cleantui" && "profile" in theme) theme.profile = "clean_tui"
      if (Quickshell.env("WISP_TEST_ADAPTER") === "omarchy") {
        // Representative host overrides, not Jared's settings or machine.
        tuiTreatment = true
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
    implicitWidth: theme.space(test.testWidth)
    implicitHeight: theme.space(test.testHeight)
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
      Components.SurfaceOutline { theme: theme; radius: 0 }
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
    if (Quickshell.env("WISP_TEST_DOCK")) bridge.workspaceLayout.dock = Quickshell.env("WISP_TEST_DOCK")
    bridge.workspaceLayout.activityCollapsed = Quickshell.env("WISP_TEST_COLLAPSED") === "1"
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
    if (test.mode === "roomcleanup") {
      data.conversations.push(
        {id:"hangout:ended-empty",kind:"hangout",label:"Room",unread_count:0,last_message:null},
        {id:"hangout:ended-history",kind:"hangout",label:"Room",unread_count:0,
          last_message:{id:"history",conversation_id:"hangout:ended-history",sender:{id:"jared",display_name:"Jared"},created_at:"2026-09-03T17:03:00Z",content_type:"text/plain",payload:"Saved history"}},
        {id:"hangout:active-empty",kind:"hangout",label:"Room",unread_count:0,last_message:null}
      )
      data.hangouts = [{id:"active-empty",label:null,members:[],sharing:[]}]
    }
    data.friends = [{id:"jared",display_name:"Jared",online:true,presence:"open"}, {id:"charlie",display_name:"Charlie",online:false,presence:"away"}]
    if (test.mode === "presence" || test.mode === "panelpresence") {
      data.friends = [
        {id:"jared",display_name:"Jared",online:true,presence:"open"},
        {id:"charlie",display_name:"Charlie",online:true,presence:"knock"},
        {id:"tyler",display_name:"Tyler",online:true,presence:"closed"},
        {id:"morgan",display_name:"Morgan",online:true,presence:"away"},
        {id:"jack",display_name:"Jack",online:false,presence:"closed"}
      ]
      bridge.workspaceLayout.activityRatio = 0.2
    }
    if (test.mode === "traycollapse") data.spots = [{id:"porch",name:"Porch",members:[]}, {id:"games",name:"Games",members:[]}]
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
    if (test.mode === "roomcleanup") {
      test.check(!bridge.conversationById("hangout:ended-empty"), "ended empty temporary rooms are hidden")
      test.check(!!bridge.conversationById("hangout:ended-history"), "ended rooms with visible history remain available")
      test.check(!!bridge.conversationById("hangout:active-empty"), "active empty rooms remain available")
      test.check(bridge.snapshot.conversations.length === 6, "room cleanup is a non-destructive presentation filter")
      bridge.selectConversation("hangout:active-empty")
      var ended = JSON.parse(JSON.stringify(bridge.snapshot))
      ended.hangouts = []
      bridge.applySnapshot(ended)
      test.check(bridge.activeConversationId === "", "an open empty room closes when its call ends")
    }
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
    if (test.mode === "settings" || test.mode === "themes" || test.mode === "saved") test.findItem(window.contentItem, "wispContent").toggleSettings()
    if (test.mode === "panelsettings") compactContent.toggleSettings()
  }
  Timer {
    interval: 300; running: test.mode === "workspace"
    onTriggered: bridge.workspaceLayout.reset()
  }
  Timer {
    interval: 300; running: test.mode === "cleantui"
    onTriggered: bridge.workspaceLayout.reset()
  }
  Timer {
    interval: 400; running: test.mode === "presence" || test.mode === "panelpresence"
    onTriggered: if (bridge.friendPreferences.collapsed) bridge.friendPreferences.toggleCollapsed()
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
      var before = bridge.sent.length
      var draft = bridge.draftFor("porch")
      var terminal = test.findItem(window.contentItem, "theme-terminal")
      test.check(!!terminal && terminal.enabled, "Terminal remains available in Classic")
      if (terminal) terminal.clicked()
      test.check(theme.profile === "terminal", "Settings restores Terminal live")
      test.check(bridge.draftFor("dm") === "", "chat state remains unchanged")
      var clean = test.findItem(window.contentItem, "theme-clean_tui")
      test.check(!!clean && clean.enabled, "Clean TUI is available beside Terminal and Classic")
      if (clean) clean.clicked()
      test.check(theme.profile === "clean_tui" && theme.cleanTui && theme.tui,
        "Settings selects the independent Clean TUI interface live")
      test.check(bridge.sent.length === before && bridge.draftFor("porch") === draft,
        "Clean TUI switching preserves drafts and sends no commands")
      var performative = test.findItem(window.contentItem, "palette-performative")
      test.check(!!performative && performative.enabled, "Performative palette is available in Settings")
      if (performative) performative.clicked()
      test.check(theme.paletteName === "performative" && theme.background == "#000000", "Settings selects Performative live")
      test.check(bridge.sent.length === before && bridge.draftFor("porch") === draft, "palette switching preserves drafts and sends no commands")
      var herdr = test.findItem(window.contentItem, "palette-herdr")
      test.check(!!herdr && herdr.enabled, "Herdr palette is available in Settings")
      if (herdr) herdr.clicked()
      test.check(theme.paletteName === "herdr" && theme.background == "#001419"
        && theme.accent == "#29a298" && theme.tui, "Settings selects Herdr live")
      test.check(bridge.sent.length === before && bridge.draftFor("porch") === draft, "Herdr switching preserves drafts and sends no commands")
      themeAppearance.setPalette(Quickshell.env("WISP_TEST_PALETTE") || "wisp")
    }
  }
  Timer {
    interval: 700; running: test.mode === "focus"
    onTriggered: {
      var button = test.findItem(window.contentItem, "chatOptionsButton")
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
    interval: 400; running: ["settings","themes","panelsettings","preview","empty","transfer","saved"].indexOf(test.mode) < 0
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
      var button = test.findItem(window.contentItem, "chatOptionsButton")
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
    interval: 600; running: ["identity", "panelidentity", "identityactions", "panelidentityactions"].indexOf(test.mode) >= 0
    onTriggered: {
      var target = test.compactMode ? compactSurface : window.contentItem
      var button = test.findItem(target, "identityMenuButton")
      var before = bridge.sent.length
      test.check(!!button && button.width > theme.space(100), "identity is one generous click target")
      if (button) button.clicked()
      var menu = test.findObject(target, "identityMenu", [])
      test.check(!!menu && menu.opened, "identity click opens menu")
      test.check(!!menu && menu.itemAt(0).objectName === "identitySettings", "Settings is in identity menu")
      test.check(!!menu && menu.itemAt(1).objectName === "identityNewRoom", "New Room is in identity menu")
      test.check(!!menu && menu.count === 2, "identity menu contains only Settings and New Room")
      if (menu) {
        var frame = test.findItem(menu.background, "wispSurfaceOutline")
        test.check(!!frame === !theme.hostManaged, "Wisp popup outline preserves host-managed frames")
        if (frame) test.check(frame.border.width === 1 && frame.width === menu.background.width, "popup outline follows full background")
        menu.currentIndex = 0
      }
      test.check(bridge.sent.length === before, "opening menu does not activate media or join")
    }
  }
  Timer {
    interval: 800; running: test.mode === "identityactions" || test.mode === "panelidentityactions"
    onTriggered: {
      var target = test.compactMode ? compactSurface : window.contentItem
      var menu = test.findObject(target, "identityMenu", [])
      var settings = menu ? menu.itemAt(0) : null
      if (settings) settings.triggered()
      if (menu) menu.close()
      var content = test.findItem(target, "wispContent")
      test.check(!!content && content.settingsOpen, "Settings action opens settings")
      var home = test.findItem(target, "headerHomeButton")
      test.check(!!home && home.width === theme.space(30), "Settings exposes a compact Home icon")
      var appButton = test.findItem(target, "headerOpenAppButton")
      if (home && appButton) {
        home.parent.forceLayout()
        test.check(home.x + home.width <= appButton.x, "Home sits left of Open app")
      }
      var savedDraft = bridge.draftFor("porch")
      var navigationCommands = bridge.sent.length
      if (home) home.clicked()
      test.check(content.showingChats && !test.findItem(target, "headerHomeButton"), "Home returns to chats and hides itself")
      test.check(bridge.draftFor("porch") === savedDraft && bridge.sent.length === navigationCommands, "Home preserves drafts and sends no commands")
      if (settings) settings.triggered()
      var before = bridge.sent.length
      var newRoom = menu ? menu.itemAt(1) : null
      if (newRoom) newRoom.triggered()
      var dialog = test.findObject(target, "identityRoomManager", [])
      test.check(!!dialog && dialog.opened && dialog.creating, "New Room opens creation dialog")
      test.check(bridge.sent.length === before, "opening New Room does not create or join a room")
    }
  }
  Timer {
    interval: 600; running: test.mode === "traycollapse"
    onTriggered: {
      test.trayInitialFeed = test.findFeed(compactSurface).height
      test.trayCommandCount = bridge.sent.length
      test.findItem(compactSurface, "friends-collapse").clicked()
    }
  }
  Timer {
    interval: 750; running: test.mode === "traycollapse"
    onTriggered: {
      test.trayFriendsFeed = test.findFeed(compactSurface).height
      test.check(test.trayFriendsFeed > test.trayInitialFeed, "collapsing tray friends enlarges message history")
      test.findItem(compactSurface, "rooms-collapse").clicked()
    }
  }
  Timer {
    interval: 900; running: test.mode === "traycollapse"
    onTriggered: {
      test.check(test.findFeed(compactSurface).height > test.trayFriendsFeed, "collapsing tray rooms enlarges message history")
      test.check(!test.findItem(compactSurface, "availableRoomCard"), "collapsed rooms hide room cards")
      test.check(bridge.sent.length === test.trayCommandCount && bridge.draftFor("porch") === "Here's the latest version…", "section collapse preserves draft and sends no commands")
      test.findItem(compactSurface, "rooms-collapse").clicked()
      test.findItem(compactSurface, "friends-collapse").clicked()
    }
  }
  Timer {
    interval: 1050; running: test.mode === "traycollapse"
    onTriggered: {
      test.check(Math.abs(test.findFeed(compactSurface).height - test.trayInitialFeed) < 1, "expanding sections restores original chat allocation")
      test.check(!!test.findItem(compactSurface, "availableRoomCard"), "room cards restored")
      test.findItem(compactSurface, "rooms-collapse").clicked()
      test.findItem(compactSurface, "friends-collapse").clicked()
    }
  }
  Timer {
    interval: 600; running: test.mode === "friends"
    onTriggered: {
      var star = test.findItem(compactSurface, "favorite-charlie")
      var before = bridge.sent.length
      test.check(!!star, "favorite action available in tray")
      if (star) {
        var label = test.findItem(star.parent, "friendName")
        test.check(!!label && star.x >= label.x + label.width, "star is to the right of the name")
        test.check(star.parent.height === theme.space(theme.tui ? 28 : 32), "friends use compact rows")
        test.check(star.opacity === 0, "favorite star hidden without hover or focus")
        star.forceActiveFocus(Qt.TabFocusReason)
        test.check(star.opacity === 1, "keyboard focus reveals favorite action")
      }
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
    interval:350; running:test.mode==="imagegeometry" || test.mode==="panelimagegeometry"
    onTriggered:test.setImageFixture(120,80)
  }
  Timer {
    interval:550; running:test.mode==="imagegeometry" || test.mode==="panelimagegeometry"
    onTriggered:{
      var preview=test.findItem(test.compactMode?compactSurface:window.contentItem,"chatImagePreview-geometry")
      test.check(Math.abs(preview.width*preview.pixelRatio-120)<1 && Math.abs(preview.height*preview.pixelRatio-80)<1,"small images retain native display size without upscaling")
      test.setImageFixture(80,1600)
    }
  }
  Timer {
    interval:750; running:test.mode==="imagegeometry" || test.mode==="panelimagegeometry"
    onTriggered:{
      var surface=test.compactMode?compactSurface:window.contentItem
      var preview=test.findItem(surface,"chatImagePreview-geometry"),list=test.findItem(surface,"messageList")
      test.check(preview.height<=list.height+1 && Math.abs(preview.width/preview.height-0.05)<0.001,"tall preview shrinks uniformly to fit chat height")
      test.setImageFixture(2400,600)
    }
  }
  Timer {
    interval:900; running:test.mode==="imagegeometry" || test.mode==="panelimagegeometry"
    onTriggered:{
      var surface=test.compactMode?compactSurface:window.contentItem
      var preview=test.findItem(surface,"chatImagePreview-geometry"),list=test.findItem(surface,"messageList")
      test.check(preview.width<=list.width+1 && Math.abs(preview.width/preview.height-4)<0.001,"ultrawide preview fits chat width without distortion")
      test.findItem(preview,"openChatImage-geometry").clicked(null)
    }
  }
  Timer {
    interval:1020; running:test.mode==="imagegeometry" || test.mode==="panelimagegeometry"
    onTriggered:{
      var surface=test.compactMode?compactSurface:window.contentItem
      var viewer=test.findObject(test.findFeed(surface),"chatImageViewer",[])
      var picture=test.findItem(viewer.contentItem,"nativeChatImage"),viewport=test.findItem(viewer.contentItem,"imageViewerViewport")
      test.check(viewer.visible && viewer.imageStatus===Image.Ready && !viewer.fitToWindow,"click opens separate image window at native scale")
      test.check(picture.sourceSize.width===2400 && picture.sourceSize.height===600 && Math.abs(picture.width*viewer.pixelRatio-2400)<1,"viewer retains full source resolution at one image pixel per display pixel")
      test.check(viewport.contentWidth>viewport.width,"oversized native image can be scrolled")
      test.check(viewer.width>theme.space(280),"viewer sizes itself after original image dimensions load")
      var path=Quickshell.env("WISP_CHAT_SCREENSHOT")
      if(path)viewer.contentItem.children[0].grabToImage(function(result){result.saveToFile(path.replace(".png","-viewer.png"))})
      test.findItem(viewer.contentItem,"imageFitWindow").clicked()
    }
  }
  Timer {
    interval:1130; running:test.mode==="imagegeometry" || test.mode==="panelimagegeometry"
    onTriggered:{
      var surface=test.compactMode?compactSurface:window.contentItem
      var viewer=test.findObject(test.findFeed(surface),"chatImageViewer",[])
      var picture=test.findItem(viewer.contentItem,"nativeChatImage"),viewport=test.findItem(viewer.contentItem,"imageViewerViewport")
      test.check(picture.width<=viewport.width+1 && picture.height<=viewport.height+1 && Math.abs(picture.width/picture.height-4)<0.001,"Fit preserves image proportions")
      var copy=test.findItem(viewer.contentItem,"imageCopyButton"),before=bridge.sent.length
      test.check(copy && copy.enabled && !copy.text,"viewer has an icon-only Copy to Clipboard action")
      copy.clicked()
      test.check(bridge.sent.length===before+1 && bridge.sent[before].name==="copy_chat_image" && bridge.sent[before].args.message_id==="geometry","copy uses original message image even in Fit mode")
      viewer.copyImage()
      test.check(bridge.sent.length===before+1 && !copy.enabled,"copy prevents duplicate pending requests")
      bridge.finishRequest({id:viewer.copyRequest,ok:false,error:{message:"Clipboard unavailable"}})
      test.check(copy.enabled && viewer.copyError==="Clipboard unavailable" && !viewer.copied,"copy failure is visible and retryable")
      copy.clicked()
      bridge.finishRequest({id:viewer.copyRequest,ok:true,value:{copied:true}})
      test.check(viewer.copied && !viewer.copyError,"successful copy shows confirmation")
      test.findItem(viewer.contentItem,"imageNativeSize").clicked()
      test.check(!viewer.fitToWindow && Math.abs(picture.width*viewer.pixelRatio-2400)<1,"100% restores original size")
      viewer.contentItem.Window.window.close()
      test.check(!viewer.visible && !viewer.imageSource.toString(),"closing viewer releases its image without changing chat")
    }
  }
  Timer {
    interval:400; running:test.mode==="latest" || test.mode==="panellatest"
    onTriggered:{
      var data=JSON.parse(JSON.stringify(bridge.snapshot))
      for(var i=0;i<45;i++)data.messages.push({id:"scroll-"+i,conversation_id:"porch",sender:{id:"jared",display_name:"Jared"},created_at:"2026-09-03T17:04:00Z",content_type:"text/plain",payload:"Earlier message "+i+" in this conversation."})
      bridge.snapshot=data
    }
  }
  Timer {
    interval:600; running:test.mode==="latest" || test.mode==="panellatest"
    onTriggered:{
      var feed=test.findFeed(test.compactMode?compactSurface:window.contentItem)
      var list=test.findItem(feed,"messageList")
      test.check(!feed.awayFromLatest && !test.findItem(feed,"scrollToLatestButton"),"latest button hidden while following newest messages")
      list.movementStarted();list.positionViewAtBeginning();list.movementEnded()
      test.readingPosition=list.contentY
      test.check(feed.awayFromLatest && !!test.findItem(feed,"scrollToLatestButton"),"scrolling up reveals latest button")
      test.appendScrollFixture()
    }
  }
  Timer {
    interval:750; running:test.mode==="latest" || test.mode==="panellatest"
    onTriggered:{
      var feed=test.findFeed(test.compactMode?compactSurface:window.contentItem)
      var list=test.findItem(feed,"messageList")
      test.check(Math.abs(list.contentY-test.readingPosition)<1 && feed.awayFromLatest,"incoming messages do not pull the reader down")
      var button=test.findItem(feed,"scrollToLatestButton"),before=bridge.sent.length
      test.check(button.x>=0 && button.y>=0 && button.x+button.width<=feed.width && button.y+button.height<=feed.height,"latest button stays within feed")
      button.clicked()
      test.check(!feed.awayFromLatest && list.followBottom,"click scrolls to bottom and restores following")
      test.check(bridge.sent.length===before,"latest button is a local scrolling action")
      test.appendScrollFixture()
    }
  }
  Timer {
    interval:950; running:test.mode==="latest" || test.mode==="panellatest"
    onTriggered:{
      var feed=test.findFeed(test.compactMode?compactSurface:window.contentItem)
      var list=test.findItem(feed,"messageList")
      test.check(!feed.awayFromLatest,"new arrivals stay in view after jumping to latest")
      list.movementStarted();list.positionViewAtBeginning();list.movementEnded()
      feed.conversationId="dm"
    }
  }
  Timer {
    interval:1050; running:test.mode==="latest" || test.mode==="panellatest"
    onTriggered:{
      var feed=test.findFeed(test.compactMode?compactSurface:window.contentItem)
      test.check(!feed.awayFromLatest,"switching to a short or empty chat hides the button")
      feed.conversationId="porch"
    }
  }
  Timer {
    interval:1125; running:test.mode==="latest" || test.mode==="panellatest"
    onTriggered:{
      var list=test.findItem(test.findFeed(test.compactMode?compactSurface:window.contentItem),"messageList")
      list.movementStarted();list.positionViewAtBeginning();list.movementEnded()
    }
  }
  Timer {
    interval:600; running:test.mode==="audiotooltips" || test.mode==="panelaudiotooltips"
    onTriggered:{
      var surface=test.compactMode?compactSurface:window.contentItem
      var audio=test.findItem(surface,"globalAudioControls")
      audio.muted=true;audio.deafened=true
      test.findObject(audio,"muteTooltip",[]).open()
    }
  }
  Timer {
    interval:750; running:test.mode==="audiotooltips" || test.mode==="panelaudiotooltips"
    onTriggered:{
      var surface=test.compactMode?compactSurface:window.contentItem
      var audio=test.findItem(surface,"globalAudioControls")
      var tip=test.findObject(audio,"muteTooltip",[])
      var control=test.findItem(audio,"muteControl")
      var p=tip.contentItem.mapToItem(surface,0,0)
      test.check(tip.visible && p.y>=control.mapToItem(surface,0,control.height).y,"mute tooltip appears below its button")
      test.check(p.x>=0 && p.x+tip.contentItem.width<=surface.width && p.y+tip.contentItem.height<=surface.height,"mute tooltip fits window")
      var path=Quickshell.env("WISP_CHAT_SCREENSHOT")
      if(path)surface.grabToImage(function(result){result.saveToFile(path.replace(".png","-mute.png"))})
      tip.close()
      test.findObject(audio,"deafenTooltip",[]).open()
    }
  }
  Timer {
    interval:950; running:test.mode==="audiotooltips" || test.mode==="panelaudiotooltips"
    onTriggered:{
      var surface=test.compactMode?compactSurface:window.contentItem
      var audio=test.findItem(surface,"globalAudioControls")
      var tip=test.findObject(audio,"deafenTooltip",[])
      var control=test.findItem(audio,"deafenControl")
      var p=tip.contentItem.mapToItem(surface,0,0)
      test.check(tip.visible && p.y>=control.mapToItem(surface,0,control.height).y,"deafen tooltip appears below its button")
      test.check(p.x>=0 && p.x+tip.contentItem.width<=surface.width && p.y+tip.contentItem.height<=surface.height,"deafen tooltip fits window")
      var path=Quickshell.env("WISP_CHAT_SCREENSHOT")
      if(path)tip.contentItem.parent.grabToImage(function(result){result.saveToFile(path.replace(".png","-deafen.png"))})
    }
  }
  Timer {
    interval:600; running:test.mode==="shortcuts"
    onTriggered:{
      var content=test.findItem(window.contentItem,"wispContent")
      content.forceActiveFocus()
      var before=bridge.sent.length
      keyDriver.keyClick(Qt.Key_M,Qt.NoModifier)
      keyDriver.keyClick(Qt.Key_D,Qt.NoModifier)
      test.check(bridge.sent.length===before,"unmodified M/D do not toggle audio")
      keyDriver.keyClick(Qt.Key_M,Qt.ShiftModifier)
      keyDriver.keyClick(Qt.Key_D,Qt.ShiftModifier)
      test.check(bridge.sent.length===before+2 && bridge.sent[before].name==="toggle_muted" && bridge.sent[before+1].name==="toggle_deafened","Shift+M/D toggle audio once")
      content.handleWindowKey({key:Qt.Key_M,modifiers:Qt.ShiftModifier,isAutoRepeat:true,accepted:false})
      keyDriver.keyClick(Qt.Key_M,Qt.ControlModifier|Qt.ShiftModifier)
      test.check(bridge.sent.length===before+2,"repeats and unrelated chords do not toggle audio")
      var editor=test.findItem(window.contentItem,"mainComposerEditor")
      editor.forceActiveFocus();editor.text=""
      keyDriver.keyClick("M",Qt.ShiftModifier)
      keyDriver.keyClick("D",Qt.ShiftModifier)
      test.check(editor.text==="MD" && bridge.sent.length===before+2,"capital M/D remain normal text in chat editors: text="+editor.text+", commands="+(bridge.sent.length-before))
    }
  }
  TestCase { id:keyDriver; parent:window.contentItem; name:"WindowKeys"; when:false }
  Timer {
    interval:600; running:test.mode==="addchat"
    onTriggered:{
      var chat=test.findItem(window.contentItem,"conversationPane")
      chat.commit({key:"add-source",id:"porch"});chat.activate("add-source")
      var original=Tiles.leaves(chat.tree)[0], originalId=original.id
      var pane=test.findItem(chat,"chatTile-"+original.key)
      var add=test.findItem(window.contentItem,"headerAddChatButton")
      var audio=test.findItem(window.contentItem,"globalAudioControls")
      test.check(add && add.visible && add.enabled && add.parent===audio.parent && add.x<audio.x,"Add Chat is in the header just left of mute/deafen")
      test.check(!test.findItem(pane,"addChatTileButton"),"chat panes have no Add Chat button")
      add.clicked()
      var picker=test.findObject(window.contentItem,"headerAddChatPicker",[])
      test.check(picker.visible,"Add chat opens searchable conversation picker")
      picker.chosen("dm");picker.close()
      test.check(chat.paneCount===2 && Tiles.find(chat.tree,original.key).id===originalId && Tiles.find(chat.tree,chat.activeKey).id==="dm","existing chat adds a tile without replacing the source")
      add.clicked();test.findItem(picker.contentItem,"pickerNewChat").clicked()
      var dialog=test.findObject(window.contentItem,"headerNewChatDialog",[])
      dialog.close()
      test.check(chat.paneCount===2,"canceling New chat adds no tile")
      add.clicked();test.findItem(picker.contentItem,"pickerNewChat").clicked()
      dialog.toggleFriend("jared");dialog.submit()
      bridge.finishRequest({id:dialog.requestId,ok:true,value:{id:"dm",kind:"direct",label:"Jared",members:[],unread_count:0}})
      test.check(chat.paneCount===3 && Tiles.find(chat.tree,original.key).id===originalId,"New DM from Add chat opens in a new tile")
      test.findItem(pane,"compactChatSelector").clicked()
      var panePicker=test.findObject(pane,"chatConversationMenu",[])
      panePicker.chosen("dm");panePicker.close()
      test.check(chat.paneCount===3 && Tiles.find(chat.tree,original.key).id==="dm","current-chat selector still replaces only its pane")
      while(chat.paneCount<8)chat.addConversation(original.key,"dm")
      test.check(!add.enabled,"Add chat disables at eight tiles")
      chat.addConversation(original.key,"dm")
      test.check(chat.paneCount===8,"tile limit is enforced")
      while(chat.paneCount>2)chat.closePane(Tiles.leaves(chat.tree)[chat.paneCount-1].key)
      add.clicked()
      var position=add.mapToItem(window.contentItem,picker.x,0)
      test.check(position.x>=0 && position.x+picker.width<=window.width,"header picker stays inside the window")
    }
  }
  Timer {
    interval:600; running:test.mode==="newchat"
    onTriggered:{
      var chat=test.findItem(window.contentItem,"conversationPane")
      var original=Tiles.leaves(chat.tree)[0].key
      chat.split(original,"right")
      test.tileKeys=[original,chat.activeKey]
      var pane=test.findItem(chat,"chatTile-"+original)
      test.findItem(pane,"compactChatSelector").clicked()
      var picker=test.findObject(pane,"chatConversationMenu",[])
      var before=bridge.sent.length
      test.findItem(picker.contentItem,"pickerNewChat").clicked()
      var dialog=test.findObject(pane,"newChatDialog",[])
      test.popupTarget=dialog
      test.check(dialog.visible && !dialog.canSubmit,"New chat opens an empty selection form")
      dialog.close()
      test.check(bridge.sent.length===before,"Cancel creates nothing")
      dialog.begin(); dialog.toggleFriend("jared"); dialog.submit()
      test.check(dialog.busy && bridge.sent[bridge.sent.length-1].name==="open_direct","DM submits existing direct-message API")
      before=bridge.sent.length;dialog.submit()
      test.check(bridge.sent.length===before,"busy guard prevents duplicate submissions")
      bridge.finishRequest({id:dialog.requestId,ok:false,error:{message:"Fixture failure"}})
      test.check(!dialog.busy && dialog.selectedIds[0]==="jared" && dialog.error,"failed DM retains selected friend")
      dialog.submit()
      bridge.finishRequest({id:dialog.requestId,ok:true,value:{id:"dm",kind:"direct",label:"Jared",members:[],unread_count:0}})
      test.check(!dialog.visible && Tiles.find(chat.tree,original).id==="dm","DM opens in its originating pane")
      dialog.begin();dialog.group=true
      test.findItem(dialog.contentItem,"newChatName").text="Weekend games"
      dialog.toggleFriend("jared");dialog.toggleFriend("charlie")
      test.check(dialog.canSubmit,"offline friends can be selected for group chats")
      dialog.submit()
      var args=bridge.sent[bridge.sent.length-1].args
      test.check(bridge.sent[bridge.sent.length-1].name==="create_group" && args.name==="Weekend games" && args.members.length===2 && args.request_id,"group submits name, members and retry token")
      bridge.finishRequest({id:dialog.requestId,ok:false,error:{message:"Group chats require a server update."}})
      test.check(!dialog.busy && dialog.selectedIds.length===2 && dialog.error.indexOf("server update")>=0,"older server error keeps form intact")
      var path=Quickshell.env("WISP_CHAT_SCREENSHOT")
      if(path)dialog.contentItem.parent.grabToImage(function(result){result.saveToFile(path.replace(".png","-dialog.png"))})
    }
  }
  Timer {
    interval:1000; running:test.mode==="newchat"
    onTriggered:{
      var dialog=test.popupTarget,token=dialog.groupRequestId
      dialog.submit()
      test.check(bridge.sent[bridge.sent.length-1].args.request_id===token,"retry uses same creation token")
      bridge.finishRequest({id:dialog.requestId,ok:true,value:{id:"new-group",kind:"circle",label:"Weekend games",members:[],unread_count:0}})
      var chat=test.findItem(window.contentItem,"conversationPane")
      test.check(!dialog.visible && Tiles.find(chat.tree,test.tileKeys[0]).id==="new-group","group result opens in originating pane")
      test.check(bridge.conversationById("new-group"),"creation response is immediately available in picker")
      test.check(!bridge.sent.some(function(c){return ["create_room","join_spot","join_hangout","camera","share"].indexOf(c.name)>=0}),"creating chats never joins rooms or publishes media")
      chat.activate(test.tileKeys[0])
    }
  }
  Timer {
    interval: 600; running: test.mode === "picker"
    onTriggered: {
      var selector=test.findItem(window.contentItem,"compactChatSelector")
      test.check(selector && !test.findItem(window.contentItem,"chatTabs"),"dropdown is default even in full-width chat")
      var data=JSON.parse(JSON.stringify(bridge.snapshot))
      for (var i=0;i<80;i++) data.conversations.push({id:"room-"+i,kind:"hangout",label:"Room "+String(i).padStart(2,"0"),spot_id:"room-"+i,unread_count:i%3})
      data.conversations.push({id:"closed-friend",kind:"direct",label:"Archived Friend",tab_closed:true,unread_count:5})
      bridge.snapshot=data
      selector.clicked()
    }
  }
  Timer {
    interval: 750; running: test.mode === "picker"
    onTriggered: {
      var picker=test.findObject(window.contentItem,"chatConversationMenu",[])
      test.check(picker.opened && picker.count===84,"picker includes long lists and closed chats")
      test.check(picker.rows.filter(function(r) { return r.section }).map(function(r) { return r.label }).join(",")==="Rooms,Friends List","picker categories are Rooms and Friends List")
      test.check(test.findItem(picker.contentItem,"conversationSearch").activeFocus,"opening picker focuses search")
      test.check(picker.height<=theme.space(420),"long list has bounded scrolling height")
      var before=bridge.sent.length
      picker.category="Friends List"
      test.check(picker.count===3 && picker.rows[0].label==="Friends List","category filter jumps directly to friend chats")
      picker.category="Rooms"
      test.check(picker.count===81,"category filter shows only room chats")
      picker.category=""
      picker.query="  jArEd  "
      test.check(picker.count===1 && picker.rows[1].id==="dm","search is trimmed and case-insensitive")
      picker.resetSelection(); picker.moveSelection(1)
      test.check(bridge.sent.length===before,"search and navigation do not send commands")
      picker.chooseCurrent()
      test.check(bridge.activeConversationId==="dm" && !picker.visible,"Enter-style selection opens matched DM")
      picker.open()
    }
  }
  Timer {
    interval: 1000; running: test.mode === "picker"
    onTriggered: {
      var picker=test.findObject(window.contentItem,"chatConversationMenu",[])
      test.check(picker.query==="","search resets when reopened")
      picker.query="no-such-conversation"
      test.check(picker.count===0 && picker.rows.length===0,"no-results state hides empty categories")
      picker.resetSelection(); picker.moveSelection(1); picker.chooseCurrent()
      test.check(picker.visible,"Enter with no matches does nothing")
      picker.query="Archived Friend"; picker.resetSelection(); picker.chooseCurrent()
      test.check(bridge.sent.some(function(c) { return c.name==="set_conversation_tab" && c.args.conversation_id==="closed-friend" && c.args.closed===false }),"search reopens a closed DM without clearing history")
      picker.open()
    }
  }
  Timer {
    interval: 500; running: test.mode === "tilemoves"
    onTriggered: {
      var pair={key:"pair",axis:"x",ratio:0.4,a:{key:"a",id:"porch"},b:{key:"b",id:"dm"}}
      var rects=Tiles.geometry(pair,1000,900,10,280,230)
      var noop=Tiles.planDrop(pair,"a",rects.b.x+40,450,1000,900,10,280,230,26)
      test.check(noop && noop.unchanged && noop.rect.width===rects.a.width,"adjacent left-to-left drop previews its actual existing slot")
      test.check(JSON.stringify(Tiles.move(pair,"a","b","left","new"))===JSON.stringify(pair),"no-op drop preserves divider proportions")
      var ownTop=Tiles.planDrop(pair,"a",200,8,1000,900,10,280,230,26)
      test.check(ownTop && ownTop.whole && ownTop.edge==="top" && !ownTop.unchanged,"can move a column above its neighbor from its own top edge")
      var stacked=Tiles.move(pair,"a",ownTop.target,ownTop.edge,"new")
      test.check(stacked.axis==="y" && ownTop.rect.width===1000,"top-edge move spans whole workspace")
      var nextRects=Tiles.geometry(stacked,1000,900,10,280,230)
      test.check(JSON.stringify(ownTop.rect)===JSON.stringify(nextRects.a),"drop preview matches resulting geometry exactly")
      test.check(Tiles.dropEdge(30,8,450,900)==="top" && Tiles.dropEdge(30,892,450,900)==="bottom","tall-pane corners use reachable nearest-edge top/bottom zones")
      var group=Tiles.insert(Tiles.copy(pair),"b",{key:"c",id:"friends"},"bottom","group")
      var whole=Tiles.planDrop(group,"a",500,892,1000,900,10,280,230,26)
      var moved=Tiles.move(group,"a",whole.target,whole.edge,"outer")
      test.check(moved.axis==="y" && moved.b.key==="a" && moved.a.a && Tiles.leaves(moved).length===3,"workspace edge can move a pane below an entire nested group")
      test.check(Tiles.valid(moved),"group move remains a valid persistent tree")
      var chat=test.findItem(window.contentItem,"conversationPane")
      var populated=JSON.parse(JSON.stringify(bridge.snapshot))
      populated.conversations.push({id:"long-room",kind:"circle",label:"A room with a much longer name",unread_count:12})
      bridge.snapshot=populated
      bridge.workspaceLayout.activityCollapsed=true
      chat.commit(pair)
      chat.choose("a","porch")
    }
  }
  Timer {
    interval: 800; running: test.mode === "tilemoves"
    onTriggered: {
      var chat=test.findItem(window.contentItem,"conversationPane")
      var pane=test.findItem(chat,"chatTile-a")
      var selector=test.findItem(pane,"compactChatSelector")
      test.check(selector && selector.text==="Porch","pane shows selected chat instead of clipping tabs")
      if (selector) {
        test.check(selector.width>theme.space(80) && selector.x>=0 && selector.x+selector.width<=pane.width,"compact selector retains a readable click target")
        selector.clicked()
        var menu=test.findObject(pane,"chatConversationMenu",[])
        test.check(menu && menu.count===bridge.conversations.length,"selector offers every conversation")
        if (menu) menu.close()
      }
      pane.choose("friends")
      test.check(selector && selector.text==="Friends","selected title updates even for conversations beyond the old clipped row")
      pane.choose("porch")
      var geometry=chat.rectangles.a
      var p=chat.mapFromItem(pane.parent.parent,geometry.x+geometry.width/2,8)
      chat.dragAt("a",p.x,p.y)
      test.check(chat.dropPlan && chat.dropPlan.edge==="top","UI exposes whole-workspace drop above own pane")
      chat.cancelDrag()
    }
  }
  Timer {
    interval: 500; running: test.mode === "tiles"
    onTriggered: {
      test.check(!Tiles.valid({key:"bad",axis:"x",ratio:2,a:{key:"same",id:"a"},b:{key:"same",id:"b"}}),"invalid persisted layouts are rejected")
      var pure={key:"a",id:"a"}
      for (var i=1;i<8;i++) pure=Tiles.insert(pure,"a",{key:"leaf"+i,id:String(i)},i%2?"right":"bottom","split"+i)
      test.check(Tiles.valid(pure) && Tiles.leaves(pure).length===8,"nested split tree supports eight panes")
      var min=Tiles.minimum(pure,10,280,230), rects=Tiles.geometry(pure,min.width,min.height,10,280,230)
      test.check(Tiles.leaves(pure).every(function(n) { return rects[n.key].width>=280 && rects[n.key].height>=230 }),"nested layout preserves minimum readable pane sizes")
      test.check(Tiles.dropEdge(0,50,100,100)==="left" && Tiles.dropEdge(100,50,100,100)==="right" && Tiles.dropEdge(50,0,100,100)==="top" && Tiles.dropEdge(50,100,100,100)==="bottom","all directional drop zones")
      var chat=test.findItem(window.contentItem,"conversationPane")
      chat.commit({key:"pane-0",id:"porch"})
      bridge.workspaceLayout.activityCollapsed=true
      var first=Tiles.leaves(chat.tree)[0].key
      chat.choose(first,"porch")
      chat.split(first,"right")
      var second=chat.activeKey
      chat.choose(second,"dm")
      chat.split(second,"bottom")
      var third=chat.activeKey
      chat.choose(third,"friends")
      test.tileKeys=[first,second,third]
      test.check(chat.paneCount===3,"three independent chat panes")
      test.check(Tiles.find(chat.tree,first).id==="porch" && Tiles.find(chat.tree,second).id==="dm","splitting retains existing conversations")
      bridge.setDraft("porch","Room draft")
      bridge.setDraft("dm","DM draft")
      test.tileCommands=bridge.sent.length
    }
  }
  Timer {
    interval: 650; running: test.mode === "tiles"
    onTriggered: {
      var chat=test.findItem(window.contentItem,"conversationPane"), keys=test.tileKeys
      var first=test.findItem(chat,"chatTile-"+keys[0]), second=test.findItem(chat,"chatTile-"+keys[1])
      test.check(first.currentId==="porch" && second.currentId==="dm","pane feeds select different conversations")
      test.check(test.findItem(first,"mainComposerEditor").text==="Room draft" && test.findItem(second,"mainComposerEditor").text==="DM draft","drafts are isolated by destination")
      var split=chat.tree.key, original=chat.rectangles[split].first
      chat.resize(split,30)
      test.check(chat.rectangles[split].first>original,"shared divider resizes adjacent chats")
      var r=chat.rectangles[keys[1]], point=chat.mapFromItem(first.parent.parent,r.x+r.width/2,r.y+r.height/2)
      chat.dragAt(keys[0],point.x,point.y)
      test.check(chat.dropKey===keys[1] && chat.dropEdge==="center","drag center previews a swap")
      chat.finishDrag()
      test.check(Tiles.find(chat.tree,keys[0]).id==="dm" && Tiles.find(chat.tree,keys[1]).id==="porch","center drop swaps chats")
      chat.move(keys[0],keys[2],"bottom")
      test.check(Tiles.valid(chat.tree) && chat.paneCount===3,"edge drop reparents a split without losing a pane")
      test.check(bridge.sent.length===test.tileCommands,"resize and drag send no backend commands")
      chat.detach(keys[1])
    }
  }
  Timer {
    interval: 800; running: test.mode === "tiles"
    onTriggered: {
      var chat=test.findItem(window.contentItem,"conversationPane"), key=test.tileKeys[1]
      var host=test.findObject(chat,"chatTileHost-"+key,[])
      test.check(host && host.detached && host.popoutWindow.visible,"chat opens in its own window")
      if (!host) return
      test.check(host.workspace.parent===host.popoutWindow.contentItem,"same chat component moves into pop-out")
      test.check(bridge.detachedChatFocused,"focused pop-out is included in notification focus state")
      window.visible=false
      test.check(host.popoutWindow.visible && host.workspace.visible,"pop-out remains usable while main window is hidden")
      test.check(!chat.rectangles[key],"detached pane gives space to remaining chats")
      var anchor=test.findItem(host.popoutWindow.contentItem,"chatAnchorButton")
      test.check(anchor && !anchor.text,"return action is an anchor icon")
      var path=Quickshell.env("WISP_CHAT_SCREENSHOT")
      if (path) host.workspace.grabToImage(function(result) { result.saveToFile(path.replace(".png","-popout.png")) })
      anchor.clicked()
      test.check(!host.detached && !host.popoutWindow.visible && host.workspace.parent===host,"anchor returns the same chat to main window")
      test.check(window.visible,"anchor reveals the main window")
      chat.detach(key)
      var returnAction=test.findObject(host.workspace,"wispChatOptions",[]).itemAt(0)
      test.check(returnAction.text==="Return to main window","pop-out menu preserves local reanchoring behavior")
      returnAction.triggered()
      test.check(!host.detached && host.workspace.parent===host,"pop-out menu returns its tile without closing the conversation")
      chat.detach(key)
    }
  }
  Timer {
    interval: 950; running: test.mode === "tiles"
    onTriggered: {
      var chat=test.findItem(window.contentItem,"conversationPane"), key=test.tileKeys[1]
      var host=test.findObject(chat,"chatTileHost-"+key,[])
      host.popoutWindow.contentItem.Window.window.close()
      test.check(!host.detached && !!chat.rectangles[key],"native window close reanchors chat")
      test.check(bridge.draftFor("porch")==="Room draft" && bridge.draftFor("dm")==="DM draft","pop-out and return preserve drafts")
      var closingPane=test.findItem(chat,"chatTile-"+test.tileKeys[2])
      var closeAction=test.findObject(closingPane,"wispChatOptions",[]).itemAt(0)
      test.check(closeAction.text==="Close tile" && closeAction.enabled,"chat options offer a local tile close")
      closeAction.triggered()
      test.check(chat.paneCount===2 && bridge.conversationById("friends"),"closing a pane retains its conversation")
      test.check(!bridge.sent.slice(test.tileCommands).some(function(c) { return ["send_message","set_conversation_tab","clear_chat_history","join","camera","screen_share"].indexOf(c.name)>=0 }),"tiling and pop-outs never send, close, clear, join, or publish")
      var third=test.tileKeys[0]
      chat.split(third,"bottom")
      chat.choose(chat.activeKey,"friends")
      chat.activate(Tiles.leaves(chat.tree)[0].key)
      bridge.workspaceLayout.settingsSaved()
      test.check(!test.findObject(window.contentItem,"settingsSavedNotice",[]).shown,"tile saves are silent")
    }
  }
  Timer {
    interval: 600; running: test.mode === "tilesreload"
    onTriggered: {
      var chat=test.findItem(window.contentItem,"conversationPane")
      test.check(chat.paneCount===3 && Tiles.valid(chat.tree),"saved multi-chat tree restores after restart")
      test.check(chat.detachedKeys.length===0,"restart anchors every pop-out back into main window")
      test.check(Tiles.leaves(chat.tree).map(function(n) { return n.id }).sort().join(",")==="dm,friends,porch","restored panes retain conversation destinations")
      var original=bridge.snapshot, closed=JSON.parse(JSON.stringify(original))
      closed.conversations[0].tab_closed=true
      bridge.snapshot=closed
      bridge.activeConversationId="porch"
      var before=bridge.sent.length
      chat.choose(chat.activeKey,"porch")
      test.check(bridge.sent.slice(before).some(function(c) { return c.name==="set_conversation_tab" && c.args.conversation_id==="porch" && c.args.closed===false }),"Chats can reopen a closed conversation even when its id was already active")
      bridge.snapshot=original
    }
  }
  Timer {
    interval: 600; running: test.mode === "workspace"
    onTriggered: {
      var target = window.contentItem
      var workspace = test.findItem(target, "mainWorkspace")
      var activity = test.findItem(target, "activityPane")
      var chat = test.findItem(target, "conversationPane")
      test.check(!!workspace && !!activity && !!chat, "main window has separate resizable panes")
      if (!workspace || !activity || !chat) return
      var before = bridge.sent.length
      test.check(test.findFeed(target).height > chat.height * 0.55, "message history occupies most of the conversation")
      var original = workspace.activitySize
      test.findItem(target, "activityResizeHandle").moved(60)
      test.check(workspace.activitySize > original, "activity divider resizes the pane")
      var rooms = test.findItem(target, "roomsPane")
      var originalRooms = rooms.height
      test.findItem(target, "roomsResizeHandle").moved(20)
      test.check(rooms.height >= originalRooms, "room/friend divider resizes sections")
      var composer = test.findItem(target, "composerPane")
      test.check(composer.height <= theme.space(60), "one-line composer stays compact")
      bridge.workspaceLayout.dock = "right"
      test.check(workspace.stacked ? activity.y > chat.y : activity.x > chat.x, "activity can move to trailing edge")
      bridge.workspaceLayout.dock = "top"
      test.check(workspace.stacked && activity.y < chat.y, "activity can move above chat")
      bridge.workspaceLayout.dock = "bottom"
      test.check(workspace.stacked && activity.y > chat.y, "activity can move below chat")
      bridge.workspaceLayout.dock = "auto"
      test.check(activity.width > 0 && chat.width > 0 && chat.height > 0, "panes fit window after redocking")
      var toggle = test.findItem(target, "activityCollapseButton")
      var preservedRatio = bridge.workspaceLayout.activityRatio
      for (var dock of ["left", "right", "top", "bottom"]) {
        bridge.workspaceLayout.dock = dock
        var openButton = test.findItem(target,"presence-open")
        test.check(toggle.parent===openButton.parent && toggle.x<openButton.x,"activity arrow is beside Open: " + dock)
        var beforeSize = workspace.stacked ? chat.height : chat.width
        toggle.clicked()
        test.check(!activity.visible && workspace.activitySize === 0, "collapse hides all activity: " + dock)
        test.check(chat.x === 0 && chat.y === 0 && chat.width === workspace.width && chat.height === workspace.height, "collapsed activity leaves no unused rail: " + dock)
        test.check((workspace.stacked ? chat.height : chat.width) > beforeSize, "collapse gives chat the space: " + dock)
        test.check(toggle.direction === (workspace.stacked ? (workspace.reversed ? "up" : "down") : (workspace.reversed ? "left" : "right")), "expand arrow points away from anchor: " + dock)
        toggle.clicked()
        test.check(activity.visible && bridge.workspaceLayout.activityRatio === preservedRatio, "expand restores size: " + dock)
      }
      bridge.workspaceLayout.reset()
      bridge.workspaceLayout.settingsSaved()
      test.check(!test.findObject(target,"settingsSavedNotice",[]).shown,"layout saves do not show Changes Saved")
      bridge.setDraft("porch", "line\n".repeat(30))
      test.check(bridge.sent.length === before, "layout changes never send chat/media commands")
    }
  }
  Timer {
    interval: 750; running: test.mode === "workspace"
    onTriggered: {
      var pane = test.findItem(window.contentItem, "composerPane")
      test.check(pane.height > theme.space(40), "multiline draft grows composer within available space")
      test.check(pane.height <= pane.available * 0.45, "long draft leaves room for history")
      bridge.setDraft("porch", "Short again")
    }
  }
  Timer {
    interval: 900; running: test.mode === "workspace"
    onTriggered: test.check(test.findItem(window.contentItem, "composerPane").height <= theme.space(60), "composer shrinks after shortening draft")
  }
  Timer {
    interval: 600; running: test.mode === "saved"
    onTriggered: {
      var notice = test.findObject(window.contentItem, "settingsSavedNotice", [])
      notice.clear()
      bridge.setVideoQuality("high")
      test.check(!notice.shown, "no saved notice before acknowledgement")
      bridge.finishRequest({id:"test-" + bridge.requestId,ok:false})
      test.check(!notice.shown, "failed changes never say saved")
      bridge.setVideoQuality("high")
      bridge.finishRequest({id:"test-" + bridge.requestId,ok:true})
      test.check(notice.shown, "acknowledged settings show saved notice")
      themeAppearance.setPalette("violet")
    }
  }
  Timer {
    interval: 800; running: test.mode === "saved"
    onTriggered: {
      var notice = test.findObject(window.contentItem, "settingsSavedNotice", [])
      test.check(notice.shown, "appearance save shows notice")
      notice.clear()
      bridge.notificationVolume = 36
    }
  }
  Timer {
    interval: 1000; running: test.mode === "saved"
    onTriggered: {
      test.check(test.findObject(window.contentItem, "settingsSavedNotice", []).shown, "notification file save shows notice")
      bridge.notificationVolume = 35
      var path = Quickshell.env("WISP_CHAT_SCREENSHOT")
      if (path) window.contentItem.children[0].grabToImage(function(result) { test.check(result.saveToFile(path.replace(".png", "-notice.png")), "save notice screenshot") })
    }
  }
  Timer {
    interval: 3650; running: test.mode === "saved"
    onTriggered: test.check(!test.findObject(window.contentItem, "settingsSavedNotice", []).shown, "saved notice disappears after 2.5 seconds")
  }
  Timer {
    interval: test.mode === "saved" ? 3800 : 1200; running: true
    onTriggered: {
      var surface = test.compactMode ? compactSurface : window.contentItem
      if (test.mode === "presence" || test.mode === "panelpresence") {
        for (var entry of [{id:"jared",label:"Open"},{id:"charlie",label:"Knock"},{id:"tyler",label:"Closed"},{id:"morgan",label:"Away"}]) {
          var icon = test.findItem(surface, "friendPresence-" + entry.id)
          test.check(icon && icon.imageStatus === Image.Ready && icon.label === entry.label && icon.width === theme.space(16), "presence icon renders with accessible status: " + entry.label)
          var favorite = test.findItem(surface, "favorite-" + entry.id)
          test.check(!!favorite, "friend row available for icon verification: " + entry.id)
          if (!favorite) continue
          var row = favorite.parent
          test.check(test.findItem(row, "friendConnectionDot").presence === "open", "online remains distinct from offline: " + entry.label)
          var name = test.findItem(row, "friendName")
          test.check(name.width >= name.implicitWidth, "short friend name fits beside compact icon: " + entry.id)
          test.check(!test.findText(row, entry.label.toLowerCase()), "text status replaced by icon")
        }
        var offlineFavorite = test.findItem(surface, "favorite-jack")
        test.check(!!offlineFavorite, "offline friend row available")
        var offlineRow = offlineFavorite ? offlineFavorite.parent : surface
        test.check(!test.findItem(offlineRow, "friendPresence-jack") && test.findItem(offlineRow, "friendConnectionDot").presence === "closed", "offline uses hollow connection dot only")
        test.check(!test.findText(offlineRow, "offline"), "offline label no longer takes name space")
      }
      var messageBox = test.findItem(surface, "composerMessageBox")
      if (messageBox) {
        var send = test.findItem(messageBox, "composerSendButton")
        var textViewport = test.findItem(messageBox, "composerTextViewport")
        test.check(!!send && send.parent === messageBox && !send.text, "send is an icon inside the message box")
        test.check(send.x >= 0 && send.y >= 0 && send.x + send.width <= messageBox.width && send.y + send.height <= messageBox.height, "send icon fits inside message box")
        test.check(textViewport.x + textViewport.width < send.x, "message text reserves space for send icon")
        var editor = test.findItem(messageBox, test.compactMode ? "trayComposerEditor" : "mainComposerEditor")
        var current = bridge.conversationById(messageBox.parent.conversationId)
        test.check(editor && editor.placeholderText === "Message " + (current.label === "Hangout" ? "Room" : current.label), "placeholder identifies the conversation")
        var oldId = messageBox.parent.conversationId
        messageBox.parent.conversationId = "dm"
        test.check(editor.placeholderText === "Message Jared", "placeholder follows DM destination")
        messageBox.parent.conversationId = oldId
        test.check(!test.findText(surface, "Shift+Enter for a new line"), "keyboard hint removed")
      }
      if (test.mode !== "preview") {
        var page = test.findItem(surface, "wispContent")
        if (Quickshell.env("WISP_TEST_ADAPTER") === "omarchy" && test.compactMode) {
          var statusLine = test.findItem(surface, "terminalStatusLine")
          test.check(theme.hostManaged && theme.tui, "Omarchy popup combines host styling with compact TUI structure")
          test.check(!!statusLine && statusLine.visible, "Omarchy popup includes the live compact status line")
        }
        if (page.inlineHeader) {
          var identity = test.findItem(surface, "identityMenuButton")
          var access = test.findItem(surface, "alwaysVisibleControls")
          var identityPosition = identity.mapToItem(surface, 0, 0)
          var accessPosition = access.mapToItem(surface, 0, 0)
          test.check(accessPosition.x >= identityPosition.x + identity.width && accessPosition.y >= identityPosition.y && accessPosition.y + access.height <= identityPosition.y + identity.height, "main access controls share the identity row without overlap")
        }
        test.check(!!test.findItem(surface, "headerHomeButton") === !page.showingChats, "Home visibility follows current page")
        var audio = test.findItem(surface, "globalAudioControls")
        test.check(!!audio && audio.width > 0, "mute/deafen available with or without a room and in settings")
        if (audio) {
          var position = audio.mapToItem(surface, 0, 0)
          test.check(position.y >= 0 && position.y + audio.height < surface.height, "audio controls inside window")
        }
      }
      if (test.mode === "media" || test.mode === "panelmedia") {
        var room = test.findItem(surface, "roomCard")
        test.check(!!room && room.height === theme.space(theme.tui ? 42 : 48), "occupied room cards use compact height")
      }
      if (test.mode === "cleantui") {
        var cleanWorkspace = test.findObject(surface, "mainWorkspace", [])
        var cleanActivity = test.findObject(surface, "activityPane", [])
        var cleanChat = test.findObject(surface, "conversationPane", [])
        var cleanFrame = test.findObject(surface, "conversationColorFrame", [])
        var cleanSelector = test.findObject(surface, "compactChatSelector", [])
        test.check(theme.cleanTui && theme.tui && theme.terminal,
          "Clean TUI is a distinct terminal interface profile")
        test.check(!!cleanWorkspace && !!cleanActivity && !!cleanChat
          && cleanWorkspace.visible && cleanActivity.visible && cleanChat.visible,
          "Clean TUI keeps the activity and chat workspace")
        test.check(cleanActivity && cleanActivity.width >= theme.space(219),
          "Clean TUI activity rail keeps a usable minimum width")
        test.check(cleanActivity && cleanActivity.width <= theme.space(360),
          "Clean TUI activity rail stays narrow")
        test.check(cleanActivity && cleanChat && cleanChat.width > cleanActivity.width,
          "Clean TUI gives chat more width than activity")
        test.check(cleanFrame && cleanFrame.quiet,
          "Clean TUI replaces full pane boxes with quiet section rules")
        test.check(cleanSelector && cleanSelector.background.border.width === 0,
          "Clean TUI removes the idle selector box")
        test.check(theme.statusBackground == theme.surface
          && theme.selectionBackground == theme.alpha(theme.accent, 0.18),
          "Clean TUI uses restrained status and selection surfaces")
      }
      test.check(window.width === theme.space(test.testWidth), "app width")
      test.check(window.height === theme.space(test.testHeight), "app height")
      var path = Quickshell.env("WISP_CHAT_SCREENSHOT")
      var target = test.compactMode ? compactSurface : window.contentItem.children[0]
      if (test.mode === "preview") target = preview.contentItem.children[0]
      if (test.mode === "identity" || test.mode === "panelidentity") {
        var identityMenu = test.findObject(test.compactMode ? compactSurface : window.contentItem, "identityMenu", [])
        if (identityMenu) target = identityMenu.contentItem.parent
      }
      if (test.mode === "identityactions" || test.mode === "panelidentityactions") {
        var identityRoom = test.findObject(test.compactMode ? compactSurface : window.contentItem, "identityRoomManager", [])
        if (identityRoom) target = identityRoom.contentItem.parent
      }
      if (test.mode === "menu") {
        var menu = test.findObject(window.contentItem, "wispChatOptions", [])
        test.check(!!menu && menu.opened, "chat options menu opened")
        if (menu) target = menu.contentItem.parent
      }
      if (test.mode === "picker") target=test.findObject(window.contentItem,"chatConversationMenu",[]).contentItem.parent
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
