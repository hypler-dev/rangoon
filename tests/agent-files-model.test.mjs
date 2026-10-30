import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createAgentFilesController, getAgentFilesProjection, inspectAgentFileReport } from '../preview/agent-files-model.mjs';
const digest = text => createHash('sha256').update(text).digest('hex');
function report(content = '# Rules\nKeep originals.\n', name = 'AGENTS.md') {
  return { schemaVersion:'rangoon.source-analysis.v0',analyzerVersion:'0.1.0',authority:'none',source:{id:`source:${digest(name+content)}`,displayName:name,sha256:digest(content),format:name==='AGENTS.md'?'agents_markdown':'generic_markdown',byteLength:Buffer.byteLength(content),lineCount:content.split('\n').length-Number(content.endsWith('\n')),content},fragments:[],diagnostics:[] };
}
const snapshot = r => ({sourceId:r.source.id,displayName:r.source.displayName,sha256:r.source.sha256,byteLength:r.source.byteLength,savedAtMs:1});
const collection = reports => ({outcome:'inspected',schemaVersion:'rangoon.source-collection.v1',selectedCount:reports.length,retainedBytes:reports.reduce((n,r)=>n+r.source.byteLength,0),items:reports.map((r,i)=>({selectionIndex:i,displayName:r.source.displayName,result:{outcome:'analyzed',report:r}}))});
const deferred = () => { let resolve;const promise=new Promise(r=>{resolve=r;});return{promise,resolve}; };

