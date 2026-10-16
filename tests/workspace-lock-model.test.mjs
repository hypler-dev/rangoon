import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorkspaceLockController } from '../preview/workspace-lock-model.mjs';
const nonce = 'ab'.repeat(32);
const metadata = (state = 'open', extra = {}) => ({ schemaVersion: 'rangoon.workspace-close-state.v1', storage: 'managed_encrypted', state, attemptId: state === 'open' ? null : nonce, epoch: state === 'open' ? null : '1', diagnostic: null, ...extra });
const legacy = () => metadata('unavailable', { storage: 'legacy_plaintext', attemptId: null, epoch: null, diagnostic: 'encrypted_close_unavailable' });
const deferred = () => { let resolve, reject; const promise = new Promise((a, b) => { resolve = a; reject = b; }); return { promise, resolve, reject }; };
function fixture(options = {}) {
  const calls = [], changes = [], timers = new Map();
  let serial = 0, purges = 0;
  const invoke = async (command, args) => {
    calls.push([command, args]);
    if (command === 'get_workspace_request_state') return { schemaVersion: 'rangoon.workspace-request-state.v1', storage: 'managed_encrypted', state: 'open', clientId: nonce };
    if (options.invoke) return options.invoke(command, args);
    if (command === 'get_workspace_close_state') return metadata();
    if (command === 'begin_workspace_close') return metadata('closing');
    if (command === 'acknowledge_workspace_purge') return metadata('locked');
    return { data: 'normal-source' };
  };
  const model = createWorkspaceLockController({
    invoke, beforeInvoke: options.beforeInvoke,
    purge: () => { ++purges; return options.purge?.(); },
    onChange: state => { changes.push(state); options.onChange?.(state); },
    schedule: callback => { timers.set(++serial, callback); return serial; },
    cancelSchedule: id => timers.delete(id),
  });
  return { model, calls, changes, timers, purges: () => purges };
}
const close = async model => { assert.equal(model.openConfirmation(), true); return model.confirmClose(); };

test('browser is usable without bridge or IPC; verified Legacy keeps core calls and has no Lock', async () => {
  const browser = createWorkspaceLockController();
  assert.equal(browser.bridge, undefined);
  assert.equal((await browser.initialize()).status, 'browser');
  assert.equal(browser.openConfirmation(), false);
  const f = fixture({ invoke: command => command === 'get_workspace_close_state' ? legacy() : 'local-data' });
  assert.equal((await f.model.initialize()).status, 'legacy');
  assert.equal(f.model.openConfirmation(), false);
  assert.equal(await f.model.bridge('list_snapshots'), 'local-data');
  assert.equal(f.purges(), 0);
  assert.deepEqual(f.calls.map(([command]) => command), ['get_workspace_close_state', 'list_snapshots']);
});

test('bootstrap blocks source access until exact metadata, and initializes once', async () => {
  const gate = deferred();
  const f = fixture({ invoke: () => gate.promise });
  const loading = f.model.initialize();
  await assert.rejects(f.model.bridge('list_snapshots'), /Workspace access is closed/);
  assert.equal(f.calls.length, 1);
  gate.resolve(metadata());
  await loading;
  await f.model.initialize();
  assert.equal(f.calls.length, 2);
  assert.deepEqual(f.model.getState(), { status: 'open', blocked: false, canLock: true });
});

test('confirm closes admission synchronously; acknowledgment is exact private Raw UTF-8 bytes', async () => {
  const begin = deferred();
  const f = fixture({ invoke: (command, args) => {
    if (command === 'get_workspace_close_state') return metadata();
    if (command === 'begin_workspace_close') return begin.promise;
    assert.equal(command, 'acknowledge_workspace_purge');
    assert.ok(args instanceof Uint8Array);
    assert.deepEqual(JSON.parse(new TextDecoder().decode(args)), { schemaVersion: 'rangoon.workspace-purge-ack.v1', attemptId: nonce, epoch: '1' });
    return metadata('locked');
  } });
  await f.model.initialize();
  f.model.openConfirmation();
  const closing = f.model.confirmClose();
  assert.equal(f.model.isBlocked(), true);
  assert.equal(f.purges(), 1);
  await assert.rejects(f.model.bridge('save_analysis', { sourceId: 'private' }), /closed/);
  assert.equal(await f.model.confirmClose(), false);
  begin.resolve(metadata('closing'));
  assert.equal(await closing, true);
  assert.equal(f.model.getState().status, 'locked');
  assert.equal(f.calls.length, 4);
  assert.equal(JSON.stringify(f.changes).includes(nonce), false);
  assert.equal(JSON.stringify(f.changes).includes('epoch'), false);
});

