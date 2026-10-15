import { createFlowScene, defaultCamera, normalizeCamera } from './flow-scene.mjs';
import { icon } from './icons.mjs';
import { FLOW_NODES, FLOW_EDGES, FLOW_REQUEST, FLOW_SCENARIOS, FLOW_SYSTEMS, createFlowSimulation, pointOnFlowEdge } from './flow-simulation.mjs';
import { bindWorkbenchNavigation, renderWorkbenchNavigation } from './workbench-navigation.mjs';

const escapeHtml = value => String(value ?? '').replace(/[&<>'"]/g, char => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;' })[char]);
const finite = (value, fallback = 0) => Number.isFinite(Number(value)) ? Number(value) : fallback;
const coordinate = (value, max) => Math.max(0, Math.min(max, finite(value)));
const pointList = points => Array.isArray(points) ? points.map(point => `${coordinate(point?.[0], 950)},${coordinate(point?.[1], 560)}`).join(' ') : '';
const byId = (items, id) => items.find(item => item.id === id) ?? null;
const titleCase = value => String(value ?? 'unknown').replaceAll(/[-_]/g, ' ');
const systemEntries = () => Object.entries(FLOW_SYSTEMS ?? {}).map(([id, system]) => ({ id, ...system }));
const systemFor = id => systemEntries().find(system => system.id === id) ?? byId(FLOW_NODES, id) ?? { id, label: id, title: id, type: 'system' };
const systemLabel = system => system.label ?? system.title ?? system.id;
const requestValue = key => FLOW_REQUEST?.[key] ?? 'Synthetic fixture';
const whatItDoesNotAuthorize = event => {
  if (event.stage === 'refusal' && event.reported?.includes?.('expired')) return 'Earlier approval and issued-reference observations do not permit expired consumption.';
  if (event.stage === 'refusal' || event.reported?.includes?.('refused') || event.reported?.includes?.('denied')) return 'No approval, scoped authorization, tool attempt, or successful outcome.';
  if (event.stage === 'unknown' || event.evidenceClass === 'missing') return 'No completed outcome or verified receipt.';
  if (event.stage === 'recommendation') return 'No human approval, scoped authorization, or execution grant.';
  if (event.stage === 'approval') return 'No scoped authorization, consumption, or tool result.';
  if (event.stage === 'authorization') return 'No consumption check or outcome.';
  if (event.stage === 'consumption') return 'No verified outcome receipt.';
  return 'No real connection, execution, or authority.';
};
const scenarioSummary = scenario => {
  if (scenario.id === 'denied') return 'Final result: refusal. Policy stops this synthetic request before approval or execution.';
  if (scenario.id === 'expired') return 'Final result: refusal. The execution-time boundary rejects expired scope.';
  if (scenario.id === 'unknown') return 'Final result: unknown. An accepted consumption observation has no outcome evidence; reconciliation remains required.';
  return 'Final result: synthetic completion only. This illustration is not a verified receipt or real execution.';
};
const nodeGraphic = id => ({
  agent: '<svg viewBox="0 0 48 48" aria-hidden="true" focusable="false"><rect x="10" y="12" width="28" height="24" rx="8"></rect><path d="M24 8v4M17 23h.01M31 23h.01M17 30c4 3 10 3 14 0"></path></svg>',
  gateway: '<svg viewBox="0 0 48 48" aria-hidden="true" focusable="false"><path d="M9 37V22a15 15 0 0 1 30 0v15M16 37V24a8 8 0 0 1 16 0v13M7 37h34"></path></svg>',
  policy: '<svg viewBox="0 0 48 48" aria-hidden="true" focusable="false"><path d="M12 14h24v7H12zM9 24h30v7H9zM12 34h24v5H12z"></path></svg>',
  human: '<svg viewBox="0 0 48 48" aria-hidden="true" focusable="false"><circle cx="24" cy="16" r="7"></circle><path d="M11 39c1-9 7-13 13-13s12 4 13 13M33 28l4 4 6-7"></path></svg>',
  authority: '<svg viewBox="0 0 48 48" aria-hidden="true" focusable="false"><path d="m24 8 15 10-15 10L9 18 24 8ZM9 18v12l15 10 15-10V18M24 28v12"></path></svg>',
  executor: '<svg viewBox="0 0 48 48" aria-hidden="true" focusable="false"><path d="M11 11h26v26H11zM16 24h16M24 16v16M8 24h5M35 24h5"></path></svg>',
  tool: '<svg viewBox="0 0 48 48" aria-hidden="true" focusable="false"><ellipse cx="24" cy="12" rx="13" ry="5"></ellipse><path d="M11 12v20c0 3 6 5 13 5s13-2 13-5V12M11 22c0 3 6 5 13 5s13-2 13-5"></path></svg>',
  evidence: '<svg viewBox="0 0 48 48" aria-hidden="true" focusable="false"><path d="M13 10h18l6 6v22H13zM31 10v7h6M18 24h12M18 30h9M18 36h6"></path></svg>',
}[id] ?? '<svg viewBox="0 0 48 48" aria-hidden="true" focusable="false"><circle cx="24" cy="24" r="13"></circle></svg>');

