import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp
import "app/views" as Views
import "app/ChatMarkup.js" as Markup
ShellRoot {
  id:test;property bool failed:false
  function check(ok,label){if(!ok){failed=true;console.error("ACTIVITY_SETTINGS_FAILED: "+label)}}
  function find(item,name,seen){if(!item)return null;seen=seen||[];if(seen.indexOf(item)>=0)return null;seen.push(item);if(item.objectName===name)return item;if(item.contentItem){var r=find(item.contentItem,name,seen);if(r)return r}var children=item.data||item.contentData||item.children||[];for(var i=0;i<children.length;i++){var r=find(children[i],name,seen);if(r)return r}return null}
  Wisp.WispTheme {id:theme;profile:Quickshell.env("WISP_TEST_THEME")||"soft_graphite"}
  Wisp.WispBridge {id:bridge;property var sent:[];function send(name,args){var id="test-"+(++requestId);sent.push({id:id,name:name,args:args});return id}}
  FloatingWindow {id:window;visible:true;implicitWidth:660;implicitHeight:820;color:theme.background
    Rectangle {id:canvas;anchors.fill:parent;color:theme.background
    Views.NotificationSettingsView {id:settings;anchors.left:parent.left;anchors.right:parent.right;anchors.margins:20;bridge:bridge;theme:theme}
  }}
  TestCase {id:input;parent:window.contentItem;when:false}
  Timer {running:true;interval:450;onTriggered:{
    var section=test.find(settings,"desktopActivitySection")
    test.check(!!section && !section.expanded,"activity options start grouped and collapsed")
    section.expanded=true;input.wait(30)
    var toggle=test.find(settings,"autoAwaySetting"),minutes=test.find(settings,"idleThresholdSetting")
    test.check(toggle.checked && minutes.value===30,"automatic Away defaults to thirty minutes")
    toggle.checked=false;toggle.clicked();input.wait(30)
    var last=bridge.sent[bridge.sent.length-1]
    test.check(last.name==="configure_desktop_activity" && last.args.auto_away===false && last.args.idle_minutes===30 && last.args.server_id===undefined,"visible Away changes preserve independent inactivity threshold")
    bridge.finishRequest({id:last.id,ok:true,value:{auto_away:false,idle_minutes:30,state:"active",source:"wayland"}})
    input.wait(30);test.check(minutes.enabled && !toggle.checked,"threshold stays adjustable with visible Away off")
    minutes.value=45;minutes.valueModified();last=bridge.sent[bridge.sent.length-1]
    test.check(last.args.idle_minutes===45 && last.args.auto_away===false,"changing threshold preserves disabled visible Away")
    bridge.finishRequest({id:last.id,ok:false});input.wait(30)
    test.check(!bridge.desktopActivityBusy && !!bridge.desktopActivityError,"failed save unlocks settings and shows error")
    bridge.refreshDesktopActivity();last=bridge.sent[bridge.sent.length-1]
    bridge.finishRequest({id:last.id,ok:true,value:{auto_away:false,idle_minutes:45,state:"unknown",source:"unavailable"}})
    input.wait(30);test.check(!bridge.desktopActivityError && test.find(settings,"desktopActivityStatus").text.indexOf("phone alerts enabled")>=0,"unavailable detection explains mobile fallback")
    test.check(!bridge.sent.some(function(c){return c.name.indexOf("join")>=0 || c.name==="set_presence"}),"settings never join voice or overwrite manual presence")
    if(Quickshell.env("WISP_ACTIVITY_SCREENSHOT")) {
      canvas.grabToImage(function(image){image.saveToFile(Quickshell.env("WISP_ACTIVITY_SCREENSHOT"));console.log(test.failed?"ACTIVITY_SETTINGS_FAILED":"ACTIVITY_SETTINGS_OK");Qt.quit()})
    } else {console.log(test.failed?"ACTIVITY_SETTINGS_FAILED":"ACTIVITY_SETTINGS_OK");Qt.quit()}
  }}
}
