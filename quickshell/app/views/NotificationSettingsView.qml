import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import "../components"

Column {
  id: root
  required property var bridge
  required property var theme
  property string selectedSoundEvent: ""
  spacing: theme.spacing.lg


  Text {
    Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
    Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.body; restoreMode: Binding.RestoreBindingOrValue }
     objectName: "settingsNotifications"; text: "Notifications · this device"; color: root.theme.foreground; font.bold: true }
  Text {
    Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
    width: parent.width
    text: "Muted chats keep unread badges without playing a sound."
    wrapMode: Text.Wrap
    color: root.theme.muted
    font.pixelSize: root.theme.font.caption
  }
  ChatButton {
    theme: root.theme
    text: root.bridge.notificationMuted ? "Sound muted · Enable" : "Sound enabled · Mute"
    onClicked: root.bridge.notificationMuted = !root.bridge.notificationMuted
  }
  Text { text: "Play sounds for"; color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption }
  Repeater {
    model: [{id:"other_chats",label:"Other chats, even while Wisp is focused"},{id:"unfocused",label:"Messages only while Wisp is unfocused"},{id:"always",label:"All incoming messages"}]
    RadioButton {
      id: policyChoice
      required property var modelData
      width: parent.width
      text: modelData.label
      checked: root.bridge.notificationPolicy === modelData.id
      onClicked: root.bridge.notificationPolicy = modelData.id
      ThemeControlStyle { theme: root.theme; control: policyChoice }
      contentItem: Text {
        text: policyChoice.text; wrapMode: Text.Wrap
        leftPadding: policyChoice.indicator.width + policyChoice.spacing
        color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
      }
    }
  }
  Row {
    width: parent.width
    spacing: root.theme.spacing.lg
    Text {
      Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
      Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.body; restoreMode: Binding.RestoreBindingOrValue }
       objectName: "settingsNotificationVolume"; text: "Volume"; color: root.theme.foreground; anchors.verticalCenter: parent.verticalCenter }
    Slider {
      id: trialControl0
      ThemeControlStyle { theme: root.theme; control: trialControl0 }
      width: Math.min(root.width - 120, root.theme.space(300))
      from: 0; to: 100; stepSize: 1
      value: root.bridge.notificationVolume
      onMoved: root.bridge.notificationVolume = Math.round(value)
    }
    Text {
      Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
      Binding on font.pixelSize { when: root.theme.terminal; value: root.theme.font.body; restoreMode: Binding.RestoreBindingOrValue }
       text: root.bridge.notificationVolume + "%"; color: root.theme.muted; anchors.verticalCenter: parent.verticalCenter }
  }
  Text {
    Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
    width: parent.width
    text: root.bridge.notificationSoundPath || "Default: Wisp chime"
    elide: Text.ElideMiddle
    color: root.theme.muted
    font.pixelSize: root.theme.font.caption
  }
  Flow {
    width: parent.width
    spacing: root.theme.spacing.lg
    ChatButton { theme: root.theme; text: "Choose sound…"; onClicked: { root.selectedSoundEvent=""; soundPicker.title="Choose a message notification sound"; soundPicker.open() } }
    ChatButton { theme: root.theme; text: "Use default"; onClicked: root.bridge.notificationSoundPath = "" }
    ChatButton {
      theme: root.theme; text: "Test sound"
      enabled: !root.bridge.notificationMuted && root.bridge.notificationVolume > 0
      onClicked: root.bridge.playNotificationSound()
    }
  }
  Text {
    Binding on font.family { when: root.theme.terminal; value: root.theme.font.family; restoreMode: Binding.RestoreBindingOrValue }
    width: parent.width
    visible: text !== ""
    text: root.bridge.notificationError
    wrapMode: Text.Wrap
    color: root.theme.danger
    font.pixelSize: root.theme.font.caption
  }
  FileDialog {
    id: soundPicker
    title: "Choose a notification sound"
    nameFilters: ["Audio files (*.wav *.ogg *.flac *.mp3)", "All files (*)"]
    onAccepted: {
      if (root.selectedSoundEvent) root.bridge.setEventSound(root.selectedSoundEvent,String(selectedFile))
      else root.bridge.notificationSoundPath = String(selectedFile)
      root.selectedSoundEvent = ""
    }
    onRejected: root.selectedSoundEvent = ""
  }
  CheckBox {
    id: audioCues; objectName: "audioControlSoundsSetting"; width: parent.width
    text: "Mute and deafen sounds"; checked: root.bridge.audioControlSounds
    onToggled: root.bridge.audioControlSounds = checked
    ThemeControlStyle { theme: root.theme; control: audioCues }
  }
  SettingsSection {
    theme: root.theme; title: "Voice sounds"; summary: "Joins, leaves, screen shares, and audio controls"
    objectName: "roomSoundsSection"; expanded: false
    Text { objectName: "settingsRoomSounds"; text: "Room sounds"; color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true }
    Text {
      width: parent.width; wrapMode: Text.Wrap
      text: "Choose sounds for invitations, voice activity, and screen sharing."
      color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    Flow {
      width: parent.width; spacing: root.theme.spacing.sm
      ChatButton {
        theme: root.theme; text: root.bridge.roomNotificationSounds ? "Others · Sound on" : "Others · Muted"
        onClicked: root.bridge.roomNotificationSounds = !root.bridge.roomNotificationSounds
      }
      ChatButton {
        theme: root.theme; text: root.bridge.selfRoomNotificationSounds ? "My joins/leaves · Sound on" : "My joins/leaves · Muted"
        onClicked: root.bridge.selfRoomNotificationSounds = !root.bridge.selfRoomNotificationSounds
      }
    }
    CheckBox {
      id: shareCues; objectName:"screenShareSoundsSetting"; width:parent.width
      text:"Screen-share start and stop sounds"; checked:root.bridge.screenShareSounds
      onToggled:root.bridge.screenShareSounds=checked
      ThemeControlStyle {theme:root.theme;control:shareCues}
    }
    Repeater {
      model: [{id:"room_invite",label:"Voice room invitation"},{id:"member_join",label:"Someone joins your room"},{id:"member_leave",label:"Someone leaves your room"},{id:"self_join",label:"You join a room"},{id:"self_leave",label:"You leave a room"},{id:"screen_share_start",label:"Screen share starts"},{id:"screen_share_stop",label:"Screen share ends"},{id:"audio_mute",label:"Microphone muted"},{id:"audio_unmute",label:"Microphone unmuted"},{id:"audio_deafen",label:"Deafened"},{id:"audio_undeafen",label:"Undeafened"}]
      Column {
        id: eventSoundRow
        required property var modelData
        width: parent.width; spacing: root.theme.spacing.sm
        Text { text: eventSoundRow.modelData.label; color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption; font.bold: true }
        Text {
          width: parent.width; elide: Text.ElideMiddle
          text: root.bridge.eventSoundPaths[eventSoundRow.modelData.id] || "Default Wisp sound"
          color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
        }
        Flow {
          width: parent.width; spacing: root.theme.spacing.sm
          ChatButton { theme: root.theme; text: "Choose sound…"; onClicked: { root.selectedSoundEvent=eventSoundRow.modelData.id; soundPicker.title=eventSoundRow.modelData.label; soundPicker.open() } }
          ChatButton {
            theme: root.theme; text: "Test"
            enabled: root.bridge.notificationSoundCommand(eventSoundRow.modelData.id).length>0
            onClicked: root.bridge.playNotificationSound(eventSoundRow.modelData.id)
          }
          ChatButton { theme: root.theme; text: "Restore default"; onClicked: root.bridge.setEventSound(eventSoundRow.modelData.id, "") }
        }
      }
    }
  }
  SettingsSection {
    theme: root.theme; title: "Per-chat notifications"; summary: "Mute sounds for individual conversations"
    objectName: "chatSoundsSection"; expanded: false
    Text { objectName: "settingsChatNotifications"; text: "Chat notifications"; color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true }
    Repeater {
      model: root.bridge.conversations
      ChatButton {
        required property var modelData
        width: parent.width; theme: root.theme
        text: (root.bridge.chatNotificationsMuted(modelData.id) ? "Muted · " : "Sound on · ") + modelData.label
        onClicked: root.bridge.toggleChatNotifications(modelData.id)
      }
    }
  }

  SettingsSection {
    theme: root.theme; title: "Chat navigation"; summary: "Choose how rooms open in your workspace"
    objectName: "chatNavigationSection"; expanded: false
    Text {
      text: "Channel navigation · this device"; color: root.theme.foreground
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.body; font.bold: true
    }
    CheckBox {
      id: channelTilePreference
      objectName: "channelsAsTilesSetting"
      width: parent.width
      text: "Open channels and rooms in new tiles"
      checked: root.bridge.workspaceLayout.channelsAsTiles
      onClicked: root.bridge.workspaceLayout.setChannelsAsTiles(checked)
      ThemeControlStyle { theme: root.theme; control: channelTilePreference }
      contentItem: Text {
        text: channelTilePreference.text; wrapMode: Text.Wrap
        leftPadding: channelTilePreference.indicator.width + channelTilePreference.spacing
        color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
      }
    }
    Text {
      width: parent.width; wrapMode: Text.Wrap
      text: "When off, reuse a room or channel tile. Direct messages stay in place."
      color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    Text {
      width: parent.width; wrapMode: Text.Wrap
      visible: !!root.bridge.workspaceLayout.error
      text: root.bridge.workspaceLayout.error
      color: root.theme.danger; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
  }

}
