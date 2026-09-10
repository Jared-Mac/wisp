import QtQuick
import QtQuick.Controls
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  property string pendingRevokeId: ""
  width: parent ? parent.width : 0
  spacing: root.theme.space(12)
  Row {
    width:parent.width;spacing:root.theme.spacing.md
    Text {objectName:"settingsDevices";width:parent.width-refresh.width-parent.spacing;text:"Your devices";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.title;font.weight:Font.DemiBold}
    ChatButton {id:refresh;theme:root.theme;text:"Refresh";onClicked:root.bridge.refreshDevices()}
  }
  Text {width:parent.width;wrapMode:Text.Wrap;text:"Revoke a device to remove its access.";color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
  Repeater {
    model:root.bridge.devices
    Rectangle {
      id:device;required property var modelData
      width:root.width;height:root.theme.space(52);color:root.theme.alpha(root.theme.foreground,0.035);radius:root.theme.cornerRadius
      WispIcon {id:deviceIcon;theme:root.theme;name:"screen";anchors.left:parent.left;anchors.leftMargin:root.theme.space(12);anchors.verticalCenter:parent.verticalCenter}
      Text {
        anchors.left:deviceIcon.right;anchors.leftMargin:root.theme.space(10);anchors.right:actions.left;anchors.rightMargin:root.theme.space(8);anchors.verticalCenter:parent.verticalCenter
        text:String(device.modelData.name || "Device");elide:Text.ElideRight;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body
      }
      Row {
        id:actions;anchors.right:parent.right;anchors.rightMargin:root.theme.space(8);anchors.verticalCenter:parent.verticalCenter;spacing:root.theme.spacing.sm
        ChatButton {
          theme:root.theme;destructive:true;text:device.modelData.revoked ? "Revoked" : root.pendingRevokeId===String(device.modelData.id) ? "Confirm" : "Revoke"
          enabled:!device.modelData.revoked
          onClicked:{if(root.pendingRevokeId===String(device.modelData.id)){root.bridge.revokeDevice(device.modelData.id);root.pendingRevokeId=""}else root.pendingRevokeId=String(device.modelData.id)}
        }
        ChatButton {theme:root.theme;text:"Cancel";visible:root.pendingRevokeId===String(device.modelData.id);onClicked:root.pendingRevokeId=""}
      }
    }
  }
  SettingsSection {
    theme:root.theme;title:"Invite a friend";summary:"Create a one-use account invitation";objectName:"deviceInviteSection";sectionIcon:"invite"
    ChatButton {objectName:"settingsAccountInvite";theme:root.theme;text:"Create invitation";onClicked:root.bridge.createAccountInvite("friend","",30)}
    Text {visible:!!root.bridge.lastAccountInvite;text:"One-use invitation · expires in 30 minutes";color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
    TextField {
      id:invite;visible:!!root.bridge.lastAccountInvite;width:parent.width;readOnly:true;selectByMouse:true
      text:root.bridge.lastAccountInvite ? String(root.bridge.lastAccountInvite.uri || root.bridge.lastAccountInvite.code) : ""
      ThemeControlStyle {theme:root.theme;control:invite}
    }
    ChatButton {theme:root.theme;text:"Copy invitation";visible:!!root.bridge.lastAccountInvite;onClicked:{invite.selectAll();invite.copy();invite.deselect()}}
  }
}