function selectedDetail(state) {
  const scenario = state.scenario ?? { events: [] };
  const events = Array.isArray(scenario.events) ? scenario.events : [];
  if (state.selection?.kind === 'node') {
    const system = systemFor(state.selection.id);
    if (system) return { eyebrow: `SYSTEM · ${titleCase(system.type ?? system.kind)}`, title: systemLabel(system), detail: 'Fixed synthetic system. Its label and geometry are illustrative; it does not connect to a service.', fields: [['Role', system.role ?? 'Synthetic topology role'], ['Receives', system.receives ?? 'Declared fixture input'], ['Sends', system.sends ?? 'Declared fixture output'], ['Boundary', system.boundary ?? 'No live authority'], ['Next inspect', system.next ?? 'Select a reached scripted event']] };
  }
  if (state.selection?.kind === 'event' && state.selection.index < state.cursor) {
    const event = events[state.selection.index];
    if (event) return { eyebrow: `EVENT · ${titleCase(event.stage)}`, title: event.title, detail: event.detail, fields: [['Evidence class', event.evidenceClass], ['Current state', event.reported], ['Stop reason', ['refusal', 'unknown'].includes(event.stage) ? event.detail : 'No refusal recorded in this selected event'], ["What it doesn't authorize", whatItDoesNotAuthorize(event)], ['Next inspect', event.stage === 'refusal' ? 'Inspect policy inputs, scope and the pinned revision.' : event.stage === 'unknown' ? 'Reconcile the missing outcome; do not assume success.' : event.next ?? 'Inspect the next reached boundary or its evidence']] };
  }
  return { eyebrow: 'SYNTHETIC REQUEST', title: requestValue('title'), detail: scenario.description ?? 'Choose a system or reached event to inspect this bounded fixture.', fields: [['Repository', requestValue('repository')], ['Branch', requestValue('branch')], ['Action', requestValue('action')], ['Current stop', scenarioSummary(scenario)], ['Next inspect', 'Select the visible refusal or evidence boundary']] };
}

const fieldList = fields => `<dl class="flow-fields">${fields.map(([name, value]) => `<div><dt>${escapeHtml(name)}</dt><dd>${escapeHtml(value)}</dd></div>`).join('')}</dl>`;
const fieldsNamed = (fields, names) => fields.filter(([name]) => names.has(name));
function detailSections(detail, scenario, state, events) {
  const fields = Array.isArray(detail.fields) ? detail.fields : [];
  const role = fieldsNamed(fields, new Set(['Role', 'Evidence class', 'Current state', 'Repository', 'Action']));
  const inputOutput = fieldsNamed(fields, new Set(['Receives', 'Sends', 'Branch', 'Next inspect']));
  const boundary = fieldsNamed(fields, new Set(['Boundary', 'Stop reason', "What it doesn't authorize", 'Current stop']));
  const inputRows = inputOutput.length ? inputOutput : [['Illustrated request', requestValue('title')], ['Affected resource', requestValue('repository')]];
  const provenance = [['Trace', scenario.traceId], ['Policy revision', scenario.policyRevisionId], ['Topology revision', scenario.topologyRevisionId], ['Selected event', state.selection?.kind === 'event' ? events[state.selection.index]?.id : 'No event selected'], ['Authenticity', state.authenticity], ['Execution', state.execution]];
  const section = (title, rows, open = false) => `<details class="flow-inspector-detail"${open ? ' open' : ''}><summary>${title}</summary>${fieldList(rows)}</details>`;
  return `${section('Role and current state', role.length ? role : [['Role', detail.eyebrow]], true)}${section('Inputs and outputs', inputRows)}${section('Boundary', boundary.length ? boundary : [["What it doesn't authorize", 'No real connection, execution, or authority.']])}${section('Provenance', provenance)}`;
}

