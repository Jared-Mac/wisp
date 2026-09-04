import QtQuick
import Quickshell
import "app" as Wisp
import "app/components" as Components

ShellRoot {
  id: test
  property bool failed: false
  property var originalCaret: null
  function check(value, message) {
    if (!value) { failed = true; console.error("PERFORMATIVE_FAILED " + message) }
  }
  function find(item, name) {
    if (item.objectName === name) return item
    var children = item.children || []
    for (var i = 0; i < children.length; i++) {
      var result = find(children[i], name)
      if (result) return result
    }
    return null
  }
  QtObject { id: appearance; property string palette: "wisp"; property bool managed: false }
  Wisp.WispTheme { id: theme; profile: "legacy"; appearanceController: appearance }
  Wisp.WispBridge {
    id: bridge
    property int commandCount: 0
    function send(name, args) { commandCount++; return "fixture" }
  }
  FloatingWindow {
    visible: true; implicitWidth: 480; implicitHeight: 420
    color: theme.background
    Column {
      anchors.fill: parent; anchors.margins: 16; spacing: 12
      Components.ChatButton { id: button; theme: theme; text: "Settings"; primary: true }
      Components.ChatComposer {
        id: composer; width: parent.width; bridge: bridge; theme: theme
        autoGrow: true; conversationId: "fixture"
      }
      Components.ChatComposer {
        id: trayComposer; width: parent.width; bridge: bridge; theme: theme
        conversationId: "tray-fixture"
      }
      Components.HangoutCard {
        id: room; width: parent.width; theme: theme; bridge: bridge
        hangout: ({id:"fixture",label:"Room",members:[{id:"jared",display_name:"Jared"},{id:"self",display_name:"Tyler"}]})
      }
      Components.MediaControls { id: media; width: parent.width; theme: theme; bridge: bridge }
    }
  }
  Timer {
    interval: 150; running: true
    onTriggered: {
      var editor = test.find(composer, "mainComposerEditor")
      test.check(!!editor, "editor exists")
      if (!editor) { Qt.quit(); return }
      test.originalCaret = editor.cursorDelegate
      editor.text = "This is a chat draft, not a command."
      appearance.palette = "performative"
      editor.forceActiveFocus()
    }
  }
  Timer {
    interval: 350; running: true
    onTriggered: {
      var editor = test.find(composer, "mainComposerEditor")
      test.check(theme.terminal && theme.cornerRadius === 0, "Performative overrides Classic styling")
      test.check(button.contentItem.text === "[Settings]", "controls use brackets")
      test.check(button.background.color == "#b7baad" && button.contentItem.color == "#000000", "selection uses legible ash inverse, not mint")
      test.check(theme.statusBackground != theme.accent && theme.statusText == theme.foreground, "status bar uses a neutral dark surface")
      // Font fallback metrics can round the prompt line one or two pixels up.
      test.check(trayComposer.height <= theme.space(66),
        "tray prompt and editor stay compact (height=" + trayComposer.height + ")")
      test.check(test.find(room, "roomMember-0").width < theme.space(80)
        && test.find(room, "roomMember-1").x < theme.space(90), "short room names stay grouped")
      test.check(test.find(media, "mediaAction-share").width < media.width / 2, "media actions use content widths")
      test.check(test.find(composer, "terminalChatPrompt").visible, "message prompt visible")
      test.check(editor.cursorDelegate !== test.originalCaret, "block caret enabled")
      test.check(editor.activeFocus, "editor remains focusable")
      test.check(bridge.commandCount === 0, "appearance sends no commands")
      appearance.palette = "wisp"
    }
  }
  Timer {
    interval: 550; running: true
    onTriggered: {
      var editor = test.find(composer, "mainComposerEditor")
      test.check(!theme.terminal && theme.cornerRadius === 9, "Classic style restored")
      test.check(button.contentItem.text === "Settings", "normal control label restored")
      test.check(trayComposer.editorHeight === theme.space(66) && room.height === theme.space(48), "normal theme spacing restored")
      test.check(!test.find(composer, "terminalChatPrompt").visible, "prompt hidden outside mode")
      test.check(editor.cursorDelegate === test.originalCaret, "native caret restored")
      test.check(editor.text === "This is a chat draft, not a command.", "draft preserved")
      test.check(bridge.commandCount === 0, "no media or chat commands sent")
      if (!test.failed) console.log("PERFORMATIVE_OK")
      Qt.quit()
    }
  }
}
