import QtQuick
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  width: parent ? parent.width : 0
  spacing: root.theme.spacing.sm

  Flow {
    width: parent.width
    spacing: root.theme.spacing.sm
    Repeater {
      model: ["open", "knock", "closed", "away"]
      delegate: Rectangle {
        required property string modelData
        activeFocusOnTab: true
        Accessible.role: Accessible.Button
        Accessible.name: "Who may join: " + modelData
        Keys.onSpacePressed: root.bridge.setPresence(modelData)
        Keys.onReturnPressed: root.bridge.setPresence(modelData)
        width: presenceText.implicitWidth + root.theme.spacing.lg * 2
        height: root.theme.space(30)
        radius: root.theme.cornerRadius
        color: root.bridge.selfState.presence === modelData
          ? root.theme.alpha(root.theme.accent, 0.38)
          : root.theme.alpha(root.theme.foreground, presenceMouse.containsMouse ? 0.12 : 0.055)
        border.width: root.theme.terminal || activeFocus ? 1 : 0
        border.color: activeFocus || root.bridge.selfState.presence === modelData ? root.theme.accent : root.theme.separator
        Text {
          id: presenceText
          anchors.centerIn: parent
          text: modelData.charAt(0).toUpperCase() + modelData.slice(1)
          color: root.theme.foreground
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }
        MouseArea {
          id: presenceMouse
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: root.bridge.setPresence(modelData)
        }
      }
    }
    Item { width: root.theme.spacing.sm; height: root.theme.space(30) }
    AudioStateIndicator {
      objectName: "globalAudioControls"
      bridge: root.bridge; theme: root.theme
      muted: !!root.bridge.selfState.muted || !!root.bridge.selfState.deafened
      deafened: !!root.bridge.selfState.deafened
    }
  }
}