function directionStrip(scenario, events, cursor) {
  const reached = cursor > 0 ? events[cursor - 1] : null;
  const reachedLabel = reached ? `${reached.title} · ${reached.reported}` : 'No event reached yet';
  const stages = ['Request', 'Policy', 'Decision', 'Scoped reference', 'Consumption', 'Evidence'];
  return `<section class="flow-direction-strip" aria-label="Illustrated route direction"><span>ROUTE DIRECTION</span><ol>${stages.map(stage => `<li>${stage}</li>`).join('')}</ol><strong>Reached: ${escapeHtml(reachedLabel)}</strong><small>Fixture only</small></section>`;
}

function topologyList(state) {
  const selectedId = state.selection?.kind === 'node' ? state.selection.id : null;
  return `<details class="flow-outline" aria-label="Complete flow topology list" ${state.depthUnavailable || state.graphicsUnavailable ? 'open' : ''}><summary>Complete topology list</summary><h3>Eight systems</h3><ol>${FLOW_NODES.map(node => { const system = systemFor(node.id); return `<li><button type="button" data-flow-node="${escapeHtml(node.id)}" data-flow-focus="list:${escapeHtml(node.id)}" aria-pressed="${selectedId === node.id}">${escapeHtml(titleCase(system.type ?? node.kind))}: ${escapeHtml(systemLabel(system))}</button></li>`; }).join('')}</ol><h3>Ten directed boundaries</h3><ol>${FLOW_EDGES.map(edge => `<li>${escapeHtml(edge.from)} → ${escapeHtml(edge.kind)} → ${escapeHtml(edge.to)}</li>`).join('')}</ol></details>`;
}

function controlBar(state) {
  const unavailable=state.reducedMotion||state.depthUnavailable||state.graphicsUnavailable;
  const count=state.scenario?.events?.length??0,cursor=state.cursor??0;
  return `<div class="flow-control-dock" role="group" aria-label="Replay and view controls"><button class="flow-button flow-button--primary" type="button" data-flow-play ${unavailable?'disabled':''}>${state.playing?'Pause':cursor>=count?'Replay':'Play'}</button><button class="flow-button flow-icon-button" type="button" data-flow-step="-1" aria-label="Previous event" title="Previous event" ${cursor===0?'disabled':''}>←</button><button class="flow-button flow-icon-button" type="button" data-flow-step="1" aria-label="Next event" title="Next event" ${cursor>=count?'disabled':''}>→</button><label class="flow-bar-scrub"><span>Event ${cursor}/${count}</span><input data-flow-seek type="range" min="0" max="${count}" value="${cursor}" aria-label="Scripted event position"></label><button class="flow-button" type="button" data-flow-mode="depth" aria-pressed="${state.mode==='depth'}" ${unavailable?'disabled':''}>3D</button><button class="flow-button" type="button" data-flow-mode="flat" aria-pressed="${state.mode!=='depth'}">2D</button><button class="flow-button flow-icon-button" type="button" data-flow-camera="reset" aria-label="Reset view" title="Reset view" ${unavailable||state.mode!=='depth'?'disabled':''}>${icon('focus',{size:16})}</button><details class="flow-view-options" data-flow-view-options><summary aria-label="More view controls" title="More view controls">${icon('density',{size:16})}</summary><div><p>View only · no workflow changes</p>${[['left','Orbit left'],['right','Orbit right'],['in','Zoom in'],['out','Zoom out']].map(([action,label])=>`<button class="flow-button" type="button" data-flow-camera="${action}" ${unavailable||state.mode!=='depth'?'disabled':''}>${label}</button>`).join('')}<button class="flow-button" type="button" data-flow-reset>Reset script</button></div></details></div>`;
}

