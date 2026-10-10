import QtQuick
import QtTest
import Quickshell
import "app" as Wisp
import "app/components" as Components
ShellRoot {
  id:test
  property bool failed:false
  function check(ok,label){if(!ok){failed=true;console.error("PICKER_LIFECYCLE_FAILED: "+label)}}
  function find(item,name){if(!item)return null;if(item.objectName===name)return item;for(var child of item.children || []){var result=find(child,name);if(result)return result}return null}
  Wisp.WispTheme {id:testTheme;profile:"soft_graphite"}
  QtObject {id:testBridge;property var listedConversations:[]}
  Component {id:pickerComponent;Components.ConversationPicker {theme:testTheme;bridge:testBridge}}
  FloatingWindow {id:window;visible:true;implicitWidth:800;implicitHeight:700;color:testTheme.background;Item {id:host;anchors.fill:parent}}
  TestCase {id:input;parent:host;when:false}
  Timer {running:true;interval:200;onTriggered:{
    for(var cycle=0;cycle<12;cycle++) {
      testBridge.listedConversations=[]
      var picker=pickerComponent.createObject(host,{selectedId:"local::dm-5",width:380}),chats=[]
      for(var i=0;i<20;i++)chats.push({id:"local::dm-"+i,server_id:"local",server_name:"Test server",kind:"direct",label:"Friend "+i,unread_count:i%3})
      testBridge.listedConversations=chats;input.wait(30)
      var list=test.find(picker.contentItem,"conversationSearchResults")
      test.check(list.count===0,"hidden picker keeps its view empty during startup snapshots")
      test.check(picker.count===20,"search data remains available without creating hidden delegates")
      if(cycle%2) {
        picker.open();input.wait(40)
        test.check(list.count>20 && picker.rows[list.currentIndex].id==="local::dm-5","opening populates results and selects the current chat")
        picker.query="Friend 12";input.wait(30)
        test.check(picker.count===1 && picker.rows[list.currentIndex].id==="local::dm-12","search positions the matching result")
        picker.close();input.wait(40)
        test.check(list.count===0,"closing releases list delegates before popup destruction")
        picker.open();input.wait(30)
        test.check(picker.count===20,"reopening resets search")
        picker.close();input.wait(30)
      }
      // Saved-layout restoration destroys old tile-owned pickers, including
      // pickers that have never been exposed to a window.
      picker.destroy();input.wait(40)
    }
    console.log(test.failed?"PICKER_LIFECYCLE_FAILED":"PICKER_LIFECYCLE_OK");Qt.quit()
  }}
}
