const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const activity = {};
vm.createContext(activity);
vm.runInContext(fs.readFileSync(path.join(__dirname, '../quickshell/app/RoomActivity.js'), 'utf8'), activity);
const copy = value => JSON.parse(JSON.stringify(value));
const options = {enabled:true, homes:['home'], timing:'not_in_voice', onlyEmpty:false, cooldown:5};
const now = 1_000_000;
const person = id => ({id, display_name:id});
function snapshot(members, id='home') {
  return {self:{id:'me'}, voice_server_id:'home', selected_server_id:'home', server_states:[{
    server:{id,name:'Home',connected:true}, self:{id:'me'}, friends:['friend','second'].map(person),
    spots:[{id:'lounge',name:'Lounge',active_hangout_id:members.length ? 'voice' : null,members:members.map(person)}],
    conversations:[{id:'chat',spot_id:'lounge'}]
  }]};
}
const before=snapshot([]), after=snapshot(['friend']);
const joins=(a=before,b=after,opts=options,event='hangout_changed',last={}) => copy(activity.joins(a,b,event,opts,now,last));
assert.equal(joins().length,1);
assert.deepEqual(joins()[0].names,['friend']);
assert.equal(joins()[0].conversationId,'home::chat');
for (const event of ['snapshot','server_reconnected','presence_changed','friend_added','message_created']) assert.deepEqual(joins(before,after,options,event),[]);
assert.deepEqual(joins(null),[]);
assert.deepEqual(joins(after,after),[]);
assert.deepEqual(joins(before,snapshot(['me','stranger'])),[]);
assert.deepEqual(joins(before,after,{...options,enabled:false}),[]);
assert.deepEqual(joins(before,after,{...options,homes:['other']}),[]);
assert.equal(joins(before,after,{...options,homes:['other','home']}).length,1);
let changed=copy(before); changed.server_states[0].friends=[];
assert.deepEqual(joins(changed),[],'A newly accepted friendship must not replay room membership');
changed=copy(before); changed.server_states[0].server.connected=false;
assert.deepEqual(joins(changed),[],'Reconnection must not replay joins');
changed=copy(after); changed.server_states[0].conversations=[];
assert.deepEqual(joins(before,changed),[],'No alert may expose an inaccessible chat');
changed=copy(after); changed.server_states[0].self.id='another-account';
assert.deepEqual(joins(before,changed),[],'Switching accounts must not replay joins');
changed=copy(after); changed.self.hangout_id='voice';
assert.deepEqual(joins(before,changed),[]);
assert.deepEqual(joins(before,changed,{...options,timing:'always'}),[],'Your own room is always excluded');
changed.self.hangout_id='different-room';
assert.equal(joins(before,changed,{...options,timing:'always'}).length,1);
assert.deepEqual(joins(before,after,{...options,timing:'background',focused:true}),[]);
assert.equal(joins(before,after,{...options,timing:'background',focused:false}).length,1);
assert.deepEqual(joins(snapshot(['stranger']),snapshot(['stranger','friend']),{...options,onlyEmpty:true}),[]);
assert.deepEqual(joins(before,snapshot(['friend','second']))[0].names,['friend','second']);
const last={[joins()[0].key]:now-60_000};
assert.deepEqual(joins(before,after,options,'hangout_changed',last),[]);
assert.equal(joins(before,after,{...options,cooldown:0},'hangout_changed',last).length,1);
assert.equal(joins(before,after,options,'hangout_changed',{[joins()[0].key]:now-300_000}).length,1);
const twoBefore=copy(before), twoAfter=copy(after);
twoBefore.server_states.push(snapshot([],'other').server_states[0]);
twoAfter.server_states.push(snapshot(['second'],'other').server_states[0]);
assert.equal(joins(twoBefore,twoAfter,{...options,homes:['home','other']}).length,2);
const command=copy(activity.command({names:['<b>Friend</b>'],roomName:'$(touch /tmp/nope)\nRoom',serverName:'A & B'}));
assert.equal(command[0],'notify-send');
assert(command.includes('--action=default=Open room'));
assert(command.at(-1).includes('&lt;b&gt;Friend&lt;/b&gt;'));
assert(command.at(-1).includes('$(touch /tmp/nope) Room · A &amp; B'));
console.log('Home-room alerts, identity/access checks, reconnect suppression, timing, cooldown, grouping and safe notification arguments passed');
