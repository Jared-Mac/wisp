import QtQuick
import QtQuick.Controls
import QtQml

Item {
  id: root
  required property var bridge
  required property var theme
  property bool adaptive: false
  readonly property bool narrow: adaptive && width < theme.space(140)
  readonly property bool tiny: adaptive && width < theme.space(80)
  property bool compact: false
  property bool horizontal: false
  property bool showInvite: true
  readonly property bool inlineInvite: showInvite && !narrow && (horizontal || width >= theme.space(200))
  readonly property bool member: bridge.serverMember !== false
  readonly property var account: bridge.accountActions ? bridge.accountActions.state(bridge.activeServer.id) : ({busy:false,invitation:null,error:"",feedback:"",invites:[]})
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
      text: root.tiny ? String(selector.currentText || "S").slice(0,1).toUpperCase() : (root.theme.tui ? "@ " : "") + String(selector.currentText || "No servers yet")
        + (root.bridge.servers.length && root.bridge.activeServer.connected === false ? " · offline" : "")
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
      color: selector.hovered ? root.theme.alpha(root.theme.foreground,0.07) : root.theme.refinedTui ? "transparent" : root.theme.surface
      border.width: root.theme.refinedTui && !selector.activeFocus ? 0 : 1
      border.color: selector.activeFocus ? root.theme.focusBorder : root.theme.separator
      radius: root.theme.cornerRadius
    }
    delegate: ItemDelegate {
    id: styledControl1
      required property var modelData
      required property int index
      objectName:"serverOption-"+modelData.id
      width: selector.popup.width
      height: root.theme.space(36)
      text: String(modelData.name) + (modelData.connected === false ? " · offline" : "")
      highlighted: String(modelData.id)===String(root.bridge.activeServer.id)
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
      contentItem:Item {
        Item {
          id:grip;objectName:"dragServer-"+styledControl1.modelData.id;width:root.theme.space(20);height:parent.height
          WispIcon {theme:root.theme;name:"grip";anchors.centerIn:parent;opacity:dragMouse.pressed ? 1 : 0.5}
          MouseArea {
            id:dragMouse;anchors.fill:parent;hoverEnabled:true;preventStealing:true
            property real startY:0
            cursorShape:pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
            onPressed:function(mouse){startY=mapToItem(selector.popup.contentItem,0,mouse.y).y}
            onReleased:function(mouse) {
              var end=mapToItem(selector.popup.contentItem,0,mouse.y).y
              root.bridge.serverPreferences.moveBy(styledControl1.modelData.id,Math.round((end-startY)/styledControl1.height))
            }
          }
          ToolTip.visible:dragMouse.containsMouse && !dragMouse.pressed;ToolTip.text:"Drag to reorder within this group"
        }
        Text {
          anchors.left:grip.right;anchors.leftMargin:root.theme.space(4);anchors.right:serverActions.left;anchors.rightMargin:root.theme.space(4);anchors.verticalCenter:parent.verticalCenter
          text:styledControl1.text;textFormat:Text.PlainText;elide:Text.ElideRight;color:root.theme.foreground;font:selector.font
        }
        Row {
          id:serverActions;anchors.right:parent.right;anchors.verticalCenter:parent.verticalCenter;spacing:root.theme.space(2)
          ChatButton {
            objectName:"homeServer-"+styledControl1.modelData.id
            theme:root.theme;text:"Home server";iconName:"home";iconOnly:true;forceIcon:true;width:root.theme.space(24);height:width
            readonly property bool home:root.bridge.serverPreferences.isHome(styledControl1.modelData.id)
            primary:home;enabled:!home || root.bridge.serverPreferences.homeIds.length>1
            ToolTip.visible:hovered || visualFocus;ToolTip.text:home ? "Home server" : "Set as a home server"
            onClicked:root.bridge.serverPreferences.setHome(styledControl1.modelData.id,!home)
          }
          Repeater {
            model:[-1,1]
            ChatButton {
              required property int modelData
              objectName:(modelData<0 ? "moveServerUp-" : "moveServerDown-")+styledControl1.modelData.id
              theme:root.theme;text:modelData<0 ? "Move server up" : "Move server down";iconName:"chevron";iconOnly:true;forceIcon:true
              contentItem:WispIcon {theme:root.theme;name:"chevron";rotation:parent.modelData<0 ? 180 : 0;ink:root.theme.foreground}
              width:root.theme.space(24);height:width
              enabled:root.bridge.serverPreferences.canMove(styledControl1.modelData.id,modelData)
              ToolTip.visible:hovered || visualFocus;ToolTip.text:text
              onClicked:root.bridge.serverPreferences.moveBy(styledControl1.modelData.id,modelData)
            }
          }
        }
      }
      ThemeControlStyle { theme: root.theme; control: styledControl1 }
    }
    popup.width: Math.min(Math.max(root.theme.space(300), selector.width),root.Window.window ? root.Window.window.width-root.theme.space(24) : root.theme.space(300))
    readonly property var ping: root.bridge.serverPings[String(root.bridge.activeServer.id)] || ({})
    onHoveredChanged: if(hovered && root.bridge.activeServer.connected) root.bridge.refreshServerPing(root.bridge.activeServer.id)
    onCurrentIndexChanged: if(hovered && root.bridge.activeServer.connected) root.bridge.refreshServerPing(root.bridge.activeServer.id)
    Timer {interval:10000;repeat:true;running:selector.hovered && !!root.bridge.activeServer.connected;onTriggered:root.bridge.refreshServerPing(root.bridge.activeServer.id)}
    ToolTip.visible: hovered
    ToolTip.text: String(currentText || "Server") + " · " + (!root.bridge.activeServer.connected ? "Offline" : ping.pending ? "Measuring ping…" : ping.ms!==null && ping.ms!==undefined ? Math.round(ping.ms)+" ms" : "Ping unavailable")
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
    x: root.inlineInvite ? (settingsButton.visible ? settingsButton.x - root.theme.spacing.sm : parent.width) - width : root.narrow ? (parent.width-width)/2 : parent.width-width
    width: Math.min(parent.width,root.theme.space(36)); height: root.inlineInvite ? selector.height - 2 : root.theme.space(28)
    text: root.member ? "Invite to server" : "Join a server"; iconName: "invite"; iconOnly: true; forceIcon: true
    ToolTip.visible: hovered; ToolTip.text: text
    visible: root.showInvite
    enabled: root.bridge.activeServer.connected !== false
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    ThemeControlStyle { theme: root.theme; control: inviteButton }
    onClicked: {
      if(!root.member) {root.bridge.accountActions.joinServer();return}
      invitePopup.copied=false
      invitePopup.open()
      root.bridge.accountActions.act("create_server_invite",{expires_in_minutes:720})
    }
  }
  // Status snapshots replace the server object; only a selection change dismisses the invite.
  readonly property string inviteServerId: String(root.bridge.activeServer.id || "")
  onInviteServerIdChanged: if (invitePopup.visible) invitePopup.close()
  Popup {
    id: invitePopup
    objectName: "serverInvitePopup"
    property bool copied:false
    property bool showQr:false
    property bool confirmLeave:false
    onClosed:confirmLeave=false
    width:Math.min(root.theme.space(340),root.Window.window ? root.Window.window.width-root.theme.space(24) : root.theme.space(340))
    height:Math.min(inviteContent.implicitHeight+padding*2,root.Window.window ? root.Window.window.height-root.theme.space(40) : root.theme.space(600))
    parent:Overlay.overlay
    onAboutToShow:{
      var point=root.mapToItem(parent,0,root.height)
      x=Math.max(root.theme.space(8),Math.min(point.x,parent.width-width-root.theme.space(8)))
      y=Math.max(root.theme.space(8),Math.min(point.y,parent.height-height-root.theme.space(8)))
    }
    padding:root.theme.spacing.md
    closePolicy:Popup.CloseOnEscape | Popup.CloseOnPressOutside
    background:Rectangle {color:root.theme.surface;border.color:root.theme.muted;radius:root.theme.cornerRadius}
    contentItem:ScrollView {
      clip:true
      Column {
        id:inviteContent;width:invitePopup.availableWidth;spacing:root.theme.spacing.sm
        Text {text:"Invite to server";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body;font.bold:true}
        Text {width:parent.width;wrapMode:Text.Wrap;text:"A one-use invitation to "+String(root.bridge.activeServer.name)+". Adding friends is separate.";textFormat:Text.PlainText;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
        Text {width:parent.width;wrapMode:Text.Wrap;text:root.account.error || root.account.feedback || (root.account.busy ? "Working…" : root.account.invitation ? "Expires "+new Date(root.account.invitation.expires_at).toLocaleString() : "");textFormat:Text.PlainText;color:root.account.error ? root.theme.danger : root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption;visible:!!text}
        TextField {id:inviteLink;objectName:"serverInviteLink";width:parent.width;visible:!!root.account.invitation;readOnly:true;selectByMouse:true;text:root.account.invitation ? root.account.invitation.uri : "";ThemeControlStyle {theme:root.theme;control:inviteLink}}
        ChatButton {theme:root.theme;objectName:"copyServerInvitation";text:invitePopup.copied ? "Copied!" : "Copy invitation";enabled:!!root.account.invitation;onClicked:{root.bridge.copyChatText(inviteLink.text);invitePopup.copied=true}}
        ChatButton {theme:root.theme;text:invitePopup.showQr ? "Hide QR code" : "Show QR code";enabled:!!root.account.invitation;onClicked:invitePopup.showQr=!invitePopup.showQr}
        Image {width:Math.min(parent.width,root.theme.space(224));height:visible ? width : 0;visible:invitePopup.showQr && !!root.account.invitation;source:root.account.invitation && invitePopup.showQr ? root.account.invitation.qr : "";fillMode:Image.PreserveAspectFit;Accessible.name:"Server invitation QR code"}
        ChatButton {theme:root.theme;text:"New link";enabled:!root.account.busy;onClicked:{invitePopup.copied=false;root.bridge.accountActions.act("create_server_invite",{expires_in_minutes:720})}}
        ChatButton {theme:root.theme;text:"Revoke this invitation";visible:!!root.account.invitation;enabled:!root.account.busy;onClicked:root.bridge.accountActions.act("revoke_server_invite",{invite_id:root.account.invitation.id})}
        ChatButton {theme:root.theme;text:"Manage active invitations";enabled:!root.account.busy;onClicked:root.bridge.accountActions.act("list_server_invites",{})}
        Repeater {
          model:root.account.invites
          Row {
            required property var modelData
            width:inviteContent.width;spacing:root.theme.spacing.xs
            Text {width:Math.max(0,parent.width-revoke.width-parent.spacing);text:"Expires "+new Date(parent.modelData.expires_at).toLocaleString();elide:Text.ElideRight;color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
            ChatButton {id:revoke;theme:root.theme;text:"Revoke";enabled:!root.account.busy;onClicked:root.bridge.accountActions.act("revoke_server_invite",{invite_id:parent.modelData.id})}
          }
        }
        ChatButton {theme:root.theme;text:"Join another server";onClicked:{invitePopup.close();root.bridge.accountActions.joinServer()}}
        Text {width:parent.width;visible:invitePopup.confirmLeave;wrapMode:Text.Wrap;text:"Leave this server? Your account, friends and DMs stay available.";color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
        ChatButton {theme:root.theme;text:invitePopup.confirmLeave ? "Confirm leave server" : "Leave server";visible:!(root.bridge.selfState || {}).server_owner;enabled:!root.account.busy;onClicked:{if(!invitePopup.confirmLeave){invitePopup.confirmLeave=true;return}if(root.bridge.accountActions.act("leave_server",{}))invitePopup.close()}}

      }
    }
  }
}
