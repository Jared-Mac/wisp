import QtQuick

Item {
  id: root
  required property var theme
  property bool verticalLine: true
  signal moved(real delta)
  signal resetRequested()
  activeFocusOnTab: true
  Accessible.name: verticalLine ? "Resize columns" : "Resize rows"
  Keys.onPressed: function(event) {
    if ((verticalLine && event.key === Qt.Key_Left) || (!verticalLine && event.key === Qt.Key_Up)) { moved(-20); event.accepted = true }
    if ((verticalLine && event.key === Qt.Key_Right) || (!verticalLine && event.key === Qt.Key_Down)) { moved(20); event.accepted = true }
  }
  Rectangle {
    anchors.centerIn: parent
    width: root.verticalLine ? (mouse.containsMouse || mouse.pressed ? 3 : 1) : parent.width
    height: root.verticalLine ? parent.height : (mouse.containsMouse || mouse.pressed ? 3 : 1)
    color: mouse.containsMouse || mouse.pressed || root.activeFocus ? root.theme.accent : root.theme.separator
  }
  MouseArea {
    id: mouse
    anchors.fill: parent; hoverEnabled: true; preventStealing: true
    cursorShape: root.verticalLine ? Qt.SplitHCursor : Qt.SplitVCursor
    property real lastPosition: 0
    onPressed: function(event) {
      root.forceActiveFocus(Qt.MouseFocusReason)
      var point = mapToGlobal(event.x, event.y)
      lastPosition = root.verticalLine ? point.x : point.y
    }
    onPositionChanged: function(event) {
      if (!pressed) return
      var point = mapToGlobal(event.x, event.y)
      var position = root.verticalLine ? point.x : point.y
      var delta = position - lastPosition
      lastPosition = position
      root.moved(delta)
    }
    onDoubleClicked: root.resetRequested()
  }
}
