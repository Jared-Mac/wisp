import QtQuick
import QtQuick.Controls
import QtTest
import Quickshell
import "app" as Wisp
import "app/views" as Views
import "app/components" as Components
ShellRoot {
  id:test;property bool failed:false
  function check(ok,label){if(!ok){failed=true;console.error("ACCOUNT_RECOVERY_FAILED: "+label)}}
  function find(item,name,seen){if(!item)return null;seen=seen||[];if(seen.indexOf(item)>=0)return null;seen.push(item);if(item.objectName===name)return item;if(item.contentItem){var r=find(item.contentItem,name,seen);if(r)return r}var children=item.data||item.contentData||item.children||[];for(var i=0;i<children.length;i++){var r=find(children[i],name,seen);if(r)return r}return null}
  Wisp.WispAppearance {id:appearance;environment:"desktop"}
  Wisp.WispTheme {id:theme;appearanceController:appearance;profile:Quickshell.env("WISP_TEST_THEME")||"soft_graphite"}
  Wisp.WispBridge {id:bridge;property var sent:[];function send(name,args){var id="test-"+(++requestId);sent.push({id:id,name:name,args:args});return id}}
  FloatingWindow {id:window;visible:true;implicitWidth:600;implicitHeight:960;color:theme.background
    Rectangle {id:capture;anchors.fill:parent;color:theme.background
    Column {id:canvas;anchors.fill:parent;anchors.margins:20;spacing:10
      Components.ErrorBanner {id:banner;width:parent.width;theme:theme;message:bridge.lastError;onDismissed:bridge.dismissError()}
      Views.ProfileSettingsView {id:settings;width:parent.width;bridge:bridge;theme:theme}
    }
    }
  }
  TestCase {id:input;parent:window.contentItem;when:false}
  Timer {running:true;interval:450;onTriggered:{
    bridge.profileBusy=false;bridge.profileReady=true
    bridge.recoveryEmail={email:null,pending_email:null,verified:false,delivery_available:true}
    var section=test.find(settings,"profileRecoverySection")
    test.check(!!section && !section.expanded,"email settings start grouped")
    section.expanded=true;input.wait(40)
    var email=test.find(settings,"profileRecoveryEmail"),password=test.find(settings,"profileRecoveryPassword"),send=test.find(settings,"profileVerifyEmail")
    email.text="test@example.org";password.text="test private password"
    bridge.lastError="Current password is incorrect.";input.wait(25)
    test.check(banner.visible,"error is shown")
    test.find(banner,"dismissError").clicked();input.wait(25)
    test.check(!banner.visible && bridge.lastError==="","dismiss clears banner immediately")
    test.check(password.text==="test private password" && email.text==="test@example.org","dismiss preserves unsent form fields")
    test.check(send.enabled,"email enrollment is available")
    send.clicked();var last=bridge.sent[bridge.sent.length-1]
    test.check(last.name==="set_recovery_email" && last.args.email==="test@example.org","verified enrollment command")
    test.check(password.text==="" && !JSON.stringify(bridge.requests).includes("test private password"),"password clears on submit and stays out of request metadata")
    bridge.handleLine(JSON.stringify({type:"result",id:last.id,ok:false,error:{message:"Current password is incorrect."}}));input.wait(30)
    test.check(banner.visible && !bridge.profileFeedback,"failure has one dismissible error")
    test.find(banner,"dismissError").clicked()
    var data=JSON.parse(JSON.stringify(bridge.snapshot));data.self.media.error="Test media failure";bridge.applySnapshot(data)
    test.check(bridge.lastError==="Test media failure","new media failure visible")
    bridge.dismissError();bridge.applySnapshot(data);test.check(bridge.lastError==="","same media error stays dismissed through snapshot refresh")
    data.self.media.error="";bridge.applySnapshot(data);data.self.media.error="Test media failure";bridge.applySnapshot(data)
    test.check(bridge.lastError==="Test media failure","recurring media failure is visible again")
    bridge.lastError="This longer error wraps around the available space without overlapping its close button."
    banner.width=220;input.wait(30)
    var dismiss=test.find(banner,"dismissError")
    test.check(dismiss.x>=0 && dismiss.x+dismiss.width<=banner.width && dismiss.y>=0,"close fits a narrow banner")
    if(Quickshell.env("WISP_RECOVERY_SCREENSHOT")) capture.grabToImage(function(image){image.saveToFile(Quickshell.env("WISP_RECOVERY_SCREENSHOT"));console.log(test.failed?"ACCOUNT_RECOVERY_FAILED":"ACCOUNT_RECOVERY_OK");Qt.quit()})
    else {console.log(test.failed?"ACCOUNT_RECOVERY_FAILED":"ACCOUNT_RECOVERY_OK");Qt.quit()}
  }}
}
