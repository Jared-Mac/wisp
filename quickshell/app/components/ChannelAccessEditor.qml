import QtQuick
import QtQuick.Controls

Column {
  id: root; objectName:"channelAccessEditor"
  required property var theme
  required property var people
  property string visibility: "everyone"
  property var selectedIds: []
  spacing:theme.spacing.xs
  WispComboBox {
    id:access;objectName:"channelVisibility";theme:root.theme;width:parent.width
    model:["Everyone on the server","Server admins","Selected members"]
    currentIndex:Math.max(0,["everyone","admins","members"].indexOf(root.visibility))
    onActivated:root.visibility=["everyone","admins","members"][currentIndex]
    Accessible.name:"Channel visibility"
  }
  Text {
    width:parent.width;wrapMode:Text.Wrap
    text:root.visibility==="everyone" ? "Current and future members can see this channel." : "Server admins always have access."
    color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption
  }
  Flow {
    width:parent.width;spacing:root.theme.spacing.xs;visible:root.visibility==="members"
    Repeater {
      model:root.people.filter(function(p){return p.role!=="owner" && p.role!=="admin"})
      CheckDelegate {
        id:person;required property var modelData
        text:String(modelData.display_name);height:root.theme.space(32);width:Math.min(implicitWidth,parent.width)
        checked:root.selectedIds.indexOf(String(modelData.id))>=0
        onClicked:root.selectedIds=checked ? root.selectedIds.concat([String(modelData.id)]) : root.selectedIds.filter(function(id){return id!==String(person.modelData.id)})
        ThemeControlStyle {theme:root.theme;control:person}
      }
    }
  }
}
