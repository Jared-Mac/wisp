import QtQuick
import QtQuick.Controls

ListView {
  id: root
  required property var bridge
  required property var theme
  required property var people
  property bool compact: false
  // Keep a person's open menu and keyboard focus when a request changes state.
  ListModel {id:stable;dynamicRoles:true}
  function sync() {
    var values=people || []
    for(var i=0;i<values.length;i++) {
      var key=String(values[i].server_id)+":"+String(values[i].id),found=-1
      for(var j=i;j<stable.count;j++)if(stable.get(j).key===key){found=j;break}
      if(found<0)stable.insert(i,{key:key,modelData:values[i]})
      else {
        if(found!==i)stable.move(found,i,1)
        if(JSON.stringify(stable.get(i).modelData)!==JSON.stringify(values[i]))stable.set(i,{key:key,modelData:values[i]})
      }
    }
    if(stable.count>values.length)stable.remove(values.length,stable.count-values.length)
  }
  onPeopleChanged:sync()
  Component.onCompleted:sync()
  model:stable;clip:true;boundsBehavior:Flickable.StopAtBounds
  ScrollBar.vertical:ScrollBar {}
  delegate:ServerMemberRow {required property var modelData;width:root.width;bridge:root.bridge;theme:root.theme;person:modelData;compact:root.compact}
}
