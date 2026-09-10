import QtQuick
import QtQuick.Controls
import "../components"

Column {
  id: root

  required property var bridge
  required property var theme
  readonly property var audio: bridge.audioState
  readonly property var inputDevices: audio.input_devices || []
  readonly property var outputDevices: audio.output_devices || []
  readonly property int inputLevel: Math.max(0, Math.min(100, Number(audio.input_level || 0)))
  readonly property string pttShortcut: String(bridge.pushToTalkState.shortcut || "")
  readonly property bool shortcutSupported: !!bridge.pushToTalkState.shortcut_backend
  readonly property var replacedShortcuts: bridge.pushToTalkState.shortcut_replaced || []
  property bool capturingShortcut: false
  readonly property var testState: bridge.audioTestState || ({phase:"idle"})
  readonly property bool inRoom: !!bridge.mediaState.livekit_connected || !!bridge.currentVoiceRoom
  property bool ownsTest: false
  function discardTest() {
    if (ownsTest) { ownsTest = false; bridge.audioTest("clear") }
  }
  onVisibleChanged: if (!visible) discardTest()
  Component.onDestruction: discardTest()
  Timer {
    interval: 250
    running: root.visible && root.ownsTest
    repeat: true
    onTriggered: root.bridge.audioTest("status")
  }

  width: parent ? parent.width : 0
  spacing: root.theme.space(12)
  focus: root.capturingShortcut

  function shortcutKeyName(event) {
    if (event.key >= Qt.Key_A && event.key <= Qt.Key_Z)
      return String.fromCharCode(event.key)
    if (event.key >= Qt.Key_0 && event.key <= Qt.Key_9)
      return String.fromCharCode(event.key)
    if (event.key >= Qt.Key_F1 && event.key <= Qt.Key_F35)
      return "F" + String(event.key - Qt.Key_F1 + 1)
    var names = ({})
    names[Qt.Key_Space] = "SPACE"
    names[Qt.Key_Tab] = "TAB"
    names[Qt.Key_Return] = "RETURN"
    names[Qt.Key_Enter] = "ENTER"
    names[Qt.Key_Backspace] = "BACKSPACE"
    names[Qt.Key_Delete] = "DELETE"
    names[Qt.Key_Home] = "HOME"
    names[Qt.Key_End] = "END"
    names[Qt.Key_PageUp] = "PAGEUP"
    names[Qt.Key_PageDown] = "PAGEDOWN"
    names[Qt.Key_Up] = "UP"
    names[Qt.Key_Down] = "DOWN"
    names[Qt.Key_Left] = "LEFT"
    names[Qt.Key_Right] = "RIGHT"
    names[Qt.Key_Insert] = "INSERT"
    names[Qt.Key_Pause] = "PAUSE"
    return names[event.key] || ""
  }

  function shortcutFromEvent(event) {
    var key = shortcutKeyName(event)
    if (!key) return ""
    var parts = []
    if (event.modifiers & Qt.MetaModifier) parts.push("SUPER")
    if (event.modifiers & Qt.ControlModifier) parts.push("CTRL")
    if (event.modifiers & Qt.AltModifier) parts.push("ALT")
    if (event.modifiers & Qt.ShiftModifier) parts.push("SHIFT")
    parts.push(key)
    return parts.join(" + ")
  }

  Keys.onPressed: function(event) {
    if (!root.capturingShortcut || event.isAutoRepeat) return
    if (event.key === Qt.Key_Escape) {
      root.capturingShortcut = false
      event.accepted = true
      return
    }
    var shortcut = root.shortcutFromEvent(event)
    if (!shortcut) return
    root.bridge.setPushToTalkShortcut(shortcut)
    root.capturingShortcut = false
    event.accepted = true
  }

  Item {
    width: parent.width
    height: Math.max(audioHeading.implicitHeight, refreshButton.height)

    Text {
      id: audioHeading
      anchors.left: parent.left
      anchors.verticalCenter: parent.verticalCenter
      text: "Audio"
      color: root.theme.muted
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
      font.weight: Font.Bold
    }

    ChatButton {
      id: refreshButton
      theme: root.theme; text: "Refresh"; iconName: "refresh"
      anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
      onClicked: root.bridge.refreshAudioDevices()
    }
  }

  Text {
    objectName: "settingsMicrophone"; text: "Microphone"
    color: root.theme.foreground
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.weight: Font.DemiBold
  }

  WispComboBox {
    theme: root.theme
    id: inputDevicePicker; objectName: "inputDevicePicker"
    width: parent.width; model: root.inputDevices; textRole: "name"; enabled: count > 0
    Accessible.name: "Input device"
    currentIndex: {for(var i=0;i<count;i++) if(String(model[i].id)===String(root.audio.selected_input_id || ""))return i;return -1}
    onActivated: root.bridge.setInputDevice(String(model[currentIndex].id))
    ThemeControlStyle {theme:root.theme;control:inputDevicePicker}
  }

  Text {
    visible: root.inputDevices.length === 0
    text: "No microphone detected"
    color: root.theme.danger
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
  }

  Row {
    width: parent.width
    spacing: root.theme.spacing.md

    Rectangle {
      width: Math.max(1, parent.width - levelText.width - parent.spacing)
      height: root.theme.space(7)
      anchors.verticalCenter: parent.verticalCenter
      radius: height / 2
      color: root.theme.alpha(root.theme.foreground, 0.08)

      Rectangle {
        width: parent.width * root.inputLevel / 100
        height: parent.height
        radius: parent.radius
        color: root.inputLevel > 88 ? root.theme.warning : root.theme.accent

        Behavior on width { NumberAnimation { duration: 80 } }
      }
    }

    Text {
      id: levelText
      width: root.theme.space(33)
      text: root.inputLevel + "%"
      horizontalAlignment: Text.AlignRight
      color: root.theme.muted
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
    }
  }

  Text {
    topPadding: root.theme.spacing.sm
    objectName: "settingsSpeaker"; text: "Speaker"
    color: root.theme.foreground
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.weight: Font.DemiBold
  }

  WispComboBox {
    theme: root.theme
    id: outputDevicePicker; objectName: "outputDevicePicker"
    width: parent.width; model: root.outputDevices; textRole: "name"; enabled: count > 0
    Accessible.name: "Output device"
    currentIndex: {for(var i=0;i<count;i++) if(String(model[i].id)===String(root.audio.selected_output_id || ""))return i;return -1}
    onActivated: root.bridge.setOutputDevice(String(model[currentIndex].id))
    ThemeControlStyle {theme:root.theme;control:outputDevicePicker}
  }

  Text {
    visible: root.outputDevices.length === 0
    text: "No speaker detected"
    color: root.theme.danger
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
  }

  Text {
    topPadding: root.theme.spacing.sm
    objectName: "settingsProcessing"; text: "Voice cleanup"
    color: root.theme.foreground
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.weight: Font.DemiBold
  }

  Flow {
    width: parent.width; spacing: root.theme.spacing.sm
    Repeater {
      model: [{key:"clear",label:"Clear voice"},{key:"natural",label:"Light cleanup"},{key:"studio",label:"Unprocessed"}]
      ChatButton {
        required property var modelData; theme:root.theme; text:modelData.label; iconName:""
        primary:String(root.audio.preset || "clear")===modelData.key
        onClicked:root.bridge.setAudioPreset(modelData.key)
      }
    }
  }

  Text {
    width: parent.width
    text: String(root.audio.preset || "clear") === "studio"
      ? "Original microphone sound. Best with headphones in a quiet room."
      : String(root.audio.preset || "clear") === "natural"
        ? "Light noise reduction and echo cancellation for quiet spaces."
        : "Recommended for speech. Reduces background noise, room rumble, and speaker echo while keeping quiet words."
    wrapMode: Text.WordWrap
    color: root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
  }

  SettingsSection {
    theme: root.theme; title: "Microphone test"; summary: "Record and compare your voice privately"
    objectName: "audioTestSection"; expanded: false; sectionIcon: "microphone"
    onExpandedChanged: if (!expanded) root.discardTest()
    Rectangle {
      width: parent.width
      height: testContent.implicitHeight + root.theme.spacing.lg * 2
      radius: root.theme.cornerRadius
      color: root.theme.alpha(root.theme.foreground, 0.045)

      Column {
        id: testContent
        x: root.theme.spacing.lg
        y: root.theme.spacing.lg
        width: parent.width - root.theme.spacing.lg * 2
        spacing: root.theme.spacing.sm

        Text {
          text: "Hear yourself"
          color: root.theme.foreground
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
          font.weight: Font.DemiBold
        }
        Text {
          width: parent.width
          text: root.inRoom ? "Leave your voice room to test privately."
            : "Record up to 8 seconds, then compare your original and cleaned-up voice. Uses your selected microphone and speaker."
          wrapMode: Text.WordWrap
          color: root.theme.muted
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }
        Text {
          width: parent.width
          text: root.testState.phase === "recording"
            ? "Recording · " + (Number(root.testState.duration_ms || 0) / 1000).toFixed(1) + " / 8 seconds"
            : root.testState.phase === "playing"
              ? (root.testState.playback === "original" ? "Playing original microphone" : "Playing processed voice")
              : Number(root.testState.duration_ms || 0) > 0 ? "Sample ready · " + (Number(root.testState.duration_ms) / 1000).toFixed(1) + " seconds" : "Your sample stays in memory and is discarded when you close Audio settings."
          wrapMode: Text.WordWrap
          color: root.testState.phase === "recording" ? root.theme.accent : root.theme.muted
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }
        Rectangle {
          visible: root.testState.phase === "recording"
          width: parent.width
          height: root.theme.space(6)
          radius: height / 2
          color: root.theme.alpha(root.theme.foreground, 0.08)
          Rectangle {
            width: parent.width * Math.min(100, Number(root.testState.input_level || 0)) / 100
            height: parent.height
            radius: parent.radius
            color: root.theme.accent
            Behavior on width { NumberAnimation { duration: 80 } }
          }
        }
        Flow {
          width: parent.width
          spacing: root.theme.spacing.sm
          Repeater {
            model: [
              {label: root.testState.phase === "recording" ? "Finish recording" : root.testState.phase === "playing" ? "Stop playback" : "Record sample", action: root.testState.phase === "recording" || root.testState.phase === "playing" ? "stop" : "record"},
              {label:"Play processed", action:"play_processed"},
              {label:"Play original", action:"play_original"},
              {label:"Discard", action:"clear"}
            ]
            delegate: ChatButton {
              required property var modelData
              theme: root.theme; text: modelData.label
              iconName: modelData.action === "record" ? "microphone" : modelData.action === "stop" ? "stop" : modelData.action === "clear" ? "trash" : "play"
              enabled: !root.inRoom && !root.bridge.audioTestBusy && (modelData.action === "record" || modelData.action === "stop" || (root.testState.phase === "ready" && Number(root.testState.duration_ms || 0) > 0))
              onClicked: {
                root.ownsTest = true
                root.bridge.audioTest(modelData.action)
              }
            }
          }
        }
        Text {
          visible: text.length > 0
          width: parent.width
          text: String(root.bridge.audioTestError || root.testState.error || "")
          wrapMode: Text.WordWrap
          color: root.theme.danger
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }
      }
    }
  }
  SettingsSection {
    theme: root.theme; title: "Push to talk"; summary: "Hold a key to speak"; objectName:"pushToTalkSection"; sectionIcon:"keyboard"
    CheckBox {
      id: pttEnabled; objectName:"settingsPushToTalk"; text:"Enable push to talk"; checked:root.bridge.pushToTalkState.enabled
      onToggled:root.bridge.setPushToTalk(checked)
      ThemeControlStyle {theme:root.theme;control:pttEnabled}
    }
    Text {objectName:"settingsShortcut";text:"Global shortcut";color:root.theme.foreground;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body}
    Text {
      width:parent.width;wrapMode:Text.Wrap
      text:root.capturingShortcut ? "Press a key combination. Esc cancels." : root.shortcutSupported ? "Works globally through Omarchy/Hyprland." : "Global shortcuts require Omarchy/Hyprland."
      color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption
    }
    Flow {
      width:parent.width;spacing:root.theme.spacing.sm
      ChatButton {
        theme:root.theme;iconName:"keyboard";text:root.capturingShortcut ? "Press keys…" : root.pttShortcut || "Set shortcut";enabled:root.shortcutSupported
        onClicked:{root.capturingShortcut=true;root.forceActiveFocus()}
      }
      ChatButton {theme:root.theme;text:"Clear";visible:root.pttShortcut.length>0 && !root.capturingShortcut;onClicked:{root.capturingShortcut=false;root.bridge.setPushToTalkShortcut(null)}}
    }
    Text {visible:root.replacedShortcuts.length>0;width:parent.width;wrapMode:Text.Wrap;text:"Replaced on "+root.pttShortcut+": "+root.replacedShortcuts.join(" · ");color:root.theme.warning;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
  }
  SettingsSection {theme:root.theme;title:"Connection recovery";summary:"Retry limits and automatic reconnection";objectName:"voiceRecoverySection"; sectionIcon:"refresh"
  CheckBox {
    id: voiceReconnect; objectName: "voiceReconnectSetting"
    width: parent.width; text: "Automatically reconnect voice"
    checked: root.bridge.voiceRecovery.enabledSetting
    onToggled: root.bridge.voiceRecovery.enabledSetting = checked
    ThemeControlStyle { theme: root.theme; control: voiceReconnect }
    contentItem: Text {
      text: voiceReconnect.text; wrapMode: Text.Wrap
      leftPadding: voiceReconnect.indicator.width + voiceReconnect.spacing
      color: root.theme.foreground; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
  }
  Text {
    width: parent.width; wrapMode: Text.Wrap
    text: "Up to 6 attempts within 2 minutes. Disconnecting, joining another room, or exiting Wisp cancels retries. Camera and sharing stay off."
    color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
  }

  }
  SettingsSection {theme:root.theme;title:"Processing status";summary:"Voice cleanup quality and performance";objectName:"audioDiagnosticsSection"
  Rectangle {
    visible: !!root.audio.denoiser_active
    width: parent.width
    height: root.theme.space(54)
    radius: root.theme.cornerRadius
    color: root.theme.alpha(root.theme.accent, 0.12)

    Column {
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.leftMargin: root.theme.spacing.lg
      anchors.rightMargin: root.theme.spacing.lg
      anchors.verticalCenter: parent.verticalCenter
      spacing: root.theme.spacing.xs

      Text {
        width: parent.width
        text: String(root.audio.denoiser || "deepfilternet") === "webrtc"
          ? "Voice cleanup · lightweight mode"
          : "Voice cleanup · full quality"
        elide: Text.ElideRight
        color: root.theme.accent
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
        font.weight: Font.DemiBold
      }

      Text {
        width: parent.width
        readonly property real processingMs: Number(root.audio.processing_time_us || 0) / 1000
        readonly property real queueMs: Number(root.audio.capture_queue_ms || 0)
        readonly property bool delayed: processingMs > 10 || queueMs > 20
        text: root.bridge.mediaState.livekit_connected
          ? (delayed ? "Audio is catching up" : "Ready for clear conversation")
          : "Applies to calls and microphone tests"
        elide: Text.ElideRight
        color: delayed ? root.theme.warning : root.theme.muted
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }
    }
  }

  }
}
