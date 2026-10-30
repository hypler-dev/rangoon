import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorkspaceLockController } from '../preview/workspace-lock-model.mjs';

const c0 = 'ab'.repeat(32);
const c1 = 'cd'.repeat(32);
const close = (state = 'locked', id = state === 'open' ? null : c0, epoch = state === 'open' ? null : '1') => ({ schemaVersion: 'rangoon.workspace-close-state.v1', storage: 'managed_encrypted', state, attemptId: id, epoch, diagnostic: null });
const legacy = () => ({ schemaVersion: 'rangoon.workspace-close-state.v1', storage: 'legacy_plaintext', state: 'unavailable', attemptId: null, epoch: null, diagnostic: 'encrypted_close_unavailable' });
const control = (state = 'locked', extra = {}) => ({ schemaVersion: 'rangoon.workspace-control-state.v1', close: close(state, state === 'closing' ? c1 : undefined, state === 'closing' ? '2' : undefined), phase: state, action: null, outcome: 'observed', diagnostic: null, reload: false, ...extra });
const protection = (layout = 'absent', view = close()) => ({ schemaVersion: 'rangoon.workspace-protection-state.v1', close: view, layout, diagnostic: layout === 'absent' || layout === 'locked' ? null : `workspace_protection_${layout}` });
const deferred = () => { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; };

function fixture(options = {}) {
  const calls = [], timers = new Map(); let serial = 0; let purges = 0; let reloads = 0;
  const invoke = async (command, args, config) => {
    calls.push([command, args, config]);
    if (options.invoke) return options.invoke(command, args, config);
    if (command === 'get_workspace_close_state') return options.initial ?? close('open');
    if (command === 'get_workspace_request_state') return { schemaVersion: 'rangoon.workspace-request-state.v1', storage: 'managed_encrypted', state: 'open', clientId: c0 };
    if (command === 'get_workspace_control_state') return options.control ?? control();
    if (command === 'get_workspace_protection_state') return options.protection ?? protection();
    if (command === 'prepare_workspace_control') return control('closing', { phase: 'awaiting_confirmation', action: 'create_empty', outcome: 'prepared' });
    if (command === 'acknowledge_workspace_purge') return close('closing', c1, '2');
    if (command === 'confirm_workspace_control') return control('open', { phase: 'open', outcome: 'opened', reload: true });
    return { data: 'private source' };
  };
  const model = createWorkspaceLockController({ invoke, beforeInvoke: options.beforeInvoke, purge: () => { ++purges; return options.purge?.(purges); }, reload: () => { ++reloads; }, schedule: callback => { timers.set(++serial, callback); return serial; }, cancelSchedule: id => timers.delete(id), pollInterval: 10 });
  return { model, calls, timers, purges: () => purges, reloads: () => reloads };
}

test('Browser and Legacy keep existing workflows without E1c control IPC', async () => {
  const browser = createWorkspaceLockController();
  assert.equal((await browser.initialize()).status, 'browser');
  const f = fixture({ initial: legacy() });
  assert.equal((await f.model.initialize()).status, 'legacy');
  assert.deepEqual(await f.model.bridge('list_snapshots'), { data: 'private source' });
  assert.deepEqual(f.calls.map(([name]) => name), ['get_workspace_close_state', 'list_snapshots']);
});

test('locked C0 purges freshly before protected absent layout enables only setup', async () => {
  const f = fixture({ initial: close(), control: control(), protection: protection('absent') });
  const state = await f.model.initialize();
  assert.deepEqual(state, { status: 'protection', blocked: true, canLock: false, layout: 'absent', canProtect: true, pending: false });
  assert.equal(f.purges(), 1);
  await assert.rejects(f.model.bridge('list_snapshots'), /closed/);
  assert.deepEqual(f.calls.map(([name]) => name), ['get_workspace_close_state', 'get_workspace_control_state', 'get_workspace_protection_state']);
});

