import test from 'node:test';
import assert from 'node:assert/strict';
import { projectWorkflowScene, frameForSelection } from '../preview/workflow-scene.mjs';
import { buildSceneGeometry } from '../preview/flow-scene.mjs';
import { createWorkflowController } from '../preview/workflow-model.mjs';
import { bindWorkflowView } from '../preview/workflow-view.mjs';

async function graph(onChange = () => {}) {
  const controller = createWorkflowController({ playground: true, onChange });
  await controller.beginNew();
  controller.addNode('input', { x: 0, y: 0 });
  controller.addNode('output', { x: 300, y: 90 });
  controller.connectControl(0, 'next', 1);
  controller.connectData(0, 'text', 1, 'text');
  return controller;
}

test('projection preserves exact IDs, separate edge kinds, saved positions and directional sockets', async () => {
  const controller = await graph(), state = controller.getState(), before = structuredClone(state);
  const projected = projectWorkflowScene(state);
  assert.equal(projected.available, true);
  assert.deepEqual(projected.poses.map(node => node.id), state.definition.nodes.map(node => node.id));
  assert.deepEqual(projected.edges.map(edge => edge.id), ['control:0', 'data:0']);
  assert.notDeepEqual(projected.route('control:0'), projected.route('data:0'));
  for (const edge of projected.edges) {
    assert.deepEqual(projected.pointOnEdge(edge.id, 0), projected.route(edge.id)[0]);
    assert.deepEqual(projected.pointOnEdge(edge.id, 1), projected.route(edge.id).at(-1));
    assert.ok(projected.pointOnEdge(edge.id, .5).every(Number.isFinite));
  }
  assert.equal(projected.pointOnEdge('unknown', .5), null);
  assert.equal(projected.pointOnEdge('data:0', Infinity), null);
  assert.deepEqual(state, before);
  controller.reorderNode(0, 1);
  const reordered = projectWorkflowScene(controller.getState());
  assert.equal(reordered.edges[0].from, state.definition.nodes[0].id);
  assert.equal(reordered.edges[0].to, state.definition.nodes[1].id);
});

test('malformed and oversized graphs visibly fall back without dropping any definition record', async () => {
  const controller = await graph();
  for (const mutate of [state => state.layout.positions[0].x = NaN, state => state.definition.nodes[0].operation.kind = 'untrusted', state => state.definition.controlEdges[0].toNode = 'missing', state => state.definition.nodes.push({ ...state.definition.nodes[0] })]) {
    const state = controller.getState(); mutate(state);
    assert.equal(projectWorkflowScene(state).available, false);
  }
  for (let index = 2; index < 25; index++) controller.addNode('input', { x: index * 40, y: 200 });
  const state = controller.getState();
  assert.match(projectWorkflowScene(state).reason, /24 nodes/);
  assert.equal(state.definition.nodes.length, 25);
  const supported = structuredClone(state); supported.definition.nodes.pop(); supported.layout.positions.pop();
  const geometry = buildSceneGeometry(projectWorkflowScene(supported));
  assert.ok(geometry.values.length / 6 < 250000);
  assert.ok(Array.from(geometry.values).every(Number.isFinite));
});

test('guided sections retain exact direct neighbors and edge endpoints without mutating controller state',async()=>{
  const controller=await graph();controller.addNode('check',{x:650,y:250});controller.addNode('output',{x:900,y:350});controller.connectControl(2,'next',3);
  const state=controller.getState(),before=structuredClone(state),projection=projectWorkflowScene(state);
  const nodeFrame=frameForSelection(projection,{kind:'node',index:0});
  assert.deepEqual(nodeFrame.nodeIds,state.definition.nodes.slice(0,2).map(node=>node.id));
  const edgeFrame=frameForSelection(projection,{kind:'control',index:1});
  assert.deepEqual(edgeFrame.nodeIds,state.definition.nodes.slice(2).map(node=>node.id));
  assert.notDeepEqual(nodeFrame.center,edgeFrame.center);
  assert.deepEqual(frameForSelection(projection,{kind:'data',index:0}).nodeIds,nodeFrame.nodeIds);
  for(const selection of [null,{}, {kind:'node',index:-1}, {kind:'node',index:.5}, {kind:'node',index:Infinity}, {kind:'node',index:99}, {kind:'control',index:99}, {kind:'unknown',index:0}]) assert.equal(frameForSelection(projection,selection),null);
  assert.equal(frameForSelection({available:false},{kind:'node',index:0}),null);
  assert.deepEqual(state,before);assert.deepEqual(controller.getState(),before);
});

