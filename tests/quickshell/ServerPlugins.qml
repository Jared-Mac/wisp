import QtQuick
import QtQuick.Controls
import Quickshell
import "app" as Wisp
ShellRoot {
  id: test
  property int ticks: 0
  property bool clicked: false
  function find(item,name) {
    if(!item)return null
    if(item.objectName===name)return item
    for(var child of item.children || []){var found=find(child,name);if(found)return found}
    return null
  }
  Wisp.WispTheme {id:theme;profile:"soft_graphite"}
  Wisp.WispBridge {id:bridge}
  FloatingWindow {id:window;visible:true;implicitWidth:1000;implicitHeight:800;color:theme.background
    Wisp.WispContent {id:page;anchors.fill:parent;bridge:bridge;theme:theme;presentation:"app";logoSource:Qt.resolvedUrl("app/assets/waveform.svg")}
  }
  Timer {interval:100;repeat:true;running:true;onTriggered:{
    test.ticks++
    if(test.ticks>150){console.error("SERVER_PLUGINS_FAILED: shortcut did not open editor: "+bridge.plugins.feedback);Qt.quit();return}
    if(!test.clicked){
      var shortcut=test.find(page,"serverPluginShortcut")
      if(!shortcut || !shortcut.visible || !shortcut.enabled)return
      test.clicked=true;shortcut.clicked()
      return
    }
    if(bridge.plugins.view && bridge.plugins.view.title==="Valheim Modpacks" && bridge.plugins.serverMode && page.currentPage==="plugins"){
      console.log("SERVER_PLUGINS_OK: main shortcut prepared and opened editor on first click")
      Qt.quit()
    }
  }}
}
