import test from 'node:test';
import assert from 'node:assert/strict';
import { createDesktopStartupController, decodeDesktopStartupView } from '../preview/desktop-startup-model.mjs';

const id = 'a'.repeat(64);
const dto = (state = 'fresh', changes = {}) => ({ schemaVersion: 'rangoon.workspace-startup.v1', state, startupId: id, attemptId: 'b'.repeat(64), epoch: '1', actions: { fresh: ['setup', 'quit'], legacy_found: ['review_conversion', 'quit'], conversion_review: ['copy', 'cancel', 'quit'], copying: ['quit'], restart_required: ['quit'], verify_required: ['verify', 'quit'], recovery_required: ['recover', 'quit'], handoff: [], unavailable: ['quit'] }[state], copyPhase: state === 'copying' ? 'copying_records' : null, retainedOriginal: { fresh: 'not_applicable', legacy_found: 'unassessed', conversion_review: 'present_unencrypted', copying: 'present_unencrypted', restart_required: 'present_unencrypted', verify_required: 'not_verified', recovery_required: 'not_verified', handoff: 'not_applicable', unavailable: 'not_applicable' }[state], diagnostic: null, ...changes });
const deferred = () => { let resolve; const promise = new Promise(result => { resolve = result; }); return { promise, resolve }; };

test('strict decode refuses malformed, replay-shaped and null-id DTOs', async () => {
  for (const value of [dto('fresh', { actions: ['quit', 'setup'] }), dto('fresh', { actions: ['setup', 'setup'] }), dto('fresh', { unknown: true }), dto('copying', { copyPhase: null }), dto('fresh', { retainedOriginal: 'unassessed' }), dto('legacy_found', { retainedOriginal: 'not_applicable' }), dto('fresh', { startupId: null }), dto('unavailable', { startupId: null, attemptId: null, epoch: null, actions: [], retainedOriginal: 'not_applicable' })]) {
    const model = createDesktopStartupController({ invoke: async () => value });
    await model.initialize();
    assert.equal(model.getState().status, value.state === 'unavailable' ? 'ready' : 'unavailable');
  }
  const subset = createDesktopStartupController({ invoke: async () => dto('recovery_required', { actions: ['quit'] }) });
  await subset.initialize();
  assert.equal(subset.getState().status, 'ready');
});

test('terminal handoff accepts only all-null source-free terminal DTOs', async () => {
  for (const retainedOriginal of ['not_applicable', 'present_unencrypted', 'not_verified']) {
    const handoff = dto('handoff', {
      startupId: null,
      attemptId: null,
      epoch: null,
      retainedOriginal,
    });
    assert.equal(decodeDesktopStartupView(handoff)?.retainedOriginal, retainedOriginal);
  }
  for (const changes of [
    { startupId: null },
    { startupId: null, attemptId: null },
    { attemptId: null, epoch: null },
    { actions: ['quit'] },
    { copyPhase: 'checking_copy' },
    { diagnostic: 'startup_busy' },
    { retainedOriginal: 'unassessed' },
  ]) {
    assert.equal(decodeDesktopStartupView(dto('handoff', changes)), null);
  }
  assert.equal(decodeDesktopStartupView(dto('unavailable', {
    startupId: null,
    attemptId: null,
    epoch: null,
    actions: [],
    retainedOriginal: 'not_verified',
  })), null);
});

test('terminal handoff cannot create an action request and stays terminal after disposal', async () => {
  const calls = [];
  const handoff = dto('handoff', {
    startupId: null,
    attemptId: null,
    epoch: null,
    retainedOriginal: 'present_unencrypted',
  });
  const model = createDesktopStartupController({ invoke: async (...args) => { calls.push(args); return handoff; } });
  await model.initialize();
  assert.equal(model.getState().status, 'handoff');
  assert.equal(await model.perform('quit'), false);
  assert.equal(calls.length, 1);
  model.dispose();
  assert.deepEqual(await model.refresh(), model.getState());
  assert.equal(await model.perform('quit'), false);
  assert.equal(calls.length, 1);
});

test('action sends exact bounded JSON, then blocks repeat on same native cycle', async () => {
  const calls = [];
  const model = createDesktopStartupController({ invoke: async (command, payload) => {
    calls.push([command, payload]);
    return command === 'workspace_startup_status' ? dto() : dto('conversion_review');
  } });
  await model.initialize();
  assert.equal(await model.perform('setup'), true);
  const payload = calls[1][1];
  assert.equal(payload instanceof Uint8Array, true);
  assert.equal(payload.byteLength <= 512, true);
  assert.equal(payload.request, undefined);
  assert.deepEqual(JSON.parse(new TextDecoder().decode(payload)), {
    schemaVersion: 'rangoon.workspace-startup-action.v1',
    startupId: id,
    attemptId: 'b'.repeat(64),
    epoch: '1',
    action: 'setup',
  });
  assert.equal(await model.perform('copy'), true);
  assert.equal(await model.perform('copy'), false);
  assert.equal(calls.length, 3);
});

test('cancel replacement and native errors remain closed without browser fallback', async () => {
  let action = false;
  const model = createDesktopStartupController({ invoke: async command => command === 'workspace_startup_status' ? dto('conversion_review') : (action = true, dto('legacy_found')) });
  await model.initialize();
  assert.equal(await model.perform('cancel'), true);
  assert.equal(action, true);
  assert.equal(model.getState().view.state, 'legacy_found');
  const failed = createDesktopStartupController({ invoke: async () => { throw Error('native lost'); } });
  await failed.initialize();
  assert.equal(failed.getState().status, 'unavailable');
});

test('late and disposed work never replaces current generation', async () => {
  const pending = deferred();
  const model = createDesktopStartupController({ invoke: () => pending.promise });
  const start = model.initialize();
  model.dispose();
  pending.resolve(dto());
  await start;
  assert.equal(model.getState().status, 'unavailable');
  assert.equal(model.getState().view, null);
});

test('initialize coalesces one native status request by promise identity', async () => {
  const pending = deferred();
  const model = createDesktopStartupController({ invoke: () => pending.promise });
  const first = model.initialize();
  assert.equal(model.initialize(), first);
  pending.resolve(dto());
  await first;
});

test('reentrant disposal after working publish prevents native mutation', async () => {
  const calls = [];
  let model;
  model = createDesktopStartupController({
    invoke: async (command, payload) => { calls.push([command, payload]); return dto(); },
    onChange: state => { if (state.status === 'working') model.dispose(); },
  });
  await model.initialize();
  assert.equal(await model.perform('setup'), false);
  assert.deepEqual(calls.map(([command]) => command), ['workspace_startup_status']);
});

test('late action result after disposal returns false and cannot restore readiness', async () => {
  const pending = deferred();
  const model = createDesktopStartupController({ invoke: command => command === 'workspace_startup_status' ? Promise.resolve(dto()) : pending.promise });
  await model.initialize();
  const action = model.perform('setup');
  model.dispose();
  pending.resolve(dto('conversion_review'));
  assert.equal(await action, false);
  assert.equal(model.getState().status, 'unavailable');
});
