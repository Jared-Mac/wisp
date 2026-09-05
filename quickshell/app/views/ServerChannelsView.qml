import QtQuick
import QtQuick.Controls
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  property bool showHeader: true
  signal selected()
  width: parent ? parent.width : 0
  spacing: root.theme.space(1)
  readonly property var channels: root.bridge.conversations.filter(function(conversation) {
    return (conversation.server_channel || !!conversation.spot_id)
      && String(conversation.server_id)===String(root.bridge.activeServer.id)
  }).sort(function(left,right) {
    return String(left.category_name || "").localeCompare(String(right.category_name || ""))
      || String(left.label).localeCompare(String(right.label))
  })

  Text {
    objectName: "serverChannelsHeader"
    visible: root.showHeader
    width: parent.width
    height: root.theme.space(22)
    verticalAlignment: Text.AlignVCenter
    text: root.theme.tui ? "┌─ /channels · " + root.channels.length : "CHANNELS"
    color: root.theme.accent
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.bold: true
  }

  Repeater {
    model: root.channels
    delegate: Row {
      id: channelRow
      required property var modelData
      width: root.width
      height: root.theme.space(28)
      spacing: root.theme.space(2)
      ChatButton {
        objectName: "serverChannel-" + String(channelRow.modelData.raw_id || channelRow.modelData.id)
        width: Math.max(0, channelRow.width - tileButton.width - channelRow.spacing)
        height: channelRow.height
        theme: root.theme
        textAlignment: Text.AlignLeft
        leftPadding: root.theme.space(6)
        text: (root.theme.tui ? "# " : "") + String(channelRow.modelData.label)
          + (channelRow.modelData.unread_count ? " · " + channelRow.modelData.unread_count : "")
        primary: String(root.bridge.activeConversationId) === String(channelRow.modelData.id)
        Accessible.name: "Open " + String(channelRow.modelData.label)
          + (channelRow.modelData.spot_id ? " room chat" : " channel") + " on " + String(channelRow.modelData.server_name)
        onClicked: { root.bridge.openChannel(channelRow.modelData.id, false); root.selected() }
      }
      ChatButton {
        id: tileButton
        objectName: "serverChannelTile-" + String(channelRow.modelData.raw_id || channelRow.modelData.id)
        width: root.theme.space(32)
        height: channelRow.height
        theme: root.theme
        text: "+"
        Accessible.name: "Open " + String(channelRow.modelData.label) + " in a new tile"
        ToolTip.visible: hovered
        ToolTip.delay: 500
        ToolTip.text: "Open in a new tile"
        onClicked: { root.bridge.openChannel(channelRow.modelData.id, true); root.selected() }
      }
    }
  }

  Text {
    visible: root.channels.length===0
    width: parent.width
    text: "(no text channels)"
    color: root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
  }
}
