import QtQuick
import QtQuick.Controls

Column {
  id: root
  required property var bridge
  required property var theme
  required property var person
  readonly property string relationship: bridge.friendships.relationship(person)
  readonly property var state: bridge.friendships.state(person.server_id)
  readonly property bool busy: state.loading || !!state.action
  spacing: theme.spacing.xs
  visible: relationship!=="self" && relationship!=="friend"
  ChatButton {
    objectName:"addFriend"; theme:root.theme; width:parent.width
    text:root.relationship==="incoming" ? "Accept friend request" : root.relationship==="outgoing" ? "Request sent" : "Add friend"
    iconName:root.relationship==="outgoing" ? "check" : "invite"
    enabled:!root.busy && root.relationship!=="outgoing" && root.bridge.friendships.connected(root.person.server_id)
    onClicked:root.bridge.friendships.act(root.person,root.relationship==="incoming" ? "accept" : "send")
  }
  ChatButton {
    objectName:"dismissFriendRequest"; theme:root.theme; width:parent.width
    visible:root.relationship==="incoming" || root.relationship==="outgoing"; enabled:!root.busy
    text:root.relationship==="incoming" ? "Decline" : "Cancel request"
    onClicked:root.bridge.friendships.act(root.person,"dismiss")
  }
}
