import QtQuick
import "../ActionIcons.js" as Icons

Item {
  id: root
  required property var theme
  property string name: ""
  property color ink: theme.foreground
  implicitWidth: theme.space(18)
  implicitHeight: implicitWidth
  Accessible.ignored: true
  Image {
    anchors.fill: parent
    source: Icons.source(root.name, root.ink)
    sourceSize.width: width * Screen.devicePixelRatio
    sourceSize.height: height * Screen.devicePixelRatio
    fillMode: Image.PreserveAspectFit
  }
}
