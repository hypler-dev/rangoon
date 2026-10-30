import test from 'node:test';
import assert from 'node:assert/strict';
import { createDesktopStartupSession } from '../preview/desktop-startup-session.mjs';

const a = 'a'.repeat(64);
const b = 'b'.repeat(64);
const c = 'c'.repeat(64);
const deferred = () => { let resolve; let reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };
const close = (state = 'locked', attemptId = a, epoch = '1') => ({
  schemaVersion: 'rangoon.workspace-close-state.v1', storage: 'managed_encrypted', state,
  attemptId: state === 'open' ? null : attemptId, epoch: state === 'open' ? null : epoch, diagnostic: null,
});
const handoff = () => ({
  schemaVersion: 'rangoon.workspace-startup.v1', state: 'handoff', startupId: null, attemptId: null, epoch: null,
  actions: [], copyPhase: null, retainedOriginal: 'not_applicable', diagnostic: null,
});
const startup = (state = 'fresh', changes = {}) => ({
  schemaVersion: 'rangoon.workspace-startup.v1', state, startupId: a, attemptId: b, epoch: '1',
  actions: { fresh: ['setup', 'quit'], conversion_review: ['copy', 'cancel', 'quit'] }[state] ?? ['quit'],
  copyPhase: null, retainedOriginal: state === 'conversion_review' ? 'present_unencrypted' : 'not_applicable', diagnostic: null, ...changes,
});

function scheduler() {
  const pending = new Map(); let serial = 0;
  return {
    schedule(callback) { pending.set(++serial, callback); return serial; },
    cancel(id) { pending.delete(id); },
    size: () => pending.size,
    async run() { const item = pending.entries().next().value; if (!item) return false; pending.delete(item[0]); await item[1](); return true; },
  };
}
const flush = async () => { await Promise.resolve(); await Promise.resolve(); await Promise.resolve(); };
const eventually = async predicate => {
  for (let count = 0; count < 40; ++count) {
    if (predicate()) return;
    await Promise.resolve();
  }
  assert.fail('condition did not settle');
};

test('C0 purges before first startup status', async () => {
  const calls = []; let purges = 0;
  const session = createDesktopStartupSession({
    invoke: async (command, payload) => {
      calls.push([command, payload]);
      if (command === 'get_workspace_close_state') return close();
      assert.equal(command, 'workspace_startup_status');
      assert.equal(purges, 1);
      return startup();
    },
    purge: () => { ++purges; },
    schedule: () => 1,
    cancelSchedule: () => {},
  });
  assert.equal((await session.initialize()).status, 'ready');
  assert.deepEqual(calls.map(([name]) => name), ['get_workspace_close_state', 'workspace_startup_status']);
  session.dispose();
});

test('C1 acknowledgment runs while startup status is pending and does not deadlock', async () => {
  const status = deferred(); const clock = scheduler(); const calls = []; let purges = 0;
  const session = createDesktopStartupSession({
    invoke: async (command, payload) => {
      calls.push([command, payload]);
      if (command === 'get_workspace_close_state') return calls.filter(([name]) => name === command).length === 1 ? close() : close('closing', c, '2');
      if (command === 'workspace_startup_status') return status.promise;
      if (command === 'acknowledge_workspace_purge') return close('closing', c, '2');
      throw Error('unexpected');
    },
    purge: () => { ++purges; }, schedule: clock.schedule, cancelSchedule: clock.cancel,
  });
  const started = session.initialize();
  await eventually(() => clock.size() === 1 && calls.some(([name]) => name === 'workspace_startup_status'));
  assert.equal(await clock.run(), true);
  assert.equal(calls.some(([name]) => name === 'acknowledge_workspace_purge'), true);
  assert.equal(purges, 2);
  status.resolve(startup());
  await started;
  session.dispose();
});

