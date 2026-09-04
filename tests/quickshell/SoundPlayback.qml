import QtQuick
import Quickshell
import "app" as Wisp

ShellRoot {
  Wisp.WispBridge {
    id: bridge
    Component.onCompleted: {
      notificationVolume=40; notificationMuted=false
      setEventSound("member_leave", "file://"+Quickshell.env("WISP_SOUND_DIR")+"/message.wav")
      playNotificationSound("self_join")
      playNotificationSound("member_join")
      playNotificationSound("member_leave")
      playNotificationSound("self_leave")
    }
  }
  Timer {
    interval: 1200; running: true
    onTriggered: {
      if (bridge.notificationError || bridge.soundQueue.length) console.error("SOUND_TEST_FAILED",bridge.notificationError)
      else console.log("SOUND_PLAYBACK_OK")
      bridge.notificationMuted=true
      bridge.playNotificationSound("self_join")
      Qt.quit()
    }
  }
}
