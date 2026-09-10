import QtQuick
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components

ShellRoot {
  id: test
  property bool failed: false
  function check(value,message) {if(!value){failed=true;console.error("SOUNDBOARD_FAILED: "+message)}}
  function equal(a,b) {check(a===b,"Expected "+a+" to equal "+b)}
  QtObject {id:colors;property string palette:Quickshell.env("WISP_TEST_PALETTE") || "wisp";property bool managed:false}
  Wisp.WispTheme {id:theme; profile:Quickshell.env("WISP_TEST_THEME") || "clean_tui";appearanceController:colors}
  Item {
    id: bridge
    property alias soundboard: board
    property bool daemonConnected: true
    property var requests: ({})
    property var sent: []
    property int serial: 0
    property var currentVoiceRoom: null
    property string voiceServerId: "a"
    property var activeServer: ({id:"b"})
    property bool effectiveMuted: false
    property var selfState: ({deafened:false})
    property bool admin: false
    property var ownModeration: ({})
    function participantServer(person) {return {server:{id:person.server_id,name:person.server_id === "a" ? "Friends" : "Gaming"},self:{id:person.server_id+"-me",server_admin:admin}}}
    function replaceEntry(map,key,value) {var copy=Object.assign({},map);copy[key]=value;return copy}
    function send(name,args) {serial++;sent.push({id:String(serial),name:name,args:args});return String(serial)}
    function reply(id,ok,value) {var action=requests[id];delete requests[id];board.reply({ok:ok,value:value,error:{message:"Test failure"}},action)}
    Wisp.WispSoundboard {id:board;bridge:bridge}
  }
  FloatingWindow {
    id: window; visible:true; implicitWidth:Number(Quickshell.env("WISP_TEST_WIDTH")) || 440;implicitHeight:Number(Quickshell.env("WISP_TEST_HEIGHT")) || 760
    Rectangle {
      id: surface; anchors.fill:parent; color:theme.background
      Components.SoundboardPopup {id:popup;bridge:bridge;theme:theme}
      Components.SoundboardView {id:library;anchors.fill:parent;anchors.margins:20;bridge:bridge;theme:theme;serverId:"a"}
    }
  }
  function find(item,name) {
    if (item.objectName===name) return item
    for (var child of item.children || []) {var found=find(child,name);if(found)return found}
    return null
  }
  TestCase {
    id: runner; name:"Soundboard"; when: false; parent: window.contentItem
    function test_library_and_cancellation() {
      wait(100)
      test.check(bridge.sent.every(function(c){return c.name==="soundboard_list" || c.name==="soundboard_status"}), "Opening a library never plays or joins")
      bridge.reply(bridge.sent.filter(function(c){return c.name==="soundboard_list"}).slice(-1)[0].id,true,{sounds:[{id:"one",owner_id:"a-me",owner_name:"Me",name:"Applause",duration_ms:1500},{id:"two",owner_id:"someone",owner_name:"Friend",name:"Bell",duration_ms:400}]})
      wait(100)
      test.equal(find(library,"soundboardList").count,2)
      test.check(find(library,"soundboardPreview-one").enabled)
      test.check(!find(library,"soundboardPlay-one").enabled)
      test.check(find(library,"soundboardRemove-one").visible)
      test.check(!find(library,"soundboardRemove-two").visible)
      bridge.admin=true; wait(30);test.check(find(library,"soundboardRemove-two").visible);bridge.admin=false
      bridge.currentVoiceRoom={id:"room"};wait(30)
      test.check(find(library,"soundboardPlay-one").enabled)
      test.check(!find(library,"soundboardPreview-one").enabled)
      bridge.effectiveMuted=true;wait(30)
      test.check(!find(library,"soundboardPlay-one").enabled)
      test.check(find(library,"soundboardPreview-one").enabled)
      board.play("a","one",true);var play=String(bridge.serial)
      test.check(board.pending)
      board.stop();var stop=String(bridge.serial)
      bridge.reply(stop,true,{playing:false,previewing:false})
      bridge.reply(play,true,{previewing:true})
      test.check(!board.pending && !board.previewing,"Late playback reply cannot undo Stop")
      library.selectedPath="file:///tmp/test.wav";find(library,"soundboardName").text="Keep draft"
      board.upload("a","Keep draft",library.selectedPath);var upload=String(bridge.serial)
      board.invalidate();bridge.reply(upload,false,{})
      test.check(!board.busy.a);test.equal(find(library,"soundboardName").text,"Keep draft")
      board.upload("a","Keep draft",library.selectedPath);upload=String(bridge.serial)
      board.invalidate();bridge.reply(upload,true,{})
      test.check(!board.busy.a);test.equal(library.selectedPath,"");test.equal(find(library,"soundboardName").text,"")
      library.selectedPath="file:///tmp/another.wav";find(library,"soundboardName").text="Wrong server"
      library.serverId="b";wait(80)
      test.equal(library.selectedPath,"");test.equal(find(library,"soundboardName").text,"")
      test.equal(find(library,"soundboardList").count,0)
      test.check(!library.canPlay)
      var old=String(bridge.serial);board.reset();bridge.reply(old,true,{sounds:[{id:"stale"}]})
      test.check(!board.catalogs.b,"Disconnected requests cannot repopulate another session")
      library.serverId="a";board.catalogs={a:[{id:"one",owner_id:"a-me",owner_name:"Me",name:"Applause",duration_ms:1500},{id:"two",owner_id:"someone",owner_name:"Friend",name:"Bell",duration_ms:400}]}
      bridge.currentVoiceRoom=null;bridge.effectiveMuted=false
      wait(100)
      var screenshot=Quickshell.env("WISP_SOUNDBOARD_SCREENSHOT")
      if(screenshot) {surface.grabToImage(function(result){result.saveToFile(screenshot)});wait(200)}
      bridge.currentVoiceRoom={id:"room"};popup.open();wait(100)
      test.check(popup.visible && popup.width<=window.width && popup.height<=window.height,"Call popup fits its window")
      var callLibrary=test.find(popup.contentItem,"settingsSoundboard")
      test.check(!!callLibrary && callLibrary.serverId===bridge.voiceServerId,"Call popup follows the voice server")
      test.check(callLibrary.playOnly && !test.find(callLibrary,"soundboardUpload").visible,"Playback menu keeps upload forms in Manage sounds")
      var quickPlay=test.find(callLibrary,"soundboardQuickPlay-one")
      test.check(quickPlay && quickPlay.visible && quickPlay.enabled,"Sound name is a direct playback control")
      mouseClick(quickPlay,quickPlay.width/2,quickPlay.height/2)
      var played=bridge.sent[bridge.sent.length-1]
      test.check(played.name==="soundboard_play" && played.args.server_id==="a" && played.args.sound_id==="one","Click sends the selected sound to the voice server")
      bridge.reply(played.id,true,{playing:true})
      board.stop();bridge.reply(String(bridge.serial),true,{playing:false})
      bridge.effectiveMuted=true;wait(30);test.check(!quickPlay.enabled,"Muted call cannot send sounds")
      var previewToggle=test.find(callLibrary,"soundboardPreviewMode")
      mouseClick(previewToggle,previewToggle.width/2,previewToggle.height/2);wait(30)
      test.check(callLibrary.previewMode && quickPlay.enabled,"Muted microphone permits explicitly selected private preview")
      mouseClick(quickPlay,quickPlay.width/2,quickPlay.height/2)
      var preview=bridge.sent[bridge.sent.length-1]
      test.check(preview.name==="soundboard_preview" && preview.args.sound_id==="one","Preview pad never broadcasts to the room")
      bridge.reply(preview.id,true,{previewing:true});board.stop();bridge.reply(String(bridge.serial),true,{previewing:false})
      bridge.effectiveMuted=false;wait(30)
      test.check(!quickPlay.enabled && callLibrary.previewMode,"Unmuting never silently changes a preview into room playback")
      mouseClick(previewToggle,previewToggle.width/2,previewToggle.height/2);wait(30)
      quickPlay.forceActiveFocus();keyClick(Qt.Key_Right);wait(30)
      var second=test.find(callLibrary,"soundboardQuickPlay-two")
      test.check(second.activeFocus,"Arrow keys move between sound pads")
      keyClick(Qt.Key_Space);wait(30)
      var keyboardPlay=bridge.sent[bridge.sent.length-1]
      test.check(keyboardPlay.name==="soundboard_play" && keyboardPlay.args.sound_id==="two","Space plays the focused sound")
      bridge.reply(keyboardPlay.id,true,{playing:false})
      var manage=test.find(popup.contentItem,"soundboardManageButton")
      mouseClick(manage,manage.width/2,manage.height/2);wait(30)
      test.check(!callLibrary.playOnly && test.find(callLibrary,"soundboardUpload").visible,"Manage sounds exposes uploads")
      mouseClick(manage,manage.width/2,manage.height/2);wait(30)
      var defaults=["Air Horn","Applause","Boing","Drum Roll","Level Up","Ping","Rimshot","Sad Trombone"]
      board.catalogs={a:defaults.map(function(n,i){return {id:String(i),name:n,owner_id:"a-me",owner_name:"Me",duration_ms:1300}})}
      wait(50)
      var pads=test.find(callLibrary,"soundboardPads")
      test.check(popup.width<=theme.space(420) && popup.height<theme.space(480),"Eight sounds fit a compact popup")
      test.check(pads.columns>=2 && pads.count===8,"Sounds use a responsive grid")
      test.find(callLibrary,"soundboardSearch").text="trombone";wait(30)
      test.equal(pads.count,1)
      test.find(callLibrary,"soundboardSearch").text="";wait(30)
      var stopButton=test.find(callLibrary,"soundboardStop")
      var stopPosition=stopButton.mapToItem(popup.contentItem,0,0)
      test.check(stopPosition.y>=0 && stopPosition.y+stopButton.height<=popup.contentItem.height,"Stop remains reachable above the scrolling sounds")
      if(screenshot) {popup.contentItem.grabToImage(function(result){result.saveToFile(screenshot.replace(".png","-popup.png"))});wait(200)}
      bridge.currentVoiceRoom=null;wait(50);test.check(!popup.visible,"Leaving closes call popup")
      popup.open();wait(50)
      test.equal(test.find(popup.contentItem,"settingsSoundboard").serverId,"b")
      test.check(!test.find(popup.contentItem,"settingsSoundboard").canPlay,"Outside a call, selected server is browse/preview only")
      popup.close()
      board.reset();board.catalogs={a:[{name:"PING",id:"existing",owner_id:"someone"}]}
      board.loading=({});library.serverId="a";wait(30);board.loading=({})
      find(library,"soundboardName").text="Unfinished upload"
      var packStart=bridge.sent.length
      board.addDefaults("a")
      test.check(board.busy.a,"Starter uploads mark the selected server busy")
      var added=0
      while(board.busy.a && added<9) {
        var next=bridge.sent[bridge.sent.length-1]
        test.check(next.name==="soundboard_upload" && next.args.server_id==="a" && next.args.name!=="Ping","Starter queue skips existing names and stays on its server")
        test.check(next.args.path.indexOf("assets/soundboard/")>=0,"Starter upload uses the bundled audio file")
        board.invalidate();bridge.reply(next.id,true,{});added++
      }
      test.equal(added,7)
      test.check(!board.busy.a && board.feedback.a.indexOf("7")>=0,"All missing starter sounds finish sequentially")
      test.equal(find(library,"soundboardName").text,"Unfinished upload")
      test.check(bridge.sent.slice(packStart).every(function(c){return c.name==="soundboard_upload" || c.name==="soundboard_list"}),"Adding starters never plays media")
      board.reset();board.catalogs={a:[]};board.addDefaults("a");var failed=bridge.sent[bridge.sent.length-1]
      bridge.reply(failed.id,false,{})
      test.check(!board.busy.a && !board.starterQueues.a && board.feedback.a.indexOf("Test failure")>=0,"A failed starter upload stops the queue and reports the error")
      board.reset();var full=[];for(var n=0;n<64;n++)full.push({name:"Custom "+n})
      board.catalogs={a:full};var beforePack=bridge.sent.length;board.addDefaults("a")
      test.check(!board.busy.a && bridge.sent.length===beforePack,"Full libraries retain existing sounds")

    }
  }
  Timer {interval:400;running:true;onTriggered:{runner.test_library_and_cancellation();if(!test.failed)console.log("SOUNDBOARD_OK");Qt.quit()}}
}