test('empty graphs refuse depth while generated section bounds stay finite and deterministic',()=>{
  assert.match(projectWorkflowScene({definition:{nodes:[],controlEdges:[],dataEdges:[]}}).reason,/Add a module/);
  let seed=271828;const next=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed/2**32;};
  for(let sample=0;sample<48;sample++) {
    const nodes=Array.from({length:2+sample%7},(_,index)=>({id:`n${index}`,operation:{kind:['input','capability','check','branch','checkpoint','output'][index%6]}}));
    const state={definition:{nodes,controlEdges:nodes.slice(1).map((node,index)=>({fromNode:nodes[index].id,toNode:node.id})),dataEdges:[]},layout:{positions:nodes.map((_,index)=>({nodeIndex:index,x:Math.floor(next()*200000)-100000,y:Math.floor(next()*200000)-100000}))}};
    const before=structuredClone(state),projection=projectWorkflowScene(state);assert.equal(projection.available,true);
    for(let index=0;index<nodes.length;index++) {
      const frame=frameForSelection(projection,{kind:'node',index});
      assert.ok(frame.center.every(value=>Number.isFinite(value)&&Math.abs(value)<=12));assert.ok(frame.framing>=.65&&frame.framing<=1.6);
      assert.deepEqual(frame,frameForSelection(projection,{kind:'node',index}));assert.ok(frame.nodeIds.includes(nodes[index].id));
    }
    assert.deepEqual(state,before);
  }
});

function rootFixture() {
  const listeners = new Map(), frames = new Map(), media = new Map(); let sequence = 0;
  const document = { hidden: false, activeElement: null, documentElement: { dataset: { theme: 'dark' } }, addEventListener(type, listener) { listeners.set(`document:${type}`, listener); }, removeEventListener(type) { listeners.delete(`document:${type}`); } };
  const window = { innerWidth: 1440, requestAnimationFrame(fn) { frames.set(++sequence, fn); return sequence; }, cancelAnimationFrame(id) { frames.delete(id); }, addEventListener() {}, removeEventListener() {}, matchMedia(query) { const value = { matches: false, addEventListener(type, listener) { media.set(query, { value, listener }); }, removeEventListener() {} }; return value; } };
  document.defaultView = window;
  const root = { ownerDocument: document, html: '', canvas: null,
    set innerHTML(html) { this.html = html; this.canvas = html.includes('data-workflow-scene-canvas') ? { remove() {}, replaceWith: canvas => { root.canvas = canvas; } } : null; },
    querySelector(selector) { if (selector === '[data-workflow-scene-canvas]') return this.canvas; return null; },
    querySelectorAll() { return []; }, addEventListener(type, listener) { listeners.set(type, listener); }, removeEventListener(type) { listeners.delete(type); },
    click(dataset) { const target = { dataset, closest: () => target }; listeners.get('click')({ target }); },
    flush(time) { const next = frames.entries().next().value; if (next) { frames.delete(next[0]); next[1](time); } },
  };
  return { root, document, frames, media, listeners };
}