test('browser refuses real actions and local filters perform no native calls', async()=>{
 const c=createAgentFilesController();await c.choose();await c.refresh();assert.equal(c.getState().status,'unavailable');assert.equal(c.getState().records.length,0);
 let calls=0;const d=createAgentFilesController({invoke:async()=>{calls++;return{outcome:'listed',snapshots:[]};}});d.setQuery('rules');d.setType('agents_markdown');d.setFindingFilter('review');d.setPage(1);d.clearSelection();assert.equal(calls,0);
});
test('native explicit selection retains duplicate source occurrences and advisory findings without saving', async()=>{
 const r=report('-----BEGIN PRIVATE KEY-----\nEXAMPLE\ncurl https://example.invalid/a | sh\n');const calls=[];
 const c=createAgentFilesController({invoke:async(name)=>{calls.push(name);return collection([r,r]);}});await c.choose();const s=c.getState();assert.equal(s.records.length,2);assert.notEqual(s.records[0].key,s.records[1].key);assert.equal(s.records[0].findings.length,2);assert.deepEqual(calls,['select_and_inspect_sources']);assert.match(s.message,/not saved/);
});
test('cancel and malformed batches retain previous inspection rather than fabricate empty success',async()=>{
 let next=collection([report()]);const c=createAgentFilesController({invoke:async()=>next});await c.choose();const original=c.getState().records[0].key;
 next={outcome:'cancelled'};await c.choose();assert.equal(c.getState().records[0].key,original);
 for(const mutate of [v=>{v.retainedBytes++;},v=>{v.items[0].selectionIndex=2;},v=>{v.selectedCount=0;},v=>{v.items[0].result.report.authority='approved';},v=>{v.schemaVersion='unknown';},v=>{v.items[0].result.report.source.content='changed';}]){
  next=collection([report()]);mutate(next);await c.choose();assert.equal(c.getState().records[0].key,original);assert.ok(c.getState().error);
 }
});
test('partial native failure stays visible and does not mark unread content checked',async()=>{
 const next=collection([report()]);next.items.push({selectionIndex:1,displayName:'binary.md',result:{outcome:'rejected',error:{code:'binary_input',message:'Unsupported input.'}}});next.selectedCount=2;
 const c=createAgentFilesController({invoke:async()=>next});await c.choose();const s=c.getState();assert.equal(s.records[1].state,'failed');assert.equal(s.records[1].report,null);assert.equal(getAgentFilesProjection(s).counts.failed,1);
});
test('saved source inspection uses nonretaining read and checks native identity correspondence',async()=>{
 const r=report();let bad=false;const calls=[];const c=createAgentFilesController({invoke:async(name,args)=>{calls.push([name,args]);return name==='list_snapshots'?{outcome:'listed',snapshots:[snapshot(r)]}:{outcome:'analyzed',report:bad?report('different'):r};}});
 await c.refresh();const key=c.getState().records[0].key;await c.open(key);assert.equal(c.getState().records[0].state,'checked');assert.deepEqual(calls[1],['read_composition_source',{sourceId:r.source.id}]);assert.ok(!calls.some(([n])=>n==='open_snapshot'||n==='save_analysis'));
 const d=createAgentFilesController({invoke:async(name)=>name==='list_snapshots'?{outcome:'listed',snapshots:[snapshot(r)]}:{outcome:'analyzed',report:report('different')}});await d.refresh();await d.open(d.getState().records[0].key);assert.equal(d.getState().records[0].state,'failed');
});
test('batch checking is sequential, cancellation stops future reads and retains only selected saved text',async()=>{
 const reports=[report('one'),report('two','more.md')];const wait=deferred();const calls=[];const c=createAgentFilesController({invoke:async(name,args)=>{calls.push(name);if(name==='list_snapshots')return{outcome:'listed',snapshots:reports.map(snapshot)};return wait.promise;}});
 await c.refresh();c.selectPage();const checking=c.checkSelected();await Promise.resolve();c.cancelCheck();assert.equal(c.getState().pending,'check');wait.resolve({outcome:'analyzed',report:reports[0]});await checking;
 assert.equal(calls.filter(n=>n==='read_composition_source').length,1);assert.equal(c.getState().pending,null);assert.equal(c.getState().checkCompleted,0);assert.ok(c.getState().records.every(r=>r.report===null));
});
test('lock purge discards late selected, saved, inspector and batch results',async()=>{
 for(const action of ['choose','refresh','open','check']){
  const r=report();const wait=deferred();let ready=false;const c=createAgentFilesController({invoke:async(name)=>{if(name==='list_snapshots'&&!ready)return{outcome:'listed',snapshots:[snapshot(r)]};return wait.promise;}});
  if(['open','check'].includes(action)){await c.refresh();c.selectPage();}ready=true;
  const p=action==='choose'?c.choose():action==='refresh'?c.refresh():action==='open'?c.open(c.getState().records[0].key):c.checkSelected();
  c.purgeForWorkspaceLock();wait.resolve(action==='choose'?collection([r]):action==='refresh'?{outcome:'listed',snapshots:[snapshot(r)]}:{outcome:'analyzed',report:r});await p;assert.equal(c.getState().records.length,0);assert.equal(c.getState().pending,null);
 }
});
test('pagination bounds DOM input while filters preserve marks and open identity',async()=>{
 const reports=Array.from({length:128},(_,i)=>report(`value-${i}`,`rules-${i}.md`));const c=createAgentFilesController({invoke:async()=>({outcome:'listed',snapshots:reports.map(snapshot)})});await c.refresh();c.selectPage();let p=getAgentFilesProjection(c.getState());assert.equal(p.pageRows.length,50);assert.equal(p.markedCount,50);assert.equal(p.pageCount,3);
 c.setPage(2);assert.equal(getAgentFilesProjection(c.getState()).pageRows.length,28);c.setQuery('rules-127');p=getAgentFilesProjection(c.getState());assert.equal(p.pageRows.length,1);assert.equal(p.page,0);assert.equal(p.markedCount,50);assert.equal(p.hiddenMarkedCount,50);
});
test('saved inventory rejects duplicate IDs, oversized counts and invalid metadata without truncation',async()=>{
 let next={outcome:'listed',snapshots:[snapshot(report())]};const c=createAgentFilesController({invoke:async()=>next});await c.refresh();for(const entries of [[snapshot(report()),snapshot(report())],Array(129).fill(snapshot(report())),[{...snapshot(report()),byteLength:262145}]]){next={outcome:'listed',snapshots:entries};await c.refresh();assert.equal(c.getState().records.length,1);assert.ok(c.getState().error);}
});
test('old age, model names and unfamiliar methods do not become outdated claims',()=>{
 const r=report('# Rules from 2019\nopenai.ChatCompletion.create is an example.\n');assert.deepEqual(inspectAgentFileReport(r),[]);const findings=inspectAgentFileReport(report(' \n'));assert.equal(findings[0].ruleId,'content.empty');assert.ok(findings.every(f=>!f.title.includes('Outdated')));
});

