import QtQuick
import QtQuick.Controls

ChatButton {
  id: root
  objectName: "chatPinsButton"
  required property var bridge
  required property string conversationId
  text: "Pinned messages"; iconName: "pin"; iconOnly: true; forceIcon: true
  width: theme.space(30); height: theme.space(30)
  visible: !!bridge.conversationById(conversationId)
  enabled: visible && !(bridge.conversationById(conversationId) || {}).pending_access
  onClicked: pins.open()
  PinsPopup {id:pins;bridge:root.bridge;theme:root.theme;conversationId:root.conversationId}
}
