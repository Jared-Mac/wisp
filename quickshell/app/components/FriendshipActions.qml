import QtQuick
import QtQuick.Controls

Flow {
  id: root
  required property var bridge
  required property var theme
  required property var person
  property bool compact: false
  readonly property string relationship: bridge.friendships.relationship(person)
  readonly property var state: bridge.friendships.state(person.server_id)
  readonly property bool busy: state.loading || !!state.action
  spacing: theme.spacing.xs
  visible: relationship!=="self" && relationship!=="friend"
  ChatButton {
    objectName:"addFriend"; theme:root.theme; width:root.compact && dismiss.visible ? Math.max(1,parent.width-dismiss.width-root.spacing) : parent.width
    text:root.relationship==="incoming" ? root.compact ? "Accept" : "Accept friend request" : root.relationship==="outgoing" ? "Request sent" : "Add friend"
    iconName:root.relationship==="outgoing" ? "check" : "invite"
    primary:root.compact && root.relationship==="incoming"
    enabled:!root.busy && root.relationship!=="outgoing" && root.bridge.friendships.connected(root.person.server_id)
    onClicked:root.bridge.friendships.act(root.person,root.relationship==="incoming" ? "accept" : "send")
  }
  ChatButton {
    id:dismiss;objectName:"dismissFriendRequest"; theme:root.theme; width:root.compact ? Math.min(parent.width,root.theme.space(30)) : parent.width
    iconName:"close";iconOnly:root.compact;forceIcon:root.compact
    ToolTip.visible:hovered || visualFocus;ToolTip.text:text
    visible:root.relationship==="incoming" || root.relationship==="outgoing"; enabled:!root.busy
    text:root.relationship==="incoming" ? "Decline" : "Cancel request"
    onClicked:root.bridge.friendships.act(root.person,"dismiss")
  }
}
