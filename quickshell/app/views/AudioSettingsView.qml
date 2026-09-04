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
  readonly property int deepfilterStrength: Math.max(0, Math.min(100,
    Number(audio.deepfilter_strength === undefined ? 100 : audio.deepfilter_strength)))
  readonly property string pttShortcut: String(bridge.pushToTalkState.shortcut || "")
  readonly property bool shortcutSupported: !!bridge.pushToTalkState.shortcut_backend
  readonly property var replacedShortcuts: bridge.pushToTalkState.shortcut_replaced || []
  property bool capturingShortcut: false
  property int pendingDeepfilterStrength: 100

  width: parent ? parent.width : 0
  spacing: root.theme.spacing.sm
  focus: root.capturingShortcut

  function commitDeepfilterStrength() {
    strengthUpdate.stop()
    if (root.pendingDeepfilterStrength !== root.deepfilterStrength)
      root.bridge.setDeepfilterStrength(root.pendingDeepfilterStrength)
  }

  Timer {
    id: strengthUpdate
    interval: 90
    onTriggered: root.commitDeepfilterStrength()
  }

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
      text: "AUDIO"
      color: root.theme.muted
      font.family: root.theme.font.family
      font.pixelSize: root.theme.font.caption
      font.weight: Font.Bold
    }

    Rectangle {
      id: refreshButton
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      width: refreshText.implicitWidth + root.theme.spacing.lg * 2
      height: root.theme.space(25)
      radius: root.theme.cornerRadius
      color: root.theme.alpha(root.theme.foreground, refreshMouse.containsMouse ? 0.12 : 0.055)

      Text {
        id: refreshText
        anchors.centerIn: parent
        text: "Refresh"
        color: root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }

      MouseArea {
        id: refreshMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.bridge.refreshAudioDevices()
      }
    }
  }

  Text {
    text: "Microphone"
    color: root.theme.foreground
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.weight: Font.DemiBold
  }

  Repeater {
    model: root.inputDevices
    delegate: Rectangle {
      required property var modelData
      readonly property bool selected: String(root.audio.selected_input_id || "") === String(modelData.id)
      width: root.width
      height: root.theme.space(32)
      radius: root.theme.cornerRadius
      color: selected
        ? root.theme.alpha(root.theme.accent, 0.34)
        : root.theme.alpha(root.theme.foreground, inputMouse.containsMouse ? 0.12 : 0.055)

      Text {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: root.theme.spacing.lg
        anchors.rightMargin: root.theme.spacing.lg
        anchors.verticalCenter: parent.verticalCenter
        text: String(modelData.name || modelData.id)
        elide: Text.ElideMiddle
        color: root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }

      MouseArea {
        id: inputMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: if (!parent.selected) root.bridge.setInputDevice(String(modelData.id))
      }
    }
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
    text: "Speaker"
    color: root.theme.foreground
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.weight: Font.DemiBold
  }

  Repeater {
    model: root.outputDevices
    delegate: Rectangle {
      required property var modelData
      readonly property bool selected: String(root.audio.selected_output_id || "") === String(modelData.id)
      width: root.width
      height: root.theme.space(32)
      radius: root.theme.cornerRadius
      color: selected
        ? root.theme.alpha(root.theme.accent, 0.34)
        : root.theme.alpha(root.theme.foreground, outputMouse.containsMouse ? 0.12 : 0.055)

      Text {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: root.theme.spacing.lg
        anchors.rightMargin: root.theme.spacing.lg
        anchors.verticalCenter: parent.verticalCenter
        text: String(modelData.name || modelData.id)
        elide: Text.ElideMiddle
        color: root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }

      MouseArea {
        id: outputMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: if (!parent.selected) root.bridge.setOutputDevice(String(modelData.id))
      }
    }
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
    text: "Processing"
    color: root.theme.foreground
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
    font.weight: Font.DemiBold
  }

  Row {
    spacing: root.theme.spacing.sm

    Repeater {
      model: ["natural", "clear", "studio"]
      delegate: Rectangle {
        required property string modelData
        readonly property bool selected: String(root.audio.preset || "clear") === modelData
        width: presetText.implicitWidth + root.theme.spacing.lg * 2
        height: root.theme.space(29)
        radius: root.theme.cornerRadius
        color: selected
          ? root.theme.alpha(root.theme.accent, 0.34)
          : root.theme.alpha(root.theme.foreground, presetMouse.containsMouse ? 0.12 : 0.055)

        Text {
          id: presetText
          anchors.centerIn: parent
          text: modelData.charAt(0).toUpperCase() + modelData.slice(1)
          color: root.theme.foreground
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }

        MouseArea {
          id: presetMouse
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: if (!parent.selected) root.bridge.setAudioPreset(modelData)
        }
      }
    }
  }

  Text {
    width: parent.width
    text: "Natural uses WebRTC cleanup · Clear uses continuous DeepFilterNet suppression · Studio is unprocessed"
    wrapMode: Text.WordWrap
    color: root.theme.muted
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
  }

  Rectangle {
    visible: String(root.audio.preset || "clear") === "clear"
      && String(root.audio.denoiser || "deepfilternet") === "deepfilternet"
    width: parent.width
    height: visible ? root.theme.space(72) : 0
    radius: root.theme.cornerRadius
    color: root.theme.alpha(root.theme.foreground, 0.045)

    Column {
      anchors.fill: parent
      anchors.leftMargin: root.theme.spacing.lg
      anchors.rightMargin: root.theme.spacing.lg
      anchors.topMargin: root.theme.spacing.sm
      anchors.bottomMargin: root.theme.spacing.sm
      spacing: root.theme.spacing.xs

      Item {
        width: parent.width
        height: strengthLabel.implicitHeight

        Text {
          id: strengthLabel
          anchors.left: parent.left
          text: "DeepFilterNet strength"
          color: root.theme.foreground
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
          font.weight: Font.DemiBold
        }

        Text {
          anchors.right: parent.right
          text: Math.round(strengthSlider.value) + "%"
          color: root.theme.accent
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }
      }

      Slider {
        id: strengthSlider
        width: parent.width
        height: root.theme.space(28)
        from: 0
        to: 100
        stepSize: 1
        value: root.deepfilterStrength
        onMoved: {
          root.pendingDeepfilterStrength = Math.round(value)
          strengthUpdate.restart()
        }
        onPressedChanged: if (!pressed) root.commitDeepfilterStrength()
        ThemeControlStyle { theme: root.theme; control: strengthSlider }
      }
    }
  }

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
        text: "Neural denoiser · " + String(root.audio.denoiser || "deepfilternet")
          + " · " + String(root.audio.processing_latency_ms || 30) + " ms latency"
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
        text: "Compute " + processingMs.toFixed(1) + " ms"
          + " · queue " + queueMs.toFixed(0) + " ms"
          + (Number(root.audio.processing_deadline_misses || 0) > 0
            ? " · " + String(root.audio.processing_deadline_misses) + " late total"
            : "")
        elide: Text.ElideRight
        color: delayed ? root.theme.warning : root.theme.muted
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }
    }
  }

  Rectangle {
    width: parent.width
    height: root.theme.space(48)
    radius: root.theme.cornerRadius
    color: root.theme.alpha(root.theme.foreground, 0.045)

    Column {
      anchors.left: parent.left
      anchors.leftMargin: root.theme.spacing.lg
      anchors.verticalCenter: parent.verticalCenter
      spacing: root.theme.spacing.xs

      Text {
        text: "Push to talk"
        color: root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
        font.weight: Font.DemiBold
      }

      Text {
        text: "Keep the microphone closed until you hold Talk"
        color: root.theme.muted
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }
    }

    Rectangle {
      anchors.right: parent.right
      anchors.rightMargin: root.theme.spacing.lg
      anchors.verticalCenter: parent.verticalCenter
      width: pttText.implicitWidth + root.theme.spacing.lg * 2
      height: root.theme.space(28)
      radius: root.theme.cornerRadius
      color: root.bridge.pushToTalkState.enabled
        ? root.theme.alpha(root.theme.accent, 0.42)
        : root.theme.alpha(root.theme.foreground, pttToggleMouse.containsMouse ? 0.12 : 0.07)

      Text {
        id: pttText
        anchors.centerIn: parent
        text: root.bridge.pushToTalkState.enabled ? "On" : "Off"
        color: root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
        font.weight: Font.DemiBold
      }

      MouseArea {
        id: pttToggleMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.bridge.setPushToTalk(!root.bridge.pushToTalkState.enabled)
      }
    }
  }

  Rectangle {
    width: parent.width
    height: root.theme.space(58)
    radius: root.theme.cornerRadius
    color: root.theme.alpha(root.theme.foreground, 0.045)

    Column {
      anchors.left: parent.left
      anchors.leftMargin: root.theme.spacing.lg
      anchors.right: shortcutActions.left
      anchors.rightMargin: root.theme.spacing.lg
      anchors.verticalCenter: parent.verticalCenter
      spacing: root.theme.spacing.xs

      Text {
        text: "Global shortcut"
        color: root.theme.foreground
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
        font.weight: Font.DemiBold
      }

      Text {
        width: parent.width
        text: root.capturingShortcut
          ? "Press a letter, number, function key, or combination · Esc cancels"
          : root.shortcutSupported
            ? "Works globally through Omarchy/Hyprland"
            : "Shortcut setup currently requires Omarchy/Hyprland"
        elide: Text.ElideRight
        color: root.capturingShortcut ? root.theme.accent : root.theme.muted
        font.family: root.theme.font.family
        font.pixelSize: root.theme.font.caption
      }
    }

    Row {
      id: shortcutActions
      anchors.right: parent.right
      anchors.rightMargin: root.theme.spacing.lg
      anchors.verticalCenter: parent.verticalCenter
      spacing: root.theme.spacing.sm

      Rectangle {
        visible: root.pttShortcut.length > 0 && !root.capturingShortcut
        width: visible ? clearShortcutText.implicitWidth + root.theme.spacing.lg * 2 : 0
        height: root.theme.space(28)
        radius: root.theme.cornerRadius
        color: root.theme.alpha(root.theme.foreground, clearShortcutMouse.containsMouse ? 0.13 : 0.07)

        Text {
          id: clearShortcutText
          anchors.centerIn: parent
          text: "Clear"
          color: root.theme.muted
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
        }

        MouseArea {
          id: clearShortcutMouse
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: {
            root.capturingShortcut = false
            root.bridge.setPushToTalkShortcut(null)
          }
        }
      }

      Rectangle {
        width: shortcutText.implicitWidth + root.theme.spacing.lg * 2
        height: root.theme.space(28)
        radius: root.theme.cornerRadius
        color: root.capturingShortcut
          ? root.theme.alpha(root.theme.accent, 0.42)
          : root.theme.alpha(root.theme.foreground, shortcutMouse.containsMouse ? 0.13 : 0.07)
        opacity: root.shortcutSupported ? 1 : 0.5

        Text {
          id: shortcutText
          anchors.centerIn: parent
          text: root.capturingShortcut ? "Press keys…" : (root.pttShortcut || "Set shortcut")
          color: root.theme.foreground
          font.family: root.theme.font.family
          font.pixelSize: root.theme.font.caption
          font.weight: Font.DemiBold
        }

        MouseArea {
          id: shortcutMouse
          anchors.fill: parent
          enabled: root.shortcutSupported
          hoverEnabled: enabled
          cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
          onClicked: {
            root.capturingShortcut = true
            root.forceActiveFocus()
          }
        }
      }
    }
  }

  Text {
    visible: root.replacedShortcuts.length > 0
    width: parent.width
    text: "Replaced on " + root.pttShortcut + ": " + root.replacedShortcuts.join(" · ")
    wrapMode: Text.WordWrap
    color: root.theme.warning
    font.family: root.theme.font.family
    font.pixelSize: root.theme.font.caption
  }
}
