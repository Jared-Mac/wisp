// Real loopback event sockets: opt-in clients only, private recipients, no snapshots.
const assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),net=require('node:net');
const {spawn}=require('node:child_process');
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
(async()=>{
  const dir=fs.mkdtempSync(path.join(os.tmpdir(),'wisp-typing-events-'));
  const listener=net.createServer();await new Promise(resolve=>listener.listen(0,'127.0.0.1',resolve));
  const port=listener.address().port;await new Promise(resolve=>listener.close(resolve));
  const base=`http://127.0.0.1:${port}`;
  const server=spawn(process.argv[2] || 'target/debug/wisp-server',[],{env:{...process.env,WISP_SERVER_ADDR:`127.0.0.1:${port}`,WISP_DATABASE_URL:`sqlite://${dir}/test.sqlite3`,WISP_ALLOW_DEV_SESSIONS:'true',WISP_REQUIRE_CHAT_E2EE:'false'},stdio:'ignore'});
  const sockets=[];
  const ids=[1,2,3].map(n=>`00000000-0000-4000-8000-00000000000${n}`);
  try {
    let ready=false;
    for(let i=0;i<100;i++){try{if((await fetch(base+'/healthz')).ok){ready=true;break}}catch{}await pause(50)}
    assert(ready,'loopback server ready');
    const post=async(user,route,body)=>fetch(base+route,{method:'POST',headers:{Authorization:`Bearer dev:${user}`,'Content-Type':'application/json'},body:JSON.stringify(body)});
    const direct=await post(ids[0],'/v1/conversations/direct',{friend:ids[1]});assert(direct.ok);
    const conversation=await direct.json();
    async function connect(user,optIn){
      const events=[],socket=new WebSocket(`ws://127.0.0.1:${port}/v1/events?token=dev:${user}${optIn?'&typing=true':''}`);
      sockets.push(socket);socket.addEventListener('message',e=>events.push(JSON.parse(e.data)));
      await new Promise((resolve,reject)=>{socket.addEventListener('open',resolve,{once:true});socket.addEventListener('error',reject,{once:true})});
      for(let i=0;i<40&&!events.some(e=>e.name==='snapshot');i++)await pause(25);
      assert(events.some(e=>e.name==='snapshot'));return events;
    }
    const member=await connect(ids[1],true),legacy=await connect(ids[1],false),outsider=await connect(ids[2],true),sender=await connect(ids[0],true);
    await pause(100);for(const events of [member,legacy,outsider,sender])events.length=0;
    const body={conversation_id:conversation.id,active:true};
    assert.equal((await post(ids[2],'/v1/typing',body)).status,403);
    assert((await post(ids[0],'/v1/typing',body)).ok);
    for(let i=0;i<80&&!member.some(e=>e.name==='chat_typing');i++)await pause(25);
    const activity=member.filter(e=>e.name==='chat_typing');assert.equal(activity.length,1);assert.equal(activity[0].payload.active,true);
    assert.equal(activity[0].payload.display_name,'Owner');
    for(const events of [legacy,outsider,sender])assert(!events.some(e=>e.name==='chat_typing'),'typing must not leak to legacy clients, outsiders, or self');
    assert(!member.some(e=>e.name==='snapshot'||e.name==='message_created'),'typing does not become a message or snapshot');
    assert((await post(ids[0],'/v1/typing',{...body,active:false})).ok);
    for(let i=0;i<80&&!member.some(e=>e.name==='chat_typing'&&!e.payload.active);i++)await pause(25);
    assert(member.some(e=>e.name==='chat_typing'&&!e.payload.active));
    console.log('Live event sockets: opt-in, private audience, self exclusion, stop and no chat/snapshot side effects passed');
  } finally { for(const socket of sockets)socket.close();server.kill('SIGTERM');await new Promise(resolve=>server.once('exit',resolve));fs.rmSync(dir,{recursive:true,force:true}) }
})().catch(error=>{console.error(error);process.exitCode=1});
