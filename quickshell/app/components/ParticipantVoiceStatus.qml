import QtQuick
import QtQuick.Controls

Row {
  id: root
  objectName: "participantVoiceStatus"
  required property var theme
  property var moderation: ({})
  property bool muted: false
  property bool deafened: false
  property bool localMuted: false
  readonly property bool serverMuted: !!moderation.muted || !!moderation.deafened
  readonly property bool serverDeafened: !!moderation.deafened
  readonly property string description: [
    serverMuted ? "Server muted" : muted ? "Microphone muted" : "",
    serverDeafened ? "Server deafened" : deafened ? "Self deafened" : "",
    localMuted ? "Muted for you" : ""
  ].filter(function(label) { return label.length > 0 }).join(" · ")
  visible: muted || deafened || serverMuted || serverDeafened || localMuted
  spacing: theme.spacing.xs
  Image {
    id: microphone; objectName: "participantMicrophoneStatus"
    visible: root.muted || root.serverMuted
    width: root.theme.space(16); height: width
    source: Qt.resolvedUrl(root.serverMuted ? "../assets/microphone-server-muted.svg" : "../assets/microphone-muted.svg")
    Accessible.role: Accessible.StaticText
    Accessible.name: root.serverMuted ? "Server muted" : "Microphone muted"
    HoverHandler { id: microphoneHover }
    ToolTip.visible: microphoneHover.hovered; ToolTip.text: Accessible.name
  }
  Image {
    id: headphones; objectName: "participantDeafenStatus"
    visible: root.deafened || root.serverDeafened
    width: root.theme.space(16); height: width
    source: Qt.resolvedUrl(root.serverDeafened ? "../assets/server-deafened.svg" : "../assets/deafened.svg")
    Accessible.role: Accessible.StaticText
    Accessible.name: root.serverDeafened ? "Server deafened" : "Self deafened"
    HoverHandler { id: headphonesHover }
    ToolTip.visible: headphonesHover.hovered; ToolTip.text: Accessible.name
  }
  Image {
    id: speaker; objectName: "participantLocalMuteStatus"
    visible: root.localMuted
    width: root.theme.space(16); height: width
    source: Qt.resolvedUrl("../assets/speaker-local-muted.svg")
    Accessible.role: Accessible.StaticText; Accessible.name: "Muted for you"
    HoverHandler { id: speakerHover }
    ToolTip.visible: speakerHover.hovered; ToolTip.text: Accessible.name
  }
}