test('Cancel returns to open without native begin or losing drafts', async () => {
  const f = fixture(); await f.model.initialize();
  assert.equal(f.model.openConfirmation(), true);
  assert.equal(f.model.getState().status, 'confirming');
  assert.equal(f.model.cancelConfirmation(), true);
  assert.equal(f.model.getState().status, 'open');
  assert.deepEqual(await f.model.bridge('open_snapshot'), { data: 'normal-source' });
  assert.equal(f.purges(), 0);
  assert.equal(f.calls.some(([cmd]) => cmd === 'begin_workspace_close'), false);
});

test('malformed metadata and old/native transport errors never permit source bootstrap', async () => {
  let getterCalls = 0;
  const getter = metadata(); Object.defineProperty(getter, 'state', { get: () => { ++getterCalls; return 'open'; }, enumerable: true });
  const cases = [undefined, null, [], {}, 'open', metadata('open', { extra: true }), metadata('open', { schemaVersion: 'v2' }), metadata('open', { attemptId: nonce }), metadata('open', { epoch: '1' }), metadata('open', { diagnostic: 'invalid_workspace_close_request' }), metadata('open', { storage: 'unknown' }), getter, Object.create(metadata()), metadata('closing', { epoch: '01' }), metadata('closing', { epoch: 1 }), metadata('closing', { epoch: '18446744073709551616' }), metadata('closing', { attemptId: nonce.toUpperCase() }), legacy() && { ...legacy(), diagnostic: null }];
  for (const value of cases) {
    const f = fixture({ invoke: () => value });
    assert.equal((await f.model.initialize()).status, 'recovery_required');
    assert.equal(f.purges(), 1);
    await assert.rejects(f.model.bridge('list_snapshots'), /closed/);
    assert.equal(f.calls.length, 1);
  }
  const failed = fixture({ invoke: () => { throw new Error('private-native-error'); } });
  await failed.model.initialize();
  assert.equal(failed.model.getState().status, 'recovery_required');
  assert.equal(getterCalls, 0);
});

test('ordinary bridge cannot issue any lifecycle command', async () => {
  const f = fixture(); await f.model.initialize();
  for (const command of ['get_workspace_close_state', 'begin_workspace_close', 'acknowledge_workspace_purge', 'get_workspace_request_state']) await assert.rejects(f.model.bridge(command, {}), /closed/);
  assert.equal(f.calls.length, 2);
});

test('reentrant preflight close prevents the original effect', async () => {
  let f, closing;
  f = fixture({ beforeInvoke: () => { closing = close(f.model); } });
  await f.model.initialize();
  await assert.rejects(f.model.bridge('save_analysis', { sourceId: 'private' }), /closed/);
  await closing;
  assert.equal(f.calls.some(([cmd]) => cmd === 'save_analysis'), false);
});

test('late source successes and errors, including writes, are replaced by fixed closed error', async () => {
  for (const command of ['open_snapshot', 'save_analysis']) for (const failed of [false, true]) {
    const pending = deferred();
    const f = fixture({ invoke: cmd => cmd === command ? pending.promise : cmd === 'get_workspace_close_state' ? metadata() : metadata(cmd === 'begin_workspace_close' ? 'closing' : 'locked') });
    await f.model.initialize();
    const result = f.model.bridge(command, { sourceId: 'private' });
    const rejected = assert.rejects(result, error => error.message === 'Workspace access is closed.');
    await close(f.model);
    if (failed) pending.reject(new Error('source-in-error')); else pending.resolve({ content: 'source-in-result' });
    await rejected;
    assert.equal(f.model.getState().status, 'locked');
  }
});

test('transport and completion-observer reentrant closes cannot deliver old results or errors', async () => {
  for (const boundary of ['transport', 'completion']) for (const failed of [false, true]) {
    let f, closing;
    f = fixture({
      invoke: command => {
        if (command === 'get_workspace_close_state') return metadata();
        if (command === 'begin_workspace_close') return metadata('closing');
        if (command === 'acknowledge_workspace_purge') return metadata('locked');
        if (boundary === 'transport') closing = close(f.model);
        if (failed) throw new Error('private-old-error');
        return 'private-old-result';
      },
      beforeInvoke: () => () => { if (boundary === 'completion') closing = close(f.model); },
    });
    await f.model.initialize();
    await assert.rejects(f.model.bridge('open_snapshot'), error => error.message === 'Workspace access is closed.');
    await closing;
  }
});