function geometryRootFixture() {
  const fixture = rootFixture();
  const priorCss = globalThis.CSS;
  if (!globalThis.CSS) globalThis.CSS = { escape: value => String(value) };
  const marker = { attributes: new Map(), setAttribute(name, value) { this.attributes.set(name, String(value)); } };
  const label = { setAttribute() {} };
  let pathData = '', emptyCalls = 0; const geometryCalls = [];
  const wire = {
    dataset: { workflowSvgWire: 'control:0' },
    getAttribute(name) { return name === 'd' ? pathData : null; },
    setAttribute(name, value) { if (name === 'd') pathData = String(value); },
    getTotalLength() { if (!pathData) { emptyCalls++; const error = new Error('Empty path'); error.name = 'InvalidStateError'; throw error; } return 100; },
    getPointAtLength(value) { if (!pathData) { emptyCalls++; const error = new Error('Empty path'); error.name = 'InvalidStateError'; throw error; } geometryCalls.push(value); return { x: 25 + value, y: 50 }; },
  };
  const svg = { querySelector(selector) { if (selector.includes('data-workflow-svg-wire')) return wire; if (selector.includes('data-workflow-svg-label')) return label; return null; }, setAttribute() {} };
  const stage = { offsetWidth: 480, offsetHeight: 320, getBoundingClientRect: () => ({ left: 0, top: 0 }), querySelector: selector => selector === '.workflow-wire-diagram' ? svg : null };
  const source = { getBoundingClientRect: () => ({ right: 120, top: 40, height: 20 }) };
  const target = { getBoundingClientRect: () => ({ left: 280, top: 100, height: 20 }) };
  const originalQuery = fixture.root.querySelector.bind(fixture.root);
  const originalAll = fixture.root.querySelectorAll.bind(fixture.root);
  Object.defineProperty(fixture.root, 'innerHTML', {
    set(html) { pathData = ''; this.html = html; this.canvas = html.includes('data-workflow-scene-canvas') ? { remove() {}, replaceWith: canvas => { fixture.root.canvas = canvas; } } : null; },
  });
  fixture.root.querySelector = selector => {
    if (selector === '[data-workflow-graph-stage]') return stage;
    if (selector === '[data-workflow-design-marker]') return marker;
    if (selector.includes('data-workflow-control-source')) return source;
    if (selector.includes('[data-workflow-node="1"] .workflow-node-select')) return target;
    return originalQuery(selector);
  };
  fixture.root.querySelectorAll = selector => selector === '[data-workflow-svg-wire]' ? [wire] : originalAll(selector);
  return { ...fixture, marker, wire, get emptyCalls() { return emptyCalls; }, geometryCalls, restoreCss() { if (priorCss === undefined) delete globalThis.CSS; else globalThis.CSS = priorCss; } };
}

test('alternate views, guided framing and one-shot preview are view-only; edits and undo update same scene', async () => {
  let binding; const controller = await graph(() => binding?.render()); const fixture = rootFixture();
  const calls = [], scenes = [];
  binding = bindWorkflowView(fixture.root, controller, { initialMode: 'depth', sceneFactory: (canvas, projection) => { const scene = { draw: value => calls.push(value), invalidateProjection() {}, setTopology: graph => calls.push({ rebuilt: graph.fingerprint }), dispose: () => calls.push({ disposed: true }) }; scenes.push(scene); return scene; } });
  assert.ok(fixture.root.html.indexOf('data-workflow-mode="flat"') < fixture.root.html.indexOf('data-workflow-scene-focus'));
  assert.doesNotMatch(fixture.root.html, /data-workflow-camera|data-workflow-fit|drag background to orbit/);
  assert.equal((fixture.root.html.match(/data-workflow-mode="flat"/g) ?? []).length, 1);
  const before = controller.getState();
  fixture.root.click({ workflowMode: 'flat' }); fixture.root.click({ workflowMode: 'depth' });
  fixture.root.click({ workflowSceneFocus: '' }); fixture.root.click({ workflowSceneOverview: '' });
  assert.deepEqual(controller.getState(), before);
  assert.equal(scenes.length, 1);
  controller.select({ kind: 'control', index: 0 });
  const selected = controller.getState(); fixture.root.click({ workflowPreview: '' });
  for (const time of [0, 200, 400, 600, 800, 1000]) fixture.root.flush(time);
  assert.equal(fixture.frames.size, 0);
  assert.ok(calls.some(call => call.motion?.edgeId === 'control:0' && call.progress > 0));
  assert.deepEqual(controller.getState(), selected);
  assert.equal(controller.editFields({ kind: 'workflow' }, { title: 'Retained pending field' }), true);
  const pending = controller.getState(); fixture.root.click({ workflowMode: 'flat' }); fixture.root.click({ workflowMode: 'depth' });
  assert.deepEqual(controller.getState(), pending);
  controller.applyFields(); const title = controller.getState().definition.title;
  controller.addNode('check', { x: 500, y: 200 });
  assert.equal(controller.getState().definition.nodes.length, 3);
  assert.ok(calls.some(call => call.rebuilt));
  controller.undo(); assert.equal(controller.getState().definition.nodes.length, 2);
  assert.equal(controller.getState().definition.title, title);
  binding.dispose(); assert.equal(fixture.frames.size, 0); assert.equal(calls.filter(call => call.disposed).length, 1);
});

