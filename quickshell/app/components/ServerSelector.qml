import QtQuick
import QtQuick.Controls
import QtQml

Item {
  id: root
  required property var bridge
  required property var theme
  property bool compact: false
  signal settingsRequested()
  implicitHeight: selector.height + inviteButton.height + root.theme.spacing.xs
  TextMetrics { id: serverMetrics; font: selector.font; text: serverLabel.text }
  TextMetrics { id: settingsMetrics; font: selector.font; text: root.theme.tui ? "[settings]" : "settings" }

  ChatButton {
    id: settingsButton
    objectName: "serverSettingsShortcut"
    anchors.right: parent.right
    anchors.verticalCenter: selector.verticalCenter
    height: selector.height - 2
    theme: root.theme
    visible: root.bridge.canManageServer
    text: serverMetrics.advanceWidth + settingsMetrics.advanceWidth + root.theme.space(12)
      + arrow.width + root.theme.spacing.sm * 3 > root.width ? "stngs" : "settings"
    Accessible.name: "Server settings"
    ToolTip.visible: hovered
    ToolTip.text: "Server settings"
    onClicked: { selector.popup.close(); root.settingsRequested() }
  }

  ComboBox {
    id: selector
    objectName: "activeServerSelector"
    anchors.left: parent.left
    anchors.right: settingsButton.visible ? settingsButton.left : parent.right
    anchors.rightMargin: settingsButton.visible ? root.theme.spacing.sm : 0
    height: root.theme.space(root.compact ? 28 : 32)
    padding: 0
    leftPadding: root.theme.spacing.sm
    rightPadding: arrow.width
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
      id: serverLabel
      verticalAlignment: Text.AlignVCenter
      text: (root.theme.tui ? "@ " : "") + String(selector.currentText || "Server")
        + (root.bridge.activeServer.connected === false ? " · offline" : "")
      elide: Text.ElideRight
      color: root.bridge.activeServer.connected === false ? root.theme.muted : root.theme.foreground
      font: selector.font
    }
    indicator: Item {
      id: arrow
      objectName: "serverDropdownArrow"
      x: selector.width - width
      width: root.theme.space(26)
      height: selector.height
      Text {
        anchors.centerIn: parent; text: "▾"
        color: root.theme.foreground; font: selector.font
      }
      MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: {
          selector.forceActiveFocus()
          if (selector.popup.visible) selector.popup.close()
          else selector.popup.open()
        }
      }
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
  Button {
    id: inviteButton
    objectName: "serverInviteFriend"
    anchors.top: selector.bottom
    anchors.topMargin: root.theme.spacing.xs
    anchors.left: parent.left
    anchors.right: parent.right
    height: root.theme.space(28)
    text: "Invite friend"
    enabled: root.bridge.activeServer.connected !== false
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    ThemeControlStyle { theme: root.theme; control: parent }
    onClicked: {
      root.bridge.lastAccountInvite = null
      root.bridge.lastError = ""
      invitePopup.copied = false
      invitePopup.open()
      root.bridge.createAccountInvite("friend", "", 30)
    }
  }
  Connections { target: root.bridge; function onActiveServerChanged() { invitePopup.close() } }
  Popup {
    id: invitePopup
    objectName: "serverInvitePopup"
    property bool copied: false
    width: Math.min(360, root.width)
    y: root.height
    padding: root.theme.spacing.md
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    background: Rectangle { color: root.theme.surface; border.color: root.theme.muted; radius: root.theme.cornerRadius }
    contentItem: Column {
      spacing: root.theme.spacing.sm
      Text {
        width: parent.width
        text: "Invite a friend"
        color: root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.body
      }
      Text {
        width: parent.width
        wrapMode: Text.Wrap
        textFormat: Text.PlainText
        text: root.bridge.lastAccountInvite ? "Send this one-use link to your friend. It expires in 30 minutes. Open it with Wisp installed, or paste it into Create account." : root.bridge.lastError || "Creating invitation…"
        color: root.theme.muted
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }
      TextField {
        id: inviteLink
        objectName: "serverInviteLink"
        width: parent.width
        visible: !!root.bridge.lastAccountInvite
        readOnly: true
        selectByMouse: true
        text: root.bridge.lastAccountInvite ? String(root.bridge.lastAccountInvite.uri || root.bridge.lastAccountInvite.code) : ""
        ThemeControlStyle { theme: root.theme; control: parent }
      }
      Button {
        width: parent.width
        text: invitePopup.copied ? "Copied!" : "Copy invite link"
        enabled: !!root.bridge.lastAccountInvite
        ThemeControlStyle { theme: root.theme; control: parent }
        onClicked: { inviteLink.selectAll(); inviteLink.copy(); inviteLink.deselect(); invitePopup.copied = true }
      }
    }
  }

}
