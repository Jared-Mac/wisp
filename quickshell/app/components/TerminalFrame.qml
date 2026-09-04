import QtQuick

// A non-interactive TUI frame. Labels interrupt the rule like a curses window;
// real controls and scrolling remain in the host, not in this decoration.
Item {
  id: root
  required property var theme
  property string title: ""
  property color ink: theme.surfaceBorder
  property color titleInk: ink
  property bool emphasized: false
  readonly property bool quiet: theme.cleanTui
  visible: theme.tui
  Rectangle {
    anchors.fill: parent; anchors.topMargin: root.theme.space(7)
    visible: !root.quiet
    color: "transparent"; border.width: root.emphasized ? 2 : 1; border.color: root.ink
  }
  Rectangle {
    visible: root.quiet
    x: caption.x + caption.width + root.theme.space(8)
    y: Math.round(caption.height / 2)
    width: Math.max(0, parent.width - x)
    height: 1
    color: root.theme.alpha(root.ink, root.emphasized ? 0.9 : 0.55)
  }
  Rectangle {
    visible: root.quiet && root.emphasized
    x: 0; y: root.theme.space(2)
    width: root.theme.space(2); height: root.theme.space(16)
    color: root.ink
  }
  Rectangle {
    x: root.quiet ? root.theme.space(4) : root.theme.space(9); y: 0
    width: Math.min(parent.width - x * 2, caption.implicitWidth + root.theme.space(12))
    height: caption.implicitHeight
    color: root.theme.background
    Text {
      id: caption
      anchors.fill: parent
      text: root.quiet ? root.title : "─ " + root.title + " ─"
      elide: Text.ElideRight
      color: root.titleInk
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
  }
}