test('an entirely unread replacement preserves earlier selected originals and findings',async()=>{
 let next=collection([report()]);const c=createAgentFilesController({invoke:async()=>next});await c.choose();const before=c.getState().records[0];
 next={outcome:'inspected',schemaVersion:'rangoon.source-collection.v1',selectedCount:1,retainedBytes:0,items:[{selectionIndex:0,displayName:'bad.md',result:{outcome:'failed',error:{code:'read_failed',message:'Synthetic refusal.'}}}]};
 await c.choose();assert.equal(c.getState().records[0].key,before.key);assert.equal(c.getState().records[0].report.source.content,before.report.source.content);assert.equal(c.getState().error.code,'source_collection_unread');
});
test('received source text stays immutable through returned view state',async()=>{
 const r=report();const c=createAgentFilesController({invoke:async()=>collection([r])});await c.choose();const view=c.getState();assert.throws(()=>{view.records[0].report.source.content='mutated';},TypeError);view.records[0].name='view copy';assert.equal(c.getState().records[0].name,'AGENTS.md');
});
test('malformed fragment metadata refuses oversized retained payloads',async()=>{
 const original=collection([report()]);let next=original;const c=createAgentFilesController({invoke:async()=>next});await c.choose();const key=c.getState().records[0].key;
 next=collection([report()]);next.items[0].result.report.fragments=[{id:`fragment:${'a'.repeat(64)}`,kind:'section',heading:null,span:{startByte:0,endByte:1,startLine:1,endLine:1},text:'x'.repeat(262145),reviewState:'unreviewed'}];await c.choose();assert.equal(c.getState().records[0].key,key);assert.ok(c.getState().error);
});
test('initial saved inventory reuses scoped startup metadata without another native read',()=>{
 let calls=0;const c=createAgentFilesController({invoke:async()=>{calls++;}});assert.equal(c.acceptInitialSavedSnapshots([snapshot(report())]),true);assert.equal(calls,0);assert.equal(c.getState().records.length,1);assert.equal(c.acceptInitialSavedSnapshots([]),false);assert.equal(c.getState().records.length,1);
});

test('activity times distinguish native saved records from session observations without inventing revisions', async()=>{
 const r=report();let clock=1800000000123;
 const c=createAgentFilesController({now:()=>clock,invoke:async name=>name==='select_and_inspect_sources'?collection([r]):name==='list_snapshots'?{outcome:'listed',snapshots:[{...snapshot(r),savedAtMs:1700000000456}]}:{outcome:'analyzed',report:r}});
 await c.choose();let selected=c.getState().records[0];assert.equal(selected.selectedAtMs,clock);assert.equal(selected.checkedAtMs,clock);assert.equal(selected.savedAtMs,null);
 clock+=1000;await c.refresh();let saved=c.getState().records.find(x=>x.origin==='saved');assert.equal(saved.savedAtMs,1700000000456);assert.equal(saved.selectedAtMs,null);assert.equal(saved.checkedAtMs,null);
 await c.open(saved.key);assert.equal(c.getState().records.find(x=>x.key===saved.key).checkedAtMs,clock);
 clock+=1000;await c.refresh();assert.equal(c.getState().records.find(x=>x.key===saved.key).checkedAtMs,clock-1000);
 c.mark(saved.key);await c.checkSelected();assert.equal(c.getState().records.find(x=>x.key===saved.key).checkedAtMs,clock);
 assert.ok(c.getState().records.every(x=>!Object.hasOwn(x,'revisionId')&&!Object.hasOwn(x,'modifiedAtMs')));
 c.purgeForWorkspaceLock();assert.deepEqual(c.getState().records,[]);
});
test('invalid or unavailable observation clock never fabricates a timestamp',async()=>{
 for(const now of [()=>NaN,()=>-1,()=>Infinity,()=>{throw Error('clock unavailable');}]){
  const c=createAgentFilesController({now,invoke:async()=>collection([report()])});await c.choose();assert.equal(c.getState().records[0].checkedAtMs,null);assert.equal(c.getState().records[0].selectedAtMs,null);
 }
});