test('validated busy, recovery, and unavailable protection layouts remain finite closed views', async () => {
  for (const layout of ['busy', 'recovery_required', 'unavailable']) {
    const f = fixture({ initial: close(), control: control(), protection: protection(layout) });
    await f.model.initialize();
    assert.deepEqual(f.model.getState(), { status: 'protection', blocked: true, canLock: false, layout, canProtect: false, pending: false });
    assert.equal(f.purges(), 1);
    assert.equal(await f.model.beginProtection(), false);
  }
});

test('prepare, fresh C1 purge, exact Raw acknowledgment and confirm reload only after exact opened', async () => {
  const f = fixture({ initial: close(), control: control(), protection: protection('absent') });
  await f.model.initialize();
  assert.equal(await f.model.beginProtection(), true);
  assert.equal(f.purges(), 2);
  const wire = f.calls.filter(([name]) => ['prepare_workspace_control', 'acknowledge_workspace_purge', 'confirm_workspace_control'].includes(name));
  assert.equal(wire.length, 3);
  assert.deepEqual(JSON.parse(new TextDecoder().decode(wire[0][1])), { schemaVersion: 'rangoon.workspace-control-prepare.v1', attemptId: c0, epoch: '1', action: 'create_empty' });
  assert.deepEqual(JSON.parse(new TextDecoder().decode(wire[1][1])), { schemaVersion: 'rangoon.workspace-purge-ack.v1', attemptId: c1, epoch: '2' });
  assert.deepEqual(JSON.parse(new TextDecoder().decode(wire[2][1])), { schemaVersion: 'rangoon.workspace-control-cycle.v1', attemptId: c1, epoch: '2' });
  assert.equal(f.reloads(), 1);
  assert.equal(f.model.getState().status, 'reloading');
});

test('locked layout sends exact Raw unlock request through C1 acknowledgment and reload', async () => {
  const f = fixture({ initial: close(), invoke: name => {
    if (name === 'get_workspace_close_state') return close();
    if (name === 'get_workspace_control_state') return control();
    if (name === 'get_workspace_protection_state') return protection('locked');
    if (name === 'prepare_workspace_control') return control('closing', { phase: 'awaiting_confirmation', action: 'unlock', outcome: 'prepared' });
    if (name === 'acknowledge_workspace_purge') return close('closing', c1, '2');
    if (name === 'confirm_workspace_control') return control('open', { phase: 'open', outcome: 'opened', reload: true });
    throw new Error(`unexpected ${name}`);
  } });
  assert.equal((await f.model.initialize()).layout, 'locked');
  assert.equal(await f.model.beginProtection(), true);
  const wire = f.calls.filter(([name]) => ['prepare_workspace_control', 'acknowledge_workspace_purge', 'confirm_workspace_control'].includes(name));
  assert.deepEqual(wire.map(([name]) => name), ['prepare_workspace_control', 'acknowledge_workspace_purge', 'confirm_workspace_control']);
  assert.deepEqual(JSON.parse(new TextDecoder().decode(wire[0][1])), { schemaVersion: 'rangoon.workspace-control-prepare.v1', attemptId: c0, epoch: '1', action: 'unlock' });
  assert.equal(f.purges(), 2);
  assert.equal(f.reloads(), 1);
});

test('fresh C1 purge failure blocks acknowledgment, native sheet confirmation, and reload', async () => {
  const f = fixture({ initial: close(), control: control(), protection: protection('locked'), purge: count => count !== 2 });
  await f.model.initialize();
  assert.equal(await f.model.beginProtection(), false);
  assert.equal(f.purges(), 2);
  assert.equal(f.calls.some(([name]) => name === 'acknowledge_workspace_purge' || name === 'confirm_workspace_control'), false);
  assert.equal(f.reloads(), 0);
  assert.equal(f.model.getState().status, 'recovery_required');
});

