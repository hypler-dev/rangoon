import test from 'node:test';
import assert from 'node:assert/strict';
import { FLOW_SCENARIOS, FLOW_NODES, FLOW_EDGES, createFlowSimulation, pointOnFlowEdge } from '../preview/flow-simulation.mjs';

test('closed immutable scenarios remain synthetic with distinct trace/event identities', () => {
  assert.equal(FLOW_SCENARIOS.length, 4);
  const events = new Set();
  for (const scenario of FLOW_SCENARIOS) {
    assert.ok(scenario.traceId.startsWith('synthetic-trace:'));
    assert.ok(scenario.events.length <= 8);
    assert.throws(() => { scenario.events.push({}); }, TypeError);
    for (const event of scenario.events) {
      assert.ok(!events.has(event.id)); events.add(event.id);
      assert.ok(FLOW_NODES.some(node => node.id === event.nodeId));
      assert.ok(event.edgeId === null || FLOW_EDGES.some(edge => edge.id === event.edgeId));
    }
  }
  const state = createFlowSimulation().snapshot();
  assert.deepEqual([state.authority, state.authenticity, state.authoritativeEligibility, state.execution], ['none', 'unverified', 'unknown', 'unavailable']);
  assert.equal(state.playing, false);
  assert.throws(() => { state.cursor = 100; }, TypeError);
});

test('policy allow, human decision, authorization, consumption and outcome stay distinct', () => {
  const model = createFlowSimulation();
  model.seek(3);
  assert.deepEqual(model.snapshot().scenario.events.slice(0, model.snapshot().cursor).map(event => event.stage), ['request', 'evaluation', 'recommendation']);
  assert.match(model.snapshot().scenario.events[2].detail, /not a Rangoon approval/);
  model.seek(7);
  assert.deepEqual(model.snapshot().scenario.events.map(event => event.stage), ['request', 'evaluation', 'recommendation', 'approval', 'authorization', 'consumption', 'outcome']);
  assert.equal(model.snapshot().execution, 'unavailable');
});

test('denial and expiry never illustrate an attempt; missing outcome has no moving evidence packet', () => {
  const model = createFlowSimulation();
  for (const id of ['denied', 'expired']) {
    model.choose(id);
    assert.ok(!model.snapshot().scenario.events.some(event => ['consumption', 'outcome'].includes(event.stage)));
    assert.equal(model.snapshot().scenario.events.at(-1).stage, 'refusal');
  }
  model.choose('unknown'); model.seek(6); model.play();
  assert.equal(model.motion(), null);
  model.seek(7);
  assert.equal(model.snapshot().scenario.events.at(-1).reported, 'effect unknown');
  assert.equal(model.snapshot().scenario.events.at(-1).evidenceClass, 'missing');
});

test('scrubbing validates exact integer bounds and cannot select unseen or foreign events', () => {
  const model = createFlowSimulation();
  for (const cursor of [-1, 8, 0.5, NaN, Infinity, '2', {}, null]) assert.equal(model.seek(cursor), false);
  assert.equal(model.snapshot().cursor, 0);
  assert.equal(model.selectEvent(0), false);
  assert.equal(model.choose('__proto__'), false);
  assert.equal(model.selectNode('foreign'), false);
  assert.equal(model.setMode('3d'), false);
  model.seek(2); assert.equal(model.selectEvent(0), true);
  assert.equal(model.selectEvent(2), false);
  assert.equal(model.step(-1), true);
  assert.equal(model.snapshot().selection.index, 0);
  assert.equal(model.step(2), false);
});

test('frame ticks are bounded and notify only at discrete script boundaries', () => {
  let changes = 0;
  const model = createFlowSimulation({ onChange: () => changes++ });
  model.play(); assert.equal(changes, 1);
  for (let i = 0; i < 6; i++) model.tick(60_000);
  assert.equal(changes, 1); assert.equal(model.snapshot().cursor, 0);
  assert.ok(model.motion().x >= 220 && model.motion().x <= 260);
  model.tick(60_000);
  assert.equal(model.snapshot().cursor, 1); assert.equal(changes, 2);
  const before = model.snapshot();
  for (const delta of [-1, NaN, Infinity, '250']) assert.equal(model.tick(delta), false);
  assert.deepEqual(model.snapshot(), before);
  for (let i = 0; i < 60; i++) model.tick(250);
  assert.equal(model.snapshot().cursor, 7); assert.equal(model.snapshot().playing, false);
  assert.equal(model.play(), false); assert.equal(model.motion(), null);
});

test('scenario/reset/motion preference/disposal cancel play and preserve deterministic replay', () => {
  const model = createFlowSimulation();
  model.seek(3); const selected = model.snapshot().selection;
  model.play(); model.tick(250); model.pause();
  assert.equal(model.snapshot().playing, false);
  model.seek(3); assert.deepEqual(model.snapshot().selection, selected);
  assert.equal(model.snapshot().progress, 0);
  model.setMode('depth'); model.play(); model.setReducedMotion(true);
  assert.equal(model.snapshot().mode, 'flat'); assert.equal(model.play(), false);
  assert.equal(model.setMode('depth'), false); assert.equal(model.step(), true);
  model.setReducedMotion(false); assert.equal(model.snapshot().playing, false);
  model.play(); model.choose('denied'); assert.equal(model.snapshot().cursor, 0);
  assert.equal(model.snapshot().playing, false); assert.equal(model.snapshot().selection, null);
  model.setMode('depth'); model.reset(); assert.equal(model.snapshot().mode, 'flat');
  model.dispose();
  for (const call of [() => model.play(), () => model.seek(1), () => model.choose('mediated'), () => model.pause(), () => model.tick(250)]) assert.equal(call(), false);
  assert.equal(model.motion(), null);
});

test('wire motion follows bounded polyline geometry rather than page coordinates', () => {
  assert.deepEqual(pointOnFlowEdge('denial', 0), { x: 585, y: 100 });
  assert.deepEqual(pointOnFlowEdge('denial', 1), { x: 355, y: 100 });
  assert.deepEqual(pointOnFlowEdge('denial', 0.5), { x: 470, y: 60 });
  for (const value of [-0.1, 1.1, NaN, Infinity, '0.5']) assert.equal(pointOnFlowEdge('request', value), null);
  assert.equal(pointOnFlowEdge('foreign', 0.5), null);
});

test('concrete request and system explanations stay immutable and provider-independent', async () => {
  const { FLOW_REQUEST, FLOW_SYSTEMS, FLOW_NODES } = await import('../preview/flow-simulation.mjs');
  assert.equal(FLOW_REQUEST.sourceKind, 'synthetic');
  assert.equal(FLOW_REQUEST.action, 'pull_request.create');
  assert.ok(Object.isFrozen(FLOW_REQUEST));
  assert.deepEqual(Object.keys(FLOW_SYSTEMS), FLOW_NODES.map(node => node.id));
  for (const system of Object.values(FLOW_SYSTEMS)) assert.ok(Object.isFrozen(system));
  assert.match(FLOW_SYSTEMS.policy.boundary, /not human approval/);
  assert.match(FLOW_SYSTEMS.authority.boundary, /not a connected backend/);
  assert.match(FLOW_SYSTEMS.evidence.boundary, /never means success/);
});
