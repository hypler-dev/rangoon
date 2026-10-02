import {test,after,before} from 'node:test';
import assert from 'node:assert/strict';
import {createSiteServer} from '../scripts/serve.mjs';
import {mkdtemp,symlink,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawn} from 'node:child_process';
let server,base;
before(async()=>{server=createSiteServer(new URL('../dist/',import.meta.url).pathname);await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));base=`http://127.0.0.1:${server.address().port}`;});
after(()=>new Promise(resolve=>server.close(resolve)));
test('serves nested pages, Markdown and machine-readable policy',async()=>{
 for(const [path,type]of [['/','text/html'],['/product/skills/','text/html'],['/lnsat/index.md','text/markdown'],['/llms.txt','text/plain'],['/ai.txt','text/plain'],['/.well-known/ai-policy.json','application/json']]){
  const response=await fetch(base+path);assert.equal(response.status,200,path);assert.match(response.headers.get('content-type'),new RegExp(type));assert.ok((await response.text()).length>50);
 }
});
test('provides true 404 responses, rejects writes, and does not expose source',async()=>{
 for(const path of ['/not-a-page/','/.git/config','/../package.json','/%2e%2e%2fpackage.json','/scripts/serve.mjs'])assert.ok([403,404].includes((await fetch(base+path)).status),path);
 assert.equal((await fetch(base+'/',{method:'POST',body:'x'})).status,405);
});
test('canonicalizes directories and advertises Markdown companions',async()=>{
 const response=await fetch(base+'/product/skills',{redirect:'manual'});assert.equal(response.status,308);assert.equal(response.headers.get('location'),'/product/skills/');
 const page=await fetch(base+'/product/skills/');assert.match(page.headers.get('link'),/\/product\/skills\/index.md/);assert.match(page.headers.get('content-security-policy'),/frame-ancestors 'none'/);
});
test('HEAD retains GET metadata without a body',async()=>{
 const response=await fetch(base+'/llms.txt',{method:'HEAD'});assert.equal(response.status,200);assert.ok(Number(response.headers.get('content-length'))>0);assert.equal(await response.text(),'');
});
test('starts and serves through a release symlink',async()=>{
 const temp=await mkdtemp(join(tmpdir(),'rangoon-release-'));
 let child;
 try{
  await symlink(new URL('../',import.meta.url).pathname,join(temp,'current'),'dir');
  child=spawn(process.execPath,[join(temp,'current/scripts/serve.mjs')],{env:{...process.env,PORT:'0',SITE_ROOT:join(temp,'current/dist')},stdio:['ignore','pipe','pipe']});
  const address=await new Promise((resolve,reject)=>{
   const timeout=setTimeout(()=>reject(new Error('Release server did not start')),5000);
   child.once('exit',code=>{clearTimeout(timeout);reject(new Error(`Release server exited: ${code}`));});
   child.stdout.on('data',data=>{const match=String(data).match(/http:\/\/127\.0\.0\.1:(\d+)/);if(match){clearTimeout(timeout);resolve(`http://127.0.0.1:${match[1]}`);}});
  });
  assert.equal((await fetch(address+'/')).status,200);
  assert.equal((await fetch(address+'/.health')).status,200);
 }finally{if(child&&!child.killed){const exited=new Promise(resolve=>child.once('exit',resolve));child.kill();await exited;}await rm(temp,{recursive:true,force:true});}
});
