import QtQuick
import QtQuick.Controls

Flickable {
  id:root
  objectName:"trayFocusedWorkspace"
  required property var bridge
  required property var theme
  contentWidth:width;contentHeight:messages.implicitHeight
  clip:true;boundsBehavior:Flickable.StopAtBounds
  ScrollBar.vertical:ScrollBar {}
  MessagesView {id:messages;width:parent.width;availableHeight:root.height;bridge:root.bridge;theme:root.theme}
}