function scene(state) {
  const selectedId = state.selection?.kind === 'node' ? state.selection.id : null;
  const events = state.scenario?.events ?? [];
  const active = state.playing ? events[state.cursor] : state.selection?.kind === 'event' ? events[state.selection.index] : null;
  const useFlat = state.mode !== 'depth' || state.depthUnavailable || state.graphicsUnavailable || state.reducedMotion;
  const graphicsMessage = state.graphicsUnavailable ? `3D graphics unavailable${state.graphicsReason ? `: ${escapeHtml(state.graphicsReason)}` : ''}. Complete 2D/list inspection remains available.` : '3D scene uses local synthetic geometry only.';
  return `<div class="flow-stage-wrap"><div class="flow-stage" data-flow-stage data-flow-mode="${useFlat ? 'flat' : 'depth'}" tabindex="0" role="group" aria-label="System map"><canvas data-flow-canvas aria-hidden="true"></canvas><div class="flow-stage__nodes" aria-label="Inspect a system">${FLOW_NODES.map(node => { const system = systemFor(node.id); const on = active?.nodeId === node.id; const authorityMark = node.id === 'authority' ? '<span class="flow-node__authority-mark"><img src="assets/authority/lnsat-mark.webp" alt="LNSAT example only"><small>Example only</small></span>' : ''; return `<button class="flow-node${on ? ' flow-node--active' : ''}" type="button" data-flow-node="${escapeHtml(node.id)}" data-flow-projected data-flow-focus="graph:${escapeHtml(node.id)}" aria-pressed="${selectedId === node.id}" style="--flow-x:${coordinate(node.x, 950) / 9.5}%;--flow-y:${coordinate(node.y, 560) / 5.6}%"><span class="flow-node__mesh">${nodeGraphic(node.id)}</span><span class="flow-node__type">${escapeHtml(titleCase(system.type ?? node.kind))}</span><span class="flow-node__title">${escapeHtml(systemLabel(system))}</span>${authorityMark}${on ? `<span class="flow-node__cue" data-flow-cue>${escapeHtml(active.reported)} · ${escapeHtml(titleCase(active.stage))}</span>` : '<span class="flow-node__cue" data-flow-cue hidden></span>'}</button>`; }).join('')}</div>${useFlat ? '<div class="flow-stage__flat" data-flow-flat aria-hidden="true">2D fixture map</div>' : ''}<p class="flow-scene-status" data-flow-graphics-state role="status">${graphicsMessage}</p></div>${controlBar(state)}</div>${topologyList(state)}`;
}

