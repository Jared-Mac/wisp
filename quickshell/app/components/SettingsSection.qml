import QtQuick
import QtQuick.Controls

Item {
  id: root
  required property var theme
  property string title: ""
  property string summary: ""
  property bool expanded: false
  property string sectionIcon: "sliders"
  default property alias content: body.data
  function expandForSearch() { expanded = true }
  width: parent ? parent.width : 0
  implicitHeight: header.height + (expanded ? body.implicitHeight + theme.spacing.lg : 0)
  Rectangle { width: parent.width; height: 1; color: root.theme.separator }
  Button {
    id: header
    width: parent.width
    height: root.theme.space(root.summary ? 64 : 44)
    Accessible.name: root.title
    Accessible.description: (root.expanded ? "Expanded. " : "Collapsed. ") + root.summary
    onClicked: root.expanded = !root.expanded
    background: Rectangle { radius: root.theme.cornerRadius; color: header.hovered ? root.theme.alpha(root.theme.foreground,0.045) : "transparent"; border.width: header.visualFocus ? 1 : 0; border.color: root.theme.accent }
    contentItem: Item {
      WispIcon { id: symbol; theme: root.theme; name: root.sectionIcon; visible: root.theme.friendly; anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter }
      Column {
        anchors.left: root.theme.friendly ? symbol.right : parent.left; anchors.leftMargin: root.theme.friendly ? root.theme.spacing.lg : 0
        anchors.right: chevron.left; anchors.rightMargin: root.theme.spacing.lg; anchors.verticalCenter: parent.verticalCenter
        spacing: root.theme.space(3)
        Text { width: parent.width; text: root.title; elide: Text.ElideRight; color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.weight: Font.DemiBold }
        Text { width: parent.width; visible: !!root.summary; text: root.summary; elide: Text.ElideRight; color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption }
      }
      WispIcon { id: chevron; theme: root.theme; name: "chevron"; rotation: root.expanded ? 180 : 0; anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter }
    }
  }
  Column {
    id: body; anchors.top: header.bottom; width: parent.width; visible: root.expanded; spacing: root.theme.spacing.lg
  }
}
