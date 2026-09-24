var wispEveryday = [
  {name:"wink", keywords:"playful cheeky"},
  {name:"cool", keywords:"sunglasses chill"},
  {name:"think", keywords:"thinking hmm curious"},
  {name:"blush", keywords:"shy embarrassed"},
  {name:"sweat", keywords:"nervous awkward oops"},
  {name:"facepalm", keywords:"oops disbelief"},
  {name:"peek", keywords:"lurk watching hiding"},
  {name:"party", keywords:"celebrate celebration confetti"},
  {name:"coffee", keywords:"break cozy tired"},
  {name:"popcorn", keywords:"movie watching snack"},
  {name:"salute", keywords:"respect thanks"},
  {name:"shrug", keywords:"unsure dunno whatever"}
]
var wispMoments = [
  {name:"hug", keywords:"love care comfort thanks"},
  {name:"music", keywords:"listening headphones song"},
  {name:"gaming", keywords:"game controller play"},
  {name:"bonk", keywords:"oops silly hammer"},
  {name:"melting", keywords:"overwhelmed hot tired"},
  {name:"comfy", keywords:"blanket cozy relax"}
]
var wispNames = ["smile","laugh","love","cry","angry","wave","sleep","shock","gg","hype","yes","no"]
  .concat(wispEveryday.concat(wispMoments).map(function(item){return item.name}))
var standard = ["😀","😂","🥹","😍","🤔","😭","😎","😴","👍","👎","❤️","🔥","🎉","👀","✅","💯","🙌","🙏","👋","✨","💀","🚀","🍿","☕"]
function escape(value) { return String(value).replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;").replace(/'/g,"&#39;") }
function safeLink(url) { return /^https?:\/\/[^\s<>"'\\]+$/i.test(String(url)) }
function trimLink(value) {
  var link=value.replace(/[.,!?;:]+$/g,"")
  while (/\)$/.test(link) && (link.match(/\)/g)||[]).length > (link.match(/\(/g)||[]).length) link=link.slice(0,-1)
  return link
}
function parts(value) {
  var result=[], at=0, text=String(value), re=/(?:https?:\/\/|www\.)[^\s<>"']+|wisp\.you\/[a-z]+(?:-[a-z]+){0,3}[0-9]{12}|:wisp_[a-z]+:|:e_[0-9a-f-]{36}:|@"(?:[^"\\\n]|\\["\\])+"|@[A-Za-z0-9_]+(?:[.-][A-Za-z0-9_]+)*/gi, match
  while ((match=re.exec(text)) !== null) {
    if (match[0][0]==="@" && match.index>0 && !/[\s([{>]/.test(text[match.index-1])) continue
    if (match.index>at) result.push({text:text.slice(at,match.index)})
    var raw=match[0]
    if (raw[0] === "@") result.push({text:raw,mention:raw[1]==='"' ? raw.slice(2,-1).replace(/\\(["\\])/g,"$1") : raw.slice(1)})
    else if (raw[0] === ":") result.push({text:raw,emoji:raw})
    else {
      var url=trimLink(raw), href=/^(?:www\.|wisp\.you\/)/i.test(url)?"https://"+url:url
      result.push(safeLink(href)?{text:url,href:href}:{text:url})
      if (url.length<raw.length) result.push({text:raw.slice(url.length)})
    }
    at=match.index+raw.length
  }
  if(at<text.length) result.push({text:text.slice(at)})
  return result
}
function richText(text, emojiUrl, size, color, mentionKnown) {
  return parts(text).map(function(part) {
    if(part.href) return '<a href="'+escape(part.href)+'" style="color:'+escape(color || "#67baff")+'">'+escape(part.text)+'</a>'
    if(part.mention && mentionKnown && mentionKnown(part.mention)) return '<span style="color:'+escape(color || "#67baff")+'"><b>'+escape("@"+part.mention)+'</b></span>'
    var source=part.emoji ? emojiUrl(part.emoji) : ""
    if(source) return '<img src="'+escape(source)+'" width="'+size+'" height="'+size+'" alt="'+escape(part.text)+'" />'
    return escape(part.text).replace(/\n/g,"<br>")
  }).join("")
}
function mentions(text, name) {
  return !!name && parts(text).some(function(part) { return part.mention && part.mention.toLowerCase()===String(name).toLowerCase() })
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
