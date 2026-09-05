import QtQuick
import Quickshell

Column {
  id:root
  required property var theme
  required property string videoId
  property bool playing:false
  property string errorText:""
  readonly property bool windowVisible:!Window.window || Window.window.visible
  onVisibleChanged:if(!visible)playing=false
  onWindowVisibleChanged:if(!windowVisible)playing=false
  spacing:theme.space(6)
  width:Math.min(parent.width,theme.space(640))
  Flow {
    width:parent.width;spacing:root.theme.space(6)
    Text {text:"YouTube";color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption; height:root.theme.space(28);verticalAlignment:Text.AlignVCenter}
    ChatButton {
      objectName:"playEmbed-"+root.videoId;theme:root.theme;text:root.playing?"close video":"play here"
      onClicked:{
        if(root.playing){root.playing=false;return}
        if(Quickshell.env("WISP_WEB_EMBEDS_READY")!=="1"){root.errorText="Update the Wisp web runtime to play videos here.";return}
        root.errorText="";root.playing=true
      }
    }
    ChatButton {theme:root.theme;text:"browser";onClicked:Qt.openUrlExternally("https://www.youtube.com/watch?v="+root.videoId)}
  }
  Loader {
    id:player;width:parent.width;height:active?Math.max(200,width*9/16):0
    active:root.playing && width>=200
    onActiveChanged:if(active)setSource("YouTubePlayer.qml",{videoId:root.videoId})
    onStatusChanged:if(status===Loader.Error){root.errorText="Video player unavailable. Install Qt WebEngine and restart Wisp.";root.playing=false}
  }
  Text {width:parent.width;wrapMode:Text.WordWrap;visible:text!=="";text:root.errorText || (player.item?player.item.errorText:"") || (root.playing&&root.width<200?"Widen this chat to play the video.":"");color:root.theme.muted;font.family:root.theme.font.family;font.pixelSize:root.theme.font.caption}
}
