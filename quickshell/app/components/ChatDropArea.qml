import QtQuick

DropArea {
  id: root
  objectName: "chatDropArea"
  required property var bridge
  required property var theme
  required property string conversationId
  enabled: conversationId !== "" && !bridge.sendingConversations[conversationId]
  function acceptCopy(drag) {
    if (drag.hasUrls && (drag.supportedActions & Qt.CopyAction)) drag.accept(Qt.CopyAction)
    else drag.accepted = false
  }
  function handleDrop(drop) {
    if (!enabled || !drop.hasUrls || !(drop.supportedActions & Qt.CopyAction)) { drop.accepted = false; return }
    root.bridge.importChatFiles(root.conversationId, drop.urls)
    // Never authorize the drag source to move/delete the original file.
    drop.accept(Qt.CopyAction)
  }
  onEntered: function(drag) { acceptCopy(drag) }
  onPositionChanged: function(drag) { acceptCopy(drag) }
  onDropped: function(drop) { handleDrop(drop) }
  Rectangle {
    anchors.fill: parent
    visible: root.containsDrag
    radius: root.theme.cornerRadius
    color: root.theme.alpha(root.theme.surface, 0.96)
    border.color: root.theme.accent; border.width: 2
    Text {
      Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
      anchors.centerIn: parent
      text: "Drop to attach"
      color: root.theme.foreground; font.pixelSize: root.theme.font.title
    }
  }
}
