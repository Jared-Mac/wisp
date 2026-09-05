import QtQuick
import QtQuick.Controls
import Quickshell
import "../components"
import "../ChatTiles.js" as Tiles

Item {
  id: root
  required property var bridge
  required property var theme
  property bool activityStacked: false
  signal revealMainRequested()
  property var tree: ({key:"pane-0",id:""})
  property string activeKey: "pane-0"
  property int serial: 0
  property bool routing: false
  property bool loaded: false
  property string dragKey: ""
  property string dropKey: ""
  property string dropEdge: ""
  property var dropPlan: null
  property var detachedKeys: []
  property var popoutFocus: ({})
  readonly property var dockTree: {
    var result=Tiles.copy(tree)
    detachedKeys.forEach(function(k) { if (result) result=Tiles.remove(result,k) })
    return result
  }
  readonly property real gap: theme.space(theme.cleanTui ? 6 : 10)
  readonly property real minWidth: theme.space(theme.cleanTui ? 360 : 280)
  readonly property real minHeight: theme.space(230)
  readonly property var minimum: dockTree ? Tiles.minimum(dockTree,gap,minWidth,minHeight) : ({width:0,height:0})
  readonly property var rectangles: dockTree ? Tiles.geometry(dockTree,canvas.contentWidth,canvas.contentHeight,gap,minWidth,minHeight) : ({})
  readonly property int paneCount: panes.count
  Binding { target: root.bridge; property: "mediaTileHost"; value: root }
  function videoFor(id) {
    if (String(id).indexOf("video:") !== 0) return null
    try { return JSON.parse(String(id).slice(6)) } catch (_) { return null }
  }
  function openVideo(video) {
    var id = "video:" + JSON.stringify({participant:String(video.participant),source:String(video.source)})
    var existing = Tiles.leaves(tree).filter(function(n) { return n.id === id })[0]
    if (existing) return
    if (paneCount >= 16) { bridge.lastError = "Close a tile before watching another stream."; bridge.watchVideo(video,false); return }
    // Streaming leaves are ephemeral and never automatically re-watch on launch.
    var leaf = {key:key(),id:id}
    commit(Tiles.insert(Tiles.copy(tree), activeKey, leaf, "right", key()))
    if (!bridge.mainWindowOpen || !bridge.workspaceLayout.streamsAsTiles || paneCount > 8) detach(leaf.key)
  }
  function syncVideos() {
    var next = Tiles.copy(tree)
    Tiles.leaves(tree).forEach(function(n) {
      var video = root.videoFor(n.id)
      if (!video) return
      var present = root.bridge.remoteVideos.some(function(v) { return v.participant === video.participant && v.source === video.source && (v.subscribed || v.surface_open) })
      if (!present) next = Tiles.remove(next, n.key)
    })
    if (!next) next = {key:key(),id:""}
    if (JSON.stringify(next) !== JSON.stringify(tree)) commit(next)
  }
  Binding { target: root.bridge; property: "detachedChatFocused"; value: Object.keys(root.popoutFocus).some(function(k) { return root.popoutFocus[k] }) }
  function detach(nodeKey) {
    if (detachedKeys.indexOf(nodeKey)<0) detachedKeys=detachedKeys.concat([nodeKey])
    activate(nodeKey)
  }
  function attach(nodeKey) {
    detachedKeys=detachedKeys.filter(function(k) { return k!==nodeKey })
    activate(nodeKey)
    revealMainRequested()
  }
  function recordFocus(nodeKey,focused) {
    var next=Object.assign({},popoutFocus); next[nodeKey]=focused; popoutFocus=next
    if (focused) activate(nodeKey)
  }
  function key() { return "tile-" + Date.now() + "-" + (++serial) }
  function reconcile(model, nodes) {
    var keys = nodes.map(function(n) { return n.key })
    for (var i=model.count-1;i>=0;i--) if (keys.indexOf(model.get(i).nodeKey)<0) model.remove(i)
    for (var j=0;j<nodes.length;j++) {
      var found=false
      for (var k=0;k<model.count;k++) if (model.get(k).nodeKey===nodes[j].key) found=true
      if (!found) model.append({nodeKey:nodes[j].key})
    }
  }
  function commit(next) {
    tree=next
    reconcile(panes,Tiles.leaves(tree)); reconcile(dividers,Tiles.splits(tree))
    detachedKeys=detachedKeys.filter(function(k) { return !!Tiles.find(root.tree,k) })
    if (!Tiles.find(tree,activeKey)) activeKey=Tiles.leaves(tree)[0].key
    if (loaded) {
      var saved = Tiles.copy(tree)
      Tiles.leaves(tree).forEach(function(n) { if (root.videoFor(n.id)) saved = saved ? Tiles.remove(saved,n.key) : null })
      bridge.workspaceLayout.chatTiles=JSON.stringify(saved || {key:"pane-0",id:""})
    }
  }
  function initialize() {
    if (loaded || !bridge.workspaceLayout.ready || bridge.conversations.length===0) return
    var next=tree
    try { var saved=JSON.parse(bridge.workspaceLayout.chatTiles); if (Tiles.valid(saved)) next=saved } catch (_) {}
    loaded=true
    commit(next)
    syncConversations()
    if (bridge.activeConversationId) route(bridge.activeConversationId)
    openPendingConversations()
  }
  function openPendingConversations() {
    if (!loaded || !bridge.pendingConversationTiles.length) return
    var pending = bridge.pendingConversationTiles
    bridge.pendingConversationTiles = []
    pending.forEach(function(id) {
      var conversation = root.bridge.conversationById(id)
      if (!conversation) { root.bridge.lastError = "This channel is no longer available."; return }
      id = String(conversation.id)
      var leaves = Tiles.leaves(root.tree)
      var existing = leaves.filter(function(n) { return n.id === id })[0]
      if (existing) {
        root.activate(existing.key)
        root.route(id)
        if (root.detachedKeys.indexOf(existing.key) < 0) root.revealMainRequested()
        return
      }
      var empty = leaves.filter(function(n) { return !n.id })[0]
      if (empty) root.choose(empty.key, id)
      else if (root.paneCount < 8) root.addConversation(root.activeKey, id)
      else {
        root.bridge.lastError = "All 8 tiles are in use. Close a tile before opening another."
        root.revealMainRequested()
        return
      }
      root.revealMainRequested()
    })
  }
  function syncConversations() {
    if (!loaded) { initialize(); return }
    var open=bridge.conversations.filter(function(c) { return !c.tab_closed })
    var next=Tiles.copy(tree), used=Tiles.leaves(next).map(function(n) { return n.id })
    Tiles.leaves(next).forEach(function(n) {
      if (root.videoFor(n.id)) return
      var c=bridge.conversationById(n.id)
      if (c && !c.tab_closed && String(n.id)!==String(c.id)) {
        n.id=String(c.id)
        used.push(n.id)
      } else if (!c || c.tab_closed) {
        var candidate=open.filter(function(c) { return used.indexOf(String(c.id))<0 })[0]
        n.id=candidate ? String(candidate.id) : ""
        used.push(n.id)
      }
    })
    if (JSON.stringify(next)!==JSON.stringify(tree)) commit(next)
  }
  function activate(nodeKey) {
    var node=Tiles.find(tree,nodeKey)
    if (!node || node.a) return
    activeKey=nodeKey
    if (videoFor(node.id)) return
    routing=true
    var conversation=bridge.conversationById(node.id)
    if (node.id && (bridge.activeConversationId!==node.id || (conversation && conversation.tab_closed))) bridge.selectConversation(node.id)
    routing=false
  }
  function choose(nodeKey,id) {
    var next=Tiles.copy(tree), node=Tiles.find(next,nodeKey)
    if (!node) return
    if (videoFor(node.id)) { addConversation(nodeKey,id); return }
    node.id=String(id); commit(next); activate(nodeKey)
  }
  function route(id) {
    if (routing || !id || !loaded) return
    var match=Tiles.leaves(tree).filter(function(n) { return n.id===String(id) })[0]
    if (match) {
      activeKey=match.key
      if (detachedKeys.indexOf(match.key)>=0) {
        for (var i=0;i<panes.count;i++) if (panes.get(i).nodeKey===match.key) {
          var host=paneRepeater.itemAt(i)
          if (host) host.popoutWindow.reveal()
        }
      }
    }
    else {
      var next=Tiles.copy(tree)
      if (videoFor(Tiles.find(next,activeKey).id)) {
        var chat=Tiles.leaves(next).filter(function(n) { return !root.videoFor(n.id) })[0]
        if (!chat) { addConversation(activeKey,id); return }
        activeKey=chat.key
      }
      Tiles.find(next,activeKey).id=String(id); commit(next)
    }
  }
  function split(nodeKey,edge) {
    if (panes.count>=8) return
    var used=Tiles.leaves(tree).map(function(n) { return n.id })
    var other=bridge.conversations.filter(function(c) { return !c.tab_closed && used.indexOf(String(c.id))<0 })[0]
    var leaf={key:key(),id:other?String(other.id):""}
    commit(Tiles.insert(Tiles.copy(tree),nodeKey,leaf,edge,key()))
    activate(leaf.key)
  }
  function addConversation(nodeKey,id) {
    if(panes.count>=8 || !Tiles.find(tree,nodeKey)) return
    var rect=rectangles[nodeKey]
    var edge=rect && rect.width>=theme.space(600)?"right":"bottom"
    var leaf={key:key(),id:String(id)}
    commit(Tiles.insert(Tiles.copy(tree),nodeKey,leaf,edge,key()))
    activate(leaf.key)
  }
  function closePane(nodeKey) {
    var node = Tiles.find(tree,nodeKey), video = node ? videoFor(node.id) : null
    if (panes.count<=1 && !video) return
    if (video) bridge.watchVideo(video,false)
    commit(Tiles.remove(Tiles.copy(tree),nodeKey) || {key:key(),id:""})
    activate(activeKey) // Closing a tile never closes/deletes the conversation.
  }
  function resize(nodeKey,delta) {
    var rect=rectangles[nodeKey], next=Tiles.copy(tree), node=Tiles.find(next,nodeKey)
    if (!rect || !node) return
    node.ratio=Math.max(0.08,Math.min(0.92,(rect.first+delta)/rect.available)); commit(next)
  }
  function move(source,target,edge) { commit(Tiles.move(tree,source,target,edge,key())) }
  function dragAt(source,x,y) {
    dragKey=source; dropKey=""; dropEdge=""
    var point=root.mapToItem(canvas.contentItem,x,y)
    dropPlan=Tiles.planDrop(dockTree,source,point.x,point.y,canvas.contentWidth,canvas.contentHeight,gap,minWidth,minHeight,theme.space(26))
    if (dropPlan) { dropKey=dropPlan.target; dropEdge=dropPlan.edge }
  }
  function finishDrag() {
    if (dragKey && dropKey && dropPlan && !dropPlan.unchanged) move(dragKey,dropKey,dropEdge)
    cancelDrag()
  }
  function cancelDrag() { dragKey=""; dropKey=""; dropEdge=""; dropPlan=null }
  Component.onCompleted: { commit(tree); initialize() }
  Connections {
    target: root.bridge.workspaceLayout
    function onReadyChanged() { root.initialize() }
    function onResetRequested() {
      Tiles.leaves(root.tree).forEach(function(n) { var video=root.videoFor(n.id); if(video) root.bridge.watchVideo(video,false) })
      root.detachedKeys=[]
      root.commit({key:root.key(),id:String(root.bridge.activeConversationId || "")})
      root.syncConversations()
    }
  }
  Connections {
    target: root.bridge
    function onConversationsChanged() { root.syncConversations() }
    function onActiveConversationIdChanged() { root.route(root.bridge.activeConversationId) }
    function onPendingConversationTilesChanged() { root.openPendingConversations() }
    function onMediaWatchReady(video) { root.openVideo(video) }
    function onRemoteVideosChanged() { root.syncVideos() }
    function onMainWindowOpenChanged() {
      if (!root.bridge.mainWindowOpen) Tiles.leaves(root.tree).forEach(function(n) { if (root.videoFor(n.id)) root.detach(n.key) })
    }
  }
  ListModel { id: panes }
  ListModel { id: dividers }
  Flickable {
    id: canvas
    anchors.fill: parent
    contentWidth: Math.max(width,root.minimum.width)
    contentHeight: Math.max(height,root.minimum.height)
    clip: true; boundsBehavior: Flickable.StopAtBounds
    ScrollBar.horizontal: ScrollBar {}
    ScrollBar.vertical: ScrollBar {}
    Repeater {
      id: paneRepeater
      model: panes
      Item {
        id: tileHost
        required property string nodeKey
        required property int index
        readonly property string contentId: { var node=Tiles.find(root.tree,nodeKey); return node ? node.id : "" }
        readonly property var video: root.videoFor(contentId)
        readonly property var rect: root.rectangles[nodeKey] || ({x:0,y:0,width:0,height:0})
        readonly property bool detached: root.detachedKeys.indexOf(nodeKey)>=0
        readonly property bool popoutActive: detached && popout.contentItem.Window.active
        readonly property alias popoutWindow: popout
        readonly property alias workspace: workspace
        onPopoutActiveChanged: root.recordFocus(nodeKey,popoutActive)
        Component.onDestruction: root.recordFocus(nodeKey,false)
        objectName: "chatTileHost-" + nodeKey
        x: rect.x; y: rect.y; width: rect.width; height: rect.height
        FloatingWindow {
          id: popout
          objectName: "chatPopout-" + tileHost.nodeKey
          visible: tileHost.detached
          title: (tileHost.video ? tileHost.video.participant + " · " + tileHost.video.source : workspace.current ? workspace.label(workspace.current) : "Chat") + " — Wisp"
          implicitWidth: root.theme.space(640); implicitHeight: root.theme.space(720)
          minimumSize: Qt.size(root.minWidth,root.minHeight)
          color: root.theme.background
          function reveal() {
            minimized=false
            Qt.callLater(function() { if (popout.contentItem.Window.window) popout.contentItem.Window.window.requestActivate() })
          }
          onClosed: root.attach(tileHost.nodeKey)
          onVisibleChanged: if (visible) reveal()
        }
        ConversationWorkspace {
        id: workspace
        visible: !tileHost.video
        objectName: "chatTile-" + tileHost.nodeKey
        parent: tileHost.detached ? popout.contentItem : tileHost
        anchors.fill: parent
        bridge: root.bridge; theme: root.theme
        tiled: true
        detached: tileHost.detached
        selectedId: { var node=Tiles.find(root.tree,tileHost.nodeKey); return node ? node.id : "" }
        paneActive: root.activeKey===tileHost.nodeKey
        canSplit: !detached && root.paneCount<8
        canClosePane: !detached && root.paneCount>1
        onConversationChosen: function(id) { root.choose(tileHost.nodeKey,id) }
        onActivated: root.activate(tileHost.nodeKey)
        onSplitRequested: function(edge) { root.split(tileHost.nodeKey,edge) }
        onClosePaneRequested: root.closePane(tileHost.nodeKey)
        onPopOutRequested: root.detach(tileHost.nodeKey)
        onDockRequested: root.attach(tileHost.nodeKey)
        onTileDragged: function(px,py) { var p=mapToItem(root,px,py); root.dragAt(tileHost.nodeKey,p.x,p.y) }
        onTileDropped: root.finishDrag()
        onTileDragCanceled: root.cancelDrag()
        }
        Loader {
          active: !!tileHost.video
          parent: tileHost.detached ? popout.contentItem : tileHost
          anchors.fill: parent
          sourceComponent: RemoteVideoTile {
            bridge: root.bridge; theme: root.theme; video: tileHost.video; detached: tileHost.detached
            onPopOutRequested: root.detach(tileHost.nodeKey)
            onDockRequested: root.attach(tileHost.nodeKey)
            onCloseRequested: root.closePane(tileHost.nodeKey)
            onTileDragged: function(px,py) { var p=mapToItem(root,px,py); root.dragAt(tileHost.nodeKey,p.x,p.y) }
            onTileDropped: root.finishDrag()
            onTileDragCanceled: root.cancelDrag()
          }
        }
      }
    }
    Repeater {
      model: dividers
      ResizeHandle {
        required property string nodeKey
        readonly property var rect: root.rectangles[nodeKey] || ({x:0,y:0,width:0,height:0})
        objectName: "chatDivider-" + nodeKey
        visible: !!root.rectangles[nodeKey]
        theme: root.theme
        verticalLine: { var n=Tiles.find(root.tree,nodeKey); return n ? n.axis==="x" : true }
        x:rect.x; y:rect.y; width:rect.width; height:rect.height
        onMoved: function(delta) { root.resize(nodeKey,delta) }
        onResetRequested: { var next=Tiles.copy(root.tree); Tiles.find(next,nodeKey).ratio=0.5; root.commit(next) }
      }
    }
    Rectangle {
      readonly property var rect: root.dropPlan ? root.dropPlan.rect : ({x:0,y:0,width:0,height:0})
      visible: !!root.dropPlan
      z:100
      x:rect.x; y:rect.y; width:rect.width; height:rect.height
      color:root.theme.alpha(root.theme.accent,0.2); border.color:root.theme.accent; border.width:2
      Text { anchors.centerIn:parent; text:root.dropPlan && root.dropPlan.unchanged?"Already here":root.dropEdge==="center"?"Swap chats":root.dropPlan && root.dropPlan.whole?"Move to workspace "+root.dropEdge:"Move here"; color:root.theme.foreground; font.family:root.theme.font.family; font.pixelSize:root.theme.font.caption }
    }
    Column {
      visible: !root.dockTree
      anchors.centerIn: parent
      spacing: root.theme.spacing.lg
      Text { text: "Your chats are in separate windows."; color: root.theme.muted; font.family: root.theme.font.family; font.pixelSize: root.theme.font.caption }
      ChatButton { theme: root.theme; text: "Return all chats"; onClicked: { root.detachedKeys=[]; root.revealMainRequested() } }
    }
  }
}