test('stale C0, refusal, malformed response, and delivery loss remain closed and private', async () => {
  for (const response of [control('locked', { close: close('locked', 'ef'.repeat(32)), }), control('closing', { phase: 'awaiting_confirmation', action: 'create_empty', outcome: 'refused', diagnostic: 'workspace_control_unavailable' }), null]) {
    const f = fixture({ initial: close(), control: control(), protection: protection(), invoke: (name, args) => {
      if (name === 'get_workspace_close_state') return close();
      if (name === 'get_workspace_control_state') return control();
      if (name === 'get_workspace_protection_state') return protection();
      if (name === 'prepare_workspace_control') return response;
      return close('closing', c1, '2');
    } });
    await f.model.initialize();
    assert.equal(await f.model.beginProtection(), false);
    assert.equal(f.model.getState().blocked, true);
    assert.equal(f.reloads(), 0);
    assert.equal(JSON.stringify(f.model.getState()).includes(c0), false);
    assert.equal(JSON.stringify(f.model.getState()).includes(c1), false);
  }
});

test('managed open observer purges and acknowledges exact native close before polling', async () => {
  const f = fixture({ initial: close('open'), invoke: name => {
    if (name === 'get_workspace_close_state') return close('open');
    if (name === 'get_workspace_request_state') return { schemaVersion: 'rangoon.workspace-request-state.v1', storage: 'managed_encrypted', state: 'open', clientId: c0 };
    if (name === 'get_workspace_control_state') return control('closing', { phase: 'closing' });
    if (name === 'acknowledge_workspace_purge') return close('closing', c1, '2');
    throw new Error(`unexpected ${name}`);
  } });
  await f.model.initialize();
  const callback = f.timers.values().next().value; await callback();
  assert.equal(f.model.getState().status, 'closing');
  assert.equal(f.purges(), 1);
  const acknowledgment = f.calls.find(([name]) => name === 'acknowledge_workspace_purge');
  assert.deepEqual(JSON.parse(new TextDecoder().decode(acknowledgment[1])), { schemaVersion: 'rangoon.workspace-purge-ack.v1', attemptId: c1, epoch: '2' });
  await assert.rejects(f.model.bridge('list_snapshots'), /closed/);
});

test('managed open observer closes locally on malformed observation', async () => {
  for (const observed of [null, control('closing', { phase: 'open' })]) {
    const f = fixture({ initial: close('open'), invoke: name => {
      if (name === 'get_workspace_close_state') return close('open');
      if (name === 'get_workspace_request_state') return { schemaVersion: 'rangoon.workspace-request-state.v1', storage: 'managed_encrypted', state: 'open', clientId: c0 };
      if (name === 'get_workspace_control_state') return observed;
      return { source: 'private' };
    } });
    await f.model.initialize();
    const callback = f.timers.values().next().value; await callback();
    assert.equal(f.model.getState().status, 'recovery_required');
    assert.equal(f.purges(), 1);
    assert.equal(f.calls.some(([name]) => name === 'acknowledge_workspace_purge'), false);
    await assert.rejects(f.model.bridge('list_snapshots'), /closed/);
  }
});

test('prepared C1 must have fresh attempt and epoch before native acknowledgment', async () => {
  for (const stale of [close('closing', c0, '2'), close('closing', c1, '1')]) {
    const f = fixture({ initial: close(), invoke: name => {
      if (name === 'get_workspace_close_state') return close();
      if (name === 'get_workspace_control_state') return control();
      if (name === 'get_workspace_protection_state') return protection();
      if (name === 'prepare_workspace_control') return control('closing', { close: stale, phase: 'awaiting_confirmation', action: 'create_empty', outcome: 'prepared' });
      throw new Error(`unexpected ${name}`);
    } });
    await f.model.initialize();
    assert.equal(await f.model.beginProtection(), false);
    assert.equal(f.calls.some(([name]) => name === 'acknowledge_workspace_purge' || name === 'confirm_workspace_control'), false);
    assert.equal(f.model.getState().status, 'recovery_required');
  }
});

test('late source result is fenced when explicit Lock closes admission', async () => {
  const pending = deferred();
  const f = fixture({ invoke: name => {
    if (name === 'get_workspace_close_state') return close('open');
    if (name === 'get_workspace_request_state') return { schemaVersion: 'rangoon.workspace-request-state.v1', storage: 'managed_encrypted', state: 'open', clientId: c0 };
    if (name === 'begin_workspace_close') return close('closing');
    if (name === 'acknowledge_workspace_purge') return close('locked');
    if (name === 'list_snapshots') return pending.promise;
    return control('open');
  } });
  await f.model.initialize();
  const source = f.model.bridge('list_snapshots');
  assert.equal(f.model.openConfirmation(), true);
  await f.model.confirmClose();
  pending.resolve({ source: 'private' });
  await assert.rejects(source, /closed/);
});

