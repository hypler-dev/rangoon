import test from 'node:test';
import assert from 'node:assert/strict';
import { createAuthorityPlannerDrag } from '../preview/authority-planner-drag.mjs';

function fixture(overrides = {}) {
  const state = { revision: 3, ...overrides };
  const frames = new Map();
  const calls = [];
  const props = new Map();
  const classes = new Set();
  let sequence = 0;
  const source = { isConnected: true, style: { setProperty: (key, value) => props.set(key, value), removeProperty: key => props.delete(key) }, classList: { add: value => classes.add(value), remove: value => classes.delete(value) }, setPointerCapture() {}, hasPointerCapture: () => true, releasePointerCapture() {} };
  const drag = createAuthorityPlannerDrag({ getState: () => state, requestFrame: callback => { frames.set(++sequence, callback); return sequence; }, cancelFrame: id => frames.delete(id), hitRole: (x, y) => x === 60 && y === 60 ? 'gateway' : null, commit: (...args) => { calls.push(args); return true; } });
  const event = (x, y, extra = {}) => ({ button: 0, isPrimary: true, pointerId: 9, clientX: x, clientY: y, preventDefault() {}, ...extra });
  const tick = () => { const callbacks = [...frames.values()]; frames.clear(); callbacks.forEach(callback => callback()); };
  return { state, frames, calls, props, classes, source, drag, event, tick };
}

test('threshold coalesces one visual frame then commits exactly once to matching role', () => {
  const f = fixture();
  assert.equal(f.drag.start(f.event(10, 10), f.source, 'agentgateway'), true);
  f.drag.move(f.event(13, 13));
  assert.equal(f.frames.size, 0);
  f.drag.move(f.event(60, 60));
  f.drag.move(f.event(61, 61));
  assert.equal(f.frames.size, 1);
  f.tick();
  assert.equal(f.props.get('--plan-drag-x'), '51px');
  assert.equal(f.drag.finish(f.event(60, 60)), true);
  assert.deepEqual(f.calls, [['agentgateway', 'gateway']]);
  assert.equal(f.drag.finish(f.event(60, 60)), false);
  assert.equal(f.props.size, 0);
});

test('stationary, wrong-role, outside, wrong-pointer and invalid-pointer starts do not commit', () => {
  const f = fixture();
  assert.equal(f.drag.start(f.event(0, 0, { button: 2 }), f.source, 'agentgateway'), false);
  assert.equal(f.drag.start(f.event(0, 0), f.source, 'agentgateway'), true);
  assert.equal(f.drag.finish(f.event(60, 60)), false);
  assert.equal(f.drag.start(f.event(0, 0), f.source, 'agentgateway'), true);
  f.drag.move(f.event(22, 22));
  assert.equal(f.drag.finish(f.event(22, 22)), false);
  assert.deepEqual(f.calls, []);
  assert.equal(f.drag.start(f.event(0, 0), f.source, 'agentgateway'), true);
  assert.equal(f.drag.move(f.event(30, 30, { pointerId: 8 })), false);
  f.drag.cancel();
});

for (const interruption of ['cancel', 'stale', 'disconnect', 'dispose']) test(`${interruption} clears visual state and defers no commit`, () => {
  const f = fixture();
  f.drag.start(f.event(0, 0), f.source, 'agentgateway');
  f.drag.move(f.event(60, 60));
  if (interruption === 'cancel') f.drag.cancel(9);
  if (interruption === 'stale') f.state.revision++;
  if (interruption === 'disconnect') f.source.isConnected = false;
  if (interruption === 'dispose') f.drag.dispose();
  f.tick();
  assert.deepEqual(f.calls, []);
  assert.equal(f.props.size, 0);
  assert.equal(f.classes.size, 0);
  assert.equal(f.frames.size, 0);
  assert.equal(f.drag.isActive(), false);
});
