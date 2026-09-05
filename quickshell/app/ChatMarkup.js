var wispNames = ["smile","laugh","love","cry","angry","wave","sleep","shock","gg","hype","yes","no"]
var standard = ["😀","😂","🥹","😍","🤔","😭","😎","😴","👍","👎","❤️","🔥","🎉","👀","✅","💯","🙌","🙏","👋","✨","💀","🚀","🍿","☕"]
function escape(value) { return String(value).replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;").replace(/'/g,"&#39;") }
function safeLink(url) { return /^https?:\/\/[^\s<>"'\\]+$/i.test(String(url)) }
function trimLink(value) {
  var link=value.replace(/[.,!?;:]+$/g,"")
  while (/\)$/.test(link) && (link.match(/\)/g)||[]).length > (link.match(/\(/g)||[]).length) link=link.slice(0,-1)
  return link
}
function parts(value) {
  var result=[], at=0, text=String(value), re=/(?:https?:\/\/|www\.)[^\s<>"']+|:wisp_[a-z]+:|:e_[0-9a-f-]{36}:/gi, match
  while ((match=re.exec(text)) !== null) {
    if (match.index>at) result.push({text:text.slice(at,match.index)})
    var raw=match[0]
    if (raw[0] === ":") result.push({text:raw,emoji:raw})
    else {
      var url=trimLink(raw), href=/^www\./i.test(url)?"https://"+url:url
      result.push(safeLink(href)?{text:url,href:href}:{text:url})
      if (url.length<raw.length) result.push({text:raw.slice(url.length)})
    }
    at=match.index+raw.length
  }
  if(at<text.length) result.push({text:text.slice(at)})
  return result
}
function richText(text, emojiUrl, size, color) {
  return parts(text).map(function(part) {
    if(part.href) return '<a href="'+escape(part.href)+'" style="color:'+escape(color || "#67baff")+'">'+escape(part.text)+'</a>'
    var source=part.emoji ? emojiUrl(part.emoji) : ""
    if(source) return '<img src="'+escape(source)+'" width="'+size+'" height="'+size+'" alt="'+escape(part.text)+'" />'
    return escape(part.text).replace(/\n/g,"<br>")
  }).join("")
}
function youtube(text) {
  var ids=[], found={}
  parts(text).forEach(function(part) {
    if(!part.href) return
    var parsed=/^https?:\/\/(?:www\.|m\.)?(youtube\.com|youtu\.be|youtube-nocookie\.com)(\/[^#]*)(?:#.*)?$/i.exec(part.href)
    if(!parsed) return
    var path=parsed[2], m=parsed[1].toLowerCase()==="youtu.be" ? /^\/([A-Za-z0-9_-]{11})(?:[?\/]|$)/.exec(path)
      : /^\/(?:shorts|embed|live)\/([A-Za-z0-9_-]{11})(?:[?\/]|$)/.exec(path)
    if(!m && /^\/watch\?/.test(path)) m=/(?:\?|&)v=([A-Za-z0-9_-]{11})(?:&|$)/.exec(path)
    if(m && !found[m[1]]) {found[m[1]]=true;ids.push(m[1])}
  })
  return ids
}
