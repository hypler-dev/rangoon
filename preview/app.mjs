import { routes, canCompile, workflowOutcome, filterItems } from './model.mjs';

const $ = (selector) => document.querySelector(selector);
const escape = (value) => String(value).replace(/[&<>"']/g, char => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));
let theme = 'dark';
try { theme = localStorage.getItem('rangoon-theme') === 'light' ? 'light' : 'dark'; } catch { /* Theme storage is optional. */ }
const state = { route:'command', theme, dense:false, focus:false, scenario:'sample', selected:'review', imported:false, candidate:'review', selectedCandidates:new Set(['review','tests']), mode:'merge', resolved:false, compiled:false, filter:'', connector:'git', evidenceFilter:'all', selectedEvidence:'ev1', outcome:'approval', simulation:false, graphView:'graph', history:[], draftCreated:false };
const icons = ['◈','↳','⌗','⋈','▤','⌘','⌁','◎','↗'];
const capabilities = [
  {id:'review',name:'Security Review',family:'Security',version:'1.2.0',status:'Reviewed',harness:'Codex · Claude',source:'AGENTS.md',span:'4–6',detail:'Review changed code for unsafe input handling. Keep the findings tied to source evidence.',color:'orange',lines:[4,5,6],proposal:'A reusable review skill with a read-only tool requirement and evidence-linked findings.'},
  {id:'tests',name:'Test & Validate',family:'Engineering',version:'1.1.0',status:'Draft',harness:'Codex · Claude',source:'AGENTS.md',span:'8–11',detail:'Run the declared project checks in a bounded test workspace. Preserve failed and partial results.',color:'cyan',lines:[8,9,10,11],proposal:'A test capability with named validators, bounded runtime and a structured result contract.'},
  {id:'release',name:'Release Review',family:'Operations',version:'0.3.0',status:'Needs review',harness:'Not qualified',source:'AGENTS.md',span:'13–15',detail:'Prepare a release diff and evidence bundle. Activation requires a separate authority path.',color:'purple',lines:[13,14,15],proposal:'A release-planning skill that prepares artifacts without granting deployment authority.'},
  {id:'docs',name:'Documentation',family:'Productivity',version:'1.0.0',status:'Reviewed',harness:'Codex · Claude',source:'SKILL.md',span:'1–9',detail:'Explain the changed behavior, verification and open questions in project documentation.',color:'green',lines:[1,2],proposal:'A source-grounded documentation capability.'},
  {id:'incident',name:'Incident Triage',family:'Operations',version:'0.2.0',status:'Draft',harness:'Not qualified',source:'incident.md',span:'1–18',detail:'Reconstruct the request-to-outcome path and isolate missing evidence before proposing a retry.',color:'red',lines:[1,2],proposal:'A readback-first incident workflow.'},
];
const sourceLines = ['# Atlas engineering','', '## Code review','Inspect changed files and trace untrusted inputs.','Use read-only repository access for review.','Attach file and line evidence to each finding.','', '## Verification','Run the named checks in a disposable workspace.','Record failed and incomplete checks explicitly.','Never describe a simulation as a real run.','', '## Release preparation','Prepare exact artifacts, digests and target diff.','Request separate authorization for activation.'];
const graphNodes = [
  {id:'source',name:'Atlas workspace',kind:'SOURCE',icon:'⌂',status:'Imported',note:'3 instruction files',x:3,y:36,color:'cyan'},
  {id:'review',name:'Security Review',kind:'SKILL',icon:'◈',status:'Reviewed',note:'AGENTS.md · lines 4–6',x:28,y:10,color:'orange'},
  {id:'tests',name:'Test & Validate',kind:'SKILL',icon:'⌗',status:'Draft',note:'AGENTS.md · lines 8–11',x:28,y:61,color:'cyan'},
  {id:'bundle',name:'Review bundle',kind:'COMPOSITION',icon:'▤',status:'Needs review',note:'2 skills · pinned inputs',x:54,y:36,color:'purple'},
  {id:'gate',name:'Policy + approval',kind:'AUTHORITY',icon:'◇',status:'Not connected',note:'LNSAT reference path',x:79,y:10,color:'orange'},
  {id:'receipt',name:'Execution evidence',kind:'RECEIPT',icon:'◎',status:'Unavailable',note:'No action executed',x:79,y:61,color:'green'},
];
const connectorRecords = [
  {id:'git',name:'Git repository',icon:'⑂',category:'Development',scope:'Read selected repository',operation:'repository.read',effect:'Read only',credential:'Not configured',authority:'Required for future managed writes',color:'orange'},
  {id:'files',name:'Local workspace',icon:'▣',category:'Files',scope:'Explicitly selected folder',operation:'workspace.snapshot',effect:'Bounded read',credential:'OS folder grant',authority:'No external action permission',color:'cyan'},
  {id:'mcp',name:'MCP server',icon:'⌁',category:'Tools & resources',scope:'Per-resource delegated scope',operation:'tool.invoke',effect:'Operation dependent',credential:'Not configured',authority:'Exact action path required',color:'purple'},
  {id:'model',name:'Analysis provider',icon:'✧',category:'Inference',scope:'Approved source excerpts only',operation:'analysis.propose',effect:'Potential data transfer',credential:'Not configured',authority:'Data policy and provider permission required',color:'green'},
  {id:'lnsat',name:'LNSAT',icon:'◇',category:'Authority provider',scope:'Qualified engine contract',operation:'Authority and evidence reference',effect:'Versioned authority boundary',credential:'Not connected',authority:'Runtime support remains gated',color:'orange'},
  {id:'slack',name:'Slack',icon:'⌘',category:'Communication',scope:'Proposed future adapter',operation:'message.send',effect:'External write',credential:'Not implemented',authority:'Would require explicit action authorization',color:'purple'},
];
const ledger = [
  {id:'ev1',name:'Source snapshot',type:'Provenance',status:'Sample',detail:'Atlas / AGENTS.md. Original content retained before any proposed transformation.',refs:'snapshot:atlas-example / source lines 1–15'},
  {id:'ev2',name:'Merge conflict',type:'Review',status:'Blocked',detail:'Release capability requests network access; review capability restricts it. Retain the restrictive boundary before generating a draft.',refs:'Security Review 1.2.0 → Release Review 0.3.0'},
  {id:'ev3',name:'Response interrupted',type:'Simulation',status:'Unknown',detail:'Synthetic connection-loss scenario. The external effect is not confirmed. Reconcile before any retry.',refs:'sample-run:03 / no live action'},
  {id:'ev4',name:'Desktop qualification',type:'Release',status:'Missing',detail:'macOS, Windows and Linux are initial targets. Installer and recovery evidence does not exist yet.',refs:'R1 qualification spike → R6 desktop release'},
];
const badge = (text) => `<span class="badge ${/Blocked|Missing|Needs review|Not connected/.test(text)?'amber':/Unknown/.test(text)?'purple':/Reviewed|Imported/.test(text)?'green':''}"><i></i>${escape(text)}</span>`;
const button = (text, action, primary=false, extra='') => `<button class="button ${primary?'primary':''}" data-action="${action}" ${extra}>${text}</button>`;
const routeButton = (text, route, primary=false) => `<button class="button ${primary?'primary':''}" data-route="${route}">${text}</button>`;
const field = (name,value) => `<div class="field"><dt>${name}</dt><dd>${value}</dd></div>`;
const cap = () => capabilities.find(item=>item.id===state.candidate) || capabilities[0];
const scenarioPicker = () => `<label class="scenario">Scenario <select id="scenario">${[['sample','Sample workspace'],['empty','Empty workspace'],['partial','Partial import'],['blocked','Authority blocked'],['unknown','Outcome unknown']].map(([id,name])=>`<option value="${id}" ${state.scenario===id?'selected':''}>${name}</option>`).join('')}</select></label>`;

function rail() {
  return `<aside class="rail"><a class="brand" href="#command"><img src="assets/brand-symbol.png" alt=""><span>Rangoon<span class="brand-dot">.ai</span></span></a><div class="workspace-picker"><span class="workspace-icon">A</span><div><strong>Atlas engineering</strong><small>Personal workspace</small></div><span>⌄</span></div><p class="nav-label">WORKSPACE</p><nav aria-label="Main navigation">${routes.map(([id,name],i)=>`${i===1?'<p class="nav-label">CAPABILITY STUDIO</p>':i===5?'<p class="nav-label">OPERATE & GOVERN</p>':''}<button data-route="${id}" class="nav-item ${state.route===id?'active':''}" ${state.route===id?'aria-current="page"':''}><span aria-hidden="true">${icons[i]}</span>${name}${id==='evidence'?'<em>4</em>':''}</button>`).join('')}</nav><div class="rail-bottom"><img src="assets/mascot.webp" alt=""><strong>Small pieces.<br>Greater possibilities.</strong><span>Community edition · concept</span></div><div class="identity"><span class="avatar">AT</span><div><strong>Atlas team</strong><small>Synthetic workspace</small></div></div></aside>`;
}
function header() {
  return `<header class="topbar"><button class="search-launch" data-action="palette"><span>⌕</span> Search assets, workflows, evidence… <kbd>⌘ K</kbd></button><div class="top-controls"><span class="demo-label">DESIGN PREVIEW <i></i> SAMPLE DATA</span><button class="icon-button" data-action="density" aria-label="${state.dense?'Standard':'Compact'} density" aria-pressed="${state.dense}" title="Toggle density">☷</button><button class="icon-button" data-action="focus" aria-label="${state.focus?'Exit':'Enter'} focus mode" aria-pressed="${state.focus}" title="Focus mode">⛶</button><button class="icon-button" data-action="theme" aria-label="Switch to ${state.theme==='dark'?'light':'dark'} mode" title="Change theme">${state.theme==='dark'?'☀':'◐'}</button><span class="avatar">AT</span></div></header>`;
}
function title(kicker,name,description,actions='') {
  return `<div class="page-heading"><div><p class="eyebrow">${kicker}</p><h1 tabindex="-1">${name}</h1><p class="subtitle">${description}</p></div><div class="heading-actions">${actions}</div></div>`;
}
function panelHeading(title,extra='') { return `<div class="panel-heading"><h2>${title}</h2>${extra}</div>`; }
function empty() {
  return `<section class="empty panel"><img src="assets/mascot.webp" alt=""><p class="eyebrow">A NEW WORKSPACE</p><h1 tabindex="-1">${routes.find(([id])=>id===state.route)[1]}</h1><h2>Start with what you already know.</h2><p>Bring agent instructions into a workspace. Trace their origins, find reusable skills, and review what changes.</p>${button('Load sample workspace →','load-workspace',true)}</section>`;
}
function globalNotice() {
  if (state.scenario==='partial') return `<div class="notice">◌ <strong>Partial import.</strong> Two source formats were recognized. One script stays inert and needs review. ${routeButton('Inspect sources','import')}</div>`;
  if (state.scenario==='blocked') return `<div class="notice">◇ <strong>Execution blocked.</strong> Authority provider is unavailable. You can still inspect, compose and preview artifacts.</div>`;
  if (state.scenario==='unknown') return `<div class="notice unknown">◎ <strong>Outcome unknown.</strong> A simulated response was interrupted. Reconcile evidence before retrying. ${routeButton('Open evidence','evidence')}</div>`;
  return '';
}
function graph() {
  if (state.graphView==='list') return `<div class="graph-list">${graphNodes.map(n=>`<button data-node="${n.id}"><span class="node-icon ${n.color}">${n.icon}</span><strong>${n.name}</strong><span>${n.kind}</span>${badge(n.status)}</button>`).join('')}</div>`;
  return `<div class="graph-canvas" role="group" aria-label="Capability graph; equivalent list view is available"><svg viewBox="0 0 1000 400" preserveAspectRatio="none" aria-hidden="true"><path d="M190 188 H225 V84 H280 M225 188 V288 H280 M440 84 H485 V188 H540 M440 288 H485 V188 M700 188 H745 V84 H790 M745 188 V288 H790"/><path class="evidence-path" d="M870 132 V245"/></svg>${graphNodes.map(n=>`<button class="graph-node ${n.color} ${state.selected===n.id?'selected':''}" data-node="${n.id}" style="--x:${n.x}%;--y:${n.y}%" aria-pressed="${state.selected===n.id}"><span class="node-type">${n.kind}</span><span class="node-title"><span class="node-icon ${n.color}">${n.icon}</span>${n.name}</span><small>${n.note}</small>${badge(n.status)}</button>`).join('')}<span class="canvas-caption">ATLAS / CAPABILITY MAP <span>Dependency ─── &nbsp; Evidence ┄┄┄</span></span></div>`;
}
function nodeInspector() {
  const n = graphNodes.find(n=>n.id===state.selected) || graphNodes[1];
  const skill = capabilities.find(c=>c.id===n.id);
  return `<aside class="inspector panel">${panelHeading('Inspector','<span class="mini-label">PROVENANCE</span>')}<div class="inspector-body"><span class="node-icon large ${n.color}">${n.icon}</span><h2>${n.name}</h2>${badge(n.status)}<p>${skill?.detail || 'This relationship ties the proposed capability bundle to its source and evidence requirements.'}</p><dl>${field('Record',n.kind.toLowerCase())}${field('Version',skill?.version || 'Sample configuration')}${field('Source',skill ? `${skill.source} · ${skill.span}` : 'Atlas capability graph')}${field('Execution authority','Not granted')}</dl><h3>Evidence chain</h3><ol class="chain"><li>Original source retained</li><li>Transformation requires review</li><li>Exact target remains explicit</li><li class="muted">Runtime evidence unavailable</li></ol>${button('Trace source →','trace-source')}${routeButton('View evidence','evidence')}</div></aside>`;
}
function command() {
  return `<section class="hero"><div class="hero-content"><p class="eyebrow">COMPOSE / GOVERN / TEST / DEPLOY</p><h1 tabindex="-1">Build capable agents.<br><em>Keep them in bounds.</em></h1><p>Turn scattered instructions into reusable capabilities.<br>Keep the source, the decision, and the evidence connected.</p><div class="button-row">${routeButton('＋ Import a project','import',true)}${routeButton('Explore the workbench ↗','decompose')}</div></div><span class="hero-caption">MORE CAPABILITY. CLEARER BOUNDARIES.</span></section><div class="summary-strip"><div><span>Workspace assets</span><strong>${capabilities.length}<small>sample skills</small></strong></div><div><span>Source lineage</span><strong>3<small>instruction files</small></strong></div><div><span>Review queue</span><strong>${state.resolved?0:1}<small>merge conflict${state.resolved?'s':''}</small></strong></div><div><span>Runtime evidence</span><strong class="text-amber">—<small>not connected</small></strong></div></div><div class="workbench-layout"><section class="panel graph-panel">${panelHeading('Capability map',`<div class="segmented"><button data-graph="graph" aria-pressed="${state.graphView==='graph'}">Graph</button><button data-graph="list" aria-pressed="${state.graphView==='list'}">List</button></div>`)}${graph()}<div class="panel-bottom"><span><i class="cyan-dot"></i> Atlas engineering / review pipeline</span>${routeButton('Open workflow →','workflows')}</div></section>${nodeInspector()}</div><section class="panel recent">${panelHeading('Work in progress','<span class="mini-label">SYNTHETIC SCENARIO</span>')}<div class="activity-row"><span class="event-icon orange">⋈</span><div><strong>${state.resolved?'Network boundary retained':'Security + release rules have a conflict'}</strong><p>${state.resolved?'The sample draft preserves the restrictive boundary. Activation remains separate.':'Review the network-access requirement before creating a derived skill.'}</p></div>${routeButton('Resolve conflict →','merge')}</div><div class="activity-row"><span class="event-icon cyan">⌗</span><div><strong>Three candidate capabilities detected</strong><p>Inspect source spans and choose what becomes a skill.</p></div>${routeButton('Review candidates →','decompose')}</div></section>`;
}
function importing() {
  return `${title('DISCOVER / 01','Import & Analyze','Bring existing agent projects into a traceable workspace.',button('Load Atlas sample','import',true))}<div class="stepper"><span class="done">1 Source</span><span class="${state.imported?'done':''}">2 Analyze</span><span>3 Review</span><span>4 Create assets</span></div><div class="workbench-layout"><section class="panel">${panelHeading('Choose a source',badge(state.imported?'Imported':'Sample ready'))}<div class="source-options"><button class="active" data-action="import">▣ Local files <small>Sample available</small></button><button data-action="source-unavailable">⑂ Git repository <small>Requires host bridge</small></button><button data-action="source-unavailable">▤ Archive <small>Requires import service</small></button></div>${!state.imported?`<div class="dropzone"><span>↳</span><h2>Start with an existing project.</h2><p>Inspect an example containing AGENTS.md, SKILL.md and a release script.</p>${button('Open sample project','import',true)}<a class="button" href="analyze.html">Open desktop file analysis</a><small>Sample files stay inert. Real folder access is not connected.</small></div>`:`<div class="inventory"><div class="file-row"><span>▤</span><div><strong>AGENTS.md</strong><small>15 lines · 3 capability candidates</small></div>${badge('Imported')}</div><div class="file-row"><span>▤</span><div><strong>SKILL.md</strong><small>Documentation skill · existing format</small></div>${badge('Imported')}</div><div class="file-row"><span>⌘</span><div><strong>release.sh</strong><small>Preserved as a reference · never executed</small></div>${badge('Needs review')}</div><div class="notice"><strong>Nothing executes during import.</strong> Instructions and scripts remain source material.</div><div class="panel-bottom"><span>2 recognized · 1 reference</span>${routeButton('Review decomposition →','decompose',true)}</div></div>`}</section><aside class="panel inspector">${panelHeading('Import contract')}<div class="inspector-body"><h2>Source stays the source.</h2><p>Every proposed capability carries its origin, revision and exact source span.</p><ol class="chain"><li>Inventory before interpretation</li><li>Preserve original bytes</li><li>Show unsupported formats</li><li>Review before creating assets</li></ol><h3>Initial format targets</h3><div class="tags"><span>AGENTS.md</span><span>CLAUDE.md</span><span>SKILL.md</span><span>JSON / YAML</span></div><p class="muted">Target adapters need versioned validation before a support claim.</p></div></aside></div>`;
}
function decompose() {
  const c = cap();
  return `${title('DISCOVER / 02','Decompose','Smaller pieces. Preserved meaning. A source trail for every capability.',button(state.draftCreated?'Drafts prepared':'Prepare selected drafts','drafts',true))}<div class="decompose-layout"><aside class="panel source-tree">${panelHeading('Source files')}<div class="tree-project">⑂ Atlas engineering</div><button class="tree-row active" data-action="source-file">▤ AGENTS.md <span>3</span></button><button class="tree-row" data-action="source-file">▤ SKILL.md <span>1</span></button><button class="tree-row" data-action="script-file">⌘ release.sh <span>!</span></button><div class="source-foot">SOURCE SNAPSHOT<br><strong>Original retained</strong><small>Sample content · no disk reads</small></div></aside><section class="panel source-view">${panelHeading('AGENTS.md','<span class="mini-label">SOURCE / READ ONLY</span>')}<div class="code-source">${sourceLines.map((line,i)=>`<div class="source-line ${c.lines.includes(i+1)?'highlight':''}"><span>${i+1}</span><code>${escape(line)||' '}</code></div>`).join('')}</div><div class="panel-bottom"><span>Selected span: ${c.span}</span><span>Provenance retained ↗</span></div></section><section class="panel candidate-panel">${panelHeading('Detected modules','<span class="count">3</span>')}<div class="candidates">${capabilities.slice(0,3).map(item=>`<div class="candidate ${state.candidate===item.id?'active':''}"><button data-candidate="${item.id}"><span class="node-icon ${item.color}">▤</span><strong>${item.name}</strong><small>${item.source} / lines ${item.span}</small></button><label><input type="checkbox" data-check-candidate="${item.id}" ${state.selectedCandidates.has(item.id)?'checked':''}><span class="sr-only">Select ${item.name}</span></label></div>`).join('')}</div><div class="module-detail"><p class="eyebrow">${c.family}</p><h2>${c.name}</h2><p>${c.proposal}</p><dl>${field('Detection','Rule-based sample')}${field('Source span',c.span)}${field('Next state','Draft, awaiting review')}</dl></div><div class="panel-bottom"><span>${state.selectedCandidates.size} selected</span>${badge(state.draftCreated?'Drafts prepared':'Needs review')}</div></section></div>`;
}
function merge() {
  const ready = canCompile({conflicts:state.resolved?0:1,semanticLoss:false});
  const mergeBody = `<div class="merge-flow"><article class="mini-node orange"><span class="node-icon orange">◈</span><h3>Security Review</h3><p>Read-only repository access.<br>No external network.</p><small>v1.2.0 / AGENTS.md</small></article><div class="flow-arrow">→</div><article class="merge-result"><span class="node-icon purple">⋈</span><h2>Engineering Review</h2><p>One reviewed capability.<br>Two preserved sources.</p>${badge(ready?'Draft ready':'Blocked')}</article><div class="flow-arrow reverse">←</div><article class="mini-node cyan"><span class="node-icon cyan">↗</span><h3>Release Review</h3><p>Prepare release evidence.<br>Requests network access.</p><small>v0.3.0 / release.md</small></article></div><div class="conflict-box"><div><p class="eyebrow">${ready?'RESOLUTION RECORDED':'CONFLICT 01 / NETWORK ACCESS'}</p><h3>${ready?'Keep the stricter boundary.':'These requirements cannot both apply.'}</h3><p>${ready?'The draft retains no-network review. A future activation action requires a separate, exact authority decision.':'One input prohibits network access; the other requests it. A merge cannot silently weaken an inherited restriction.'}</p></div>${button(ready?'Reopen conflict':'Keep no-network boundary','resolve',!ready)}</div>`;
  const splitBody = `<div class="split-ledger"><article><span class="node-icon orange">▤</span><h2>Engineering Review</h2><p>Split into independently testable pieces while retaining shared context.</p></article><div>${['Security checks → Security Review','Validation commands → Test & Validate','Artifact checklist → Release Review'].map((text,i)=>`<div class="file-row"><span>0${i+1}</span><strong>${text}</strong>${badge('Assigned')}</div>`).join('')}<div class="notice">Shared project context is referenced by every output. No source fragment is silently discarded.</div></div></div>`;
  const historyBody = `<ol class="history-list"><li>Two source revisions selected.</li><li>Network-access conflict detected.</li>${state.history.map(item=>`<li>${escape(item)}</li>`).join('')}</ol>`;
  return `${title('CAPABILITY STUDIO','Merge & Split','Reorganize capabilities without losing their origins or weakening constraints.')}<div class="tabs">${['merge','split','history'].map(mode=>`<button data-mode="${mode}" aria-pressed="${state.mode===mode}" class="${state.mode===mode?'active':''}">${mode[0].toUpperCase()+mode.slice(1)}</button>`).join('')}</div><section class="panel">${panelHeading(state.mode==='merge'?'Compose a derived skill':state.mode==='split'?'Source assignment ledger':'Transformation history','<span class="mini-label">PROPOSAL / NO ACTIVATION</span>')}${state.mode==='merge'?mergeBody:state.mode==='split'?splitBody:historyBody}<div class="panel-bottom"><span>${ready?'Inert compilation available':'Resolve the conflict before compiling'}</span>${state.mode==='history'?'':button('Preview compiled draft →','compile',true,ready?'':'disabled')}</div></section>${state.compiled?`<section class="panel compile-output">${panelHeading('Compiled draft preview',badge('Not deployed'))}<pre><code>${state.mode==='split'?'# Split draft set\n\n1. Security Review — original security span\n2. Test & Validate — original verification span\n3. Release Review — original artifact checklist\n\nShared context: Atlas engineering (reference retained)\nBoundary: no-network review retained\nAuthority: no execution authorization created':'# Engineering Review\n\n+ Review changed code and record source evidence.\n+ Preserve the no-network review boundary.\n+ Prepare release artifacts without activation.\n\nSources: Security Review@1.2.0, Release Review@0.3.0\nTarget: AGENTS.md / sample adapter\nAuthority: no execution authorization created'}</code></pre></section>`:''}`;
}
function skillsPage() {
  const items = filterItems(capabilities,state.filter,['name','family','harness']);
  const chosen = capabilities.find(c=>c.id===state.selected) || capabilities[0];
  return `${title('BUILD / LIBRARY','Skills','Versioned, reusable capabilities. Know what changed and what depends on it.',routeButton('Discover a skill →','decompose',true))}<div class="workbench-layout"><section class="panel">${panelHeading('Capability library',`<input id="skill-filter" type="search" value="${escape(state.filter)}" placeholder="Filter skills…" aria-label="Filter skills">`)}<div class="table-wrap"><table><thead><tr><th>Skill</th><th>Targets</th><th>Version</th><th>Review state</th></tr></thead><tbody>${items.map(c=>`<tr class="${chosen.id===c.id?'selected':''}"><td><button class="table-select" data-skill="${c.id}"><span class="node-icon ${c.color}">▤</span><span><strong>${c.name}</strong><small>${c.family}</small></span></button></td><td>${c.harness}</td><td class="mono">${c.version}</td><td>${badge(c.status)}</td></tr>`).join('')}</tbody></table>${items.length?'':'<div class="empty-small">No skills match. Try “Security” or “Claude”.</div>'}</div><div class="panel-bottom"><span>${items.length} of ${capabilities.length} sample skills</span><span>Review state ≠ execution authority</span></div></section><aside class="panel inspector">${panelHeading('Skill detail')}<div class="inspector-body"><span class="node-icon large ${chosen.color}">▤</span><h2>${chosen.name}</h2>${badge(chosen.status)}<p>${chosen.detail}</p><dl>${field('Version',chosen.version)}${field('Origin',`${chosen.source}:${chosen.span}`)}${field('Harness targets',chosen.harness)}${field('Deployment','Not deployed')}</dl><h3>Lineage</h3><ol class="chain"><li>${chosen.source}</li><li>Reviewed extraction proposal</li><li>${chosen.name} ${chosen.version}</li></ol>${button('Open source workbench →','trace-source')}</div></aside></div>`;
}
function workflows() {
  const result = workflowOutcome(state.outcome);
  return `${title('BUILD / ORCHESTRATE','Workflows','Make the next step—and its authority boundary—explicit.',button('Run sample simulation','simulate',true))}<div class="workbench-layout"><section class="panel">${panelHeading('Engineering review pipeline',badge('Draft'))}<div class="workflow-canvas">${[['①','Source change','Selected repository snapshot','cyan'],['◈','Review + test','Pinned skill revisions','orange'],['◇','Policy evaluation','Exact proposed action','purple'],['◉','Human approval','Only when policy requires it','orange'],['◎','Receipt or unknown','Reconcile before another effect','green']].map(([icon,name,note,color],i)=>`<div class="workflow-node"><span class="node-icon ${color}">${icon}</span><div><strong>${name}</strong><small>${note}</small></div>${i===3?badge(!state.simulation?'Not reached':state.outcome==='approval'?'Awaiting approval':state.outcome==='denied'?'Blocked':'Outcome unknown'):''}</div>${i<4?'<span class="workflow-link">↓</span>':''}`).join('')}</div><div class="panel-bottom"><span>Simulation uses synthetic events only</span>${badge(state.simulation?'Simulation complete':'Not run')}</div></section><aside class="panel inspector">${panelHeading('Simulation scenario')}<div class="inspector-body"><h2>Exercise the difficult path.</h2><p>Select a result to inspect how the workflow should respond.</p><div class="scenario-buttons">${[['approval','Approval required'],['denied','Policy denied'],['unknown','Outcome unknown']].map(([id,name])=>`<button class="${state.outcome===id?'selected':''}" data-outcome="${id}" aria-pressed="${state.outcome===id}">${name}</button>`).join('')}</div>${state.simulation?`<div class="notice ${state.outcome==='unknown'?'unknown':''}"><strong>${state.outcome==='approval'?'Awaiting approval':state.outcome==='denied'?'Blocked by policy':'Outcome unknown'}</strong><p>${state.outcome==='approval'?'The exact action would wait for a scoped decision. Approval would still need authorization and one-time consumption.':result.label}</p></div><dl>${field('External actions','None executed')}${field('Automatic retry','Disabled')}${field('Evidence class','Simulation')}</dl>`:'<p class="muted">Run the sample to create a visible simulated result.</p>'}</div></aside></div>`;
}
function connectors() {
  const c = connectorRecords.find(c=>c.id===state.connector);
  return `${title('CONNECT / INTEGRATIONS','Connectors','Access is a connection. Authority is a separate decision.')}<div class="workbench-layout"><section class="panel">${panelHeading('Connection catalog',badge('Planned adapters'))}<div class="connector-grid">${connectorRecords.map(c=>`<button class="connector ${c.id===state.connector?'selected':''}" data-connector="${c.id}" aria-pressed="${c.id===state.connector}"><span class="node-icon large ${c.color}">${c.icon}</span><h3>${c.name}</h3><p>${c.category}</p><span class="connector-state">○ ${c.credential}</span></button>`).join('')}</div><div class="panel-bottom"><span>Catalog targets · no live integration implied</span><span>Versioned adapter contracts</span></div></section><aside class="panel inspector">${panelHeading('Connection detail')}<div class="inspector-body"><span class="node-icon large ${c.color}">${c.icon}</span><h2>${c.name}</h2><p>${c.scope}</p><dl>${field('Operation',c.operation)}${field('Side effect',c.effect)}${field('Credential/access',c.credential)}${field('Action authority',c.authority)}</dl><div class="notice">Installation, credentials and successful connection tests never grant an action permission.</div>${button('View configuration requirements','connector-info')}</div></aside></div>`;
}
function evidencePage() {
  const entries = state.evidenceFilter==='all'?ledger:ledger.filter(item=>item.status.toLowerCase()===state.evidenceFilter);
  const e = ledger.find(item=>item.id===state.selectedEvidence);
  return `${title('GOVERN / AUDIT','Evidence Explorer','Trace what was proposed, what was decided, and what remains unknown.')}<div class="workbench-layout"><section class="panel">${panelHeading('Evidence ledger')}<div class="tabs compact-tabs">${['all','blocked','unknown','missing'].map(filter=>`<button data-evidence-filter="${filter}" aria-pressed="${state.evidenceFilter===filter}" class="${state.evidenceFilter===filter?'active':''}">${filter}</button>`).join('')}</div><div class="evidence-list">${entries.map(item=>`<button data-evidence="${item.id}" class="${e.id===item.id?'selected':''}"><span class="event-icon ${item.status==='Unknown'?'purple':'cyan'}">◎</span><div><small>${item.id} / ${item.type}</small><strong>${item.name}</strong><p>${item.detail}</p></div>${badge(item.status)}</button>`).join('')}</div></section><aside class="panel inspector">${panelHeading('Evidence detail')}<div class="inspector-body"><p class="eyebrow">${e.id} / ${e.type}</p><h2>${e.name}</h2>${badge(e.status)}<p>${e.detail}</p><dl>${field('Reference',e.refs)}${field('Origin','Synthetic fixture')}${field('Live verification','Unavailable')}</dl>${e.status==='Unknown'?'<div class="notice unknown">An interrupted response is not proof of failure. No blind retry.</div>':''}</div></aside></div>`;
}
function release() {
  return `${title('DEPLOY / ROADMAP','One product. Three desktops.','A shared capability workbench, with platform support earned by evidence.')}<div class="platform-grid">${[['macOS','Apple silicon','Signed app + notarized DMG','Keychain · folder permissions · WebView'],['Windows','Windows 11 / x64','Signed installer','WebView2 · Credential Manager · path semantics'],['Linux','x64 / Ubuntu LTS baseline','AppImage or deb qualification','WebKitGTK · secret service · Wayland/X11']].map(([name,cpu,artifact,note])=>`<article class="panel platform"><span class="platform-symbol">${name==='macOS'?'⌘':name==='Windows'?'⊞':'◈'}</span><p class="eyebrow">INITIAL RELEASE TARGET</p><h2>${name}</h2><p>${cpu}</p><strong>${artifact}</strong><small>${note}</small>${badge('Evidence missing')}</article>`).join('')}</div><section class="panel roadmap">${panelHeading('Build sequence','<span class="mini-label">PROPOSED / FOR REVIEW</span>')}${[['01','Capability workbench','Inert import → lineage → decompose → merge/split → test → compile/export.','All three desktops'],['02','Governed execution','One qualified LNSAT path, disposable Git consequence, receipt and reconciliation.','Separate runtime gate'],['03','Self-hosted teams','Linux service, identity, workspace roles, backup/restore and durable jobs.','Owner controlled'],['04','Enterprise cloud','Hosted operations, private registries, SSO, retention and hybrid workers.','Future expansion']].map(([num,name,detail,b])=>`<div class="roadmap-row"><span>${num}</span><div><h3>${name}</h3><p>${detail}</p></div>${badge(b)}</div>`).join('')}<div class="notice">Desktop management can ship before governed native execution. Every release advertises only its qualified OS, architecture and action paths.</div></section>`;
}
function footer() {
  return `<footer class="bottom-strip"><span><i class="cyan-dot"></i> Local design preview</span><span class="authority-chain">Source <b>→</b> Capability <b>→</b> Review <b>→</b> Authority <b>→</b> Evidence</span><span>Execution <strong>not connected</strong></span></footer>`;
}
const views = {command,import:importing,decompose,merge,skills:skillsPage,workflows,connectors,evidence:evidencePage,release};
function render({preserveFocus=true}={}) {
  const active = document.activeElement;
  const identity = active?.id || '';
  const caret = active instanceof HTMLInputElement ? active.selectionStart : null;
  const selector = active?.dataset ? Object.entries(active.dataset).find(([key])=>['action','route','candidate','skill','node','connector','evidence','outcome','mode','graph','checkCandidate','evidenceFilter'].includes(key)) : null;
  document.documentElement.dataset.theme=state.theme;
  document.title=`Rangoon — ${routes.find(([id])=>id===state.route)[1]} preview`;
  $('#app').innerHTML=`<div class="app ${state.focus?'focus':''} ${state.dense?'dense':''}">${rail()}<div class="main-shell">${header()}<main id="main" tabindex="-1">${state.route==='command'?'':scenarioPicker()}${globalNotice()}${state.scenario==='empty'?empty():views[state.route]()}</main>${footer()}</div></div>`;
  if(preserveFocus) {
    const kebab = selector?.[0].replace(/[A-Z]/g, char=>'-'+char.toLowerCase());
    const target = identity ? document.getElementById(identity) : selector ? document.querySelector(`[data-${kebab}="${CSS.escape(selector[1])}"]`) : null;
    (target || $('main h1') || $('main'))?.focus({preventScroll:true});
    if(caret!==null && target instanceof HTMLInputElement && target.type!=='search') target.setSelectionRange(caret,caret);
  }
}
let returnFocus;
function openPalette() {
  if ($('#command-dialog')) return;
  returnFocus=document.activeElement;
  const dialog=document.createElement('dialog');dialog.id='command-dialog';dialog.setAttribute('aria-label','Navigate Rangoon');
  dialog.innerHTML='<div class="palette-head"><span>⌕</span><input id="palette-search" aria-label="Search pages" placeholder="Where would you like to go?"><button data-action="close-palette" aria-label="Close navigation">Esc</button></div><div id="palette-results"></div><p class="palette-foot">Navigate the sample workspace · No external action</p>';
  document.body.append(dialog);updatePalette('');dialog.showModal();$('#palette-search').focus();
  dialog.addEventListener('close',()=>{dialog.remove();returnFocus?.isConnected && returnFocus.focus();});
  dialog.addEventListener('click',event=>{if(event.target===dialog)dialog.close();});
}
function updatePalette(query) {
  const matches=routes.filter(([,name])=>name.toLowerCase().includes(query.toLowerCase()));
  $('#palette-results').innerHTML=matches.length?matches.map(([id,name])=>`<button data-route="${id}"><span>${name}</span><span>↗</span></button>`).join(''):'<p class="empty-small">No matching page.</p>';
}
function notify(message) {
  $('#toast')?.remove();const div=document.createElement('div');div.id='toast';div.setAttribute('role','status');div.textContent=message;document.body.append(div);setTimeout(()=>div.remove(),5000);
}
function focusRoute() {
  ($('main h1') || $('main'))?.focus({preventScroll:true});
  window.scrollTo({top:0});
}
function navigate(route) {
  if(!views[route])return;
  $('#command-dialog')?.close();state.route=route;state.filter='';location.hash=route;render({preserveFocus:false});focusRoute();
}
document.addEventListener('click',event=>{
  const control=event.target.closest('button,[data-action]');if(!control)return;
  const d=control.dataset;
  if(d.route)return navigate(d.route);
  if(d.node){state.selected=d.node;render();return;}
  if(d.candidate){state.candidate=d.candidate;render();return;}
  if(d.skill){state.selected=d.skill;render();return;}
  if(d.mode){state.mode=d.mode;state.compiled=false;render();return;}
  if(d.graph){state.graphView=d.graph;render();return;}
  if(d.connector){state.connector=d.connector;render();return;}
  if(d.evidenceFilter){state.evidenceFilter=d.evidenceFilter;const entry=ledger.find(x=>d.evidenceFilter==='all'||x.status.toLowerCase()===d.evidenceFilter);state.selectedEvidence=entry?.id||'ev1';render();return;}
  if(d.evidence){state.selectedEvidence=d.evidence;render();return;}
  if(d.outcome){state.outcome=d.outcome;state.simulation=false;render();return;}
  switch(d.action){
    case 'trace-source':if(['review','tests','release'].includes(state.selected)){state.candidate=state.selected;return navigate('decompose');}if(state.selected==='source')return navigate('decompose');return notify('This sample includes the AGENTS.md viewer. The selected record has no source viewer connected yet.');
    case 'palette':return openPalette();
    case 'close-palette':return $('#command-dialog')?.close();
    case 'theme':state.theme=state.theme==='dark'?'light':'dark';try{localStorage.setItem('rangoon-theme',state.theme);}catch{}break;
    case 'density':state.dense=!state.dense;break;
    case 'focus':state.focus=!state.focus;break;
    case 'load-workspace':state.scenario='sample';break;
    case 'import':state.imported=true;state.scenario='sample';break;
    case 'drafts':if(!state.selectedCandidates.size)return notify('Select at least one source candidate.');state.draftCreated=true;notify(`${state.selectedCandidates.size} draft proposals prepared in this sample. No source files changed.`);break;
    case 'resolve':state.resolved=!state.resolved;state.compiled=false;state.history.push(state.resolved?'Retained the no-network boundary.':'Reopened network conflict for review.');break;
    case 'compile':if(state.mode==='history')return;if(!canCompile({conflicts:state.resolved?0:1,semanticLoss:false}))return;state.compiled=true;state.history.push(`Generated an inert ${state.mode} draft preview; no deployment.`);break;
    case 'simulate':state.simulation=true;break;
    case 'source-unavailable':return notify('Real imports require the scoped desktop bridge and import service. Use the supplied sample here.');
    case 'source-file':return notify('This workbench displays the AGENTS.md sample. Other file viewers are planned.');
    case 'script-file':return notify('release.sh is retained as an inert reference. Import never executes it.');
    case 'connector-info':return notify('A real adapter needs a version, explicit scopes, credential reference, limits and evidence contract. No connection is created here.');
    default:return;
  }
  render();
});
document.addEventListener('input',event=>{
  if(event.target.id==='skill-filter'){state.filter=event.target.value;const caret=event.target.selectionStart;render();const target=$('#skill-filter');target.focus();if(caret!==null)target.setSelectionRange(caret,caret);}
  if(event.target.id==='palette-search')updatePalette(event.target.value);
});
document.addEventListener('change',event=>{
  if(event.target.id==='scenario'){state.scenario=event.target.value;render();}
  const id=event.target.dataset.checkCandidate;
  if(id){event.target.checked?state.selectedCandidates.add(id):state.selectedCandidates.delete(id);state.draftCreated=false;render();}
});
document.addEventListener('keydown',event=>{
  if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='k'){event.preventDefault();openPalette();}
  if(event.key==='Escape' && !$('#command-dialog') && state.focus){state.focus=false;render();}
  if(event.key==='Enter' && event.target.id==='palette-search')$('#palette-results button')?.click();
});
window.addEventListener('hashchange',()=>{const route=location.hash.slice(1);if(views[route]&&state.route!==route){state.route=route;render({preserveFocus:false});focusRoute();}});
if(views[location.hash.slice(1)])state.route=location.hash.slice(1);
render();
