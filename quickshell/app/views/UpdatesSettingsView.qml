import QtQuick
import QtQuick.Controls
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  readonly property var updates: bridge.updates
  spacing: theme.spacing.lg
  Text { text:"Updates"; color:root.theme.foreground; font.family:root.theme.font.family; font.pixelSize:root.theme.font.title; font.weight:Font.DemiBold }
  Text {
    width:parent.width; wrapMode:Text.Wrap; text:root.updates.statusText
    color:root.updates.info.error ? root.theme.danger : root.theme.muted
    font.family:root.theme.font.family; font.pixelSize:root.theme.font.body
  }
  Flow {
    width:parent.width; spacing:root.theme.spacing.sm
    ChatButton { objectName:"checkForUpdates"; theme:root.theme; text:"Check for updates"; iconName:"refresh"; enabled:!root.updates.checking && !root.updates.busy; onClicked:root.updates.checkNow() }
    ChatButton { objectName:"installUpdate"; theme:root.theme; text:"Update now"; iconName:"download"; primary:true; visible:root.updates.available; enabled:root.updates.safe && !root.updates.busy; onClicked:root.updates.install(false) }
  }
  Text {
    width:parent.width; wrapMode:Text.Wrap; visible:root.updates.available && !root.updates.safe
    text:"Disconnect voice and finish drafts or transfers to install."
    color:root.theme.muted; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption
  }
  Repeater {
    model:[{key:"automatic",label:"Install updates automatically",hint:"Installs after Wisp is idle for a minute. Voice and unfinished messages are protected."},
      {key:"check_on_launch",label:"Check when Wisp starts",hint:"Look for a new release at launch."},
      {key:"background_checks",label:"Check while Wisp is running",hint:"Show an update notice when a release becomes available."}]
    Column {
      required property var modelData
      width:parent.width; spacing:root.theme.spacing.xs
      CheckBox {
        id: setting; objectName:"update-" + modelData.key; width:parent.width
        checked:!!root.updates.preferences[modelData.key]; text:modelData.label
        onClicked:root.updates.setPreference(modelData.key,checked)
        ThemeControlStyle { theme:root.theme; control:setting }
        contentItem:Text { text:setting.text; wrapMode:Text.Wrap; leftPadding:setting.indicator.width+setting.spacing; color:root.theme.foreground; font.family:root.theme.font.family; font.pixelSize:root.theme.font.body }
      }
      Text { width:parent.width; text:modelData.hint; wrapMode:Text.Wrap; color:root.theme.muted; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption }
    }
  }
  Column {
    width:parent.width; spacing:root.theme.spacing.sm
    enabled:root.updates.preferences.background_checks
    Text { text:"Check interval (minutes)"; color:root.theme.foreground; font.family:root.theme.font.family; font.pixelSize:root.theme.font.body }
    SpinBox {
      id: interval; objectName:"updateInterval"; from:1; to:1440; editable:true
      value:root.updates.preferences.interval_minutes
      Accessible.name:"Update check interval in minutes"
      onValueModified:root.updates.setPreference("interval_minutes",value)
      ThemeControlStyle { theme:root.theme; control:interval }
    }
    Text { text:"Default: 5 minutes. Manual checks are always available."; width:parent.width; wrapMode:Text.Wrap; color:root.theme.muted; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption }
  }
}