test('default binding opens flat and defers scene creation until an explicit 3D choice', async () => {
  let binding; const controller = await graph(() => binding?.render()), fixture = rootFixture();
  let sceneFactoryCalls = 0;
  binding = bindWorkflowView(fixture.root, controller, { sceneFactory: () => { sceneFactoryCalls++; return { draw() {}, invalidateProjection() {}, setTopology() {}, dispose() {} }; } });
  assert.match(fixture.root.html, /data-workflow-graph-stage/);
  assert.doesNotMatch(fixture.root.html, /data-workflow-scene-canvas/);
  assert.equal(sceneFactoryCalls, 0);
  fixture.root.click({ workflowMode: 'depth' });
  assert.match(fixture.root.html, /data-workflow-scene-canvas/);
  assert.equal(sceneFactoryCalls, 1);
  binding.dispose();
});

test('explicit focus holds section across selection and render, resets on topology, and schedules no camera motion',async()=>{
  let binding;const controller=await graph(()=>binding?.render()),fixture=rootFixture(),draws=[];
  let pending=false;const wrapped={...controller,getState:()=>({...controller.getState(),pending:pending?'inspect':null})};
  binding=bindWorkflowView(fixture.root,wrapped,{initialMode:'depth',sceneFactory:()=>({draw:value=>draws.push(value),invalidateProjection(){},setTopology(){},dispose(){}})});
  controller.select({kind:'node',index:0});const before=controller.getState();
  fixture.root.click({workflowSceneFocus:''});const frame=draws.at(-1).frame;
  assert.ok(frame);assert.deepEqual(controller.getState(),before);assert.equal(fixture.frames.size,0);
  controller.select({kind:'data',index:0});assert.deepEqual(draws.at(-1).frame,frame);
  binding.render();assert.deepEqual(draws.at(-1).frame,frame);
  const idle=draws.length,target={closest:()=>null,matches:()=>false,getAttribute:()=>null};
  fixture.document.activeElement=target;
  for(const key of ['ArrowLeft','ArrowRight','+','-','0']) fixture.listeners.get('keydown')({key,target,preventDefault(){throw Error('Camera key captured');}});
  fixture.listeners.get('pointerdown')({target,pointerId:1,clientX:0,clientY:0});fixture.listeners.get('pointermove')({target,pointerId:1,clientX:150,clientY:80});fixture.listeners.get('pointerup')({target,pointerId:1});
  assert.equal(draws.length,idle);assert.equal(fixture.frames.size,0);
  pending=true;binding.render();fixture.root.click({workflowSceneFocus:''});assert.deepEqual(draws.at(-1).frame,frame);
  assert.match(fixture.root.html.match(/data-workflow-scene-focus[^>]*>/)[0],/disabled/);
  fixture.root.click({workflowMode:'flat'});assert.doesNotMatch(fixture.root.html,/data-workflow-scene-canvas/);
  pending=false;fixture.root.click({workflowMode:'depth'});fixture.root.click({workflowSceneFocus:''});assert.ok(draws.at(-1).frame);
  const savedViewport=structuredClone(controller.getState().viewport);controller.addNode('check',{x:600,y:200});
  assert.equal(draws.at(-1).frame,null);assert.match(fixture.root.html,/Graph changed. Overview restored/);assert.deepEqual(controller.getState().viewport,savedViewport);
  fixture.root.click({workflowSceneFocus:''});fixture.root.click({workflowSceneOverview:''});assert.equal(draws.at(-1).frame,null);assert.equal(fixture.frames.size,0);
  assert.ok(draws.every(draw=>draw.camera?.yaw===-12&&draw.camera?.pitch===44&&draw.camera?.zoom===.95));
  binding.dispose();assert.equal(fixture.frames.size,0);
});