test('failed local purge still closes native Gate but never acknowledges or retries', async () => {
  for (const purge of [() => false, () => { throw new Error('private-cleanup-error'); }]) {
    const f = fixture({ purge }); await f.model.initialize(); await close(f.model);
    assert.equal(f.model.getState().status, 'recovery_required');
    assert.deepEqual(f.calls.map(([cmd]) => cmd), ['get_workspace_close_state', 'get_workspace_request_state', 'begin_workspace_close']);
    assert.equal(f.timers.size, 0);
  }
});

test('missing, rejected, foreign or malformed begin/ack never becomes Locked', async () => {
  const cases = [null, metadata('locked', { diagnostic: 'invalid_workspace_close_request' }), metadata('locked', { attemptId: 'cd'.repeat(32) }), metadata('locked', { epoch: '2' }), metadata('open'), legacy(), metadata('recovery_required', { diagnostic: 'workspace_close_incomplete' })];
  for (const value of cases) {
    const f = fixture({ invoke: command => command === 'get_workspace_close_state' ? metadata() : command === 'begin_workspace_close' ? metadata('closing') : value });
    await f.model.initialize(); assert.equal(await close(f.model), false);
    assert.equal(f.model.getState().status, 'recovery_required');
    assert.equal(f.timers.size, 0);
  }
  for (const phase of ['begin_workspace_close', 'acknowledge_workspace_purge']) {
    const f = fixture({ invoke: command => { if (command === phase) throw new Error('lost-response'); return metadata(command === 'get_workspace_close_state' ? 'open' : 'closing'); } });
    await f.model.initialize(); await close(f.model);
    assert.equal(f.model.getState().status, 'recovery_required');
  }
});

test('real Closing polls only actual metadata, exact attempt required, no timer success', async () => {
  let checks = 0;
  const f = fixture({ invoke: cmd => cmd === 'get_workspace_close_state' ? metadata(++checks === 1 ? 'open' : 'locked') : metadata('closing') });
  await f.model.initialize(); await close(f.model);
  assert.equal(f.model.getState().status, 'closing');
  assert.equal(f.timers.size, 1);
  const [id, callback] = f.timers.entries().next().value; f.timers.delete(id); await callback();
  assert.equal(f.model.getState().status, 'locked'); assert.equal(f.timers.size, 0);
  assert.equal(f.calls.filter(([cmd]) => cmd === 'acknowledge_workspace_purge').length, 1);
});

test('fresh closing/locked windows purge before inspecting native custody; duplicate ack rejection stays recovery', async () => {
  for (const initial of ['closing', 'locked']) {
    const f = fixture({ invoke: command => command === 'get_workspace_close_state' ? metadata(initial) : metadata('locked') });
    await f.model.initialize();
    assert.equal(f.purges(), 1); assert.equal(f.model.getState().status, 'locked');
    assert.equal(f.calls.length, initial === 'closing' ? 2 : 1);
  }
  const f = fixture({ invoke: command => metadata(command === 'get_workspace_close_state' ? 'closing' : 'locked', command === 'acknowledge_workspace_purge' ? { diagnostic: 'invalid_workspace_close_request' } : {}) });
  await f.model.initialize(); assert.equal(f.model.getState().status, 'recovery_required');
});

test('pagehide/dispose purges and fences stale initialization, timers and callbacks without native begin', async () => {
  const pending = deferred(); const f = fixture({ invoke: () => pending.promise });
  const initializing = f.model.initialize(); f.model.dispose(); pending.resolve(metadata()); await initializing;
  assert.equal(f.model.getState().status, 'recovery_required'); assert.equal(f.purges(), 1); assert.equal(f.calls.length, 1);
  assert.equal(f.model.openConfirmation(), false); await assert.rejects(f.model.bridge('open_snapshot'), /closed/);
  const polling = fixture({ invoke: cmd => metadata(cmd === 'get_workspace_close_state' ? 'open' : 'closing') });
  await polling.model.initialize(); await close(polling.model);
  const callback = polling.timers.values().next().value;
  polling.model.dispose(); assert.equal(polling.timers.size, 0);
  const count = polling.calls.length; await callback(); assert.equal(polling.calls.length, count);
});
