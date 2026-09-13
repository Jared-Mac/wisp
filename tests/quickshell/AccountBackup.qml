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
  function reply(value,ok,error) {
    var id=setup.requestId,action=bridge.requests[id];delete bridge.requests[id]
    setup.finish({id:id,ok:ok!==false,value:value,error:{message:error||""}},action)
  }
  Wisp.WispAppearance {id:appearance;environment:"desktop"}
  Wisp.WispTheme {id:theme;appearanceController:appearance;profile:Quickshell.env("WISP_TEST_THEME")||"soft_graphite"}
  Item {
    id:bridge;property bool daemonConnected:true;property bool privacySnapshotReady:false;property bool profileBusy:false
    property string profileRequestId:""; property bool profileReady:false
    property string profileServerId:"server"
    property var activeServerState:({self:{id:"account"}})
    property var accountBackup:({})
    property var requests:({});property int serial:0;property var last:({})
    function send(name,args) { last={name:name,args:args};return "request"+(++serial) }
  }
  Wisp.WispAccountSetup {id:setup;bridge:bridge}
  FloatingWindow {id:window;visible:true;implicitWidth:470;implicitHeight:680;color:theme.background
    Rectangle {id:canvas;anchors.fill:parent;color:theme.background
      Components.AccountSetupDialog {id:dialog;setup:setup;theme:theme;surfaceActive:true}
      Components.AccountSetupDialog {id:otherDialog;setup:setup;theme:theme;surfaceActive:true}
    }
  }
  TestCase {id:input;parent:window.contentItem;when:false}
  Timer {interval:350;running:true;onTriggered:{
    test.check(!setup.requestId,"no status request before authenticated snapshot")
    bridge.privacySnapshotReady=true;input.wait(40)
    test.check(bridge.last.name==="backup_status","authenticated account status checked automatically")
    test.reply({mode:"secure",unlocked:true,auto_sync:true});input.wait(40)
    test.check(!dialog.visible && !otherDialog.visible,"new secure account requires no extra setup")
    bridge.accountBackup={mode:"classic",identity_ready:true};input.wait(60)
    test.check(dialog.visible !== otherDialog.visible,"only one surface claims upgrade confirmation")
    var current=dialog.visible?dialog:otherDialog
    var field=test.find(current.contentItem,"accountSetupPassword"),button=test.find(current.contentItem,"accountSetupContinue")
    test.check(field.visible && !button.enabled,"one private current-password confirmation")
    test.check(!test.find(current.contentItem,"backupNewPassword"),"no new-password migration field")
    field.text="synthetic original classic password";button.clicked()
    test.check(bridge.last.name==="backup_upgrade" && bridge.last.args.current_password==="synthetic original classic password","upgrade reuses current password")
    test.check(field.text==="" && JSON.stringify(bridge.requests).indexOf("synthetic original")<0,"password cleared and absent from request journal")
    test.reply({},false,"Current password is incorrect.");input.wait(40)
    test.reply({mode:"classic",identity_ready:true});input.wait(30)
    var dismiss=test.find(current.contentItem,"dismissError")
    test.check(setup.error.length>0 && dismiss,"inline error can be dismissed")
    dismiss.clicked();test.check(setup.error==="","dismiss clears error without another account action")
    field.text="private draft";current.close();input.wait(40)
    test.check(!field.text && !dialog.visible && !otherDialog.visible,"Later clears secret and defers both surfaces")
    setup.refresh();test.reply({mode:"classic",identity_ready:true});input.wait(30)
    test.check(!dialog.visible && !otherDialog.visible,"status polling does not undo Later")
    setup.request();test.reply({mode:"classic",identity_ready:true});input.wait(40)
    current=dialog.visible?dialog:otherDialog
    field=test.find(current.contentItem,"accountSetupPassword");button=test.find(current.contentItem,"accountSetupContinue")
    field.text="synthetic original classic password";button.clicked()
    test.reply({mode:"secure",unlocked:true,auto_sync:true});input.wait(40)
    test.check(!dialog.visible && !otherDialog.visible,"successful update closes confirmation")
    bridge.accountBackup={mode:"secure",unlocked:false,repair_needed:true};input.wait(40)
    current=dialog.visible?dialog:otherDialog
    field=test.find(current.contentItem,"accountSetupPassword");field.text="discard on disconnect"
    bridge.daemonConnected=false;input.wait(40)
    test.check(!field.text && !dialog.visible && !otherDialog.visible,"disconnect clears all private drafts")
    bridge.daemonConnected=true;input.wait(40)
    var oldId=setup.requestId,oldAction=bridge.requests[oldId]
    bridge.activeServerState={self:{id:"another-account"}};input.wait(40)
    setup.finish({id:oldId,ok:true,value:{mode:"classic",identity_ready:true}},oldAction)
    test.check(!setup.status.mode,"stale reply cannot migrate a different selected account")
    test.reply({mode:"secure",unlocked:true});input.wait(30)
    bridge.accountBackup={mode:"classic",identity_ready:false};input.wait(40)
    current=dialog.visible?dialog:otherDialog
    test.check(!test.find(current.contentItem,"accountSetupContinue").visible,"missing original keys never enroll replacement identity")
    bridge.accountBackup={mode:"classic",identity_ready:true,pending:"migration",can_cancel:false};input.wait(40)
    test.check(test.find(current.contentItem,"accountSetupContinue").enabled,"lost response can resume without reentering password")
    bridge.accountBackup={mode:"classic",identity_ready:true};input.wait(30)
    var output=Quickshell.env("WISP_BACKUP_SCREENSHOT")
    if(output)current.contentItem.grabToImage(function(image){image.saveToFile(output);console.log(test.failed?"ACCOUNT_BACKUP_FAILED":"ACCOUNT_BACKUP_OK");Qt.quit()})
    else {console.log(test.failed?"ACCOUNT_BACKUP_FAILED":"ACCOUNT_BACKUP_OK");Qt.quit()}
  }}
}
