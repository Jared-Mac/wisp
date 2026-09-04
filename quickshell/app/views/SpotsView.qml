import QtQuick
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  signal joined()
  readonly property var availableSpots: {
    var hangouts = root.bridge.hangouts || []
    var activeHangoutIds = hangouts.map(function(hangout) {
      return String(hangout.id || "")
    })
    var spots = root.bridge.spots || []
    return spots.filter(function(spot) {
      var activeHangoutId = String(spot.active_hangout_id || "")
      return activeHangoutId === ""
        || activeHangoutIds.indexOf(activeHangoutId) === -1
    })
  }
  width: parent ? parent.width : 0
  spacing: root.theme.spacing.xs
  visible: availableSpots.length > 0

  Repeater {
    model: root.availableSpots
    delegate: Rectangle {
      objectName: "availableRoomCard"
      required property var modelData
      TapHandler { acceptedButtons: Qt.RightButton; onTapped: volumeMenu.open() }
      ParticipantVolumeMenu { id: volumeMenu; bridge: root.bridge; theme: root.theme; people: modelData.members || [] }
      width: root.width
      height: root.theme.space(44)
      radius: root.theme.cornerRadius
      color: root.theme.tui ? root.theme.surface : root.theme.alpha(root.theme.foreground, 0.05)
      border.width: root.theme.tui ? 0 : root.theme.terminal ? 1 : 0
      border.color: root.theme.roomBorder

      Column {
        id: spotInfo
        anchors.left: parent.left
        anchors.leftMargin: root.theme.spacing.lg
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(0, chatButton.x - spotInfo.x - root.theme.spacing.sm)

        Text {
          Binding on width { when: root.theme.terminal; value: spotInfo.width; restoreMode: Binding.RestoreBindingOrValue }
          elide: root.theme.terminal ? Text.ElideRight : Text.ElideNone
          text: (root.theme.tui ? "# " : "") + String(modelData.name || "Spot")
          color: root.theme.foreground
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.body
          font.weight: Font.DemiBold
        }

        Text {
          Binding on width { when: root.theme.terminal; value: spotInfo.width; restoreMode: Binding.RestoreBindingOrValue }
          elide: root.theme.terminal ? Text.ElideRight : Text.ElideNone
          text: (modelData.members || []).length
            ? modelData.members.map(function(member) { return member.display_name }).join(" · ")
            : "Empty · starts when you join"
          color: root.theme.muted
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }
      }

      Rectangle {
        id: joinButton
        width: joinText.implicitWidth + root.theme.spacing.lg * 2
        height: root.theme.space(30)
        anchors.right: parent.right
        anchors.rightMargin: root.theme.spacing.lg
        anchors.verticalCenter: parent.verticalCenter
        radius: root.theme.cornerRadius
        color: root.theme.tui ? (joinMouse.containsMouse ? root.theme.alpha(root.theme.accent, 0.18) : "transparent") : joinMouse.containsMouse
          ? root.theme.alpha(root.theme.accent, 0.8) : root.theme.accent

        Text {
          id: joinText
          anchors.centerIn: parent
          text: root.theme.tui ? "[join]" : "Join"
          color: root.theme.tui ? root.theme.accent : root.theme.accentText
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
          font.weight: Font.DemiBold
        }

        MouseArea {
          id: joinMouse
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: {
            root.bridge.joinSpot(modelData.id)
            root.joined()
          }
        }
      }
      ChatButton {
        id: chatButton; objectName: "openSpotChat"
        theme: root.theme; text: "Chat"
        anchors.right: joinButton.left; anchors.rightMargin: root.theme.spacing.sm
        anchors.verticalCenter: joinButton.verticalCenter
        Accessible.name: "Open " + String(modelData.name || "room") + " text chat"
        onClicked: root.bridge.openRoomChat(modelData, true)
      }
    }
  }
}
