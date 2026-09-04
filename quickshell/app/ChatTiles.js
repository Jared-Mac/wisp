// Pure split-tree operations shared by the main window and isolated tests.
function copy(tree) { return JSON.parse(JSON.stringify(tree)) }
function leaves(tree) { return tree.a ? leaves(tree.a).concat(leaves(tree.b)) : [tree] }
function splits(tree) { return tree.a ? [tree].concat(splits(tree.a), splits(tree.b)) : [] }
function find(tree, key) { return tree.key === key ? tree : tree.a ? find(tree.a, key) || find(tree.b, key) : null }
function replace(tree, key, value) {
    if (tree.key === key) return value
    if (tree.a) { tree.a = replace(tree.a, key, value); tree.b = replace(tree.b, key, value) }
    return tree
}
function remove(tree, key) {
    if (tree.key === key) return null
    if (!tree.a) return tree
    tree.a = remove(tree.a, key); tree.b = remove(tree.b, key)
    return tree.a && tree.b ? tree : tree.a || tree.b
}
function insert(tree, target, leaf, edge, splitKey) {
    var old = find(tree, target)
    if (!old) return tree
    var before = edge === "left" || edge === "top"
    return replace(tree, target, {key:splitKey, axis:edge === "left" || edge === "right" ? "x" : "y", ratio:0.5,
        a:before ? leaf : old, b:before ? old : leaf})
}
function move(tree, source, target, edge, splitKey) {
    var original = tree
    tree = copy(tree)
    var from = find(tree, source), to = find(tree, target)
    if (!from || !to || from.a || source === target || (edge === "center" && to.a)) return tree
    if (edge === "center") {
        var id = from.id; from.id = to.id; to.id = id
        return tree
    }
    tree = remove(tree, source)
    if (!tree || !find(tree,target)) return copy(original)
    tree = insert(tree, target, from, edge, splitKey)
    // Dropping an adjacent tile back in the same slot must not reset its ratio.
    return topology(tree) === topology(original) ? copy(original) : tree
}
function topology(tree) { return tree.a ? tree.axis+"("+topology(tree.a)+","+topology(tree.b)+")" : tree.key }
function minimum(tree, gap, minWidth, minHeight) {
    if (!tree.a) return {width:minWidth,height:minHeight}
    var a = minimum(tree.a,gap,minWidth,minHeight), b = minimum(tree.b,gap,minWidth,minHeight)
    return tree.axis === "x" ? {width:a.width+b.width+gap,height:Math.max(a.height,b.height)}
        : {width:Math.max(a.width,b.width),height:a.height+b.height+gap}
}
function geometry(tree, width, height, gap, minWidth, minHeight) {
    var result = {}
    function visit(node,x,y,w,h) {
        if (!node.a) { result[node.key] = {x:x,y:y,width:w,height:h}; return }
        var horizontal = node.axis === "x"
        var available = (horizontal ? w : h)-gap
        var am = minimum(node.a,gap,minWidth,minHeight), bm = minimum(node.b,gap,minWidth,minHeight)
        var first = Math.max(horizontal?am.width:am.height, Math.min(available-(horizontal?bm.width:bm.height), available*node.ratio))
        result[node.key] = {x:x+(horizontal?first:0),y:y+(horizontal?0:first),width:horizontal?gap:w,height:horizontal?h:gap,
            available:available,first:first}
        visit(node.a,x,y,horizontal?first:w,horizontal?h:first)
        visit(node.b,x+(horizontal?first+gap:0),y+(horizontal?0:first+gap),horizontal?available-first:w,horizontal?h:available-first)
    }
    visit(tree,0,0,width,height)
    return result
}
function dropEdge(x,y,width,height) {
    var distances=[x,width-x,y,height-y], closest=Math.min.apply(null,distances)
    if (closest > Math.min(90,width*0.28,height*0.28)) return "center"
    return ["left","right","top","bottom"][distances.indexOf(closest)]
}
function planDrop(tree,source,x,y,width,height,gap,minWidth,minHeight,edgeBand) {
    if (!tree || !find(tree,source) || x<0 || y<0 || x>width || y>height) return null
    var remaining=remove(copy(tree),source)
    if (!remaining) return null
    var current=geometry(tree,width,height,gap,minWidth,minHeight)
    var distances=[x,width-x,y,height-y], closest=Math.min.apply(null,distances)
    var edge="", target="", whole=closest<=edgeBand
    if (whole) {
        edge=["left","right","top","bottom"][distances.indexOf(closest)]
        target=remaining.key
    } else {
        leaves(tree).some(function(n) {
            var r=current[n.key]
            if (x<r.x || x>r.x+r.width || y<r.y || y>r.y+r.height) return false
            edge=dropEdge(x-r.x,y-r.y,r.width,r.height)
            target=n.key===source ? (edge!=="center" ? remaining.key : "") : n.key
            return true
        })
    }
    if (!target) return null
    // Preview the final geometry after removing the source, not half the old target.
    var next=move(tree,source,target,edge,"preview-split")
    var min=minimum(next,gap,minWidth,minHeight)
    var after=geometry(next,Math.max(width,min.width),Math.max(height,min.height),gap,minWidth,minHeight)
    var unchanged=topology(next)===topology(tree) && edge!=="center"
    return {target:target,edge:edge,whole:whole,unchanged:unchanged,
        rect:after[edge==="center"?target:source]}
}
function valid(tree) {
    var keys = {}, count = 0
    function check(n,depth) {
        if (!n || depth>8 || typeof n.key!=="string" || !n.key || keys[n.key]) return false
        keys[n.key]=true
        if (n.a || n.b) return (n.axis==="x" || n.axis==="y") && typeof n.ratio==="number" && isFinite(n.ratio) && n.ratio>=0.08 && n.ratio<=0.92 && check(n.a,depth+1) && check(n.b,depth+1)
        count++
        return typeof n.id==="string" && count<=8
    }
    return check(tree,0)
}
