import QtQuick
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components
import "app/AvatarPresets.js" as Presets

ShellRoot {
  id:test
  property bool failed:false
  function check(ok,label) { if(!ok){failed=true;console.error("AVATAR_PRESETS_FAILED: "+label)} }
  function find(item,name,seen) {
    if(!item)return null;seen=seen || [];if(seen.indexOf(item)>=0)return null;seen.push(item)
    if(item.objectName===name)return item
    if(item.contentItem){var r=find(item.contentItem,name,seen);if(r)return r}
    if(item.footer){var r=find(item.footer,name,seen);if(r)return r}
    var children=item.data || item.contentData || item.children || [];for(var i=0;i<children.length;i++){var r=find(children[i],name,seen);if(r)return r}return null
  }
  QtObject {
    id: appearance
    property bool managed: false
    property bool showAvatars: true
    property string palette: theme.profile==="clean_tui" ? "solarized_japan" : theme.profile
    property var colorOptions: ({senderNames:true,chatBorders:true,chatHeadings:true,roomSections:false,friendSections:false,onlineFriends:true})
  }
  Wisp.WispTheme { id:theme;profile:Quickshell.env("WISP_TEST_THEME") || "soft_graphite";appearanceController:appearance }
  QtObject {
    id:bridge
    property bool daemonConnected:false
    property var requests:({})
    property var sent:[]
    property int serial:0
    property var snapshot:({server_states:[]})
    property var activeServerState:({self:{id:"11111111-1111-4111-8111-111111111111"}})
    property var accountProfile:({display_name:"Rowan"})
    property alias avatars:avatars
    function replaceEntry(map,key,value){var next=Object.assign({},map);if(value===undefined)delete next[key];else next[key]=value;return next}
    function send(name,args){var id="avatar-"+(++serial);sent.push({id:id,name:name,args:args});return id}
    function settingsSaved(){}
  }
  Wisp.WispAvatars {id:avatars;bridge:bridge}
  FloatingWindow {
    id:window;visible:true;implicitWidth:720;implicitHeight:700;color:theme.background
    Rectangle {
      id:scene;anchors.fill:parent;color:theme.background
      Components.ProfilePicture {id:profile;width:parent.width-64;x:32;y:32;bridge:bridge;theme:theme;serverId:"lantern"}
    }
  }
  TestCase {id:input;parent:window.contentItem;when:false}
  Timer {
    running:true;interval:500
    onTriggered:{
      var choose=test.find(profile,"chooseWispAvatar"), save=test.find(profile,"saveProfilePicture")
      test.check(!!choose && !!save,"profile exposes built-in avatar chooser and save")
      choose.clicked();input.wait(700)
      var dialog=test.find(profile,"avatarPresetPicker"),grid=test.find(dialog,"avatarPresetGrid")
      test.check(dialog.opened && Presets.choices.length===15,"gallery opens with fifteen designs")
      Presets.choices.forEach(function(a){var b=test.find(dialog,"avatarPreset-"+a.id),im=test.find(dialog,"avatarArtwork-"+a.id);test.check(!!b && b.enabled && im.status===Image.Ready,"asset ready: "+a.id)})
      test.check(bridge.sent.length===0,"opening gallery performs no upload or fetch")
      test.find(dialog,"avatarPreset-drift").clicked();input.wait(100)
      test.check(profile.selectedPath.endsWith("/assets/avatars/drift.png") && profile.selectedName==="Drift" && !dialog.opened && save.visible,"choice becomes a preview")
      test.check(bridge.sent.length===0,"selecting does not publish")
      save.clicked();input.wait(20)
      var command=bridge.sent[bridge.sent.length-1]
      test.check(command.name==="upload_avatar" && command.args.server_id==="lantern" && command.args.path===profile.selectedPath,"save uses existing server-scoped avatar upload")
      test.check(profile.busy && !choose.enabled,"saving blocks duplicate actions")
      avatars.reply({id:command.id,ok:false,error:{message:"Try again"}},bridge.requests[command.id]);input.wait(20)
      test.check(!!profile.selectedPath && !profile.busy,"failure retains preview for retry")
      save.clicked();command=bridge.sent[bridge.sent.length-1]
      avatars.reply({id:command.id,ok:true,value:{}},bridge.requests[command.id]);input.wait(20)
      test.check(profile.selectedPath==="" && profile.selectedName==="" && avatars.feedback.lantern==="Profile picture saved","success clears selection")
      choose.clicked();input.wait(80);test.find(dialog,"avatarPreset-halo").clicked();input.wait(40)
      test.find(profile,"cancelProfilePicture").clicked();test.check(!profile.selectedPath,"cancel leaves saved profile alone")
      choose.clicked();input.wait(80);profile.serverId="elsewhere";input.wait(80)
      test.check(!dialog.opened && !profile.selectedPath,"server switch closes chooser and clears pending selection")
      choose.clicked();input.wait(80)
      var keyboard=test.find(dialog,"avatarPreset-glitch");input.tryCompare(keyboard,"enabled",true,2000);keyboard.forceActiveFocus();input.keyClick(Qt.Key_Space);input.wait(80)
      test.check(profile.selectedName==="Glitch","keyboard can select an avatar")
      var originalWidth=window.width;window.width=340;choose.clicked();input.wait(120)
      test.check(dialog.width<=window.width && grid.columns>=2 && grid.width<=dialog.width,"narrow gallery fits and scrolls")
      dialog.close();window.width=originalWidth;input.wait(80);choose.clicked();input.wait(200)
      test.check(bridge.sent.every(function(c){return c.name==="upload_avatar"}),"fixture never starts media or other network operations")
      var screenshot=Quickshell.env("WISP_AVATAR_SCREENSHOT")
      if(screenshot)dialog.contentItem.parent.grabToImage(function(im){im.saveToFile(screenshot);console.log(test.failed?"AVATAR_PRESETS_FAILED":"AVATAR_PRESETS_OK");Qt.quit()})
      else{console.log(test.failed?"AVATAR_PRESETS_FAILED":"AVATAR_PRESETS_OK");Qt.quit()}
    }
  }
}
