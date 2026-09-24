import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components
ShellRoot {
  id:test
  property bool failed:false
  function check(ok,label){if(!ok){failed=true;console.error("PICKERS_FAILED: "+label)}}
  function bounds(p,w,label){check(p.opened && p.x>=0 && p.y>=0 && p.x+p.width<=w.width+1 && p.y+p.height<=w.height+1,label)}
  Wisp.WispTheme {id:theme;profile:Quickshell.env("WISP_TEST_THEME") || "soft_graphite"}
  QtObject {
    id:bridge
    property var currentVoiceRoom:({id:"room",server_id:"local",members:[]})
    property string currentVoiceLabel:"Lounge"
    property var voiceFriends:Array.from({length:25},function(_,i){return {id:"friend"+i,display_name:"Friend "+i}})
    property var invitationRequests:({})
    property var invited:[]
    function inviteToRoom(person){invited.push(person.id)}
    property QtObject chatExtras:QtObject {
      property int epoch:0
      property var feedback:({})
      function refresh(server,force){}
      function library(server,query){return []}
      function load(server,emoji){}
      function url(server,emoji){return emoji.indexOf(":wisp_")===0 ? Qt.resolvedUrl("app/assets/emojis/"+emoji.slice(6,-1)+".svg") : ""}
    }
  }
  FloatingWindow {
    id:window;visible:true;implicitWidth:1000;implicitHeight:760;color:theme.background
    Item {
      id:tile;x:window.width-320;y:window.height-350;width:300;height:330
      Rectangle {anchors.fill:parent;color:theme.surface;border.color:theme.separator}
      Components.ChatButton {id:emojiButton;anchors.right:parent.right;anchors.bottom:parent.bottom;theme:theme;text:"Emoji";onClicked:emoji.showAt(emojiButton)}
      Components.EmojiPicker {id:emoji;theme:theme;bridge:bridge;serverId:"local"}
    }
    Components.ChatButton {id:inviteButton;x:24;y:40;theme:theme;text:"Invite";onClicked:invite.showAt(inviteButton)}
    Components.RoomInvitePicker {id:invite;theme:theme;bridge:bridge}
  }
  FloatingWindow {
    id:tray;visible:true;implicitWidth:300;implicitHeight:400;color:theme.background
    Components.ChatButton {id:trayButton;anchors.right:parent.right;anchors.bottom:parent.bottom;theme:theme;text:"Emoji";onClicked:emoji.showAt(trayButton)}
  }
  TestCase {id:input;parent:window.contentItem;when:false}
  Timer {running:true;interval:300;onTriggered:{
    emojiButton.clicked();input.wait(60);test.bounds(emoji,window,"right tile picker stays within main window")
    var buttonPoint=emojiButton.mapToItem(emoji.parent,0,0)
    test.check(emoji.y+emoji.height<=buttonPoint.y,"bottom tile picker opens above its button")
    test.check(emoji.width>tile.width,"picker can extend outside a narrow tile")
    emoji.close();inviteButton.clicked();input.wait(60);test.bounds(invite,window,"invite stays within main window")
    var point=inviteButton.mapToItem(invite.parent,0,inviteButton.height)
    test.check(Math.abs(invite.x-point.x)<1 && Math.abs(invite.y-point.y-theme.space(6))<1,"invite anchors directly below initiating button")
    test.check(invite.contentItem.contentHeight>invite.contentItem.height,"long invitation list scrolls")
    invite.contentItem.contentY=invite.contentItem.contentHeight-invite.contentItem.height;input.wait(30)
    test.check(bridge.invited.length===0,"scrolling never sends invitation")
    invite.close();trayButton.clicked();input.wait(60);test.bounds(emoji,tray,"shared picker switches to invoking tray overlay")
    test.check(emoji.parent!==window.contentItem && emoji.width<tray.width,"tray constrains full picker")
    tray.implicitWidth=240;tray.implicitHeight=260;input.wait(60);test.bounds(emoji,tray,"picker follows tray resizing")
    emoji.close();emojiButton.clicked();input.wait(60);test.bounds(emoji,window,"reopening returns picker to main window")
    if(Quickshell.env("WISP_PICKERS_SCREENSHOT"))emoji.contentItem.parent.grabToImage(function(im){im.saveToFile(Quickshell.env("WISP_PICKERS_SCREENSHOT"));console.log(test.failed?"PICKERS_FAILED":"PICKERS_OK");Qt.quit()})
    else {console.log(test.failed?"PICKERS_FAILED":"PICKERS_OK");Qt.quit()}
  }}
}
