import QtQuick
import QtQuick.Controls
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  width: parent ? parent.width : 0
  spacing: root.theme.space(12)
  readonly property var camera: root.bridge.cameraState
  readonly property var video: root.bridge.videoSettings
  readonly property bool publishing: root.bridge.sharing || root.bridge.cameraActive
  Row {
    width:parent.width;spacing:root.theme.spacing.md
    Text {width:parent.width-refresh.width-parent.spacing;text:"Video";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.title;font.weight:Font.DemiBold}
    ChatButton {id:refresh;theme:root.theme;text:"Refresh";onClicked:root.bridge.refreshVideoDevices()}
  }
  Text {objectName:"settingsCamera";text:"Camera";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
  WispComboBox {
    theme: root.theme
    id:cameraPicker;objectName:"cameraDevicePicker";width:parent.width;model:root.camera.devices || [];textRole:"name";enabled:count>0 && !root.camera.active
    Accessible.name:"Camera device"
    currentIndex:{for(var i=0;i<count;i++)if(String(model[i].id)===String(root.camera.selected_device_id || ""))return i;return -1}
    onActivated:root.bridge.setCameraDevice(String(model[currentIndex].id))
    ThemeControlStyle {theme:root.theme;control:cameraPicker}
  }
  Text {visible:cameraPicker.count===0;text:"No camera detected";color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
  Text {objectName:"settingsVideoQuality";text:"Stream quality";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
  WispComboBox {
    theme: root.theme
    id:qualityPicker;objectName:"videoQualityPicker";width:parent.width;enabled:!root.publishing;textRole:"label"
    model:[{value:"balanced",label:"Balanced · 720p, 30 fps"},{value:"high",label:"High · 1080p, 60 fps"},{value:"ultra",label:"Ultra · 1440p, 60 fps"}]
    currentIndex:{for(var i=0;i<count;i++)if(model[i].value===root.video.quality)return i;return 0}
    onActivated:root.bridge.setVideoQuality(model[currentIndex].value)
    ThemeControlStyle {theme:root.theme;control:qualityPicker}
  }
  CheckBox {
    id:tilePreference;objectName:"streamsAsTilesSetting";width:parent.width;text:"Open streams as tiles";checked:root.bridge.workspaceLayout.streamsAsTiles
    onToggled:root.bridge.workspaceLayout.setStreamsAsTiles(checked)
    ThemeControlStyle {theme:root.theme;control:tilePreference}
    contentItem:Text {text:tilePreference.text;wrapMode:Text.Wrap;leftPadding:tilePreference.indicator.width+tilePreference.spacing;color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
  }
  Text {width:parent.width;wrapMode:Text.Wrap;text:"Otherwise, open a separate window. You can move streams between windows and tiles anytime.";color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
  Text {visible:root.publishing;width:parent.width;wrapMode:Text.Wrap;text:"Stop sharing and camera video to change quality or codec.";color:root.theme.warning;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
  SettingsSection {
    theme:root.theme;title:"Advanced video";summary:"Codec and hardware encoding";objectName:"advancedVideoSection"
    Text {objectName:"settingsVideoCodec";text:"Video codec";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
    WispComboBox {
    theme: root.theme
      id:codecPicker;objectName:"videoCodecPicker";width:parent.width;enabled:!root.publishing;model:root.video.available_codecs || ["h264","vp8","av1"]
      currentIndex:model.indexOf(root.video.codec);displayText:currentText.toUpperCase()
      onActivated:root.bridge.setVideoCodec(model[currentIndex])
      ThemeControlStyle {theme:root.theme;control:codecPicker}
    }
    Text {width:parent.width;wrapMode:Text.Wrap;text:root.video.hardware_acceleration ? "Hardware encoding: "+String(root.video.encoder_backend || "available") : "Software encoding";color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
  }
}
