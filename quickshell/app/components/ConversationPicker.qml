import QtQuick
import QtQuick.Controls

Popup {
  id: root
  required property var bridge
  required property var theme
  property string selectedId: ""
  property alias query: search.text
  property string category: ""
  signal chosen(string conversationId)
  signal newChatRequested()
  readonly property var rows: {
    var term=query.trim().toLocaleLowerCase()
    var groups=[{label:"Rooms",items:[]},{label:"Friends List",items:[]}]
    bridge.conversations.forEach(function(c) {
      var label=String(c.label === "Hangout" ? "Room" : c.label || "Messages")
      if (term && label.toLocaleLowerCase().indexOf(term)<0) return
      var group=c.kind === "direct" || c.kind === "circle" ? 1 : 0
      if (category && category!==groups[group].label) return
      groups[group].items.push({section:false,id:String(c.id),label:label,closed:!!c.tab_closed,unread:Number(c.unread_count || 0)})
    })
    var result=[]
    groups.forEach(function(g) {
      if (!g.items.length) return
      g.items.sort(function(a,b) { return a.label.localeCompare(b.label) || a.id.localeCompare(b.id) })
      result.push({section:true,label:g.label})
      result=result.concat(g.items)
    })
    return result
  }
  readonly property int count: rows.filter(function(r) { return !r.section }).length
  padding: theme.space(8)
  margins: theme.space(8)
  height: Math.min(theme.space(420), Math.max(theme.space(120), (Overlay.overlay ? Overlay.overlay.height : 600)-theme.space(24)))
  focus: true
  closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
  background: Rectangle { color: root.theme.surface; radius: root.theme.cornerRadius; border.width: 1; border.color: root.theme.muted }
  function resetSelection() {
    var index=-1
    for (var i=0;i<rows.length;i++) {
      if (rows[i].section) continue
      if (index<0) index=i
      if (!query && rows[i].id===selectedId) { index=i; break }
    }
    results.currentIndex=index
    if (index>=0) results.positionViewAtIndex(index,ListView.Contain)
  }
  function moveSelection(delta) {
    var index=results.currentIndex
    for (var i=0;i<rows.length;i++) {
      index=(index+delta+rows.length)%rows.length
      if (!rows[index].section) { results.currentIndex=index; results.positionViewAtIndex(index,ListView.Contain); return }
    }
  }
  function chooseCurrent() {
    var row=rows[results.currentIndex]
    if (row && !row.section) { chosen(row.id); close() }
  }
  onRowsChanged: Qt.callLater(resetSelection)
  onOpened: { query=""; category=""; resetSelection(); search.forceActiveFocus() }
  contentItem: Item {
    TextField {
      id: search
      objectName: "conversationSearch"
      property bool wispTextEditor: true
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
      height: root.theme.space(34)
      placeholderText: "Search chats…"
      Accessible.name: "Search chats"
      font.family: root.theme.font.family; font.pixelSize: root.theme.font.body
      color: root.theme.foreground; placeholderTextColor: root.theme.muted
      selectByMouse: true
      background: Rectangle { color: root.theme.background; radius: root.theme.cornerRadius; border.width: 1; border.color: search.activeFocus ? root.theme.accent : root.theme.separator }
      Keys.onDownPressed: root.moveSelection(1)
      Keys.onUpPressed: root.moveSelection(-1)
      Keys.onReturnPressed: root.chooseCurrent()
      Keys.onEnterPressed: root.chooseCurrent()
    }
    Row {
      id: categories
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: search.bottom
      anchors.topMargin: root.theme.spacing.sm
      spacing: root.theme.spacing.xs
      Repeater {
        model: [{label:"All",key:"",weight:0.24},{label:"Rooms",key:"Rooms",weight:0.3},{label:"Friends List",key:"Friends List",weight:0.46}]
        ChatButton {
          required property var modelData
          theme: root.theme; text: modelData.label; primary: root.category===modelData.key
          width: (categories.width-categories.spacing*2)*modelData.weight
          height: root.theme.space(28)
          leftPadding: root.theme.space(4); rightPadding: root.theme.space(4)
          Accessible.name: "Show " + (modelData.key || "all chats")
          onClicked: { root.category=modelData.key; search.forceActiveFocus() }
        }
      }
    }
    ListView {
      id: results
      objectName: "conversationSearchResults"
      anchors.left: parent.left; anchors.right: parent.right; anchors.top: categories.bottom; anchors.bottom: newChat.top
      anchors.bottomMargin: root.theme.spacing.sm
      anchors.topMargin: root.theme.spacing.sm
      clip: true
      model: root.rows
      boundsBehavior: Flickable.StopAtBounds
      ScrollBar.vertical: ScrollBar {}
      delegate: ItemDelegate {
        id: entry
        required property var modelData
        required property int index
        width: results.width
        height: root.theme.space(modelData.section ? 28 : 36)
        enabled: !modelData.section
        highlighted: !modelData.section && results.currentIndex===index
        Accessible.name: modelData.label + (modelData.unread ? ", " + modelData.unread + " unread" : "")
        onClicked: { root.chosen(modelData.id); root.close() }
        background: Rectangle { color: entry.highlighted ? root.theme.alpha(root.theme.accent,0.24) : entry.hovered ? root.theme.alpha(root.theme.foreground,0.08) : "transparent"; radius: root.theme.cornerRadius }
        contentItem: Item {
          Text {
            anchors.left: parent.left; anchors.right: unread.left; anchors.rightMargin: root.theme.spacing.sm; anchors.verticalCenter: parent.verticalCenter
            text: entry.modelData.label + (entry.modelData.closed ? " · closed" : "")
            elide: Text.ElideRight; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
            font.bold: !!entry.modelData.section || entry.modelData.id===root.selectedId
            color: entry.modelData.section ? root.theme.muted : root.theme.foreground
          }
          Text {
            id: unread; anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
            text: entry.modelData.unread ? String(entry.modelData.unread) : ""
            color: root.theme.accent; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
          }
        }
      }
    }
    Text {
      anchors.centerIn: results
      visible: root.count===0
      text: root.query.trim() ? "No matching chats" : "No conversations yet"
      color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption
    }
    ChatButton {
      id:newChat; objectName:"pickerNewChat"
      anchors.left:parent.left; anchors.right:parent.right; anchors.bottom:parent.bottom
      theme:root.theme; text:"+ New chat"; onClicked:{root.close();root.newChatRequested()}
    }
  }
}