test('one close cycle gets one acknowledgment and a new cycle gets a fresh purge', async () => {
  const clock = scheduler(); const calls = []; let reads = 0; let purges = 0;
  const observations = [close(), close('closing', c, '2'), close('closing', c, '2'), close('closing', b, '3')];
  const session = createDesktopStartupSession({
    invoke: async command => {
      calls.push([command]);
      if (command === 'get_workspace_close_state') return observations[reads++] ?? close('open');
      if (command === 'workspace_startup_status') return startup();
      if (command === 'acknowledge_workspace_purge') return close('closing', calls.filter(([name]) => name === 'acknowledge_workspace_purge').length === 1 ? c : b, calls.filter(([name]) => name === 'acknowledge_workspace_purge').length === 1 ? '2' : '3');
      throw Error('unexpected');
    },
    purge: () => { ++purges; }, schedule: clock.schedule, cancelSchedule: clock.cancel,
  });
  await session.initialize();
  await clock.run();
  await clock.run();
  await clock.run();
  assert.equal(calls.filter(([name]) => name === 'acknowledge_workspace_purge').length, 2);
  assert.equal(purges, 3);
  session.dispose();
});

test('a stale close observation after a newer cycle stays unavailable without another acknowledgment', async () => {
  const clock = scheduler(); let reads = 0; const calls = [];
  const session = createDesktopStartupSession({
    invoke: async command => {
      calls.push(command);
      if (command === 'get_workspace_close_state') return [close(), close('closing', c, '2'), close('closing', a, '1')][reads++];
      if (command === 'workspace_startup_status') return startup();
      if (command === 'acknowledge_workspace_purge') return close('closing', c, '2');
      throw Error('unexpected');
    },
    purge: () => true, schedule: clock.schedule, cancelSchedule: clock.cancel,
  });
  await session.initialize();
  await clock.run();
  await clock.run();
  assert.equal(session.getState().status, 'unavailable');
  assert.equal(calls.filter(name => name === 'acknowledge_workspace_purge').length, 1);
});

test('malformed, mismatched, rejected lifecycle responses and failed purge stay unavailable', async () => {
  for (const mode of ['malformed', 'mismatch', 'reject', 'purge']) {
    const clock = scheduler(); const calls = [];
    const session = createDesktopStartupSession({
      invoke: async command => {
        calls.push(command);
        if (command === 'get_workspace_close_state') return calls.filter(name => name === command).length === 1 ? close() : close('closing', c, '2');
        if (command === 'workspace_startup_status') return startup();
        if (command === 'acknowledge_workspace_purge') {
          if (mode === 'malformed') return null;
          if (mode === 'mismatch') return close('closing', b, '2');
          if (mode === 'reject') throw Error('lost');
        }
        throw Error('unexpected');
      },
      purge: () => mode !== 'purge', schedule: clock.schedule, cancelSchedule: clock.cancel,
    });
    await session.initialize();
    if (mode !== 'purge') await clock.run();
    assert.equal(session.getState().status, 'unavailable');
    assert.equal(JSON.stringify(session.getState()).includes(c), false);
    session.dispose();
  }
});

test('disposal during async purge prevents acknowledgment and later state publication', async () => {
  const clock = scheduler(); const pendingPurge = deferred(); const calls = [];
  const session = createDesktopStartupSession({
    invoke: async command => {
      calls.push(command);
      if (command === 'get_workspace_close_state') return calls.filter(name => name === command).length === 1 ? close() : close('closing', c, '2');
      if (command === 'workspace_startup_status') return startup();
      throw Error('ack must not run');
    },
    purge: () => calls.filter(name => name === 'get_workspace_close_state').length === 1 ? true : pendingPurge.promise,
    schedule: clock.schedule, cancelSchedule: clock.cancel,
  });
  await session.initialize();
  const polling = clock.run();
  await flush();
  session.dispose();
  pendingPurge.resolve(true);
  await polling;
  assert.equal(calls.includes('acknowledge_workspace_purge'), false);
});

