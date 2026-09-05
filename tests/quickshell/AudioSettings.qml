import QtQuick
import QtTest
import Quickshell
import "app" as Wisp
import "app/views" as Views

ShellRoot {
  id: root
  property bool captured: false
  Wisp.WispAppearance { id: appearance; environment: "desktop" }
  Wisp.WispTheme { id: theme; profile: "performative"; appearanceController: appearance }
  QtObject {
    id: bridge
    property var audioState: ({preset: "clear", input_devices: [{id: "mic", name: "USB microphone"}], output_devices: [{id: "out", name: "Headphones"}], selected_input_id: "mic", selected_output_id: "out", denoiser_active: true, denoiser: "deepfilternet", input_level: 35})
    property var mediaState: ({livekit_connected: false})
    property var pushToTalkState: ({enabled: false, shortcut_backend: "test", shortcut: ""})
    property var commands: []
    function setAudioPreset(value) {
      commands.push(value)
      var next=JSON.parse(JSON.stringify(audioState))
      next.preset=value; next.denoiser_active=value === "clear"
      audioState=next
    }
    function setPushToTalk(enabled) { pushToTalkState={enabled: enabled, shortcut_backend:"test", shortcut:""} }
    function refreshAudioDevices() {}
    function setInputDevice(id) {}
    function setOutputDevice(id) {}
    function setPushToTalkShortcut(value) {}
  }
  FloatingWindow {
    id: window
    visible: true
    implicitWidth: 450
    implicitHeight: 650
    color: theme.background
    Rectangle {
      id: canvas
      anchors.fill: parent
      color: theme.background
      Views.AudioSettingsView { id: settings; x: 20; y: 20; width: parent.width-40; bridge: bridge; theme: theme }
    }
  }
  TestCase {
    id: checks
    name: "AudioSettings"
    when: window.visible
    function label(item, text) {
      if (item.text === text) return item
      for (var child of item.children || []) {
        var found=label(child,text)
        if (found) return found
      }
      return null
    }
    function choose(text) {
      var item=label(settings,text)
      verify(!!item,"choice exists: "+text)
      mouseClick(item.parent,item.parent.width/2,item.parent.height/2)
      wait(20)
    }
    function test_modes() {
      wait(100)
      verify(!!label(settings,"Voice cleanup · full quality"))
      choose("Light cleanup")
      compare(bridge.audioState.preset,"natural")
      choose("Unprocessed")
      compare(bridge.audioState.preset,"studio")
      choose("Clear voice")
      compare(bridge.audioState.preset,"clear")
      compare(bridge.commands.join(","),"natural,studio,clear")
      var state=JSON.parse(JSON.stringify(bridge.audioState)); state.denoiser="webrtc"; bridge.audioState=state
      wait(20)
      verify(!!label(settings,"Voice cleanup · lightweight mode"))
      verify(!!label(settings,"Applies when you join a voice room"))
      state.denoiser="deepfilternet"; bridge.audioState=JSON.parse(JSON.stringify(state))
      wait(20)
      canvas.grabToImage(function(result) {
        var path=Quickshell.env("WISP_AUDIO_SCREENSHOT")
        if (path) result.saveToFile(path)
        root.captured=true
      })
      tryCompare(root,"captured",true,2000)
      console.log("AUDIO_SETTINGS_OK")
    }
  }
  Timer { interval: 8000; running: true; onTriggered: { console.error("AUDIO_SETTINGS_TIMEOUT"); Qt.quit() } }
}
