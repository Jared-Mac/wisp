import QtQuick

Rectangle {
  id: root
  objectName: "settingsSavedNotice"
  required property var theme
  property bool shown: false
  function showSaved() { shown = true; dismiss.restart() }
  function clear() { dismiss.stop(); shown = false }
  visible: shown
  width: label.implicitWidth + theme.space(20)
  height: label.implicitHeight + theme.space(12)
  radius: theme.cornerRadius
  color: theme.surface
  border.width: 1; border.color: theme.surfaceBorder
  Text {
    id: label
    anchors.centerIn: parent
    text: "Changes Saved"
    color: root.theme.foreground
    font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Timer { id: dismiss; interval: 2500; onTriggered: root.shown = false }
}
