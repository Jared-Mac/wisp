import QtQuick
import QtQuick.Controls
import "../ChatMarkup.js" as Markup

Rectangle {
  id: root
  required property var theme
  property string message: ""
  property string lastUrl: ""
  property double lastOpened: 0
  property var launchUrl: function(url) { return Qt.openUrlExternally(url) }
  function open(url) {
    if (!Markup.safeLink(url)) return
    var now = Date.now()
    if (url !== lastUrl || now - lastOpened >= 2000) {
      if (launchUrl(url)) {
        lastUrl = url
        lastOpened = now
        message = "Opened in browser"
      } else {
        message = "Could not open browser. Try again or copy the link."
      }
    }
    dismiss.restart()
  }
  visible: message !== ""
  z: 30
  anchors.horizontalCenter: parent.horizontalCenter
  anchors.bottom: parent.bottom
  anchors.bottomMargin: theme.space(12)
  width: Math.min(parent.width - theme.space(16), label.implicitWidth + theme.space(24))
  height: label.implicitHeight + theme.space(16)
  radius: theme.cornerRadius
  color: theme.surface
  border.color: theme.accent
  Accessible.role: Accessible.StaticText
  Accessible.name: message
  Text {
    id: label
    anchors.centerIn: parent
    width: parent.width - root.theme.space(24)
    text: root.message
    color: root.theme.foreground
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    wrapMode: Text.WordWrap
    horizontalAlignment: Text.AlignHCenter
  }
  Timer { id: dismiss; interval: 2400; onTriggered: root.message = "" }
}
