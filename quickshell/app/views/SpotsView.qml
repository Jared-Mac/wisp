import QtQuick

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
  spacing: root.theme.spacing.sm
  visible: availableSpots.length > 0

  Text {
    text: "SPOTS"
    color: root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.weight: Font.Bold
  }

  Repeater {
    model: root.availableSpots
    delegate: Rectangle {
      required property var modelData
      width: root.width
      height: root.theme.space(52)
      radius: root.theme.cornerRadius
      color: root.theme.alpha(root.theme.foreground, 0.05)

      Column {
        anchors.left: parent.left
        anchors.leftMargin: root.theme.spacing.lg
        anchors.verticalCenter: parent.verticalCenter

        Text {
          text: String(modelData.name || "Spot")
          color: root.theme.foreground
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.body
          font.weight: Font.DemiBold
        }

        Text {
          text: (modelData.members || []).length
            ? modelData.members.map(function(member) { return member.display_name }).join(" · ")
            : "Empty · starts when you join"
          color: root.theme.muted
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }
      }

      Rectangle {
        width: joinText.implicitWidth + root.theme.spacing.lg * 2
        height: root.theme.space(30)
        anchors.right: parent.right
        anchors.rightMargin: root.theme.spacing.lg
        anchors.verticalCenter: parent.verticalCenter
        radius: root.theme.cornerRadius
        color: joinMouse.containsMouse
          ? root.theme.alpha(root.theme.accent, 0.8) : root.theme.accent

        Text {
          id: joinText
          anchors.centerIn: parent
          text: "Join"
          color: "white"
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
    }
  }
}