test('layout-only edits retain the focused target independently of current selection and saved viewport',async()=>{
  let binding;const controller=await graph(()=>binding?.render()),fixture=rootFixture(),draws=[];
  binding=bindWorkflowView(fixture.root,controller,{initialMode:'depth',sceneFactory:()=>({draw:value=>draws.push(value),invalidateProjection(){},setTopology(){},dispose(){}})});
  controller.select({kind:'node',index:0});fixture.root.click({workflowSceneFocus:''});const before=draws.at(-1).frame;
  const oldProjection=projectWorkflowScene(controller.getState());controller.select({kind:'node',index:1});fixture.root.click({workflowMode:'flat'});
  const viewport=structuredClone(controller.getState().viewport);assert.equal(controller.moveNode(0,-350,210),true);
  const moved=controller.getState(),projection=projectWorkflowScene(moved);
  assert.equal(projection.topologyFingerprint,oldProjection.topologyFingerprint);assert.notEqual(projection.fingerprint,oldProjection.fingerprint);
  fixture.root.click({workflowMode:'depth'});
  assert.deepEqual(draws.at(-1).frame,frameForSelection(projection,{kind:'node',index:0}));assert.notDeepEqual(draws.at(-1).frame.center,before.center);
  assert.deepEqual(controller.getState(),moved);assert.deepEqual(moved.viewport,viewport);assert.equal(fixture.frames.size,0);
  assert.doesNotMatch(fixture.root.html,/Graph changed. Overview restored/);
  controller.undo();assert.deepEqual(draws.at(-1).frame,before);
  controller.removeNode(0,true);assert.equal(draws.at(-1).frame,null);assert.match(fixture.root.html,/Graph changed. Overview restored/);
  binding.dispose();
});

test('stationary captured node-container clicks select the module without editing graph or history',async()=>{
  let binding;const controller=await graph(()=>binding?.render()),fixture=rootFixture();
  binding=bindWorkflowView(fixture.root,controller);controller.select({kind:'data',index:0});
  const before=controller.getState();fixture.root.click({workflowNode:'0'});
  const after=controller.getState();assert.deepEqual(after.selection,{kind:'node',index:0});
  assert.deepEqual(after.definition,before.definition);assert.deepEqual(after.layout,before.layout);assert.deepEqual(after.viewport,before.viewport);
  assert.equal(after.canUndo,before.canUndo);assert.equal(after.canRedo,before.canRedo);
  assert.match(fixture.root.html,/SELECTED MODULE · input/);binding.dispose();
});

test('captured node-container targets complete control connections through the same controller checks',async()=>{
  let binding;const controller=await graph(()=>binding?.render()),fixture=rootFixture();controller.removeEdge('control',0);
  binding=bindWorkflowView(fixture.root,controller);
  fixture.root.click({workflowControlSource:'0:next'});fixture.root.click({workflowNode:'1'});await Promise.resolve();
  assert.equal(controller.getState().definition.controlEdges.length,1);
  controller.undo();assert.equal(controller.getState().definition.controlEdges.length,0);
  fixture.root.click({workflowControlSource:'0:next'});fixture.root.click({workflowNode:'0'});await Promise.resolve();
  assert.equal(controller.getState().definition.controlEdges.length,0);binding.dispose();
});

test('reduced motion, visibility, selection change and disposal cancel previews without resume', async () => {
  let binding; const controller = await graph(() => binding?.render()), fixture = rootFixture();
  binding = bindWorkflowView(fixture.root, controller, { sceneFactory: () => ({ draw() {}, invalidateProjection() {}, setTopology() {}, dispose() {} }) });
  controller.select({ kind: 'data', index: 0 }); fixture.root.click({ workflowPreview: '' }); assert.equal(fixture.frames.size, 1);
  fixture.document.hidden = true; fixture.listeners.get('document:visibilitychange')(); assert.equal(fixture.frames.size, 0);
  fixture.document.hidden = false; fixture.listeners.get('document:visibilitychange')(); assert.equal(fixture.frames.size, 0);
  fixture.root.click({ workflowPreview: '' }); controller.select({ kind: 'node', index: 0 }); assert.equal(fixture.frames.size, 0);
  controller.select({ kind: 'data', index: 0 }); fixture.root.click({ workflowPreview: '' }); const preference = fixture.media.get('(prefers-reduced-motion: reduce)'); preference.value.matches = true; preference.listener({ matches: true }); assert.equal(fixture.frames.size, 0);
  fixture.root.click({ workflowPreview: '' }); assert.match(fixture.root.html, /Reduced motion: selected connection highlighted/); assert.equal(fixture.frames.size, 0); preference.value.matches = false; preference.listener({ matches: false }); fixture.root.click({ workflowPreview: '' }); const late = [...fixture.frames.values()][0]; binding.dispose(); late?.(500); assert.equal(fixture.frames.size, 0);
});


