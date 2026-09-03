import QtQuick
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  signal joined()
  signal roomLeft()
  readonly property var visibleHangouts: {
    var currentHangoutId = String(root.bridge.selfState.hangout_id || "")
    var spots = root.bridge.spots || []
    var spotHangoutIds = spots.map(function(spot) {
      return String(spot.active_hangout_id || "")
    })
    var hangouts = root.bridge.hangouts || []
    return hangouts.filter(function(hangout) {
      var hangoutId = String(hangout.id || "")
      return hangoutId === currentHangoutId
        || spotHangoutIds.indexOf(hangoutId) === -1
    })
  }
  width: parent ? parent.width : 0
  spacing: root.theme.spacing.sm
  visible: visibleHangouts.length > 0

  Text {
    text: "HANGOUTS"
    color: root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.weight: Font.Bold
  }

  Repeater {
    model: root.visibleHangouts
    delegate: Column {
      id: hangoutEntry
      required property var modelData
      width: root.width
      spacing: root.theme.spacing.sm

      HangoutCard {
        width: parent.width
        hangout: hangoutEntry.modelData
        bridge: root.bridge
        theme: root.theme
        onJoined: root.joined()
      }

      MediaControls {
        visible: root.bridge.selfState.hangout_id === hangoutEntry.modelData.id
        width: parent.width
        bridge: root.bridge
        theme: root.theme
        onLeaveRequested: root.roomLeft()
      }
    }
  }
}
