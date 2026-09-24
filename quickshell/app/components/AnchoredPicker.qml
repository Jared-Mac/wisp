import QtQuick
import QtQuick.Controls

Popup {
  id: root
  required property var theme
  property Item anchorItem: null
  property bool preferAbove: false
  property real desiredWidth: theme.space(340)
  property real desiredHeight: theme.space(320)
  readonly property real edgeMargin: theme.spacing.lg
  // Follow the invoking item's overlay, including a recreated tray window.
  parent: anchorItem ? anchorItem.Overlay.overlay : Overlay.overlay
  width: Math.max(1,Math.min(desiredWidth,parent ? parent.width-edgeMargin*2 : desiredWidth))
  height: Math.max(1,Math.min(desiredHeight,parent ? parent.height-edgeMargin*2 : desiredHeight))
  function showAt(anchor) { anchorItem=anchor; open(); Qt.callLater(reposition) }
  function reposition() {
    if (!visible || !parent) return
    if (!anchorItem) {x=(parent.width-width)/2;y=(parent.height-height)/2;return}
    var point=anchorItem.mapToItem(parent,0,0),gap=theme.space(6)
    x=Math.max(edgeMargin,Math.min(point.x,parent.width-width-edgeMargin))
    var below=point.y+anchorItem.height+gap,above=point.y-height-gap
    var fitsBelow=below+height<=parent.height-edgeMargin, fitsAbove=above>=edgeMargin
    var candidate=preferAbove ? (fitsAbove ? above : below) : (fitsBelow ? below : above)
    y=Math.max(edgeMargin,Math.min(candidate,parent.height-height-edgeMargin))
  }
  onOpened: reposition()
  onHeightChanged: Qt.callLater(reposition)
  onWidthChanged: Qt.callLater(reposition)
  onParentChanged: Qt.callLater(reposition)
  onAnchorItemChanged: if(visible && !anchorItem)close()
  Connections {
    target:root.parent
    function onWidthChanged(){root.reposition()}
    function onHeightChanged(){root.reposition()}
  }
  Connections {
    target:root.anchorItem
    function onVisibleChanged(){if(root.anchorItem && !root.anchorItem.visible)root.close()}
    function onXChanged(){root.reposition()}
    function onYChanged(){root.reposition()}
  }
}