export function renderFlowPlayground(state) {
  const scenario = state.scenario ?? { events: [] };
  const events = Array.isArray(scenario.events) ? scenario.events : [];
  const cursor = Math.max(0, Math.min(events.length, Math.floor(finite(state.cursor))));
  const detail = selectedDetail({ ...state, scenario, cursor });
  const future = index => index >= cursor;
  const theme = typeof document !== 'undefined' && document.documentElement?.dataset.theme === 'light' ? 'light' : 'dark';
  const viewUnavailable = Boolean(state.reducedMotion || state.depthUnavailable || state.graphicsUnavailable);
  const playDisabled = viewUnavailable;
  return `<div class="analysis-shell" data-area="workflows">${renderWorkbenchNavigation({ area: 'workflows', surface: 'flow-playground', theme, open: state.navigationOpen ?? true })}<main id="flow-main" class="flow-page" tabindex="-1"><header class="flow-hero"><p class="analysis-kicker">SYNTHETIC REQUEST INSPECTION · NO SYSTEMS CONNECTED</p><h1>Trace a proposed <em>action</em>.</h1><p>Inspect where this example request stops and what evidence exists.</p><div class="flow-request-card" aria-label="Illustrated proposed request"><span>PROPOSED REQUEST</span><strong>${escapeHtml(requestValue('title'))}</strong><small>${escapeHtml(requestValue('repository'))} · ${escapeHtml(requestValue('branch'))}</small><p>${escapeHtml(requestValue('action'))}</p></div><div class="flow-truth"><span><strong>SYNTHETIC</strong></span><span>NO SYSTEMS CONNECTED</span><span>NO ENGINE ACTION</span><span>AUTHORITY: ${escapeHtml(state.authority)}</span></div><a class="flow-editor-link" href="workflow-playground.html"><strong>Open 3D graph editor</strong><span>Separate local/synthetic workflow editor. This trace does not transfer.</span></a></header><div class="flow-scenario-row" aria-label="Synthetic scenarios">${FLOW_SCENARIOS.map(item => `<button class="flow-scenario" type="button" data-flow-scenario="${escapeHtml(item.id)}" aria-pressed="${item.id === state.scenarioId}"><strong>${escapeHtml(item.title)}</strong><small>${escapeHtml(item.description)}</small></button>`).join('')}</div><div class="flow-workbench"><section class="flow-canvas-panel" aria-label="Synthetic task-focused scene"><div class="flow-map-heading"><span>REQUEST ROUTE</span><strong>${escapeHtml(scenario.id==='denied'?'Policy refusal':scenario.id==='expired'?'Scope expired':scenario.id==='unknown'?'Outcome unknown':'Mediated route')}</strong><small>8 systems · 10 boundaries · synthetic</small></div>${directionStrip(scenario, events, cursor)}${scene(state)}<p class="flow-scene-note">${state.reducedMotion?'Reduced motion: manual steps only.':state.depthUnavailable?'List view: manual steps retain every system and boundary.':'Drag map to orbit · scroll to zoom · click a system to inspect its role.'}</p><details class="flow-timeline-details" open><summary>Chronological replay <span>${cursor} reached of ${events.length}</span></summary><ol class="flow-timeline" aria-label="Ordered scripted event timeline">${events.map((event, index) => `<li><button type="button" data-flow-event="${index}" ${future(index) ? 'disabled' : ''} aria-pressed="${state.selection?.kind === 'event' && state.selection.index === index}"><span class="flow-timeline__number">${String(index + 1).padStart(2, '0')}</span><span><strong>${escapeHtml(event.title)}</strong><small>${escapeHtml(titleCase(event.stage))} · ${escapeHtml(event.reported)}</small></span></button></li>`).join('')}</ol></details></section><aside class="flow-panel flow-inspector-panel" aria-label="Decision-focused inspector"><header class="flow-panel__head"><div><h2>Decision inspector</h2><p>Reached evidence only</p></div><button class="flow-inspect-end" type="button" data-flow-inspect-end>Inspect blocker / evidence</button></header><div class="flow-inspector"><div class="flow-inspector__eyebrow">${escapeHtml(detail.eyebrow)}</div><h2>${escapeHtml(detail.title)}</h2><p>${escapeHtml(detail.detail)}</p>${detailSections(detail, scenario, state, events)}<p class="flow-final-summary">${escapeHtml(scenarioSummary(scenario))}</p><p class="flow-boundary"><strong>SYNTHETIC · NO SYSTEMS CONNECTED · NO ENGINE ACTION.</strong> A recommendation is not approval, authorization, consumption, or outcome.</p></div></aside></div></main></div>`;
}

