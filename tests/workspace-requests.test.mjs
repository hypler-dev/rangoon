import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorkspaceLockController } from '../preview/workspace-lock-model.mjs';
import { createWorkspaceController } from '../preview/workspace-model.mjs';

const clientId = 'ab'.repeat(32);
const closeState = () => ({
  schemaVersion: 'rangoon.workspace-close-state.v1',
  storage: 'managed_encrypted',
  state: 'open',
  attemptId: null,
  epoch: null,
  diagnostic: null,
});
const requestState = (extra = {}) => ({
  schemaVersion: 'rangoon.workspace-request-state.v1',
  storage: 'managed_encrypted',
  state: 'open',
  clientId,
  ...extra,
});
const deferred = () => {
  let resolve;
  let reject;
  const promise = new Promise((ok, fail) => { resolve = ok; reject = fail; });
  return { promise, resolve, reject };
};

test('encrypted recovery controller routes every native action through the current request header', async () => {
  const workspaceData = {
    schemaVersion: 'rangoon.workspace-data.v2', stateId: 'workspace:' + '7'.repeat(64),
    records: { sources: 0, capabilities: 0, revisions: 0, reviews: 0, recipes: 0, applications: 0, derivations: 0, workflows: 0, workflowRevisions: 0 },
    sources: [], capabilities: [], workflows: [], databaseBytes: 12288, reusableBytes: 0,
  };
  const nativeCalls = [];
  const lock = createWorkspaceLockController({ invoke: async (command,args,options) => {
    if (command === 'get_workspace_close_state') return closeState();
    if (command === 'get_workspace_request_state') return requestState();
    assert.deepEqual(options, { headers: { 'x-rangoon-workspace-client': clientId } });
    nativeCalls.push([command,args]);
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspaceData, encryptedRecoverySupported: true };
    if (command === 'create_workspace_recovery_key') return { outcome: 'recovery_key_created' };
    if (command === 'export_encrypted_workspace_backup') return { outcome: 'encrypted_exported', format: 'rangoon.recovery.sqlcipher.v1', backupId: 'backup:' + '8'.repeat(64), byteLength: 12288 };
    if (command === 'prepare_encrypted_workspace_restore') return { outcome: 'cancelled' };
    throw new Error('unexpected native command');
  } });
  await lock.initialize();
  const controller = createWorkspaceController({ invoke: lock.bridge });
  await controller.load();
  controller.requestEncryptedExport(); await controller.createRecoveryKey(); await controller.confirmEncryptedExport(); await controller.prepareEncryptedRestore();
  assert.deepEqual(nativeCalls.map(([name]) => name), ['get_workspace_data','create_workspace_recovery_key','export_encrypted_workspace_backup','prepare_encrypted_workspace_restore']);
  assert.ok(nativeCalls.every(([,args]) => args === undefined));
  lock.dispose();
  for (const name of ['create_workspace_recovery_key','export_encrypted_workspace_backup','prepare_encrypted_workspace_restore']) await assert.rejects(lock.bridge(name), /closed/);
  assert.equal(nativeCalls.length,4);
});

function fixture(implementation, extra = {}) {
  const calls = [];
  const changes = [];
  const controller = createWorkspaceLockController({
    invoke: (command, args, options) => {
      calls.push([command, args, options]);
      return implementation(command, args, options);
    },
    onChange: state => changes.push(state),
    ...extra,
  });
  return { controller, calls, changes };
}

test('managed bootstrap observes one exact native request state then sends private header on ordinary Raw calls', async () => {
  const f = fixture((command, args, options) => {
    if (command === 'get_workspace_close_state') return closeState();
    if (command === 'get_workspace_request_state') return requestState();
    assert.equal(command, 'save_analysis');
    assert.ok(args instanceof Uint8Array);
    assert.deepEqual(options, { headers: { 'x-rangoon-workspace-client': clientId } });
    return { outcome: 'saved' };
  });
  await f.controller.initialize();
  await f.controller.initialize();
  const body = new Uint8Array([1, 2, 3]);
  assert.deepEqual(await f.controller.bridge('save_analysis', body), { outcome: 'saved' });
  assert.deepEqual(f.calls.map(([command]) => command), [
    'get_workspace_close_state',
    'get_workspace_request_state',
    'save_analysis',
  ]);
  assert.equal(JSON.stringify(f.controller.getState()).includes(clientId), false);
  assert.equal(JSON.stringify(f.changes).includes(clientId), false);
});

test('malformed request bootstrap cannot open bridge or leak a retained client identifier', async () => {
  let getterReads = 0;
  const getter = requestState();
  Object.defineProperty(getter, 'clientId', {
    enumerable: true,
    get() { ++getterReads; return clientId; },
  });
  const malformed = [
    undefined,
    null,
    [],
    {},
    requestState({ extra: true }),
    requestState({ clientId: clientId.toUpperCase() }),
    requestState({ clientId: 'aa' }),
    requestState({ state: 'unavailable' }),
    requestState({ storage: 'legacy_plaintext' }),
    getter,
  ];
  for (const result of malformed) {
    const f = fixture(command => command === 'get_workspace_close_state' ? closeState() : result);
    assert.equal((await f.controller.initialize()).status, 'recovery_required');
    await assert.rejects(f.controller.bridge('list_snapshots'), /closed/);
    assert.deepEqual(f.calls.map(([command]) => command), [
      'get_workspace_close_state',
      'get_workspace_request_state',
    ]);
  }
  assert.equal(getterReads, 0);
});

test('ordinary bridge refuses bootstrap and lifecycle calls and cannot refresh a client after disposal', async () => {
  const f = fixture(command => {
    if (command === 'get_workspace_close_state') return closeState();
    if (command === 'get_workspace_request_state') return requestState();
    return { outcome: 'ordinary' };
  });
  await f.controller.initialize();
  for (const command of [
    'workspace_startup_status',
    'workspace_startup_action',
    'get_workspace_close_state',
    'get_workspace_request_state',
    'begin_workspace_close',
    'acknowledge_workspace_purge',
  ]) {
    await assert.rejects(f.controller.bridge(command, {}), /closed/);
  }
  f.controller.dispose();
  await assert.rejects(f.controller.bridge('list_snapshots'), /closed/);
  assert.equal(f.calls.filter(([command]) => command === 'get_workspace_request_state').length, 1);
});

test('dispose or synchronous reentrancy fences an in-flight ordinary response before delivery', async () => {
  const pending = deferred();
  const f = fixture(command => {
    if (command === 'get_workspace_close_state') return closeState();
    if (command === 'get_workspace_request_state') return requestState();
    return pending.promise;
  });
  await f.controller.initialize();
  const old = f.controller.bridge('open_snapshot', { sourceId: 'private' });
  f.controller.dispose();
  pending.resolve({ source: 'private' });
  await assert.rejects(old, /closed/);

  let reentrant;
  const second = fixture((command, args, options) => {
    if (command === 'get_workspace_close_state') return closeState();
    if (command === 'get_workspace_request_state') return requestState();
    assert.deepEqual(options, { headers: { 'x-rangoon-workspace-client': clientId } });
    return { ordinary: true };
  }, {
    beforeInvoke: () => { reentrant.dispose(); },
  });
  reentrant = second.controller;
  await second.controller.initialize();
  await assert.rejects(second.controller.bridge('read_composition_source', { sourceId: 'private' }), /closed/);
  assert.equal(second.calls.some(([command]) => command === 'read_composition_source'), false);
});
