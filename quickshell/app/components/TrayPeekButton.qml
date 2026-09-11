import QtQuick
import QtQuick.Controls

ChatButton {
  id:root
  property Component panelContent
  property real panelWidth:theme.space(320)
  readonly property alias popup:peek
  property bool keepOpen:false
  hoverEnabled:true
  height:theme.space(28)
  onHoveredChanged: {
    if(hovered) {closeDelay.stop();openDelay.restart()}
    else {openDelay.stop();closeDelay.restart()}
  }
  onClicked: {
    openDelay.stop()
    if(peek.opened && keepOpen) peek.close()
    else {keepOpen=true;peek.open()}
  }
  onVisibleChanged: if(!visible)peek.close()
  Timer {id:openDelay;interval:250;onTriggered:if(root.hovered){root.keepOpen=false;peek.open()}}
  Timer {id:closeDelay;interval:250;onTriggered:if(!root.hovered && !inside.hovered && !root.keepOpen)peek.close()}
  Popup {
    id:peek;objectName:root.objectName+"Popup"
    // Resolve through the persistent button: a tray close destroys its window
    // and overlay, while the button remains for the next popup lifecycle.
    parent:root.Overlay.overlay || root
    width:Math.min(root.panelWidth,parent ? parent.width-16 : root.panelWidth)
    height:Math.min(body.implicitHeight+padding*2,parent ? parent.height-24 : root.theme.space(360),root.theme.space(420))
    padding:root.theme.space(10)
    x:parent ? Math.max(8,Math.min(parent.width-width-8,root.mapToItem(parent,0,0).x)) : 0
    y:parent ? Math.max(8,Math.min(parent.height-height-8,root.mapToItem(parent,0,0).y-height-root.theme.space(4))) : 0
    closePolicy:Popup.CloseOnEscape | Popup.CloseOnPressOutside
    onClosed:root.keepOpen=false
    background:Rectangle {color:root.theme.surface;radius:root.theme.cornerRadius;border.width:1;border.color:root.theme.separator}
    contentItem:Flickable {
      contentWidth:width;contentHeight:body.implicitHeight
      clip:true;boundsBehavior:Flickable.StopAtBounds
      ScrollBar.vertical:ScrollBar {}
      HoverHandler {id:inside;onHoveredChanged:if(!hovered)closeDelay.restart();else closeDelay.stop()}
      // Once the user interacts, keep menus stable until dismissed explicitly.
      TapHandler {onPressedChanged:if(pressed)root.keepOpen=true}
      Loader {id:body;width:parent.width;active:peek.visible;sourceComponent:root.panelContent}
    }
  }
}
