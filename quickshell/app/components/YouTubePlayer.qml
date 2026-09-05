import QtQuick
import QtWebEngine
import "../ChatMarkup.js" as Markup

WebEngineView {
  id:root
  required property string videoId
  property string errorText:""
  property int playbackState:-1
  backgroundColor:"#000000"
  profile:WebEngineProfile {offTheRecord:true}
  settings.playbackRequiresUserGesture:true
  settings.localContentCanAccessFileUrls:false
  settings.localContentCanAccessRemoteUrls:false
  settings.javascriptCanAccessClipboard:false
  settings.fullScreenSupportEnabled:false
  settings.pluginsEnabled:false
  settings.screenCaptureEnabled:false
  settings.allowRunningInsecureContent:false
  function loadVideo() {
    if(!/^[A-Za-z0-9_-]{11}$/.test(videoId))return
    // dev.wisp is the desktop application ID; the base URL identifies this
    // WebView to YouTube without exposing the user's chat or server address.
    loadHtml('<!doctype html><html><head><meta name="referrer" content="strict-origin-when-cross-origin"><style>html,body,iframe{margin:0;padding:0;width:100%;height:100%;border:0;background:#000;overflow:hidden}</style></head><body><iframe id="player" title="YouTube video" src="https://www.youtube-nocookie.com/embed/'+videoId+'?enablejsapi=1&amp;playsinline=1&amp;origin=https%3A%2F%2Fdev.wisp" referrerpolicy="strict-origin-when-cross-origin" allow="encrypted-media; picture-in-picture" allowfullscreen></iframe><script>function onYouTubeIframeAPIReady(){new YT.Player("player",{events:{onStateChange:function(e){console.log("WISP_YOUTUBE_STATE:"+e.data)},onError:function(e){console.log("WISP_YOUTUBE_ERROR:"+e.data)}}})}</script><script src="https://www.youtube.com/iframe_api"></script></body></html>',"https://dev.wisp/")
  }
  Component.onCompleted:loadVideo()
  onJavaScriptConsoleMessage:(level,message)=>{
    if(/^WISP_YOUTUBE_STATE:-?[0-9]+$/.test(message))playbackState=Number(message.split(":")[1])
    if(/^WISP_YOUTUBE_ERROR:[0-9]+$/.test(message))errorText="YouTube could not play this video here. Open it in your browser."
  }
  onPermissionRequested:permission=>permission.deny()
  onFileDialogRequested:request=>request.reject()
  onCertificateError:error=>error.rejectCertificate()
  onNewWindowRequested:request=>{if(request.userInitiated && Markup.safeLink(request.requestedUrl))Qt.openUrlExternally(request.requestedUrl)}
  onNavigationRequested:request=>{
    if(!request.isMainFrame)return
    var url=String(request.url)
    if(url==="https://dev.wisp/" || url==="about:blank" || url.indexOf("data:text/html")===0)return
    request.reject()
    if(request.navigationType===WebEngineNavigationRequest.LinkClickedNavigation && Markup.safeLink(url))Qt.openUrlExternally(url)
  }
  onLoadingChanged:info=>{if(info.status===WebEngineView.LoadFailedStatus)errorText="Video could not load. Try opening it in your browser."}
  onRenderProcessTerminated:()=>{errorText="Video player stopped. Close it and try again."}
}
