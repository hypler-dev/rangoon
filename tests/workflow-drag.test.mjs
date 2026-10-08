import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorkflowDrag } from '../preview/workflow-drag.mjs';

function fixture(overrides = {}) {
  const state = { bridgeAvailable: true, pending: null, fieldEdit: null, session: { draftId: 'test-draft' }, definition: { nodes: [{ id: 'source' }] }, layout: { positions: [{ nodeIndex: 0, x: 40, y: 60 }] }, viewport: { zoom: 1 }, ...overrides };
  const properties = new Map(); const classes = new Set(); const frames = new Map(); const calls = []; let sequence = 0; let redraws = 0;
  const node = { dataset: { workflowNode: '0' }, isConnected: true, style: { setProperty: (key, value) => properties.set(key, value), removeProperty: key => properties.delete(key) }, classList: { add: key => classes.add(key), remove: key => classes.delete(key) }, setPointerCapture() {}, hasPointerCapture: () => true, releasePointerCapture() {} };
  const gesture = createWorkflowDrag({ getState: () => state, requestFrame: callback => { frames.set(++sequence, callback); return sequence; }, cancelFrame: id => frames.delete(id), redraw: () => { redraws++; }, commit: (...args) => calls.push(args) });
  const event = (x = 0, y = 0, extra = {}) => ({ button: 0, isPrimary: true, pointerId: 7, clientX: x, clientY: y, preventDefault() {}, ...extra });
  const tick = () => { const pending = [...frames.values()]; frames.clear(); pending.forEach(callback => callback()); };
  return { state, node, properties, classes, frames, calls, gesture, event, tick, redraws: () => redraws };
}

test('many moves coalesce to one frame; preview never mutates; release commits one rounded move', () => {
  const f = fixture(); assert.equal(f.gesture.start(f.event(10, 20), f.node), true);
  for (let x = 11; x <= 51; x++) f.gesture.move(f.event(x, 37));
  assert.equal(f.frames.size, 1); assert.deepEqual(f.calls, []); assert.equal(f.state.layout.positions[0].x, 40);
  f.tick(); assert.equal(f.properties.get('--node-drag-x'), '41px'); assert.equal(f.properties.get('--node-drag-y'), '17px'); assert.equal(f.redraws(), 1);
  assert.equal(f.gesture.finish(f.event(51.6, 37.2)), true); assert.deepEqual(f.calls, [[0, 82, 77]]);
  assert.equal(f.gesture.finish(f.event(60, 50)), false); assert.equal(f.calls.length, 1); assert.equal(f.properties.size, 0); assert.equal(f.frames.size, 0);
});

test('zoom is applied once; graph coordinates stay bounded', () => {
  const f = fixture({ viewport: { zoom: 2 } }); f.gesture.start(f.event(10, 20), f.node); f.gesture.move(f.event(110, 60)); f.tick();
  assert.equal(f.properties.get('--node-drag-x'), '50px'); f.gesture.finish(f.event(110, 60)); assert.deepEqual(f.calls, [[0, 90, 80]]);
  const large = fixture(); large.gesture.start(large.event(), large.node); large.gesture.finish(large.event(1e9, -1e9)); assert.deepEqual(large.calls, [[0, 100000, -100000]]);
});

for (const interruption of ['cancel', 'lost capture', 'Escape', 'dispose']) test(`${interruption} restores visuals and cancels scheduled work without committing`, () => {
  const f = fixture(); f.gesture.start(f.event(), f.node); f.gesture.move(f.event(20, 30)); f.tick(); f.gesture.move(f.event(25, 35));
  if (interruption === 'dispose') f.gesture.dispose(); else f.gesture.cancel(interruption === 'lost capture' ? 7 : undefined);
  f.tick(); assert.equal(f.frames.size, 0); assert.equal(f.properties.size, 0); assert.equal(f.classes.size, 0); assert.deepEqual(f.calls, []); assert.equal(f.gesture.isActive(), false);
  if (interruption === 'dispose') assert.equal(f.gesture.start(f.event(), f.node), false);
});

test('wrong pointer cannot move, finish or cancel the owning gesture', () => {
  const f = fixture(); f.gesture.start(f.event(), f.node);
  assert.equal(f.gesture.move(f.event(30, 40, { pointerId: 8 })), false); assert.equal(f.gesture.finish(f.event(30, 40, { pointerId: 8 })), false); assert.equal(f.gesture.cancel(8), false);
  assert.equal(f.gesture.isActive(), true); assert.deepEqual(f.calls, []); f.gesture.cancel();
});

for (const override of [{ pending: 'save' }, { fieldEdit: {} }, { session: null }, { bridgeAvailable: false }, { viewport: { zoom: 0 } }]) test(`refuses noneditable origin ${JSON.stringify(override)}`, () => {
  const f = fixture(override); assert.equal(f.gesture.start(f.event(), f.node), false); assert.equal(f.frames.size, 0); assert.deepEqual(f.calls, []);
});

test('an explicit playground remains editable without a bridge', () => {
  const f = fixture({ bridgeAvailable: false, playground: true }); assert.equal(f.gesture.start(f.event(), f.node), true); f.gesture.finish(f.event(25, 30)); assert.deepEqual(f.calls, [[0, 65, 90]]);
});

for (const mutation of [f => { f.state.pending = 'inspect'; }, f => { f.state.session = { draftId: 'other' }; }, f => { f.state.layout.positions[0].x++; }, f => { f.state.definition.nodes[0].id = 'other'; }, f => { f.node.isConnected = false; }, f => { f.state.viewport.zoom = 2; }, f => { f.state.fieldEdit = {}; }]) test('state invalidation drops transient movement before commit', () => {
  const f = fixture(); f.gesture.start(f.event(), f.node); f.gesture.move(f.event(30, 40)); mutation(f); f.tick();
  assert.deepEqual(f.calls, []); assert.equal(f.properties.size, 0); assert.equal(f.gesture.isActive(), false);
});

test('stationary clicks, nonprimary pointers and invalid coordinates create no draft mutation', () => {
  const f = fixture(); assert.equal(f.gesture.start(f.event(0, 0, { button: 2 }), f.node), false); assert.equal(f.gesture.start(f.event(0, 0, { isPrimary: false }), f.node), false); assert.equal(f.gesture.start(f.event(NaN), f.node), false);
  f.gesture.start(f.event(), f.node); assert.equal(f.gesture.finish(f.event()), false); assert.deepEqual(f.calls, []);
});
