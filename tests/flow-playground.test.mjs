import test from 'node:test';
import assert from 'node:assert/strict';
import { renderFlowPlayground, bindFlowPlayground } from '../preview/flow-playground.mjs';
import { createFlowSimulation } from '../preview/flow-simulation.mjs';

const scenarios = [{ id: 'trace-safe', title: '<Trace & fixture>', description: '<Description & fixture>', traceId: 'trace:synthetic', policyRevisionId: 'policy:synthetic', topologyRevisionId: 'topology:synthetic', events: [{ id: 'event:request', stage: 'request', title: '<Request & input>', nodeId: 'request', edgeId: 'edge:request', reported: 'recorded', detail: '<Untrusted & detail>', evidenceClass: 'fixture' }, { id: 'event:outcome', stage: 'outcome', title: 'Missing outcome', nodeId: 'outcome', edgeId: 'edge:outcome', reported: 'unknown', detail: 'No receipt', evidenceClass: 'missing' }] }];

const state = (overrides = {}) => ({ scenario: scenarios[0], scenarioId: 'trace-safe', cursor: 1, selection: { kind: 'event', index: 0 }, mode: 'flat', playing: false, reducedMotion: false, progress: 0, sourceKind: 'synthetic', authority: 'none', authenticity: 'unverified', execution: 'unavailable', authoritativeEligibility: 'unknown', ...overrides });

test('renders task-focused synthetic request scene, escapes fixture text, and disables future events', () => {
  const html = renderFlowPlayground(state());
  assert.match(html, /SYNTHETIC/);
  assert.match(html, /NO SYSTEMS CONNECTED/);
  assert.match(html, /NO ENGINE ACTION/);
  assert.match(html, /Trace a proposed <em>action<\/em>/);
  assert.match(html, /Open a pull request/);
  assert.match(html, /example\/project/);
  assert.match(html, /fix\/input-validation/);
  assert.match(html, /<canvas data-flow-canvas aria-hidden="true"><\/canvas>/);
  assert.equal((html.match(/data-flow-projected/g) ?? []).length, 8);
  assert.match(html, /data-flow-stage[^>]*tabindex="0"[^>]*aria-label="System map"/);
  assert.match(html, /data-flow-camera="left"/);
  assert.match(html, /data-flow-camera="right"/);
  assert.match(html, /data-flow-camera="in"/);
  assert.match(html, /data-flow-camera="out"/);
  assert.match(html, /data-flow-camera="reset"/);
  assert.match(html, /Inspect blocker \/ evidence/);
  assert.match(html, /Final result: synthetic completion only/);
  assert.match(html, /href="workflow-playground.html"/);
  assert.match(html, /Open 3D graph editor/);
  assert.match(html, /Separate local\/synthetic workflow editor\. This trace does not transfer\./);
  assert.match(html, /ROUTE DIRECTION/);
  assert.match(html, /Reached: &lt;Request &amp; input&gt; · recorded/);
  assert.match(html, /Role and current state/);
  assert.match(html, /Inputs and outputs/);
  assert.match(html, /Boundary/);
  assert.match(html, /Provenance/);
  assert.match(html, /&lt;Request &amp; input&gt;/);
  assert.match(html, /&lt;Untrusted &amp; detail&gt;/);
  assert.doesNotMatch(html, /<Request & input>/);
  assert.match(html.match(/data-flow-event="1"[^>]*>/)[0], /disabled/);
  assert.match(html, /recommendation is not approval, authorization, consumption, or outcome/i);
  assert.match(html, /assets\/authority\/lnsat-mark.webp/);
  assert.match(html, /Example only/);
});

test('marks 3D/manual controls unavailable while retaining the complete 2D/list topology', () => {
  const html = renderFlowPlayground(state({ mode: 'depth', graphicsUnavailable: true, graphicsReason: 'context lost' }));
  assert.match(html.match(/data-flow-mode="depth"[^>]*>/)[0], /disabled/);
  assert.match(html.match(/data-flow-play[^>]*>/)[0], /disabled/);
  assert.match(html, /data-flow-flat/);
  assert.match(html, /3D graphics unavailable: context lost/);
  assert.match(html, /Eight systems/);
  assert.match(html, /Ten directed boundaries/);
});

test('flow rerender uses shared navigation and retains its requested compact disclosure state', () => {
  const closed = renderFlowPlayground(state({ navigationOpen: false }));
  assert.match(closed, /data-workbench-nav-drawer>/);
  assert.match(closed, /href="analyze\.html#authority-engines"/);
  assert.match(closed, /href="flow-playground\.html" aria-current="page"/);
  assert.doesNotMatch(closed, /href="analyze\.html#workflows" aria-current/);
  const open = renderFlowPlayground(state({ navigationOpen: true, cursor: 0 }));
  assert.match(open, /data-workbench-nav-drawer open>/);
  assert.match(open, /Flow playground/);
});

