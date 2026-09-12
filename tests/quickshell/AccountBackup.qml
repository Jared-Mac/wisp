import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components
ShellRoot {
  id:test; property bool failed:false
  function check(ok,label) { if(!ok) { failed=true;console.error("ACCOUNT_BACKUP_FAILED: "+label) } }
  function find(item,name) { if(!item)return null;if(item.objectName===name)return item;for(var child of item.children||[]){var result=find(child,name);if(result)return result}return null }
  Wisp.WispAppearance {id:appearance;environment:"desktop"}
  Wisp.WispTheme {id:theme;appearanceController:appearance;profile:Quickshell.env("WISP_TEST_THEME")||"soft_graphite"}
  QtObject {
    id:bridge;property bool daemonConnected:true;property bool profileBusy:false
    property var accountBackup:({mode:"classic",auto_sync:true})
    property var last:({})
    function profileAction(name,args) { last={name:name,args:args};return true }
  }
  FloatingWindow {id:window;visible:true;implicitWidth:540;implicitHeight:760;color:theme.background
    Rectangle {id:canvas;anchors.fill:parent;color:theme.background
      ScrollView {anchors.fill:parent;anchors.margins:24;contentWidth:availableWidth
        Components.AccountBackupSettings {id:settings;width:parent.width;theme:theme;bridge:bridge}
      }
    }
  }
  TestCase {id:input;parent:window.contentItem;when:false}
  Timer {interval:350;running:true;onTriggered:{
    test.check(!settings.expanded,"backup starts grouped")
    settings.expanded=true;input.wait(40)
    var old=test.find(settings,"backupOriginalPassword"),fresh=test.find(settings,"backupNewPassword"),confirm=test.find(settings,"backupConfirmPassword"),enable=test.find(settings,"backupEnable")
    old.text="synthetic old secret";fresh.text="synthetic old secret";confirm.text=fresh.text
    test.check(!enable.enabled,"migration requires a different secure password")
    fresh.text="synthetic new secret";confirm.text=fresh.text
    test.check(enable.enabled,"matching new secure password enables migration")
    enable.clicked()
    test.check(bridge.last.name==="backup_enable" && bridge.last.args.current_password==="synthetic old secret","migration is explicit")
    test.check(!old.text && !fresh.text && !confirm.text,"migration clears all password fields immediately")
    bridge.accountBackup={mode:"secure",unlocked:true,auto_sync:true,last_synced_at:100,has_changes:false};input.wait(30)
    test.check(!old.visible,"secure settings hide classic password fields")
    bridge.accountBackup={mode:"secure",unlocked:false,repair_needed:true,pending:null};input.wait(30)
    var password=test.find(settings,"backupUnlockPassword");test.check(password.visible,"locked backup asks for native password")
    password.text="discard synthetic password";settings.expanded=false;input.wait(30)
    test.check(password.text==="","collapsing settings clears private drafts")
    bridge.accountBackup={mode:"classic",pending:"migration",can_cancel:false};settings.expanded=true;input.wait(30)
    test.check(password.visible && !old.visible,"interrupted migration has a separate recovery flow")
    bridge.accountBackup={mode:"secure",unlocked:true,auto_sync:true,repair_needed:false,last_synced_at:1789200000};input.wait(30)
    var output=Quickshell.env("WISP_BACKUP_SCREENSHOT")
    if(output)canvas.grabToImage(function(image){image.saveToFile(output);console.log(test.failed?"ACCOUNT_BACKUP_FAILED":"ACCOUNT_BACKUP_OK");Qt.quit()})
    else {console.log(test.failed?"ACCOUNT_BACKUP_FAILED":"ACCOUNT_BACKUP_OK");Qt.quit()}
  }}
}
