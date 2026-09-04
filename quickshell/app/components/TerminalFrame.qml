import QtQuick

// A non-interactive TUI frame. Labels interrupt the rule like a curses window;
// real controls and scrolling remain in the host, not in this decoration.
Item {
  id: root
  required property var theme
  property string title: ""
  property color ink: theme.surfaceBorder
  property bool emphasized: false
  visible: theme.tui
  Rectangle {
    anchors.fill: parent; anchors.topMargin: root.theme.space(7)
    color: "transparent"; border.width: root.emphasized ? 2 : 1; border.color: root.ink
  }
  Rectangle {
    x: root.theme.space(9); y: 0
    width: Math.min(parent.width - x * 2, caption.implicitWidth + root.theme.space(12))
    height: caption.implicitHeight
    color: root.theme.background
    Text {
      id: caption
      anchors.fill: parent
      text: "─ " + root.title + " ─"
      elide: Text.ElideRight
      color: root.ink
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
  }
}
