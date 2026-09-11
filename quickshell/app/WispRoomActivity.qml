import QtQuick
import Quickshell.Io
import "RoomActivity.js" as Activity

Item {
  id:root
  required property var bridge
  property var lastAlerts:({})
  readonly property alias pending: notices
  ListModel {id:notices;dynamicRoles:true}
  property int serial:0
  property string error:""
  readonly property var options:({enabled:bridge.friendRoomNotifications,timing:bridge.friendRoomNotificationTiming,
    focused:bridge.appFocused,homes:bridge.serverPreferences.homeIds,
    onlyEmpty:bridge.friendRoomOnlyEmpty,cooldown:bridge.friendRoomCooldown})
  function observe(previous,next,eventName) {
    if (!bridge.notificationSoundsEnabled) return // One desktop owner, never the embedded tray adapter.
    var now=Date.now(), fresh={}
    Object.keys(lastAlerts).forEach(function(k){if(now-lastAlerts[k]<7200000)fresh[k]=lastAlerts[k]})
    Activity.joins(previous,next,eventName,options,now,fresh).forEach(function(notice) {
      // Bound live notification processes; a burst in one room is already grouped.
      if (pending.count>=3) return
      fresh[notice.key]=now
      deliver(notice)
      if (bridge.friendRoomNotificationSound) bridge.playNotificationSound("friend_room_join")
    })
    lastAlerts=fresh
  }
  function deliver(notice) {
    error=""
    notices.append({serial:++serial,payload:notice})
  }
  function testNotification() {
    if(pending.count>=3)return
    deliver({names:["A friend"],roomName:"Example room",serverName:"Wisp",conversationId:""})
  }
  function openRoom(notice) {
    if (!notice.conversationId) return
    var state=bridge.serverStates.filter(function(s){return String(s.server.id)===notice.serverId})[0]
    if (state && String((state.self || {}).id)===notice.accountId && bridge.conversationById(notice.conversationId))
      bridge.openChannel(notice.conversationId,false)
  }
  Instantiator {
    model:root.pending
    delegate:Item {
      id:job
      required property int serial
      required property var payload
      function remove() {
        var id=serial
        Qt.callLater(function(){for(var i=0;i<notices.count;i++)if(notices.get(i).serial===id){notices.remove(i);break}})
      }
      Timer {interval:60000;running:true;onTriggered:{notification.running=false;job.remove()}}
      Process {
        id:notification
        command:Activity.command(job.payload)
        running:true
        stdout:StdioCollector {onStreamFinished:if(text.trim()==="default")root.openRoom(job.payload)}
        stderr:StdioCollector {} // Keep desktop-service diagnostics out of public output.
        onExited:function(code,status) {
          if(code!==0)root.error="Couldn't show the desktop notification. Check your system notification settings and that notify-send is installed."
          job.remove()
        }
      }
    }
  }
}