test('initial Open accepts only exact terminal Handoff; browser makes zero IPC', async () => {
  let handoffs = 0;
  const accepted = createDesktopStartupSession({
    invoke: async command => command === 'get_workspace_close_state' ? close('open') : handoff(),
    onHandoff: () => { ++handoffs; }, schedule: () => 1, cancelSchedule: () => {},
  });
  await accepted.initialize();
  await flush();
  assert.equal(accepted.getState().status, 'handoff');
  assert.equal(handoffs, 1);
  accepted.dispose();
  const refused = createDesktopStartupSession({
    invoke: async command => command === 'get_workspace_close_state' ? close('open') : startup(),
    schedule: () => 1, cancelSchedule: () => {},
  });
  await refused.initialize();
  assert.equal(refused.getState().status, 'unavailable');
  let calls = 0; let browserHandoffs = 0;
  const browser = createDesktopStartupSession({ onHandoff: () => { ++browserHandoffs; } });
  const browserState = await browser.initialize();
  assert.equal(browserState.status, 'browser');
  assert.equal(calls, 0);
  assert.equal(browserHandoffs, 0);
});

test('one startup action may be pending; stale action and status results cannot reopen', async () => {
  const clock = scheduler(); const action = deferred(); const calls = [];
  const session = createDesktopStartupSession({
    invoke: command => {
      calls.push(command);
      if (command === 'get_workspace_close_state') return Promise.resolve(close());
      if (command === 'workspace_startup_status') return Promise.resolve(startup());
      if (command === 'workspace_startup_action') return action.promise;
      throw Error('unexpected');
    },
    purge: () => true, schedule: clock.schedule, cancelSchedule: clock.cancel,
  });
  await session.initialize();
  const first = session.perform('setup');
  assert.equal(await session.perform('setup'), false);
  session.dispose();
  action.resolve(startup('conversion_review'));
  assert.equal(await first, false);
  assert.equal(session.getState().status, 'unavailable');
  assert.equal(calls.filter(name => name === 'workspace_startup_action').length, 1);
  const status = deferred();
  const stale = createDesktopStartupSession({ invoke: command => command === 'get_workspace_close_state' ? Promise.resolve(close()) : status.promise, purge: () => true, schedule: clock.schedule, cancelSchedule: clock.cancel });
  const starting = stale.initialize();
  await flush();
  stale.dispose();
  status.resolve(startup());
  await starting;
  assert.equal(stale.getState().status, 'unavailable');
});

test('startup status failure fences the lifecycle pump before a later close can acknowledge', async () => {
  const clock = scheduler(); const calls = [];
  const session = createDesktopStartupSession({
    invoke: async command => {
      calls.push(command);
      if (command === 'get_workspace_close_state') return calls.filter(name => name === command).length === 1 ? close() : close('closing', c, '2');
      if (command === 'workspace_startup_status') throw Error('native lost');
      throw Error('ack must not run');
    },
    purge: () => true, schedule: clock.schedule, cancelSchedule: clock.cancel,
  });
  await session.initialize();
  assert.equal(session.getState().status, 'unavailable');
  assert.equal(clock.size(), 0);
  assert.equal(calls.includes('acknowledge_workspace_purge'), false);
});

test('handoff fences the pump and waits for an in-flight lifecycle read before callback', async () => {
  const clock = scheduler(); const status = deferred(); const observed = deferred(); const events = []; let reads = 0;
  const session = createDesktopStartupSession({
    invoke: command => {
      if (command === 'get_workspace_close_state') return ++reads === 1 ? Promise.resolve(close()) : observed.promise;
      if (command === 'workspace_startup_status') return status.promise;
      if (command === 'acknowledge_workspace_purge') throw Error('late ack');
      throw Error('unexpected');
    },
    purge: () => { events.push('purge'); return true; },
    onHandoff: () => { events.push('handoff'); },
    schedule: clock.schedule, cancelSchedule: clock.cancel,
  });
  const starting = session.initialize();
  await eventually(() => clock.size() === 1 && reads === 1);
  const pumping = clock.run();
  await eventually(() => reads === 2);
  status.resolve(handoff());
  await flush();
  assert.deepEqual(events, ['purge']);
  observed.resolve(close('closing', c, '2'));
  await pumping;
  await starting;
  await flush();
  assert.deepEqual(events, ['purge', 'handoff']);
  session.dispose();
});
