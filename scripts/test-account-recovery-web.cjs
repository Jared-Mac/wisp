const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const code = fs.readFileSync(require('node:path').join(__dirname,'../infra/account-recovery/site/recovery.js'),'utf8');
const tick = () => new Promise(resolve=>setImmediate(resolve));
async function page(mode,{token='A'.repeat(43),failure=null}={}) {
 const fields={};
 const element=id=>fields[id] ||= {hidden:true,value:'',textContent:'',disabled:false,handlers:{},addEventListener(name,fn){this.handlers[name]=fn;}};
 const requests=[],history=[];
 const context={document:{getElementById:element},location:{pathname:'/account/'+mode,hash:'#token='+token},history:{replaceState(...args){history.push(args);}},URLSearchParams,AbortController,setTimeout,clearTimeout,Uint8Array,TextDecoder,Error,
 fetch:async(path,options)=>{requests.push({path,options,body:JSON.parse(options.body)});return {ok:!failure,status:failure?400:200,body:failure?{getReader(){let done=false;return {read:async()=>done?{done:true}:(done=true,{done:false,value:Buffer.from(JSON.stringify(failure))}),cancel:async()=>{}};}}:null};}};
 vm.runInNewContext(code,context);await tick();
 return {fields,requests,history,submit:async()=>{await element('form').handlers.submit({preventDefault(){}});await tick();}};
}
(async()=>{
 let p=await page('verify-email');assert.equal(p.requests.length,0,'mail scanners cannot consume verification by GET');assert.equal(p.history[0][2],'/account/verify-email');await p.submit();assert.equal(p.requests[0].body.token,'A'.repeat(43));assert.equal(p.requests[0].path,'/v2/accounts/recovery-email/verify');
 p=await page('reset-password');assert.equal(p.requests.length,1);assert.equal(p.requests[0].path,'/v2/accounts/password-reset/inspect');p.fields.password.value='example private password';p.fields.confirm.value='mismatch';await p.submit();assert.equal(p.requests.length,1);assert.equal(p.fields.error.hidden,false);p.fields.dismiss.handlers.click();assert.equal(p.fields.error.hidden,true);assert.equal(p.fields.password.value,'example private password','dismiss preserves fields');p.fields.confirm.value=p.fields.password.value;await p.submit();assert.equal(p.requests[1].path,'/v2/accounts/password-reset/complete');assert.equal(p.fields.password.value,'');
 for(const r of p.requests){assert.equal(r.options.credentials,'omit');assert.equal(r.options.redirect,'error');assert.equal(r.options.referrerPolicy,'no-referrer');assert.equal(r.options.headers.Authorization,undefined);assert(!r.path.includes('token'));}
 p=await page('reset-password',{token:'bad'});assert.equal(p.requests.length,0);assert.equal(p.fields.form.hidden,true);
 p=await page('forgot-password');p.fields.identifier.value='example';await p.submit();assert.deepEqual(p.requests[0].body,{identifier:'example'});assert.match(p.fields.success.textContent,/If the account has a verified recovery email/);
 p=await page('verify-email',{failure:{code:'proxy_error',message:'PRIVATE SECRET ECHO'}});await p.submit();assert(!p.fields['error-text'].textContent.includes('PRIVATE'));assert.equal(p.fields.submit.disabled,false);
 console.log('Recovery browser flow, explicit verification, fragment clearing, dismissal and safe errors passed');
})().catch(error=>{console.error(error);process.exitCode=1;});
