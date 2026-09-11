import QtQuick
import QtQuick.Controls
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  property bool adaptive: false
  readonly property bool narrow: adaptive && width < theme.space(140)
  readonly property bool tiny: adaptive && width < theme.space(56)
  property bool showHeader: true
  signal selected()
  width: parent ? parent.width : 0
  spacing: root.theme.space(1)
  readonly property var channels: root.bridge.conversations.filter(function(conversation) {
    return conversation.server_channel
      && String(conversation.server_id)===String(root.bridge.activeServer.id)
  }).sort(function(left,right) {
    return String(left.category_name || "").localeCompare(String(right.category_name || ""))
      || String(left.label).localeCompare(String(right.label))
  })
  visible: channels.length > 0

  Text {
    objectName: "serverChannelsHeader"
    visible: root.showHeader && !root.tiny
    width: parent.width
    height: root.theme.space(22)
    elide: Text.ElideRight
    verticalAlignment: Text.AlignVCenter
    text: root.theme.tui ? "┌─ /channels · " + root.channels.length : "CHANNELS"
    color: root.theme.accent
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.bold: true
  }

  Repeater {
    model: root.channels
    delegate: Flow {
      id: channelRow
      required property var modelData
      width: root.width
      height: root.theme.space(root.tiny ? 56 : 28)
      spacing: root.theme.space(2)
      ChatButton {
        objectName: "serverChannel-" + String(channelRow.modelData.raw_id || channelRow.modelData.id)
        width: root.tiny ? channelRow.width : Math.max(0, channelRow.width - tileButton.width - channelRow.spacing)
        height: root.theme.space(28)
        theme: root.theme
        iconName: root.tiny ? "" : "hash"; formatLabel: false
        textAlignment: Text.AlignLeft
        leftPadding: root.theme.space(6)
        text: root.tiny ? String(channelRow.modelData.label).slice(0,1).toUpperCase() : (root.theme.tui ? "# " : "") + String(channelRow.modelData.label)
          + (root.bridge.pendingCount(channelRow.modelData.id) ? " · " + root.bridge.pendingCount(channelRow.modelData.id) : "")
        primary: String(root.bridge.activeConversationId) === String(channelRow.modelData.id)
        Accessible.name: "Open " + String(channelRow.modelData.label)
          + (channelRow.modelData.spot_id ? " room chat" : " channel") + " on " + String(channelRow.modelData.server_name)
        ToolTip.visible: hovered; ToolTip.text: Accessible.name
        onClicked: { root.bridge.openChannel(channelRow.modelData.id, false); root.selected() }
      }
      ChatButton {
        id: tileButton
        objectName: "serverChannelTile-" + String(channelRow.modelData.raw_id || channelRow.modelData.id)
        width: Math.min(root.width,root.theme.space(32))
        height: root.theme.space(28)
        theme: root.theme
        text: "+"; iconName: "add"; iconOnly: true; forceIcon: root.tiny
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
