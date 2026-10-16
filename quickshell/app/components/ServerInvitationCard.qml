import QtQuick

Rectangle {
  id: root
  required property var bridge
  required property var theme
  required property var invitation
  required property string messageId
  required property string serverId
  property bool outgoing: false
  readonly property bool expired: Date.parse(invitation.expires_at) <= bridge.invitationClock
  readonly property bool declined: bridge.accountActions.invitationDeclined(serverId,messageId)
  implicitHeight: content.implicitHeight + theme.spacing.lg * 2
  radius: theme.cornerRadius; color: theme.alpha(theme.accent,0.08)
  border.width: 1; border.color: expired || declined ? theme.separator : theme.accent
  Column {
    id: content; x: root.theme.spacing.lg; y: x
    width: Math.max(1,parent.width-x*2); spacing: root.theme.spacing.sm
    Text {
      width: parent.width; wrapMode: Text.Wrap; textFormat: Text.PlainText
      text: "Server invite · " + root.invitation.server_name
      color: root.theme.accent; font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true
    }
    Text {
      width: parent.width; wrapMode: Text.Wrap; textFormat: Text.PlainText
      text: root.expired ? "Invitation expired" : root.declined ? "Declined on this device"
        : root.outgoing ? "Invitation sent · expires " + new Date(root.invitation.expires_at).toLocaleString()
        : "Review and accept this invitation to join the server."
      color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    Flow {
      width:parent.width;spacing:root.theme.spacing.sm
      visible:!root.outgoing && !root.expired && !root.declined
      ChatButton {objectName:"acceptServerInvitation";theme:root.theme;primary:true;text:"Accept…";onClicked:root.bridge.accountActions.joinServer(root.invitation.uri)}
      ChatButton {objectName:"declineServerInvitation";theme:root.theme;text:"Decline";onClicked:root.bridge.accountActions.declineInvitation(root.serverId,root.messageId)}
    }
  }
}
