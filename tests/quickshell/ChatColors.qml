import QtQuick
import Quickshell
import "app" as Wisp
import "app/views" as Views

ShellRoot {
  id: test
  property bool failed: false
  property color original
  function find(item, name) {
    if (item.objectName === name) return item
    var children = item.children || []
    for (var i = 0; i < children.length; i++) {
      var result = find(children[i], name)
      if (result) return result
    }
    return null
  }
  function check(value, message) {
    if (!value) { failed = true; console.error("CHAT_COLORS_FAILED " + message) }
  }
  Wisp.WispBridge {
    id: bridge
    property int sent: 0
    function send(name, args) { sent++; return "fixture" }
  }
  QtObject { id: appearance; property string palette: "ash_olive"; property bool managed: false }
  Wisp.WispTheme { id: theme; profile: "performative"; appearanceController: appearance }
  FloatingWindow {
    id: window
    visible: true; implicitWidth: 1000; implicitHeight: 600
    color: theme.background
    Row {
      anchors.fill: parent; anchors.margins: 10; spacing: 10
      Views.ConversationWorkspace { id: first; width: 480; height: parent.height; bridge: bridge; theme: theme; tiled: true; selectedId: "member_c"; canClosePane: true }
      Views.ConversationWorkspace { id: second; width: 480; height: parent.height; bridge: bridge; theme: theme; tiled: true; selectedId: "owner"; paneActive: false }
    }
  }
  Component.onCompleted: {
    var data = JSON.parse(JSON.stringify(bridge.snapshot))
    data.conversations = [{id:"member_c",label:"MemberC",kind:"direct"}, {id:"owner",label:"Owner",kind:"direct"}, {id:"room",label:"Room",kind:"hangout"}]
    bridge.applySnapshot(data)
  }
  Timer {
    interval: 300; running: true
    onTriggered: {
      test.check(bridge.chatColors.ready && !bridge.chatColors.error, "assignments loaded")
      test.check(first.chatBorderColor == "#7fa9cf" && second.chatBorderColor == "#cb8897", "distinct persisted chat colors")
      var selector = test.find(first, "compactChatSelector")
      test.check(selector.width >= theme.space(88) && selector.width < theme.space(160), "short chat dropdown is content-sized")
      test.check(selector.background.color.a <= 0.10 && selector.selectorInk == theme.foreground, "chat dropdown has a quiet dark surface with legible text")
      test.original = first.chatBorderColor
      first.paneActive = false
      test.check(first.chatBorderColor === test.original, "focus retains hue")
      first.selectedId = "owner"
      test.check(first.chatBorderColor === second.chatBorderColor, "same chat matches across panes")
      first.selectedId = "member_c"
      first.detached = true
      test.check(first.chatBorderColor === test.original, "pop-out retains hue")
      var data = JSON.parse(JSON.stringify(bridge.snapshot))
      data.conversations.reverse()
      data.conversations.push({id:"aaa-new",label:"New chat",kind:"direct"})
      data.conversations.filter(function(c) { return c.id === "member_c" })[0].label = "Renamed chat with a very long label that should never fill the whole pane"
      bridge.applySnapshot(data)
      test.check(first.chatBorderColor === test.original, "rename/reorder/new chats retain hue")
    }
  }
  Timer {
    interval: 500; running: true
    onTriggered: {
      var selector = test.find(first, "compactChatSelector")
      test.check(selector.width <= theme.space(260) && selector.width <= selector.availableHeaderWidth, "long labels respect the header width and cap")
      selector.forceActiveFocus(Qt.TabFocusReason)
      test.check(selector.background.border.color === first.chatBorderColor, "keyboard focus uses the conversation color")
      appearance.palette = "wisp"
      test.check(theme.performative, "changing palette keeps Performative appearance")
      theme.profile = "terminal"
    }
  }
  Timer {
    interval: 700; running: true
    onTriggered: {
      test.check(!bridge.chatColors.error && bridge.sent === 0, "local save, no chat/media commands")
      var selector = test.find(first, "compactChatSelector")
      test.check(selector.width === selector.availableHeaderWidth && selector.selectorInk === first.chatHeadingColor, "Terminal Grid keeps full-width selector with configurable heading color")
      if (!test.failed) console.log("CHAT_COLORS_OK")
      Qt.quit()
    }
  }
}
