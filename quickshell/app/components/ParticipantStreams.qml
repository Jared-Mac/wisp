import QtQuick
import QtQuick.Controls

Flow {
  id: root
  required property var bridge
  required property var theme
  property bool adaptive: false
  property real availableWidth: theme.space(280)
  readonly property bool narrow: adaptive && availableWidth < theme.space(140)
  readonly property bool tiny: adaptive && width < theme.space(56)
  required property var person
  property bool current: false
  readonly property var videos: current ? bridge.remoteVideos.filter(function(video) {
    return video.participant === root.person.display_name
  }) : []
  spacing: theme.spacing.xs
  visible: videos.length > 0
  Repeater {
    model: root.videos
    Row {
      required property var modelData
      width: root.narrow ? Math.min(root.availableWidth,root.theme.space(28)) : implicitWidth
      spacing: root.theme.spacing.xs
      readonly property bool watching: !!modelData.subscribed || !!modelData.surface_open
      Text {
        visible: !root.narrow
        anchors.verticalCenter: parent.verticalCenter
        text: root.theme.friendly ? (modelData.source === "camera" ? "CAM" : "LIVE") : modelData.source === "camera" ? " · cam" : " · live"
        color: modelData.source === "camera" ? root.theme.accent : root.theme.danger
        font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        Accessible.name: root.person.display_name + (modelData.source === "camera" ? " is sharing a camera" : " is streaming")
      }
      ChatButton {
        objectName: "participantStream-" + root.person.id + "-" + modelData.source
        theme: root.theme; text: parent.watching ? "leave" : "watch"; iconName: root.narrow ? (parent.watching ? "close" : modelData.source === "camera" ? "camera" : "play") : ""
        iconOnly: root.narrow; forceIcon: root.narrow
        width: root.narrow ? Math.min(root.availableWidth,root.theme.space(28)) : implicitWidth
        destructive: root.narrow && modelData.source !== "camera"
        implicitHeight: root.theme.space(root.narrow ? 28 : 22)
        Accessible.name: (parent.watching ? "Stop watching " : "Watch ") + root.person.display_name + (modelData.source === "camera" ? "'s camera" : "'s stream")
        ToolTip.visible: hovered; ToolTip.text: Accessible.name
        onClicked: root.bridge.watchVideo(modelData, !parent.watching)
      }
    }
  }
}
