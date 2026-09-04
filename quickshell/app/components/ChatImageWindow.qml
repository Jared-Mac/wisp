import QtQuick
import QtQuick.Controls
import Quickshell

FloatingWindow {
  id: root
  required property var theme
  required property var bridge
  property string focusId: ""
  property url imageSource: ""
  property string messageId: ""
  property string copyRequest: ""
  property string copyError: ""
  property bool copied: false
  property bool fitToWindow: false
  readonly property real pixelRatio: Math.max(1, picture.Screen.devicePixelRatio)
  readonly property real nativeWidth: Math.max(0,picture.sourceSize.width) / pixelRatio
  readonly property real nativeHeight: Math.max(0,picture.sourceSize.height) / pixelRatio
  readonly property real imageScale: fitToWindow ? Math.min(1, viewport.width / Math.max(1,nativeWidth), viewport.height / Math.max(1,nativeHeight)) : 1
  readonly property alias imageStatus: picture.status
  visible: false
  title: "Image — Wisp"
  color: theme.background
  implicitWidth: theme.space(800)
  implicitHeight: theme.space(600)
  minimumSize: Qt.size(theme.space(280),theme.space(220))
  Component.onCompleted: focusId="image-"+(++bridge.imageViewerSerial)
  Component.onDestruction: if (bridge && focusId) bridge.setImageViewerFocus(focusId,false)
  function updateFocus() {
    if (focusId) bridge.setImageViewerFocus(focusId,visible && !!contentItem.Window.window && contentItem.Window.window.active)
  }
  onVisibleChanged: updateFocus()
  Connections { target:root.contentItem.Window.window; function onActiveChanged() { root.updateFocus() } }
  function openImage(source, id) {
    messageId=id || "";copyRequest="";copyError="";copied=false
    fitToWindow=false
    imageSource=source
    visible=true; minimized=false
    viewport.contentX=0;viewport.contentY=0
    Qt.callLater(function() {
      if (root.contentItem.Window.window) root.contentItem.Window.window.requestActivate()
    })
  }
  function copyImage() {
    if(copyRequest || !messageId || picture.status!==Image.Ready)return
    copied=false;copyError=""
    copyRequest=bridge.copyChatImage(messageId)
    if(!copyRequest)copyError="Wisp is disconnected. Reconnect and try again."
  }
  Connections {
    target:root.bridge
    function onImageCopyFinished(request,success,error) {
      if(request!==root.copyRequest)return
      root.copyRequest="";root.copied=success;root.copyError=success?"":error
      if(success)copiedTimer.restart()
    }
    function onDaemonConnectedChanged() {
      if(root.copyRequest && !root.bridge.daemonConnected) {root.copyRequest="";root.copyError="Connection lost. Try copying again."}
    }
  }
  Timer { id:copiedTimer;interval:2500;onTriggered:root.copied=false }
  function sizeToImage() {
    if (picture.status!==Image.Ready) return
    root.implicitWidth=Math.min(Math.max(root.theme.space(280),nativeWidth+root.theme.space(24)),root.screen ? root.screen.width*0.9 : root.theme.space(1000))
    root.implicitHeight=Math.min(Math.max(root.theme.space(220),nativeHeight+toolbar.height+root.theme.space(24)),root.screen ? root.screen.height*0.85 : root.theme.space(800))
  }
  onClosed: { visible=false;imageSource="" }
  Rectangle {
    anchors.fill: parent; color:root.theme.background
    SurfaceOutline { theme:root.theme }
    Row {
      id:toolbar
      x:root.theme.spacing.sm;y:root.theme.spacing.sm
      height:root.theme.space(34);spacing:root.theme.spacing.sm
      ChatButton { objectName:"imageNativeSize";theme:root.theme;text:"100%";primary:!root.fitToWindow;onClicked:root.fitToWindow=false }
      ChatButton { objectName:"imageFitWindow";theme:root.theme;text:"Fit";primary:root.fitToWindow;onClicked:root.fitToWindow=true }
      ChatButton {
        objectName:"imageCopyButton";theme:root.theme
        implicitWidth:root.theme.space(32)
        enabled:picture.status===Image.Ready && !!root.messageId && !root.copyRequest
        Accessible.name:"Copy image to clipboard"
        ToolTip.visible:hovered;ToolTip.text:root.copied?"Copied":root.copyRequest?"Copying…":"Copy to Clipboard"
        onClicked:root.copyImage()
        contentItem:Image {
          source:"data:image/svg+xml,"+encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="'+root.theme.foreground+'" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">'+(root.copied?'<path d="M5 12l4 4L19 6"/>':'<rect x="8" y="8" width="12" height="12" rx="1"/><path d="M16 8V4H4v12h4"/>')+'</svg>')
          fillMode:Image.Pad;opacity:parent.enabled?1:0.4
        }
      }
      Text {
        anchors.verticalCenter:parent.verticalCenter
        text:picture.status===Image.Ready ? picture.sourceSize.width+" × "+picture.sourceSize.height : ""
        color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption
      }
    }
    Flickable {
      id:viewport;objectName:"imageViewerViewport"
      anchors {left:parent.left;right:parent.right;top:toolbar.bottom;bottom:parent.bottom;margins:root.theme.spacing.sm}
      clip:true;boundsBehavior:Flickable.StopAtBounds
      contentWidth:Math.max(width,picture.width);contentHeight:Math.max(height,picture.height)
      ScrollBar.horizontal:ScrollBar {}
      ScrollBar.vertical:ScrollBar {}
      Image {
        id:picture;objectName:"nativeChatImage"
        source:root.imageSource;asynchronous:true
        // Decode the original file, never a thumbnail. At 100%, one source pixel
        // maps to one display pixel; oversized images are panned, not resampled.
        width:root.nativeWidth*root.imageScale;height:root.nativeHeight*root.imageScale
        x:Math.max(0,(viewport.width-width)/2);y:Math.max(0,(viewport.height-height)/2)
        fillMode:Image.PreserveAspectFit
        onStatusChanged:if(status===Image.Ready)Qt.callLater(root.sizeToImage)
        onSourceSizeChanged:if(status===Image.Ready)Qt.callLater(root.sizeToImage)
      }
    }
    Text {
      anchors.centerIn:viewport
      visible:picture.status!==Image.Ready
      text:picture.status===Image.Error?"Image unavailable":"Loading image…"
      color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.body
    }
    Label {
      visible:!!root.copyError
      anchors {left:parent.left;right:parent.right;bottom:parent.bottom;margins:root.theme.spacing.sm}
      text:root.copyError;wrapMode:Text.Wrap;padding:root.theme.spacing.sm
      color:root.theme.danger;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption
      background:Rectangle {color:root.theme.surface;border.color:root.theme.danger}
    }
    Shortcut { sequence:"Escape";enabled:root.visible;onActivated:{root.visible=false;root.imageSource=""} }
  }
}