function fakeRoot() {
  const listeners = new Map();
  const marker = { attrs: {}, setAttribute(name, value) { this.attrs[name] = value; } };
  const documentListeners = new Map();
  const document = { hidden: false, documentElement: { dataset: {} }, addEventListener(type, listener) { documentListeners.set(type, listener); }, removeEventListener(type) { documentListeners.delete(type); }, fire(type) { documentListeners.get(type)?.(); } };
  return { ownerDocument: document, html: '', writes: 0, set innerHTML(value) { this.html = value; this.writes++; }, get innerHTML() { return this.html; }, addEventListener(type, listener) { listeners.set(type, listener); }, removeEventListener(type) { listeners.delete(type); }, fire(type, event) { listeners.get(type)?.(event); }, querySelector(selector) { return selector === '[data-flow-marker]' ? marker : null; }, marker };
}

function fakeTarget(attribute, value = '') { return { disabled: false, getAttribute: name => name === attribute ? value : null, matches: selector => selector.includes(attribute), closest: () => fakeTarget(attribute, value) }; }

test('binding sends explicit actions only and cancels the scheduled frame at disposal', () => {
  const calls = [];
  const controller = { value: state({ playing: true }), snapshot() { return this.value; }, motion() { return { x: 12, y: 20 }; }, choose(id) { calls.push(['choose', id]); }, seek(value) { calls.push(['seek', value]); }, step(value) { calls.push(['step', value]); }, selectEvent(value) { calls.push(['event', value]); }, selectNode(value) { calls.push(['node', value]); }, setMode(value) { calls.push(['mode', value]); }, setReducedMotion(value) { calls.push(['reduced', value]); }, play() { calls.push(['play']); }, pause() { calls.push(['pause']); this.value = state({ playing: false }); }, reset() { calls.push(['reset']); }, tick(value) { calls.push(['tick', value]); }, dispose() { calls.push(['dispose']); } };
  const raf = []; let canceled = 0;
  const viewWindow = { requestAnimationFrame(callback) { raf.push(callback); return raf.length; }, cancelAnimationFrame() { canceled += 1; }, matchMedia() { return { matches: false, addEventListener() {}, removeEventListener() {} }; } };
  const root = fakeRoot();
  const binding = bindFlowPlayground(root, controller, { window: viewWindow });
  root.fire('click', { target: fakeTarget('data-flow-step', '1') });
  root.fire('input', { target: { value: '0', matches: selector => selector === '[data-flow-seek]' } });
  assert.deepEqual(calls.slice(0, 3), [['reduced', false], ['step', 1], ['seek', 0]]);
  assert.equal(raf.length, 1);
  raf[0](16);
  assert.deepEqual(calls.at(-1), ['tick', 0]);
  binding.dispose();
  assert.ok(canceled >= 1);
  assert.deepEqual(calls.slice(-2), [['pause'], ['dispose']]);
});

test('reduced motion prevents playback scheduling and surfaces manual-only copy', () => {
  const controller = { value: state({ reducedMotion: true }), snapshot() { return this.value; }, motion() { return null; }, setReducedMotion() {}, pause() {}, dispose() {}, tick() {} };
  const root = fakeRoot(); let rafs = 0;
  const binding = bindFlowPlayground(root, controller, { window: { requestAnimationFrame() { rafs += 1; return 1; }, cancelAnimationFrame() {}, matchMedia(query) { return { matches: query.startsWith('(prefers-reduced-motion'), addEventListener() {}, removeEventListener() {} }; } } });
  assert.equal(rafs, 0);
  assert.match(root.innerHTML, /Reduced motion: manual steps only/);
  binding.dispose();
});

test('visibility and motion changes pause playback, cancel the frame, and never resume it', () => {
  const calls = [];
  const controller = { value: state({ playing: true }), snapshot() { return this.value; }, motion() { return null; }, setReducedMotion(value) { calls.push(['reduced', value]); this.value = { ...this.value, reducedMotion: value, playing: value ? false : this.value.playing }; }, pause() { calls.push(['pause']); this.value = { ...this.value, playing: false }; }, dispose() { calls.push(['dispose']); }, tick() {} };
  let mediaListener; let canceled = 0;
  const media = { matches: false, addEventListener(type, listener) { if (type === 'change') mediaListener = listener; }, removeEventListener() {} };
  const root = fakeRoot();
  const binding = bindFlowPlayground(root, controller, { window: { requestAnimationFrame() { return 1; }, cancelAnimationFrame() { canceled += 1; }, matchMedia(query) { return query.startsWith('(prefers-reduced-motion') ? media : { matches: false, addEventListener() {}, removeEventListener() {} }; } } });
  root.ownerDocument.hidden = true;
  root.ownerDocument.fire('visibilitychange');
  mediaListener({ matches: true });
  assert.deepEqual(calls.slice(0, 3), [['reduced', false], ['pause'], ['reduced', true]]);
  assert.ok(canceled >= 1);
  binding.dispose();
  assert.deepEqual(calls.at(-2), ['pause']);
});

