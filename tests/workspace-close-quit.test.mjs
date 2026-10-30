import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorkspaceCloseQuit, renderWorkspaceCloseShell } from '../preview/workspace-lock-view.mjs';

const fresh = () => ({ status: 'protection', layout: 'absent', canProtect: true, pending: false });
const deferred = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };

function fixture(overrides = {}) {
  let state = fresh();
  let fenced = false;
  const calls = [], renders = [];
  const request = deferred();
  const quit = createWorkspaceCloseQuit({
    invoke: (...args) => { calls.push(args); return request.promise; },
    getState: () => state,
    isFenced: () => fenced,
    onChange: () => renders.push(renderWorkspaceCloseShell(state, { quitAvailable: quit.available, quitPending: quit.pending, quitFailed: quit.failed })),
    ...overrides,
  });
  return { quit, calls, renders, request, setState: value => { state = value; }, fence: () => { fenced = true; } };
}

test('closed Quit dispatches only existing empty-body command and stays pending until native exit', async () => {
  const f = fixture();
  const done = f.quit.request();
  assert.deepEqual(f.calls, [['workspace_startup_quit', {}]]);
  assert.equal(f.quit.pending, true);
  assert.match(f.renders[0], /data-workspace-protection-action disabled/);
  assert.match(f.renders[0], /data-workspace-close-quit disabled/);
  assert.match(f.renders[0], /Quitting Rangoon/);
  assert.equal(await f.quit.request(), false);
  assert.equal(f.calls.length, 1);
  f.request.resolve();
  assert.equal(await done, true);
  assert.equal(f.quit.pending, true);
  assert.equal(f.renders.length, 1);
});

test('rejected Quit redraws current closed state, never captured old state or raw errors', async () => {
  const f = fixture();
  const done = f.quit.request();
  f.setState({ status: 'recovery_required' });
  f.request.reject(new Error('private native diagnostic'));
  assert.equal(await done, false);
  assert.equal(f.quit.pending, false);
  assert.equal(f.renders.length, 2);
  assert.match(f.renders[1], /Workspace recovery required/);
  assert.doesNotMatch(f.renders[1], /Set up encrypted|private native diagnostic/);
  assert.match(f.renders[1], /Rangoon could not quit/);
});

for (const change of ['fence', 'dispose', 'open', 'busy']) {
  test(`late Quit rejection cannot revive ${change} document or transition`, async () => {
    const f = fixture();
    const done = f.quit.request();
    if (change === 'fence') f.fence();
    if (change === 'dispose') f.quit.dispose();
    if (change === 'open') f.setState({ status: 'open', canLock: true });
    if (change === 'busy') f.setState({ status: 'protection', layout: 'busy' });
    f.request.reject(new Error('unavailable'));
    assert.equal(await done, false);
    assert.equal(f.renders.length, 1);
    assert.equal(await f.quit.request(), false);
    assert.equal(f.calls.length, 1);
  });
}

test('presentation changes during pending redraw are checked again before dispatch', async () => {
  let fenced = false;
  const calls = [];
  const quit = createWorkspaceCloseQuit({ invoke: (...args) => calls.push(args), getState: fresh, isFenced: () => fenced, onChange: () => { fenced = true; } });
  assert.equal(await quit.request(), false);
  assert.deepEqual(calls, []);
});

test('unqualified states, missing native bridge and disposed views cannot request Quit', async () => {
  const f = fixture();
  for (const state of [{}, { status: 'open' }, { status: 'browser' }, { status: 'locked' }, { status: 'closing' }, { ...fresh(), pending: true }, { ...fresh(), canProtect: false }, { status: 'protection', layout: 'locked', canProtect: true }]) {
    f.setState(state);
    assert.equal(await f.quit.request(), false);
    assert.doesNotMatch(renderWorkspaceCloseShell(state, { quitAvailable: true }), /data-workspace-close-quit/);
  }
  assert.deepEqual(f.calls, []);
  assert.equal(await createWorkspaceCloseQuit({ getState: fresh }).request(), false);
  f.quit.dispose(); f.setState(fresh());
  assert.equal(await f.quit.request(), false);
});

test('synchronous native invocation failure restores safe controls without leaking exception', async () => {
  const f = fixture({ invoke: () => { throw new Error('private path'); } });
  assert.equal(await f.quit.request(), false);
  assert.equal(f.renders.length, 2);
  assert.equal(f.quit.pending, false);
  assert.match(f.renders[1], /data-workspace-close-quit>/);
  assert.doesNotMatch(f.renders[1], /private path/);
});