test('startup inventory failure is visible without a duplicate read and explicit refresh can recover',async()=>{
 let calls=0;const c=createAgentFilesController({invoke:async()=>{calls++;return {outcome:'listed',snapshots:[snapshot(report())]};}});
 assert.equal(c.acceptInitialSavedFailure({code:'read_failed',message:'Saved inventory unavailable.'}),true);
 assert.equal(c.getState().status,'failed');assert.equal(c.getState().error.code,'read_failed');assert.equal(calls,0);
 await c.refresh();assert.equal(c.getState().status,'ready');assert.equal(calls,1);
 assert.equal(c.acceptInitialSavedFailure({code:'late',message:'Late failure.'}),false);assert.equal(c.getState().records.length,1);
});


test('saved handoff uses exact metadata only, rejects duplicate, and keeps inspection on failure',async()=>{
 const r=report(),wait=deferred(),seen=[];let succeed=false;
 const c=createAgentFilesController({invoke:async(name)=>name==='list_snapshots'?{outcome:'listed',snapshots:[snapshot(r)]}:{outcome:'analyzed',report:r},openSavedSource:async reference=>{seen.push(reference);return succeed?{outcome:'opened'}:wait.promise;}});
 await c.refresh();await c.open(c.getState().records[0].key);const original=c.getState().records[0].report;
 const pending=c.openInAnalysis();assert.equal(c.getState().pending,'handoff');assert.equal(await c.openInAnalysis(),false);assert.equal(seen.length,1);
 assert.deepEqual(seen[0],{sourceId:r.source.id,sha256:r.source.sha256,byteLength:r.source.byteLength});wait.resolve({outcome:'failed'});assert.equal(await pending,false);
 assert.equal(c.getState().records[0].report,original);assert.equal(c.getState().error.code,'source_handoff_failed');
 succeed=true;assert.equal(await c.openInAnalysis(),true);assert.equal(c.getState().pending,null);
});

test('unsaved and unavailable handoffs do not invoke; late success cannot revive locked state',async()=>{
 const r=report();let calls=0;const c=createAgentFilesController({invoke:async()=>collection([r]),openSavedSource:async()=>{calls++;return {outcome:'opened'};}});
 await c.choose();assert.equal(await c.openInAnalysis(),false);assert.equal(calls,0);
 const unavailable=createAgentFilesController();assert.equal(await unavailable.openInAnalysis(),false);
 const wait=deferred();const saved=createAgentFilesController({invoke:async()=>({outcome:'listed',snapshots:[snapshot(r)]}),openSavedSource:()=>wait.promise});await saved.refresh();await saved.open(saved.getState().records[0].key);
 const handoff=saved.openInAnalysis();saved.purgeForWorkspaceLock();wait.resolve({outcome:'opened'});assert.equal(await handoff,false);assert.deepEqual(saved.getState().records,[]);
});

test('observer-triggered lock before handoff acquisition prevents callback dispatch',async()=>{
 const r=report();let lock=false,calls=0;const c=createAgentFilesController({invoke:async()=>({outcome:'listed',snapshots:[snapshot(r)]}),openSavedSource:async()=>{calls++;return {outcome:'opened'};},onChange:state=>{if(lock&&state.pending==='handoff')c.purgeForWorkspaceLock();}});
 await c.refresh();await c.open(c.getState().records[0].key);lock=true;assert.equal(await c.openInAnalysis(),false);assert.equal(calls,0);assert.deepEqual(c.getState().records,[]);
});