test('fresh closing window purges, acknowledges its exact close cycle, then refreshes protected layout', async () => {
  let reads = 0;
  const f = fixture({ invoke: name => {
    if (name === 'get_workspace_close_state') return ++reads === 1 ? close('closing') : close('locked');
    if (name === 'acknowledge_workspace_purge') return close('closing');
    if (name === 'get_workspace_control_state') return control();
    if (name === 'get_workspace_protection_state') return protection('locked');
    throw new Error(`unexpected ${name}`);
  } });
  await f.model.initialize();
  assert.equal(f.model.getState().status, 'closing');
  assert.equal(f.purges(), 1);
  const callback = f.timers.values().next().value;
  await callback();
  assert.deepEqual(f.model.getState(), { status: 'protection', blocked: true, canLock: false, layout: 'locked', canProtect: true, pending: false });
  assert.equal(f.purges(), 2);
});

test('ordinary bridge rejects lifecycle commands and reentrant close fences original work', async () => {
  const f = fixture();
  await f.model.initialize();
  for (const command of ['get_workspace_control_state', 'prepare_workspace_control', 'confirm_workspace_control', 'get_workspace_protection_state']) await assert.rejects(f.model.bridge(command, {}), /closed/);
  let closing;
  const g = fixture({ beforeInvoke: () => { closing = g.model.confirmClose(); } });
  await g.model.initialize();
  assert.equal(g.model.openConfirmation(), true);
  await assert.rejects(g.model.bridge('save_analysis', { sourceId: 'private' }), /closed/);
  await closing;
});

test('malformed bootstrap, dispose, and stale observer callbacks stay closed without native setup', async () => {
  const pending = deferred();
  const f = fixture({ invoke: () => pending.promise });
  const starting = f.model.initialize();
  f.model.dispose();
  pending.resolve(close('open'));
  await starting;
  assert.equal(f.model.getState().status, 'recovery_required');
  await assert.rejects(f.model.bridge('list_snapshots'), /closed/);
  const open = fixture();
  await open.model.initialize();
  const callback = open.timers.values().next().value;
  open.model.dispose();
  const calls = open.calls.length;
  await callback();
  assert.equal(open.calls.length, calls);
});

test('native setup and unlock cancellation returns false only after exact C1 reconciliation, then permits retry', async () => {
  for (const layout of ['absent', 'locked']) {
    const cycles = [
      { id: c1, epoch: '2' },
      { id: 'ef'.repeat(32), epoch: '3' },
    ];
    let prepared = 0;
    let active = null;
    const f = fixture({ initial: close(), invoke: name => {
      if (name === 'get_workspace_close_state') return close();
      if (name === 'get_workspace_control_state') {
        if (!active) return control();
        return control('locked', { close: close('locked', active.id, active.epoch) });
      }
      if (name === 'get_workspace_protection_state') return protection(layout, active ? close('locked', active.id, active.epoch) : close());
      if (name === 'prepare_workspace_control') {
        active = cycles[prepared++];
        return control('closing', {
          close: close('closing', active.id, active.epoch),
          phase: 'awaiting_confirmation',
          action: layout === 'absent' ? 'create_empty' : 'unlock',
          outcome: 'prepared',
        });
      }
      if (name === 'acknowledge_workspace_purge') return close('closing', active.id, active.epoch);
      if (name === 'confirm_workspace_control') return control('locked', {
        close: close('locked', active.id, active.epoch), phase: 'locked', outcome: 'cancelled',
      });
      throw new Error(`unexpected ${name}`);
    } });
    await f.model.initialize();

    assert.equal(await f.model.beginProtection(), false);
    assert.deepEqual(f.model.getState(), { status: 'protection', blocked: true, canLock: false, layout, canProtect: true, pending: false });
    assert.equal(await f.model.beginProtection(), false);
    assert.equal(prepared, 2);
    assert.equal(f.reloads(), 0);
    assert.equal(f.calls.some(([name]) => name === 'get_workspace_request_state'), false);
    await assert.rejects(f.model.bridge('save_analysis', { sourceId: 'private' }), /closed/);

    const confirms = f.calls.filter(([name]) => name === 'confirm_workspace_control');
    assert.equal(confirms.length, 2);
    for (const [index, [, payload]] of confirms.entries()) {
      assert.deepEqual(JSON.parse(new TextDecoder().decode(payload)), {
        schemaVersion: 'rangoon.workspace-control-cycle.v1', attemptId: cycles[index].id, epoch: cycles[index].epoch,
      });
    }
    const lastConfirm = f.calls.map(([name]) => name).lastIndexOf('confirm_workspace_control');
    assert.deepEqual(f.calls.slice(lastConfirm + 1).map(([name]) => name), ['get_workspace_control_state', 'get_workspace_protection_state']);
    assert.equal(f.purges(), 5);
  }
});

