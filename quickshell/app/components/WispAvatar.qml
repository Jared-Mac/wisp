import QtQuick

Rectangle {
  id: root
  required property var theme
  property string name: ""
  property var bridge: null
  property string userId: ""
  property string serverId: ""
  readonly property string source: bridge ? bridge.avatars.url(serverId,userId) : ""
  function load() { if (visible && bridge) bridge.avatars.load(serverId,userId) }
  onUserIdChanged: Qt.callLater(load)
  onServerIdChanged: Qt.callLater(load)
  onVisibleChanged: if (visible) Qt.callLater(load)
  Component.onCompleted: Qt.callLater(load)
  Connections { target: root.bridge ? root.bridge.avatars : null; function onEpochChanged() { Qt.callLater(root.load) } }
  property bool speaking: false
  implicitWidth: theme.space(28)
  implicitHeight: implicitWidth
  radius: width / 2
  readonly property int seed: { var n=0; for (var i=0;i<name.length;i++) n=(n*31+name.charCodeAt(i))>>>0; return n%5 }
  color: theme.alpha([theme.accent,theme.onlineIndicator,theme.warning,theme.secondaryAccent,theme.muted][seed], theme.light ? 0.17 : 0.2)
  border.width: speaking ? 2 : 0
  border.color: theme.accent
  Accessible.ignored: true
  Image { anchors.fill: parent; source: root.source; fillMode: Image.PreserveAspectFit; asynchronous: true }
  WispIcon { anchors.centerIn: parent; theme: root.theme; name: "wisp"; ink: root.theme.accent; visible: !root.source && root.theme.hearth; width: parent.width*0.7; height: width }
  Text {
    anchors.centerIn: parent; visible: !root.source && !root.theme.hearth
    text: root.name.trim().slice(0,1).toUpperCase() || "W"
    font.family: root.theme.font.family; font.pixelSize: parent.width*0.42; font.weight: Font.DemiBold
    color: root.theme.foreground
  }
}
