import QtQuick
import QtQuick.Controls

Item {
  id: root
  required property var bridge
  required property var theme
  property bool compact: false
  implicitHeight: selector.height

  ComboBox {
    id: selector
    objectName: "activeServerSelector"
    anchors.left: parent.left
    anchors.right: parent.right
    height: root.theme.space(root.compact ? 28 : 32)
    model: root.bridge.servers
    textRole: "name"
    valueRole: "id"
    currentIndex: {
      for (var i=0;i<root.bridge.servers.length;i++)
        if (String(root.bridge.servers[i].id)===String(root.bridge.activeServer.id)) return i
      return 0
    }
    Accessible.name: "Active server"
    onActivated: function(index) {
      var server=root.bridge.servers[index]
      if (server) root.bridge.selectServer(server.id)
    }
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    contentItem: Text {
      leftPadding: root.theme.spacing.sm
      rightPadding: root.theme.space(24)
      verticalAlignment: Text.AlignVCenter
      text: (root.theme.tui ? "@ " : "") + String(selector.currentText || "Server")
        + (root.bridge.activeServer.connected === false ? " · offline" : "")
      elide: Text.ElideRight
      color: root.bridge.activeServer.connected === false ? root.theme.muted : root.theme.foreground
      font: selector.font
    }
    background: Rectangle {
      color: selector.hovered ? root.theme.alpha(root.theme.foreground,0.07) : root.theme.surface
      border.width: 1
      border.color: selector.activeFocus ? root.theme.focusBorder : root.theme.separator
      radius: root.theme.cornerRadius
    }
    delegate: ItemDelegate {
      required property var modelData
      width: selector.width
      height: root.theme.space(32)
      text: String(modelData.name) + (modelData.connected === false ? " · offline" : "")
      highlighted: String(modelData.id)===String(root.bridge.activeServer.id)
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
      ThemeControlStyle { theme: root.theme; control: parent }
    }
    popup.background: Rectangle {
      color: root.theme.surface
      border.width: 1
      border.color: root.theme.muted
      radius: root.theme.cornerRadius
    }
  }
}
