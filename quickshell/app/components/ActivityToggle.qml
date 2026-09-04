import QtQuick

ChatButton {
  id: root
  objectName: "activityCollapseButton"
  required property var bridge
  property bool stacked: false
  readonly property bool collapsed: bridge.workspaceLayout.activityCollapsed
  readonly property bool reversed: ["right", "bottom"].indexOf(bridge.workspaceLayout.dock) >= 0
  readonly property string direction: stacked ? (reversed !== collapsed ? "down" : "up") : (reversed !== collapsed ? "right" : "left")
  width: theme.space(30); height: width
  Accessible.name: collapsed ? "Expand activity" : "Collapse activity"
  onClicked: bridge.workspaceLayout.activityCollapsed = !collapsed
  contentItem: Item {
    Canvas {
      anchors.centerIn: parent
      width: root.theme.space(12); height: width
      rotation: root.direction === "right" ? 180 : root.direction === "up" ? 90 : root.direction === "down" ? 270 : 0
      property color strokeColor: root.theme.foreground
      onStrokeColorChanged: requestPaint()
      onPaint: {
        var ctx = getContext("2d")
        ctx.reset(); ctx.strokeStyle = strokeColor; ctx.lineWidth = 1.5
        ctx.beginPath(); ctx.moveTo(width * 0.65, height * 0.2)
        ctx.lineTo(width * 0.35, height * 0.5); ctx.lineTo(width * 0.65, height * 0.8); ctx.stroke()
      }
    }
  }
}