test('real playback keeps one pending frame, updates only marker between events, and fences late disposal', () => {
  const root = fakeRoot(); let binding; let nextFrame = 0;
  const callbacks = new Map(); const archived = [];
  const controller = createFlowSimulation({ onChange: () => binding?.render() });
  const viewWindow = {
    requestAnimationFrame(callback) { const id = ++nextFrame; callbacks.set(id, callback); archived.push(callback); return id; },
    cancelAnimationFrame(id) { callbacks.delete(id); },
    matchMedia() { return { matches: false, addEventListener() {}, removeEventListener() {} }; },
  };
  binding = bindFlowPlayground(root, controller, { window: viewWindow });
  assert.equal(root.writes, 1);
  root.fire('click', { target: fakeTarget('data-flow-play') });
  assert.equal(root.writes, 2, 'model notification and action must not render twice');
  for (const timestamp of [0, 200, 400, 600, 800, 1000, 1200, 1400]) {
    assert.equal(callbacks.size, 1);
    const [id, callback] = callbacks.entries().next().value;
    callbacks.delete(id); callback(timestamp);
  }
  assert.equal(root.writes, 2, 'sub-step frames cannot rebuild the graph');
  assert.ok(Number(root.marker.attrs.cx) > 220);
  assert.equal(root.marker.attrs.visibility, 'visible');
  const [id, callback] = callbacks.entries().next().value;
  callbacks.delete(id); callback(1600);
  assert.equal(root.writes, 3); assert.equal(controller.snapshot().cursor, 1);
  binding.dispose();
  const writes = root.writes, attrs = { ...root.marker.attrs };
  archived.at(-1)(2000);
  assert.equal(callbacks.size, 0); assert.equal(root.writes, writes);
  assert.deepEqual(root.marker.attrs, attrs);
});

test('list/forced-color presentation pins flat, disables motion, and never resumes after media changes', () => {
  const root = fakeRoot(); let depthListener; let binding; let rafs = 0;
  const controller = createFlowSimulation({ onChange: () => binding?.render() });
  const viewWindow = {
    requestAnimationFrame() { rafs++; return 1; }, cancelAnimationFrame() {},
    matchMedia(query) { return { matches: !query.startsWith('(prefers-reduced-motion'), addEventListener(type, listener) { if (!query.startsWith('(prefers-reduced-motion')) depthListener = listener; }, removeEventListener() {} }; },
  };
  binding = bindFlowPlayground(root, controller, { window: viewWindow });
  assert.match(root.innerHTML.match(/data-flow-mode="depth"[^>]*>/)[0], /disabled/);
  assert.match(root.innerHTML.match(/data-flow-play[^>]*>/)[0], /disabled/);
  assert.match(root.innerHTML, /List view: manual steps/);
  assert.match(root.innerHTML, /Complete topology list/);
  assert.match(root.innerHTML, /authority → scoped authorization reference → executor/);
  assert.equal(controller.snapshot().mode, 'flat'); assert.equal(rafs, 0);
  depthListener({ matches: false });
  assert.equal(controller.snapshot().playing, false);
  root.fire('click', { target: fakeTarget('data-flow-play') });
  assert.equal(controller.snapshot().playing, true);
  depthListener({ matches: true });
  assert.equal(controller.snapshot().playing, false); assert.equal(controller.snapshot().mode, 'flat');
  binding.dispose();
});

function sceneRoot() {
  const base = fakeRoot(); let stage;
  const createStage = () => ({ dataset: {}, remove() {}, replaceWith(old) { stage = old; }, querySelector() { return null; }, querySelectorAll() { return []; }, setPointerCapture() {}, releasePointerCapture() {} });
  Object.defineProperty(base, 'innerHTML', { set(value) { base.html = value; base.writes++; stage = createStage(); }, get() { return base.html; } });
  const markerQuery = base.querySelector.bind(base);
  base.querySelector = selector => selector === '[data-flow-stage]' ? stage : selector === '[data-flow-canvas]' ? {} : markerQuery(selector);
  return base;
}

