import QtQuick

Rectangle {
  id: root
  objectName: "roomInvitationCard"
  required property var bridge
  required property var theme
  required property var invitation
  property bool outgoing: false
  readonly property bool pending: invitation.status === "pending" && Date.parse(invitation.expires_at) > bridge.invitationClock
    && (outgoing ? bridge.selfState.hangout_id === invitation.hangout_id : (bridge.roomInvitations || []).some(function(i) { return i.id === root.invitation.invitation_id }))
  implicitHeight: content.implicitHeight + theme.spacing.lg * 2
  radius: theme.cornerRadius; color: theme.alpha(theme.accent, 0.08)
  border.width: 1; border.color: pending ? theme.accent : theme.separator
  Column {
    id: content
    x: root.theme.spacing.lg; y: x
    width: Math.max(1, parent.width - x * 2); spacing: root.theme.spacing.sm
    Text {
      width: parent.width; wrapMode: Text.Wrap; textFormat: Text.PlainText
      text: "Voice invite · " + String(root.invitation.room_label || "Room")
      color: root.theme.accent; font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true
    }
    Text {
      width: parent.width; wrapMode: Text.Wrap
      text: !root.pending ? (root.invitation.status === "accepted" ? "Accepted" : root.invitation.status === "dismissed" ? "Dismissed" : "Invitation no longer available")
        : root.outgoing ? "Waiting for your friend · expires in 5 minutes"
        : (root.bridge.selfState.hangout_id && root.bridge.selfState.hangout_id !== root.invitation.hangout_id ? "Accepting leaves your current room. " : "")
          + "Join voice with your current mute/deafen settings. Camera and screen sharing stay off."
      color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    Flow {
      width: parent.width; spacing: root.theme.spacing.sm
      visible: root.pending && !root.outgoing
      ChatButton {
        objectName: "acceptVoiceInvite"; theme: root.theme; primary: true; text: "Accept & Join Voice"
        width: Math.min(implicitWidth, parent.width)
        enabled: !(root.bridge.invitationRequests || {})[root.invitation.invitation_id]
        onClicked: root.bridge.respondRoomInvitation(root.invitation, true)
      }
      ChatButton {
        objectName: "dismissVoiceInvite"; theme: root.theme; text: "Dismiss"
        enabled: !(root.bridge.invitationRequests || {})[root.invitation.invitation_id]
        onClicked: root.bridge.respondRoomInvitation(root.invitation, false)
      }
    }
  }
}
