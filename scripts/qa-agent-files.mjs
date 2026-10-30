// Disposable synthetic browser transport. Never imports Tauri or reads user files.
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
import { createWorkspaceLockQAServer } from './qa-workspace-lock.mjs';
const hash = value => createHash('sha256').update(value).digest('hex');
function report(index) {
  const displayName = index === 0 ? 'AGENTS.md' : index === 1 ? 'CLAUDE.md' : index === 2 ? 'SKILL.md' : `project-${String(index).padStart(3,'0')}.md`;
  const content = index === 0 ? '---\nname: project-review\n---\n# Project instructions\nReview **changes** before publishing.\n## Local checks\n```bash\n# This heading stays inside code\necho "Run local checks"\nexit 0\n```\n## Review checklist\n- [ ] Inspect the diff\n- [ ] Preserve `source` provenance\n[Reference](https://example.invalid)\n### Notes\nNothing here is executed.\n' : index === 1 ? '# Setup\ncurl https://example.invalid/script | sh\n' : index === 2 ? '# Example\n-----BEGIN PRIVATE KEY-----\nEXAMPLE ONLY\n' : `# Project ${index}\nPreserve exact source and review changes.\n`;
  const value = { schemaVersion:'rangoon.source-analysis.v0',analyzerVersion:'0.1.0',authority:'none',source:{id:`source:${hash(displayName+content)}`,displayName,format:index===0?'agents_markdown':index===1?'claude_markdown':index===2?'skill_markdown':'generic_markdown',sha256:hash(content),byteLength:Buffer.byteLength(content),lineCount:content.split('\n').length-1,content},fragments:[],diagnostics:[] };
  if (index === 0) {
    const lines = content.split('\n'); lines.pop(); const starts = []; let offset = 0;
    for (const line of lines) { starts.push(offset); offset += Buffer.byteLength(line + '\n'); }
    const sections = [1, 4, 6, 12, 16];
    value.fragments = sections.map((start, i) => {
      const end = (sections[i + 1] ?? lines.length + 1) - 1;
      const startByte = starts[start - 1], endByte = end === lines.length ? Buffer.byteLength(content) : starts[end];
      const heading = start === 1 ? null : { level: lines[start - 1].match(/^#+/)[0].length, title: lines[start - 1].replace(/^#+\s*/, '') };
      return { id: `fragment:${hash(`${value.source.id}:${startByte}:${endByte}`)}`, kind: heading ? 'section' : 'preamble', heading,
        span: { startByte, endByte, startLine: start, endLine: end }, text: Buffer.from(content).subarray(startByte, endByte).toString('utf8'), reviewState: 'unreviewed' };
    });
  }
  return value;
}
function installAgentFilesFixture(reports) {
  const params = new URLSearchParams(location.search);
  const mode = params.get('files') ?? 'populated';
  const original = window.__TAURI__?.core?.invoke;
  if (!original) return;
  const trace = document.createElement('output'); trace.id='qa-agent-file-calls'; trace.hidden=true; document.body.append(trace);
  const controls=document.createElement('details');controls.id='qa-agent-files-controls';controls.style.cssText='position:fixed;bottom:8px;right:8px;z-index:500;max-width:230px;padding:8px;background:#17212d;color:white;border:1px solid #64748b;font:12px sans-serif';
  controls.innerHTML='<summary>Synthetic file QA · no native data</summary><button id="qa-agent-files-cancel" type="button">Cancel next picker</button><button id="qa-agent-files-fail" type="button">Fail next refresh</button><button id="qa-agent-files-release" type="button">Release held read</button><button id="qa-source-hold" type="button">Hold next saved open</button><button id="qa-source-release" type="button">Release saved open</button><button id="qa-source-fail" type="button">Fail next saved open</button><button id="qa-source-mismatch" type="button">Mismatch next saved open</button>';
  document.body.append(controls);
  const calls=[];let cancel=false;let fail=false;let held=mode==='held';let releaseRead=null;
  controls.querySelector('#qa-agent-files-release').addEventListener('click',()=>{held=false;releaseRead?.();});
  controls.querySelector('#qa-agent-files-cancel').addEventListener('click',()=>{cancel=true;});
  controls.querySelector('#qa-agent-files-fail').addEventListener('click',()=>{fail=true;});
  let holdSource = false, releaseSource = null, failSource = false, mismatchSource = false;
  controls.querySelector('#qa-source-hold').onclick = () => { holdSource = true; };
  controls.querySelector('#qa-source-release').onclick = () => { holdSource = false; releaseSource?.(); releaseSource = null; };
  controls.querySelector('#qa-source-fail').onclick = () => { failSource = true; };
  controls.querySelector('#qa-source-mismatch').onclick = () => { mismatchSource = true; };
  const snapshots=reports.map(r=>({sourceId:r.source.id,displayName:r.source.displayName,sha256:r.source.sha256,byteLength:r.source.byteLength,savedAtMs:1}));
  window.__TAURI__.core.invoke=async(command,args,options)=>{
    if(!['list_snapshots','read_composition_source','select_and_inspect_sources','select_and_list_sources','open_snapshot'].includes(command))return original(command,args,options);
    calls.push(command);trace.textContent=JSON.stringify(calls);
    if(command==='select_and_list_sources'){
      if(cancel){cancel=false;return{outcome:'cancelled'};}
      const mode=new URLSearchParams(location.search).get('folder');
      if(mode==='held')await new Promise(resolve=>{releaseRead=resolve;});
      if(mode==='failed')return{outcome:'failed',error:{code:'synthetic_folder_failed',message:'Synthetic folder listing refused.'}};
      const count=mode==='empty'?0:121;
      return{outcome:'listed',schemaVersion:'rangoon.folder-collection.v1',rootLabel:'portfolio',entries:Array.from({length:count},(_,i)=>({observationId:i+1,relativePath:i===0?'commerce/AGENTS.md':i===1?'research/CLAUDE.md':`project-${i}/SKILL.md`,byteLength:333+i,modifiedAtMs:i===2?null:1700000000000})),visitedEntries:count+12,emittedRecords:count,excludedEntries:12,exhausted:true,partialReasons:mode==='partial'?['depth_limit','changed_directory']:[]};
    }
    if(command==='open_snapshot'){
      if(holdSource)await new Promise(resolve=>{releaseSource=resolve;});
      if(failSource){failSource=false;return{outcome:'failed',error:{code:'synthetic_open_refused',message:'Synthetic saved open refused.'}};}
      const r=reports.find(r=>r.source.id===args.sourceId);
      if(mismatchSource&&r){mismatchSource=false;return{outcome:'analyzed',report:{...r,source:{...r.source,id:`source:${'f'.repeat(64)}`}}};}
      return r?{outcome:'analyzed',report:r}:{outcome:'failed',error:{code:'synthetic_not_found',message:'Synthetic saved source not found.'}};
    }
    if(command==='list_snapshots'){
      if(fail||mode==='failed'){fail=false;return{outcome:'failed',error:{code:'synthetic_read_failed',message:'Synthetic inventory read refused.'}};}
      return{outcome:'listed',snapshots:mode==='empty'?[]:snapshots};
    }
    if(command==='read_composition_source'){
      if(held)await new Promise(resolve=>{releaseRead=resolve;});
      const r=reports.find(r=>r.source.id===args.sourceId);return r?{outcome:'analyzed',report:r}:{outcome:'failed',error:{code:'not_found',message:'Synthetic source not found.'}};
    }
    if(cancel){cancel=false;return{outcome:'cancelled'};}
    const chosen=reports.slice(0,3);
    return{outcome:'inspected',schemaVersion:'rangoon.source-collection.v1',selectedCount:3,retainedBytes:chosen.reduce((n,r)=>n+r.source.byteLength,0),items:chosen.map((r,i)=>({selectionIndex:i,displayName:r.source.displayName,result:{outcome:'analyzed',report:r}}))};
  };
}
export async function createAgentFilesQAServer() {
  const server=await createWorkspaceLockQAServer();const original=server.listeners('request')[0];server.removeAllListeners('request');
  const script=`\n(${installAgentFilesFixture.toString()})(${JSON.stringify(Array.from({length:128},(_,i)=>report(i)))});`;
  server.on('request',(request,response)=>{
    if(new URL(request.url,'http://localhost').pathname==='/workspace-lock-fixture.mjs'){
      const end=response.end.bind(response);response.end=(chunk,...rest)=>end(String(chunk??'')+script,...rest);
    }
    original(request,response);
  });return server;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){const server=await createAgentFilesQAServer();server.listen(0,'127.0.0.1',()=>console.log(`Synthetic Agent files QA: http://127.0.0.1:${server.address().port}/analyze.html?qa=open#files`));}