test('discrete inspection preserves one GPU scene, and context loss keeps inspectable state without replay', () => {
  const root = sceneRoot(); let binding, initializes = 0, frees = 0, unavailable;
  const controller = createFlowSimulation({ onChange: () => binding?.render() });controller.setMode('depth');
  binding = bindFlowPlayground(root, controller, { window: {matchMedia(){return {matches:false,addEventListener(){},removeEventListener(){}};}}, sceneFactory(canvas, hooks) { initializes++; unavailable=hooks.onUnavailable; return {draw(){},dispose(){frees++;}}; } });
  const stage = root.querySelector('[data-flow-stage]');
  controller.selectNode('policy');controller.seek(2);controller.selectNode('authority');
  assert.equal(initializes,1);assert.equal(root.querySelector('[data-flow-stage]'),stage);
  unavailable('3D graphics unavailable.');
  assert.equal(controller.snapshot().mode,'flat');assert.equal(controller.snapshot().cursor,2);assert.equal(controller.snapshot().playing,false);assert.equal(frees,1);
  assert.match(root.innerHTML,/3D graphics unavailable/);assert.match(root.innerHTML,/Complete topology list/);
  root.fire('click',{target:fakeTarget('data-flow-play')});assert.equal(controller.snapshot().playing,false);
  controller.selectNode('evidence');assert.equal(initializes,1);binding.dispose();assert.equal(frees,1);
});

test('camera gesture uses exact pointer, cancels to original view, coalesces frames and fences disposal', () => {
  const root=sceneRoot();let binding,next=0;const callbacks=new Map(),draws=[];
  const controller=createFlowSimulation({onChange:()=>binding?.render()});controller.setMode('depth');
  const viewWindow={matchMedia(){return {matches:false,addEventListener(){},removeEventListener(){}};},requestAnimationFrame(fn){callbacks.set(++next,fn);return next;},cancelAnimationFrame(id){callbacks.delete(id);}};
  binding=bindFlowPlayground(root,controller,{window:viewWindow,sceneFactory(){return {draw(value){draws.push({...value.camera});},dispose(){}};}});
  const original=draws.at(-1),stage=root.querySelector('[data-flow-stage]');
  root.fire('pointerdown',{target:{closest:selector=>selector==='[data-flow-stage]'?stage:null},button:0,pointerId:7,clientX:20,clientY:20,preventDefault(){}});
  root.fire('pointermove',{pointerId:8,clientX:100,clientY:40});assert.equal(callbacks.size,0);
  root.fire('pointermove',{pointerId:7,clientX:100,clientY:40});root.fire('pointermove',{pointerId:7,clientX:120,clientY:45});assert.equal(callbacks.size,1);
  let [id,frame]=callbacks.entries().next().value;callbacks.delete(id);frame(20);assert.notDeepEqual(draws.at(-1),original);
  root.fire('pointercancel',{pointerId:7});[id,frame]=callbacks.entries().next().value;callbacks.delete(id);frame(40);assert.deepEqual(draws.at(-1),original);
  root.fire('click',{target:fakeTarget('data-flow-camera','right')});assert.equal(callbacks.size,1);const late=callbacks.values().next().value;binding.dispose();const count=draws.length;late(60);assert.equal(callbacks.size,0);assert.equal(draws.length,count);
});

test('expiry inspection preserves earlier approval and missing outcome never becomes a completed effect', async () => {
  const {FLOW_SCENARIOS}=await import('../preview/flow-simulation.mjs');
  const expired=FLOW_SCENARIOS.find(s=>s.id==='expired');const html=renderFlowPlayground(state({scenario:expired,scenarioId:'expired',cursor:expired.events.length,selection:{kind:'event',index:expired.events.length-1}}));
  assert.match(html,/Earlier approval and issued-reference observations do not permit expired consumption/);
  const unknown=FLOW_SCENARIOS.find(s=>s.id==='unknown');const missing=renderFlowPlayground(state({scenario:unknown,scenarioId:'unknown',cursor:unknown.events.length,selection:{kind:'event',index:unknown.events.length-1}}));
  assert.match(missing,/reconciliation remains required/);assert.match(missing,/do not assume success/);
});

test('active 3D stage becomes complete 2D presentation on reduced motion and does not auto resume', () => {
  const root=sceneRoot();let binding,listener,draws=0;
  const controller=createFlowSimulation({onChange:()=>binding?.render()});controller.setMode('depth');
  const viewWindow={matchMedia(query){return {matches:false,addEventListener(type,fn){if(query.startsWith('(prefers-reduced-motion'))listener=fn;},removeEventListener(){}};}};
  binding=bindFlowPlayground(root,controller,{window:viewWindow,sceneFactory(){return {draw(){draws++;},dispose(){}};}});
  const stage=root.querySelector('[data-flow-stage]');assert.equal(stage.dataset.flowMode,'depth');const count=draws;
  listener({matches:true});assert.equal(root.querySelector('[data-flow-stage]'),stage);assert.equal(stage.dataset.flowMode,'flat');assert.equal(controller.snapshot().mode,'flat');assert.equal(controller.snapshot().playing,false);assert.equal(draws,count);
  listener({matches:false});assert.equal(stage.dataset.flowMode,'flat');assert.equal(controller.snapshot().playing,false);binding.dispose();
});
