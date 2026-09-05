import QtQuick
import QtQuick.Controls

ChatButton {
  id: root
  required property var bridge
  required property string conversationId
  readonly property var conversation: bridge.conversationById(conversationId)
  readonly property bool direct: !!conversation && conversation.kind === "direct"
  readonly property var target: direct ? bridge.directCallTarget(conversationId) : bridge.roomVoiceTarget(conversationId)
  objectName: direct ? "callConversation" : "joinConversationVoice"
  visible: !!target && (!target.current || direct)
  enabled: !!target && target.connected && !target.current && (!direct || target.available)
  text: direct ? "call" : "join voice"
  Accessible.name: direct ? "Call " + (target ? target.name : "friend") : "Join voice in this conversation"
  ToolTip.visible: hovered
  ToolTip.text: direct && target ? target.current ? "Already in voice with " + target.name
    : !target.online ? target.name + " is offline" : !target.available ? target.name + " is not available for calls"
    : target.knock ? "Ask " + target.name + " to call" : Accessible.name : Accessible.name
  onClicked: { if (direct) root.bridge.callConversation(root.conversationId); else root.bridge.joinConversationVoice(root.conversationId) }
}
