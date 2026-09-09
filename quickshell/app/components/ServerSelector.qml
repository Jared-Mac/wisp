import QtQuick
import QtQuick.Controls
import QtQml

Item {
  id: root
  required property var bridge
  required property var theme
  property bool adaptive: false
  readonly property bool narrow: adaptive && width < theme.space(140)
  readonly property bool tiny: adaptive && width < theme.space(56)
  property bool compact: false
  property bool horizontal: false
  property bool showInvite: true
  readonly property bool inlineInvite: horizontal && showInvite && !narrow
  signal settingsRequested()
  implicitHeight: selector.height + (narrow && settingsButton.visible ? settingsButton.height + root.theme.spacing.xs : 0) + (root.showInvite && !inlineInvite ? inviteButton.height + root.theme.spacing.xs : 0)
  TextMetrics { id: serverMetrics; font: selector.font; text: serverLabel.text }
  TextMetrics { id: settingsMetrics; font: selector.font; text: root.theme.tui ? "[settings]" : "settings" }

  ChatButton {
    id: settingsButton
    objectName: "serverSettingsShortcut"
    anchors.right: parent.right
    y: root.narrow ? selector.height + root.theme.spacing.xs : 1
    height: root.narrow ? root.theme.space(28) : selector.height - 2
    theme: root.theme
    iconName: "settings"; iconOnly: root.theme.friendly || root.narrow; forceIcon: root.narrow
    Binding on implicitWidth { when: root.theme.friendly || root.narrow; value: root.narrow ? root.width : root.theme.space(36); restoreMode: Binding.RestoreBindingOrValue }
    visible: root.bridge.canManageServer
    text: serverMetrics.advanceWidth + settingsMetrics.advanceWidth + root.theme.space(12)
      + arrow.width + root.theme.spacing.sm * 3 > root.width ? "stngs" : "settings"
    Accessible.name: "Server settings"
    ToolTip.visible: hovered
    ToolTip.text: "Server settings"
    onClicked: { selector.popup.close(); root.settingsRequested() }
  }

  WispComboBox {
    theme: root.theme
    id: selector
    objectName: "activeServerSelector"
    anchors.left: parent.left
    anchors.right: root.inlineInvite ? inviteButton.left : settingsButton.visible && !root.narrow ? settingsButton.left : parent.right
    anchors.rightMargin: root.inlineInvite || settingsButton.visible && !root.narrow ? root.theme.spacing.sm : 0
    height: root.theme.space(root.theme.friendly ? 40 : root.compact ? 28 : 32)
    padding: 0
    leftPadding: root.tiny ? 0 : root.theme.spacing.sm
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
    font.pixelSize: root.theme.friendly ? root.theme.font.body : root.theme.font.caption
    font.weight: root.theme.friendly ? Font.DemiBold : Font.Normal
    contentItem: Text {
      id: serverLabel
      verticalAlignment: Text.AlignVCenter
      horizontalAlignment: root.tiny ? Text.AlignHCenter : Text.AlignLeft
      text: root.tiny ? String(selector.currentText || "S").slice(0,1).toUpperCase() : (root.theme.tui ? "@ " : "") + String(selector.currentText || "Server")
        + (root.bridge.activeServer.connected === false ? " · offline" : "")
      elide: Text.ElideRight
      color: root.bridge.activeServer.connected === false ? root.theme.muted : root.theme.foreground
      font: selector.font
    }
    indicator: Item {
      id: arrow
      objectName: "serverDropdownArrow"
      x: selector.width - width
      visible: !root.tiny
      width: root.tiny ? 0 : root.theme.space(26)
      height: selector.height
      Text {
        visible: !root.theme.friendly
        anchors.centerIn: parent; text: "▾"
        color: root.theme.foreground; font: selector.font
      }
      WispIcon { anchors.centerIn: parent; theme: root.theme; name: "chevron"; visible: root.theme.friendly }
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
    id: styledControl1
      required property var modelData
      width: selector.popup.width
      height: root.theme.space(32)
      text: String(modelData.name) + (modelData.connected === false ? " · offline" : "")
      highlighted: String(modelData.id)===String(root.bridge.activeServer.id)
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
      ThemeControlStyle { theme: root.theme; control: styledControl1 }
    }
    popup.width: Math.max(root.theme.space(220), selector.width)
    ToolTip.visible: hovered; ToolTip.text: String(currentText || "Server")
    popup.background: Rectangle {
      color: root.theme.surface
      border.width: 1
      border.color: root.theme.muted
      radius: root.theme.cornerRadius
    }
  }
  ChatButton {
    theme: root.theme
    id: inviteButton
    objectName: "serverInviteFriend"
    y: root.inlineInvite ? 1 : selector.height + (root.narrow && settingsButton.visible ? settingsButton.height + root.theme.spacing.xs : 0) + root.theme.spacing.xs
    x: root.inlineInvite ? (settingsButton.visible ? settingsButton.x - root.theme.spacing.sm : parent.width) - width : 0
    width: root.inlineInvite ? root.theme.space(36) : parent.width; height: root.inlineInvite ? selector.height - 2 : root.theme.space(28)
    text: "Invite friend"; iconName: "invite"; iconOnly: root.narrow || root.inlineInvite; forceIcon: root.narrow || root.inlineInvite
    ToolTip.visible: hovered; ToolTip.text: "Invite a friend to this server"
    visible: root.showInvite
    enabled: root.bridge.activeServer.connected !== false
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    ThemeControlStyle { theme: root.theme; control: inviteButton }
    onClicked: {
      root.bridge.lastAccountInvite = null
      root.bridge.lastError = ""
      invitePopup.copied = false
      invitePopup.open()
      root.bridge.createAccountInvite("friend", "", 30)
    }
  }
  // Status snapshots replace the server object; only a selection change dismisses the invite.
  readonly property string inviteServerId: String(root.bridge.activeServer.id || "")
  onInviteServerIdChanged: if (invitePopup.visible) invitePopup.close()
  Popup {
    id: invitePopup
    objectName: "serverInvitePopup"
    property bool copied: false
    width: root.theme.space(300)
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
        ThemeControlStyle { theme: root.theme; control: inviteLink }
      }
      ChatButton {
    id: styledControl2
        theme: root.theme
        width: parent.width
        text: invitePopup.copied ? "Copied!" : "Copy invite link"
        enabled: !!root.bridge.lastAccountInvite
        ThemeControlStyle { theme: root.theme; control: styledControl2 }
        onClicked: { inviteLink.selectAll(); inviteLink.copy(); inviteLink.deselect(); invitePopup.copied = true }
      }
    }
  }

}