test('cancelled control must be exact C1 locked and non-reloading; malformed or lost delivery stays closed', async () => {
  const malformed = control('locked', { close: close('locked', c0, '1'), phase: 'locked', outcome: 'cancelled' });
  const crossCycle = control('locked', { close: close('locked', 'ef'.repeat(32), '3'), phase: 'locked', outcome: 'cancelled' });
  const wrongStorage = control('locked', { close: legacy(), phase: 'locked', outcome: 'cancelled' });
  const opened = control('open', { close: close('open'), phase: 'open', outcome: 'cancelled', reload: true });
  const reloadShaped = control('locked', { close: close('locked', c1, '2'), phase: 'locked', outcome: 'cancelled', reload: true });
  for (const cancelled of [null, malformed, crossCycle, wrongStorage, opened, reloadShaped, new Error('delivery lost')]) {
    const f = fixture({ initial: close(), control: control(), protection: protection('absent'), invoke: name => {
      if (name === 'get_workspace_close_state') return close();
      if (name === 'get_workspace_control_state') return control();
      if (name === 'get_workspace_protection_state') return protection('absent');
      if (name === 'prepare_workspace_control') return control('closing', { phase: 'awaiting_confirmation', action: 'create_empty', outcome: 'prepared' });
      if (name === 'acknowledge_workspace_purge') return close('closing', c1, '2');
      if (name === 'confirm_workspace_control') {
        if (cancelled instanceof Error) throw cancelled;
        return cancelled;
      }
      throw new Error(`unexpected ${name}`);
    } });
    await f.model.initialize();
    assert.equal(await f.model.beginProtection(), false);
    assert.equal(f.model.getState().status, 'recovery_required');
    assert.equal(f.model.getState().blocked, true);
    assert.equal(f.model.getState().canProtect, false);
    assert.equal(f.reloads(), 0);
    assert.equal(f.calls.some(([name]) => name === 'get_workspace_request_state'), false);
  }
});

