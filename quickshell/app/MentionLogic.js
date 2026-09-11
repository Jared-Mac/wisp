function token(name) {
  name=String(name)
  return /^[A-Za-z0-9_]+(?:[.-][A-Za-z0-9_]+)*$/.test(name)
    ? "@"+name : '@"'+name.replace(/\\/g,"\\\\").replace(/"/g,'\\"')+'"'
}

function query(text, cursor) {
  var before=String(text).slice(0,cursor)
  var match=/(?:^|[\s([{>])(@(?:"(?:[^"\\\n]|\\.)*|[A-Za-z0-9_.-]*))$/.exec(before)
  if (!match) return null
  var value=match[1].slice(1)
  var quoted=value[0]==='"', suffix=String(text).slice(cursor)
  var rest=quoted ? /^(?:[^"\\\n]|\\.)*"/.exec(suffix) : /^[A-Za-z0-9_.-]*/.exec(suffix)
  if (quoted) value=value.slice(1).replace(/\\(["\\])/g,"$1")
  return {start:cursor-match[1].length,end:cursor+(rest ? rest[0].length : 0),text:value}
}

function suggestions(people, query) {
  if (!query) return []
  var seen={}, needle=query.text.toLowerCase()
  return (people || []).filter(function(person) {
    var name=String(person.display_name || ""), key=name.toLowerCase()
    if (!name || seen[key] || key.indexOf(needle)<0) return false
    seen[key]=true; return true
  }).sort(function(a,b) {
    var an=a.display_name.toLowerCase(),bn=b.display_name.toLowerCase()
    return Number(bn.indexOf(needle)===0)-Number(an.indexOf(needle)===0) || an.localeCompare(bn)
  }).slice(0,8)
}
