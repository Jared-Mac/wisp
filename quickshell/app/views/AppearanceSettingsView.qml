import QtQuick
import QtQuick.Controls
import "../components"

Column {
  id: root
  required property var theme
  readonly property var appearance: theme.appearanceController
  readonly property bool editable: !!appearance && !appearance.managed
  width: parent ? parent.width : 0
  spacing: theme.space(16)
  Text { objectName: "settingsAppearance"; text: "Make Wisp feel like you"; color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.title; font.weight: Font.DemiBold }
  Text {
    width: parent.width; wrapMode: Text.WordWrap
    text: root.editable ? "Choose a look. Your chats and layout stay in place." : "Appearance follows your Omarchy shell."
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }
  Flow {
    id: presets; width: parent.width; spacing: root.theme.space(12)
    readonly property int columns: width >= root.theme.space(630) ? 3 : width >= root.theme.space(400) ? 2 : 1
    Repeater {
      model: [
        {profile:"soft_graphite",label:"Soft Graphite",detail:"Quiet dark · blue accents",base:"#181e25",rail:"#222b34",accent:"#8ec5ee",ink:"#e8ecf3",radius:8},
        {profile:"daylight",label:"Daylight",detail:"Warm light · calm and clear",base:"#f7f6f1",rail:"#eaede5",accent:"#3e6b5a",ink:"#24372f",radius:6},
        {profile:"hearth",label:"Hearth",detail:"Cozy dark · a little playful",base:"#24212d",rail:"#353041",accent:"#f0b898",ink:"#f4edf6",radius:12}
      ]
      Button {
        id: preset; required property var modelData
        objectName: "theme-" + modelData.profile
        width: (presets.width-presets.spacing*(presets.columns-1))/presets.columns; height: root.theme.space(170)
        enabled: root.editable; checkable: true; checked: !!root.appearance && root.appearance.profile === modelData.profile
        Accessible.name: modelData.label + (checked ? ", selected" : "")
        Accessible.description: modelData.detail
        padding: root.theme.space(12)
        onClicked: root.appearance.setProfile(modelData.profile)
        background: Rectangle { radius: root.theme.space(10); color: root.theme.alpha(root.theme.foreground,preset.hovered ? 0.07 : 0.025); border.width: preset.checked || preset.visualFocus ? 2 : 1; border.color: preset.checked || preset.visualFocus ? root.theme.accent : root.theme.separator }
        contentItem: Column {
          spacing: root.theme.space(8)
          Rectangle {
            width: parent.width; height: root.theme.space(88); radius: preset.modelData.radius; color: preset.modelData.base
            Rectangle { x:8; y:8; width:parent.width*0.27; height:parent.height-16; radius:4; color:preset.modelData.rail
              Repeater { model:3; Rectangle { required property int index; x:6; y:12+index*16; width:parent.width-12; height:5; radius:2; color:preset.modelData.accent; opacity:index===0 ? 1 : 0.3 } }
            }
            Rectangle { x:parent.width*0.33; y:8; width:parent.width*0.63; height:parent.height-16; radius:preset.modelData.radius; color:preset.modelData.rail
              Repeater { model:3; Rectangle { required property int index; x:8; y:12+index*14; width:parent.width*(index===1 ? 0.6 : 0.78); height:4; radius:2; color:preset.modelData.ink; opacity:index===0 ? 0.8 : 0.3 } }
              Rectangle { x:8; y:parent.height-15; width:parent.width-16; height:8; radius:4; color:preset.modelData.accent; opacity:0.4 }
            }
          }
          Row {
            width: parent.width; spacing: root.theme.space(6)
            Text { width: parent.width-(selected.visible ? selected.width+parent.spacing : 0); text:preset.modelData.label; elide:Text.ElideRight; color:root.theme.foreground; font.family:root.theme.font.family; font.pixelSize:root.theme.font.body; font.weight:Font.DemiBold }
            WispIcon { id:selected; visible:preset.checked; theme:root.theme; name:"check"; ink:root.theme.accent }
          }
          Text { width:parent.width; text:preset.modelData.detail; elide:Text.ElideRight; color:root.theme.muted; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption }
        }
      }
    }
  }
  Row {
    width:parent.width; spacing:root.theme.space(12)
    Text { width:root.theme.space(94); anchors.verticalCenter:parent.verticalCenter; text:"More styles"; color:root.theme.foreground; font.family:root.theme.font.family; font.pixelSize:root.theme.font.body }
    WispComboBox {
    theme: root.theme
      id: otherStyles; objectName:"otherAppearanceStyles"; width:Math.max(1,parent.width-root.theme.space(106)); enabled:root.editable
      model:[{label:"Terminal and classic styles…",profile:""},{label:"Clean TUI",profile:"clean_tui"},{label:"Terminal Grid",profile:"terminal"},{label:"TUI",profile:"performative"},{label:"Herdr",profile:"herdr"},{label:"Classic · default",profile:"legacy"}]
      textRole:"label"; currentIndex:{for(var i=1;i<model.length;i++) if(root.appearance && model[i].profile===root.appearance.profile)return i;return 0}
      onActivated:if(model[currentIndex].profile)root.appearance.setProfile(model[currentIndex].profile)
      ThemeControlStyle {theme:root.theme;control:otherStyles}
    }
  }
  Column {
    width: parent.width; spacing: root.theme.space(6)
    Text { text:"Chat layout"; color:root.theme.foreground; font.family:root.theme.font.family; font.pixelSize:root.theme.font.body }
    WispComboBox {
      id: chatLayoutPicker; objectName:"chatLayoutSetting"
      theme:root.theme; width:parent.width; enabled:!!root.appearance; textRole:"label"
      model:[{key:"grouped",label:"Grouped · default",detail:"One avatar and header per sender, with a continuous chat background."},
        {key:"compact",label:"Compact",detail:"Names beside messages, quiet timestamps, and no chat avatars."},
        {key:"soft_groups",label:"Soft groups",detail:"Subtle cards collect consecutive messages from the same person."}]
      currentIndex: {for(var i=0;i<model.length;i++)if(model[i].key===root.theme.chatLayout)return i;return 0}
      onActivated:root.appearance.setChatLayout(model[currentIndex].key)
      Accessible.name:"Chat layout"
      ThemeControlStyle {theme:root.theme;control:chatLayoutPicker}
    }
    Text {
      width:parent.width; wrapMode:Text.WordWrap
      text:chatLayoutPicker.model[chatLayoutPicker.currentIndex].detail+" Applies to the app and tray."
      color:root.theme.muted; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption
    }
  }
  CheckBox {
    id: avatarSetting; objectName: "showAvatarsSetting"; width: parent.width
    text: "Show user avatars"; checked: root.theme.showAvatars; enabled: !!root.appearance
    onToggled: root.appearance.setShowAvatars(checked)
    ThemeControlStyle { theme: root.theme; control: avatarSetting }
  }
  SettingsSection {
    objectName:"appearanceCustomization"; theme:root.theme; title:"Customize colors"; summary:"Palette, names, borders, and section accents"; sectionIcon:"palette"
    Text {objectName:"settingsPalette";text:"Color palette";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
    WispComboBox {
    theme: root.theme
      id: palettePicker; objectName:"appearancePalettePicker"; width:parent.width; enabled:root.editable; textRole:"label"
      model:[{key:"soft_graphite",label:"Soft Graphite"},{key:"daylight",label:"Daylight"},{key:"hearth",label:"Hearth"},{key:"wisp",label:"Wisp blue"},{key:"graphite",label:"Graphite"},{key:"violet",label:"Violet"},{key:"ember",label:"Ember"},{key:"ash_olive",label:"Ash & Olive"},{key:"herdr",label:"Solarized Japan"},{key:"astra",label:"Astra"}]
      currentIndex:{for(var i=0;i<model.length;i++)if(root.appearance && model[i].key===root.appearance.palette)return i;return 0}
      onActivated:root.appearance.setPalette(model[currentIndex].key)
      ThemeControlStyle {theme:root.theme;control:palettePicker}
    }
    Text {objectName:"settingsAccents";text:"Use accent colors for";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
    Repeater {
      model:[{key:"chatBorders",label:"Chat borders"},{key:"chatHeadings",label:"Chat headings"},{key:"roomSections",label:"Room sections"},{key:"friendSections",label:"Friends section"},{key:"friendNames",label:"Online friends' names"},{key:"senderNames",label:"Message senders' names"}]
      CheckBox {
        id: accentOption; required property var modelData
        objectName:"color-option-"+modelData.key; width:parent.width; enabled:root.editable
        text:modelData.label; checked:root.theme.colorEnabled(modelData.key)
        onToggled:root.appearance.setColorOption(modelData.key,checked)
        ThemeControlStyle {theme:root.theme;control:accentOption}
      }
    }
  }
  Text {
    width:parent.width; wrapMode:Text.WordWrap; visible:!!root.appearance && root.appearance.error!=="";text:root.appearance ? root.appearance.error : ""
    color:root.theme.danger;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption
  }
}