test('cancelled C1 requires fresh authoritative control and protection before setup becomes retryable', async () => {
  const cancelled = control('locked', { close: close('locked', c1, '2'), phase: 'locked', outcome: 'cancelled' });
  const observedC1 = control('locked', { close: close('locked', c1, '2') });
  const run = async ({ refreshedControl = observedC1, refreshedProtection = protection('absent', close('locked', c1, '2')) } = {}) => {
    let controls = 0;
    let protections = 0;
    const f = fixture({ initial: close(), invoke: name => {
      if (name === 'get_workspace_close_state') return close();
      if (name === 'get_workspace_control_state') {
        if (++controls < 3) return control();
        if (refreshedControl instanceof Error) throw refreshedControl;
        return refreshedControl;
      }
      if (name === 'get_workspace_protection_state') {
        if (++protections === 1) return protection('absent');
        if (refreshedProtection instanceof Error) throw refreshedProtection;
        return refreshedProtection;
      }
      if (name === 'prepare_workspace_control') return control('closing', { phase: 'awaiting_confirmation', action: 'create_empty', outcome: 'prepared' });
      if (name === 'acknowledge_workspace_purge') return close('closing', c1, '2');
      if (name === 'confirm_workspace_control') return cancelled;
      throw new Error(`unexpected ${name}`);
    } });
    await f.model.initialize();
    assert.equal(await f.model.beginProtection(), false);
    assert.equal(f.reloads(), 0);
    assert.equal(f.calls.some(([name]) => name === 'get_workspace_request_state'), false);
    return f;
  };

  for (const refreshedControl of [
    control('locked', { close: close('locked', 'ef'.repeat(32), '3') }),
    control('open'),
    new Error('control delivery lost'),
  ]) {
    const f = await run({ refreshedControl });
    assert.equal(f.model.getState().status, 'recovery_required');
    assert.equal(f.model.getState().canProtect, false);
    assert.equal(f.calls.filter(([name]) => name === 'get_workspace_protection_state').length, 1);
  }

  for (const refreshedProtection of [
    protection('absent', close('locked', 'ef'.repeat(32), '3')),
    new Error('protection delivery lost'),
    protection('locked', close('locked', c1, '2')),
  ]) {
    const f = await run({ refreshedProtection });
    assert.equal(f.model.getState().status, 'recovery_required');
    assert.equal(f.model.getState().canProtect, false);
  }

  for (const layout of ['busy', 'recovery_required', 'unavailable']) {
    const f = await run({ refreshedProtection: protection(layout, close('locked', c1, '2')) });
    assert.deepEqual(f.model.getState(), { status: 'protection', blocked: true, canLock: false, layout, canProtect: false, pending: false });
  }
});

test('purge failure or disposal while cancellation is in flight cannot reopen or reload', async () => {
  const purgeFailed = fixture({ initial: close(), control: control(), protection: protection('absent'), purge: count => count !== 2 });
  await purgeFailed.model.initialize();
  assert.equal(await purgeFailed.model.beginProtection(), false);
  assert.equal(purgeFailed.calls.some(([name]) => name === 'confirm_workspace_control'), false);
  assert.equal(purgeFailed.reloads(), 0);
  assert.equal(purgeFailed.model.getState().status, 'recovery_required');

  const late = deferred();
  const disposed = fixture({ initial: close(), control: control(), protection: protection('absent'), invoke: name => {
    if (name === 'get_workspace_close_state') return close();
    if (name === 'get_workspace_control_state') return control();
    if (name === 'get_workspace_protection_state') return protection('absent');
    if (name === 'prepare_workspace_control') return control('closing', { phase: 'awaiting_confirmation', action: 'create_empty', outcome: 'prepared' });
    if (name === 'acknowledge_workspace_purge') return close('closing', c1, '2');
    if (name === 'confirm_workspace_control') return late.promise;
    throw new Error(`unexpected ${name}`);
  } });
  await disposed.model.initialize();
  const opening = disposed.model.beginProtection();
  disposed.model.dispose();
  late.resolve(control('locked', { close: close('locked', c1, '2'), phase: 'locked', outcome: 'cancelled' }));
  assert.equal(await opening, false);
  assert.equal(disposed.model.getState().status, 'recovery_required');
  assert.equal(disposed.reloads(), 0);
  assert.equal(disposed.calls.some(([name]) => name === 'get_workspace_request_state'), false);
});

test('folder metadata command is workspace-bound and closed workspaces never invoke it',async()=>{
 const open=fixture();await open.model.initialize();await open.model.bridge('select_and_list_sources');
 const call=open.calls.find(([name])=>name==='select_and_list_sources');assert.deepEqual(call[2],{headers:{'x-rangoon-workspace-client':c0}});
 const locked=fixture({initial:close(),control:control(),protection:protection('locked')});await locked.model.initialize();
 await assert.rejects(locked.model.bridge('select_and_list_sources'),/closed/);assert.equal(locked.calls.some(([name])=>name==='select_and_list_sources'),false);
});