export function bindFlowPlayground(root, controller, { window: viewWindow = globalThis.window, sceneFactory = createFlowScene } = {}) {
  let disposed=false, frame=null, previousFrame=null, lastRender=null, scene=null, camera={...defaultCamera}, cameraDirty=false;
  let rendering=false, batching=false, deferred=false, gesture=null, graphicsUnavailable=false, graphicsReason='';
  const depthMedia=viewWindow?.matchMedia?.('(max-width: 1000px), (forced-colors: active)')??null;
  const media=viewWindow?.matchMedia?.('(prefers-reduced-motion: reduce)')??null;
  let depthUnavailable=Boolean(depthMedia?.matches);
  const doc=root?.ownerDocument??globalThis.document;
  const navigation=bindWorkbenchNavigation(root,{window:viewWindow,document:doc});
  const current=()=>controller.snapshot();
  const cancelFrame=()=>{if(frame!==null)viewWindow?.cancelAnimationFrame?.(frame);frame=null;previousFrame=null;cameraDirty=false;};
  const announce=text=>{const output=doc?.querySelector?.('#flow-announcer');if(output)output.textContent=text;};
  const stopEvent=state=>state.cursor?state.scenario?.events?.[state.cursor-1]:null;
  const draw=()=>{
    if(disposed)return;
    const state=current(), reached=stopEvent(state), active=state.playing?state.scenario?.events?.[state.cursor]:state.selection?.kind==='event'?state.scenario?.events?.[state.selection.index]:null;
    if(state.mode==='depth'&&!depthUnavailable&&!state.reducedMotion&&!graphicsUnavailable)scene?.draw({camera,theme:doc?.documentElement?.dataset.theme??'dark',activeNodeId:state.selection?.kind==='node'?state.selection.id:active?.nodeId,blockedNodeId:reached?.stage==='refusal'?reached.nodeId:null,unknownNodeId:reached?.stage==='unknown'?reached.nodeId:null,motion:controller.motion?.(),progress:state.progress});
    const marker=root.querySelector?.('[data-flow-marker]'),motion=controller.motion?.();
    if(marker){marker.setAttribute('visibility',motion?'visible':'hidden');marker.setAttribute('cx',String(motion?coordinate(motion.x,950):-20));marker.setAttribute('cy',String(motion?coordinate(motion.y,560):-20));}
  };
  const schedule=()=>{
    if(disposed||frame!==null||doc?.hidden||!viewWindow?.requestAnimationFrame)return;
    const state=current();if(!cameraDirty&&(!state.playing||state.reducedMotion||depthUnavailable||graphicsUnavailable))return;
    frame=viewWindow.requestAnimationFrame(timestamp=>{frame=null;if(disposed)return;const delta=previousFrame===null?0:Math.max(0,Math.min(250,timestamp-previousFrame));previousFrame=timestamp;cameraDirty=false;if(current().playing)controller.tick(delta);draw();schedule();});
  };
  const focusIdentity=()=>{
    const active=doc?.activeElement;
    for(const attribute of ['data-flow-focus','data-flow-scenario','data-flow-event','data-flow-mode','data-flow-step','data-flow-play','data-flow-reset','data-flow-theme','data-flow-seek','data-flow-inspect-end','data-flow-camera'])if(active?.hasAttribute?.(attribute))return {attribute,value:active.getAttribute(attribute)};
    return null;
  };
  const restoreFocus=saved=>{
    if(!saved||!root.querySelectorAll)return;
    let next=Array.from(root.querySelectorAll('['+saved.attribute+']')).find(el=>el.getAttribute(saved.attribute)===saved.value);
    const invisible=el=>el?.getClientRects&&el.getClientRects().length===0;
    if(invisible(next)&&saved.attribute==='data-flow-focus')next=Array.from(root.querySelectorAll('[data-flow-focus]')).find(el=>el.getAttribute('data-flow-focus')===saved.value.replace(/^(graph|flat):/,'list:'));
    if(!next||next.disabled||invisible(next))next=root.querySelector?.('[data-flow-reset]');next?.focus?.({preventScroll:true});
  };
  const cancelGesture=(restore=true)=>{if(!gesture)return;const old=gesture;gesture=null;try{old.stage?.releasePointerCapture?.(old.id);}catch{}if(restore){camera=old.camera;cameraDirty=true;schedule();}};
  const unavailable=reason=>{
    graphicsUnavailable=true;graphicsReason=reason;cancelGesture();cancelFrame();scene?.dispose();scene=null;
    controller.pause();controller.setMode('flat');render();announce(reason);
  };
  const syncStage=()=>{
    const state=current(),reached=stopEvent(state),stage=root.querySelector?.('[data-flow-stage]');if(!stage)return;
    stage.dataset.flowMode=state.mode==='depth'&&!depthUnavailable&&!graphicsUnavailable&&!state.reducedMotion?'depth':'flat';stage.dataset.graphics=graphicsUnavailable?'unavailable':scene?'ready':'idle';
    const message=stage.querySelector?.('[data-flow-graphics-state]');if(message)message.textContent=graphicsUnavailable?graphicsReason:state.mode==='depth'?'3D · drag to orbit · use view controls for keyboard access':'2D topology · all systems remain inspectable';
    for(const button of stage.querySelectorAll?.('[data-flow-node]')??[]){const id=button.getAttribute('data-flow-node');button.setAttribute('aria-pressed',String(state.selection?.kind==='node'&&state.selection.id===id));button.classList?.toggle('flow-node--active',reached?.nodeId===id);button.classList?.toggle('flow-node--blocked',reached?.stage==='refusal'&&reached.nodeId===id);button.classList?.toggle('flow-node--unknown',reached?.stage==='unknown'&&reached.nodeId===id);const cue=button.querySelector?.('[data-flow-cue]');if(cue){cue.textContent=reached?.nodeId===id?reached.reported:'';cue.hidden=reached?.nodeId!==id;}}
  };
  const render=()=>{
    if(disposed)return;if(rendering||batching){deferred=true;return;}rendering=true;
    try {
      const state=current(),theme=doc?.documentElement?.dataset.theme??'dark';
      const fingerprint=JSON.stringify([state.scenarioId,state.cursor,state.selection,state.mode,state.playing,state.reducedMotion,depthUnavailable,graphicsUnavailable,theme]);
      if(lastRender!==fingerprint){
        cancelGesture();const optionsOpen=Boolean(root.querySelector?.('[data-flow-view-options]')?.open);const saved=focusIdentity(),navState=navigation.capture(),oldStage=root.querySelector?.('[data-flow-stage]');oldStage?.remove?.();
        root.innerHTML=renderFlowPlayground({...state,depthUnavailable,graphicsUnavailable,graphicsReason,navigationOpen:navigation.compact()?navState.open:true});
        if(oldStage)root.querySelector?.('[data-flow-stage]')?.replaceWith?.(oldStage);
        const options=root.querySelector?.('[data-flow-view-options]');if(options)options.open=optionsOpen;lastRender=fingerprint;
        if(!scene&&!graphicsUnavailable&&!depthUnavailable&&!state.reducedMotion&&state.mode==='depth'){
          const canvas=root.querySelector?.('[data-flow-canvas]');
          if(canvas)scene=sceneFactory(canvas,{onUnavailable:unavailable,onProject:points=>{for(const p of points){const button=Array.from(root.querySelectorAll?.('[data-flow-projected]')??[]).find(el=>el.getAttribute('data-flow-node')===p.id);if(button){button.style.left=p.x+'%';button.style.top=p.y+'%';button.style.visibility=p.visible?'visible':'hidden';}}}});
        }
        syncStage();restoreFocus(saved);navigation.restore(navState);announce((state.scenario?.title??'Synthetic scenario')+': scripted event '+state.cursor+' of '+(state.scenario?.events?.length??0)+'.');
      }
      draw();schedule();
    } finally {rendering=false;if(deferred){deferred=false;render();}}
  };
  const act=(method,...args)=>{
    if(method==='play'&&(depthUnavailable||graphicsUnavailable||current().reducedMotion))return;
    cancelGesture();batching=true;
    try{if(method==='play'&&current().cursor>=current().scenario.events.length)controller.seek(0);controller[method](...args);}finally{batching=false;}
    if(!current().playing)cancelFrame();deferred=false;render();
  };
  const cameraAction=action=>{
    if(disposed||depthUnavailable||graphicsUnavailable||current().reducedMotion||current().mode!=='depth')return;
    const next={...camera};if(action==='left')next.yaw-=8;else if(action==='right')next.yaw+=8;else if(action==='in')next.zoom+=.1;else if(action==='out')next.zoom-=.1;else if(action==='reset')Object.assign(next,defaultCamera);else return;
    camera=normalizeCamera(next);cameraDirty=true;schedule();if(action==='reset')announce('3D view reset. No request or workflow changed.');
  };
  const click=event=>{
    const target=event.target?.closest?.('[data-flow-scenario],[data-flow-node],[data-flow-event],[data-flow-mode],[data-flow-step],[data-flow-play],[data-flow-reset],[data-flow-theme],[data-flow-camera],[data-flow-inspect-end]');if(!target||target.disabled)return;
    if(target.matches('[data-flow-scenario]')){act('choose',target.getAttribute('data-flow-scenario'));act('seek',current().scenario.events.length);}
    else if(target.matches('[data-flow-node]'))act('selectNode',target.getAttribute('data-flow-node'));
    else if(target.matches('[data-flow-event]'))act('selectEvent',Number(target.getAttribute('data-flow-event')));
    else if(target.matches('[data-flow-mode]'))act('setMode',target.getAttribute('data-flow-mode'));
    else if(target.matches('[data-flow-step]'))act('step',Number(target.getAttribute('data-flow-step')));
    else if(target.matches('[data-flow-play]'))act(current().playing?'pause':'play');
    else if(target.matches('[data-flow-reset]'))act('seek',0);
    else if(target.matches('[data-flow-inspect-end]'))act('seek',current().scenario.events.length);
    else if(target.matches('[data-flow-camera]'))cameraAction(target.getAttribute('data-flow-camera'));
    else if(target.matches('[data-flow-theme]')&&doc?.documentElement){doc.documentElement.dataset.theme=doc.documentElement.dataset.theme==='light'?'dark':'light';render();}
  };
  const input=event=>{if(event.target?.matches?.('[data-flow-seek]'))act('seek',Number(event.target.value));};
  const down=event=>{
    const stage=event.target?.closest?.('[data-flow-stage]');if(!stage||event.target?.closest?.('button,input,summary')||event.button!==0||!Number.isInteger(event.pointerId)||![event.clientX,event.clientY].every(Number.isFinite)||current().mode!=='depth'||depthUnavailable||graphicsUnavailable||current().reducedMotion)return;
    act('pause');gesture={id:event.pointerId,x:event.clientX,y:event.clientY,camera:{...camera},stage};try{stage.setPointerCapture?.(event.pointerId);}catch{gesture=null;return;}event.preventDefault?.();
  };
  const move=event=>{if(!gesture||gesture.id!==event.pointerId||![event.clientX,event.clientY].every(Number.isFinite))return;camera=normalizeCamera({...gesture.camera,yaw:gesture.camera.yaw+(event.clientX-gesture.x)*.12,pitch:gesture.camera.pitch+(event.clientY-gesture.y)*.1});cameraDirty=true;schedule();};
  const up=event=>{if(gesture?.id===event.pointerId)cancelGesture(false);};
  const cancel=event=>{if(gesture?.id===event.pointerId)cancelGesture();};
  const key=event=>{if(event.key==='Escape'){cancelGesture();return;}if(!event.target?.matches?.('[data-flow-stage]'))return;const action={ArrowLeft:'left',ArrowRight:'right','+':'in','-':'out','0':'reset'}[event.key];if(action){event.preventDefault?.();cameraAction(action);}};
  const wheel=event=>{if(!event.target?.closest?.('[data-flow-stage]')||current().mode!=='depth'||depthUnavailable||graphicsUnavailable||current().reducedMotion||!Number.isFinite(event.deltaY)||event.deltaY===0)return;event.preventDefault?.();cameraAction(event.deltaY<0?'in':'out');};
  const hidden=()=>{if(doc?.hidden){cancelGesture();controller.pause();cancelFrame();render();}};
  const motionChange=event=>{cancelGesture();controller.setReducedMotion(Boolean(event.matches));cancelFrame();render();};
  const depthChange=event=>{cancelGesture();depthUnavailable=Boolean(event.matches);controller.pause();cancelFrame();if(depthUnavailable)controller.setMode('flat');render();};
  const resize=()=>{if(!disposed){cameraDirty=true;schedule();}};
  const listeners={click,input,pointerdown:down,pointermove:move,pointerup:up,pointercancel:cancel,lostpointercapture:cancel,keydown:key,wheel};
  for(const [type,fn]of Object.entries(listeners))root.addEventListener(type,fn,type==='wheel'?{passive:false}:undefined);
  doc?.addEventListener?.('visibilitychange',hidden);media?.addEventListener?.('change',motionChange);depthMedia?.addEventListener?.('change',depthChange);viewWindow?.addEventListener?.('resize',resize);
  if(depthUnavailable)controller.setMode('flat');controller.setReducedMotion(Boolean(media?.matches));render();
  return {render,dispose(){if(disposed)return;cancelGesture(false);disposed=true;cancelFrame();scene?.dispose();scene=null;navigation.dispose();controller.pause();controller.dispose();for(const [type,fn]of Object.entries(listeners))root.removeEventListener(type,fn);doc?.removeEventListener?.('visibilitychange',hidden);media?.removeEventListener?.('change',motionChange);depthMedia?.removeEventListener?.('change',depthChange);viewWindow?.removeEventListener?.('resize',resize);}};
}

if(typeof document!=='undefined'){
  const app=document.querySelector('#flow-app');if(app){let view;const controller=createFlowSimulation({onChange:()=>view?.render()});controller.choose('denied');controller.seek(3);controller.setMode('depth');view=bindFlowPlayground(app,controller,{window});window.addEventListener('beforeunload',()=>view.dispose(),{once:true});}
}