test('handoff failure reasons are bounded and unknown callback results cannot claim success',async()=>{
 const r=report();let result;const c=createAgentFilesController({invoke:async(name)=>name==='list_snapshots'?{outcome:'listed',snapshots:[snapshot(r)]}:{outcome:'analyzed',report:r},openSavedSource:async()=>result});
 await c.refresh();await c.open(c.getState().records[0].key);
 for(const outcome of ['busy','mismatch','removed','unrecognized','constructor','__proto__']){result={outcome};assert.equal(await c.openInAnalysis(),false);assert.equal(c.getState().error.code,['busy','mismatch','removed'].includes(outcome)?`source_handoff_${outcome}`:'source_handoff_failed');}
 result=true;assert.equal(await c.openInAnalysis(),false);
});

function folderWindow(count=3, extra={}) {
 return {outcome:'listed',schemaVersion:'rangoon.folder-collection.v1',rootLabel:'projects',entries:Array.from({length:count},(_,i)=>({observationId:i+1,relativePath:`project-${i}/AGENTS.md`,byteLength:333,modifiedAtMs:1700000000000})),visitedEntries:count+2,emittedRecords:count,excludedEntries:0,exhausted:true,partialReasons:[],...extra};
}
test('folder rows are paged metadata only and opening/filtering never reads content',async()=>{
 const calls=[];const c=createAgentFilesController({now:()=>1800000000000,invoke:async(command,args)=>{calls.push([command,args]);return folderWindow(121,{partialReasons:['depth_limit']});}});
 await c.chooseFolder();let s=c.getState();assert.equal(s.records.length,121);assert.equal(s.folder.rootLabel,'projects');assert.match(s.message,/partial listing/);
 assert.equal(s.records[0].origin,'folder');assert.equal(s.records[0].state,'metadata');assert.equal(s.records[0].sourceId,null);assert.equal(s.records[0].report,null);assert.equal(s.records[0].listedAtMs,1800000000000);
 await c.open(s.records[0].key);c.mark(s.records[0].key);c.selectPage();assert.equal(c.getState().markedKeys.length,0);
 c.setPage(2);assert.equal(getAgentFilesProjection(c.getState()).pageRows.length,21);c.setQuery('project-120/');assert.equal(getAgentFilesProjection(c.getState()).filtered.length,1);
 await c.openInAnalysis();await c.checkSelected();assert.deepEqual(calls,[['select_and_list_sources',undefined]]);
 c.clearFolder();assert.equal(c.getState().folder,null);assert.equal(c.getState().records.length,0);
});
test('folder replacement, quiet cancellation and failure preserve prior source selection',async()=>{
 const r=report();let response=folderWindow();const c=createAgentFilesController({invoke:async name=>name==='select_and_inspect_sources'?collection([r]):response});
 await c.choose();const key=c.getState().selectedKey;await c.chooseFolder();assert.equal(c.getState().selectedKey,key);assert.ok(c.getState().records.find(r=>r.key===key).report);
 const before=c.getState();response={outcome:'cancelled'};await c.chooseFolder();assert.equal(c.getState().message,before.message);assert.deepEqual(c.getState().records,before.records);
 response={outcome:'failed',error:{code:'folder_inventory_unsupported',message:'Choose individual files instead.'}};await c.chooseFolder();assert.deepEqual(c.getState().records,before.records);assert.equal(c.getState().error.code,'folder_inventory_unsupported');
 response=folderWindow(1);await c.chooseFolder();assert.equal(c.getState().records.filter(r=>r.origin==='folder').length,1);assert.ok(c.getState().records.find(r=>r.key===key).report);
});
test('folder inventory rejects malformed shapes, traversal, duplicates and unsafe metadata atomically',async()=>{
 for(const mutate of [x=>x.sourceId='source:bad',x=>x.rootLabel='con',x=>x.rootLabel='work:',x=>x.rootLabel='work.',x=>x.entries[0].relativePath='.env.md',x=>x.entries[0].content='untrusted source',x=>x.schemaVersion='other',x=>x.entries[0].relativePath='../AGENTS.md',x=>x.entries[0].relativePath='.ssh/secret.md',x=>x.entries[0].observationId=2,x=>x.entries[1].relativePath=x.entries[0].relativePath,x=>x.entries[0].modifiedAtMs=1.5,x=>x.entries[0].byteLength=Number.MAX_SAFE_INTEGER+1,x=>x.emittedRecords=99,x=>x.visitedEntries=10001,x=>x.excludedEntries=100,x=>x.exhausted=false,x=>x.partialReasons=['unknown'],x=>x.partialReasons=['record_limit'],x=>x.partialReasons=['entry_limit'],x=>x.partialReasons=['read_error','read_error'],x=>x.entries[0].relativePath='a\u202e.md',x=>x.entries[0].relativePath='dir/😀.md',x=>x.entries[0].relativePath='dir/space\u00a0.md']){
  let response=folderWindow();const c=createAgentFilesController({invoke:async()=>response});await c.chooseFolder();const previous=c.getState().records;response=folderWindow();mutate(response);await c.chooseFolder();assert.equal(c.getState().error.code,'folder_inventory_invalid');assert.deepEqual(c.getState().records,previous);
 }
});
test('folder result count and actual UTF-8 byte limits reject without partial append',async()=>{
 for(const response of [folderWindow(1001),folderWindow(1000)]){
  if(response.entries.length===1000)for(const [i,row]of response.entries.entries())row.relativePath=`${'界'.repeat(240)}/${'界'.repeat(240)}/${'界'.repeat(240)}/${'界'.repeat(240)}/${i}.md`;
  const c=createAgentFilesController({invoke:async()=>response});await c.chooseFolder();assert.equal(c.getState().error.code,'folder_inventory_invalid');assert.equal(c.getState().records.length,0);
 }
});
test('folder request is single-flight and Lock/dispose fence held native results',async()=>{
 for(const method of ['purgeForWorkspaceLock','dispose']){
  const wait=deferred();let calls=0;const c=createAgentFilesController({invoke:async()=>{calls++;return wait.promise;}});const pending=c.chooseFolder();await c.chooseFolder();assert.equal(calls,1);c[method]();wait.resolve(folderWindow());await pending;assert.equal(c.getState().folder,null);assert.equal(c.getState().records.length,0);
 }
});
test('saved refresh and explicit source selection preserve folder metadata without creating history links',async()=>{
 const r=report();const c=createAgentFilesController({invoke:async command=>command==='select_and_list_sources'?folderWindow():command==='list_snapshots'?{outcome:'listed',snapshots:[snapshot(r)]}:collection([r])});
 await c.chooseFolder();await c.refresh();await c.choose();assert.equal(c.getState().records.filter(r=>r.origin==='folder').length,3);assert.equal(c.getState().records.filter(r=>r.origin==='saved').length,1);assert.equal(c.getState().records.filter(r=>r.origin==='selected').length,1);
 c.clearSelectedFiles();assert.equal(c.getState().records.length,4);c.getState().folder.partialReasons.push('spoof');assert.deepEqual(c.getState().folder.partialReasons,[]);
});


test('metadata count/filter stays distinct from actionable unchecked saved sources',async()=>{
 const r=report();const c=createAgentFilesController({invoke:async()=>folderWindow()});c.acceptInitialSavedSnapshots([snapshot(r)]);await c.chooseFolder();
 let p=getAgentFilesProjection(c.getState());assert.equal(p.counts.metadata,3);assert.equal(p.counts.unchecked,1);
 c.setFindingFilter('metadata');assert.equal(getAgentFilesProjection(c.getState()).filtered.length,3);
 c.setFindingFilter('unchecked');assert.equal(getAgentFilesProjection(c.getState()).filtered.length,1);assert.equal(getAgentFilesProjection(c.getState()).filtered[0].origin,'saved');
});