test('connection source survives view switch and completes through exact shared controller ports', async () => {
  let binding; const controller = await graph(() => binding?.render()), fixture = rootFixture();
  controller.removeEdge('data', 0);
  binding = bindWorkflowView(fixture.root, controller);
  const before = controller.getState(); fixture.root.click({ workflowDataSource: '0:text' });
  fixture.root.click({ workflowMode: 'depth' });
  assert.match(fixture.root.html, /Data source text selected/);
  assert.deepEqual(controller.getState(), before);
  fixture.root.click({ workflowDataTarget: '1:text' }); await Promise.resolve();
  assert.equal(controller.getState().definition.dataEdges.length, 1);
  assert.equal(controller.getState().definition.dataEdges[0].fromNode, before.definition.nodes[0].id);
  controller.undo(); assert.equal(controller.getState().definition.dataEdges.length, 0); binding.dispose();
});

test('graphics loss exposes exact retry, retains graph and never restarts animation automatically', async () => {
  let binding; const controller = await graph(() => binding?.render()), fixture = rootFixture(), callbacks = [];
  let disposed = 0;
  binding = bindWorkflowView(fixture.root, controller, { initialMode: 'depth', sceneFactory: (canvas, projection, hooks) => { callbacks.push(hooks); return { draw() {}, invalidateProjection() {}, setTopology() {}, dispose() { disposed++; } }; } });
  const before = controller.getState(); callbacks[0].onUnavailable('Graphics context lost.'); binding.render();
  assert.match(fixture.root.html, /Retry 3D/); assert.doesNotMatch(fixture.root.html, /data-workflow-scene-canvas/);
  fixture.root.click({ workflowSceneRetry: '' }); assert.equal(callbacks.length, 2); assert.equal(disposed, 1);
  assert.deepEqual(controller.getState(), before); fixture.root.flush(0); assert.equal(fixture.frames.size, 0); binding.dispose();
});

test('graphics loss keeps flat preview available and exposes a secondary retry path', async () => {
  let binding; const controller = await graph(() => binding?.render()), fixture = rootFixture(), callbacks = [];
  let sceneFactoryCalls = 0;
  binding = bindWorkflowView(fixture.root, controller, { initialMode: 'depth', sceneFactory: (canvas, projection, hooks) => { sceneFactoryCalls++; callbacks.push(hooks); return { draw() {}, invalidateProjection() {}, setTopology() {}, dispose() {} }; } });
  callbacks[0].onUnavailable('Graphics context lost.'); binding.render();
  fixture.root.click({ workflowMode: 'flat' });
  controller.select({ kind: 'data', index: 0 }); fixture.root.click({ workflowPreview: '' });
  assert.equal(fixture.frames.size, 1);
  assert.match(fixture.root.html, /data-workflow-scene-retry/);
  fixture.root.click({ workflowSceneRetry: '' });
  assert.equal(sceneFactoryCalls, 2);
  assert.match(fixture.root.html, /data-workflow-scene-canvas/);
  binding.dispose();
});

test('flat preview lays DOM wire geometry before sampling and survives later selection and menu renders', async () => {
  let binding; const controller = await graph(() => binding?.render()), fixture = geometryRootFixture();
  try {
    binding = bindWorkflowView(fixture.root, controller);
    controller.select({ kind: 'control', index: 0 }); fixture.root.click({ workflowPreview: '' });
    assert.equal(fixture.emptyCalls, 0);
    assert.match(fixture.wire.getAttribute('d'), /^M /);
    assert.ok(fixture.geometryCalls.length > 0);
    assert.equal(fixture.marker.attributes.get('visibility'), 'visible');
    assert.equal(fixture.frames.size, 1);
    fixture.root.click({ workflowSelectNode: '0' });
    assert.equal(fixture.frames.size, 0);
    fixture.root.click({ workflowMenu: 'canvas' });
    assert.match(fixture.root.html, /data-workflow-context/);
  } finally {
    binding?.dispose();
    fixture.restoreCss();
  }
});
