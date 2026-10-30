import test from 'node:test';
import assert from 'node:assert/strict';
import { bindWorkspaceView } from '../preview/workspace-view.mjs';
import { bindModelAssistanceView } from '../preview/model-assistance-view.mjs';

function rootFixture() {
  const listeners = new Map();
  const document = {
    activeElement: null,
    body: null,
    createElement() { return { className: '', append() {} }; },
  };
  const root = {
    ownerDocument: document,
    html: '',
    contains: () => false,
    set innerHTML(value) { this.html = value; },
    get innerHTML() { return this.html; },
    querySelector() { return null; },
    querySelectorAll() { return []; },
    addEventListener(type, listener) { listeners.set(type, listener); },
    removeEventListener(type, listener) { if (listeners.get(type) === listener) listeners.delete(type); },
  };
  return { root, listeners };
}

test('same-view refresh preserves an open record disclosure and summary focus without reopening after purge', t => {
  const priorCSS = globalThis.CSS;
  globalThis.CSS = { escape: value => value };
  t.after(() => { if (priorCSS === undefined) delete globalThis.CSS; else globalThis.CSS = priorCSS; });
  const fixture = rootFixture();
  const document = fixture.root.ownerDocument;
  const id = 'workspace-source-test';
  let disclosure;
  const replaceNodes = () => {
    disclosure = { id, open: false, setAttribute(name) { if (name === 'open') this.open = true; } };
    disclosure.summary = { matches: selector => selector.includes('summary'), closest: () => disclosure, focus() { document.activeElement = this; } };
  };
  replaceNodes();
  Object.defineProperty(fixture.root, 'innerHTML', { configurable: true, get() { return this.html; }, set(value) { this.html = value; replaceNodes(); } });
  fixture.root.contains = element => element === disclosure.summary;
  fixture.root.querySelectorAll = selector => selector === 'details.workbench-technical[open][id]' && disclosure.open ? [disclosure] : [];
  fixture.root.querySelector = selector => selector === `#${id}` ? disclosure : selector === `#${id} > summary` ? disclosure.summary : null;
  const state = { status: 'ready', pending: null, message: 'Ready.', workspace: { records: {}, sources: [], capabilities: [], workflows: [], databaseBytes: 0, reusableBytes: 0 } };
  const binding = bindWorkspaceView(fixture.root, { getState: () => state }, { getStorageMode: () => 'managed_encrypted' });
  disclosure.open = true;
  disclosure.summary.focus();
  const oldSummary = disclosure.summary;
  binding.render();
  assert.notEqual(disclosure.summary, oldSummary);
  assert.equal(disclosure.open, true);
  assert.equal(document.activeElement, disclosure.summary);
  binding.purgeForWorkspaceLock();
  disclosure.open = false;
  binding.render();
  assert.equal(disclosure.open, false);
  assert.equal(fixture.listeners.size, 0);
});

test('workspace and model bindings stop late redraws and remove listeners on workspace lock purge', () => {
  const workspace = rootFixture();
  const workspaceState = { status: 'ready', pending: null, message: 'Secret workspace state', workspace: { records: {}, sources: [], capabilities: [], workflows: [], databaseBytes: 0, reusableBytes: 0 } };
  const workspaceBinding = bindWorkspaceView(workspace.root, { getState: () => workspaceState });
  const workspaceHtml = workspace.root.html;
  workspaceBinding.purgeForWorkspaceLock();
  workspaceState.message = 'Late source-bearing state';
  workspaceBinding.render();
  assert.equal(workspace.root.html, workspaceHtml);
  assert.equal(workspace.listeners.size, 0);

  const model = rootFixture();
  const modelState = { endpoint: 'local', bridgeAvailable: false, pending: null, generation: '1', message: 'Secret model state', snapshots: [], capabilities: [] };
  const modelBinding = bindModelAssistanceView(model.root, { getState: () => modelState });
  const modelHtml = model.root.html;
  modelBinding.purgeForWorkspaceLock();
  modelState.message = 'Late model source state';
  modelBinding.render();
  assert.equal(model.root.html, modelHtml);
  assert.equal(model.listeners.size, 0);
});

test('workspace binding rechecks storage mode for clicks and invalidates stale export UI', () => {
  const fixture = rootFixture();
  let mode = 'legacy_plaintext';
  let state = {
    status: 'ready', pending: null, message: 'Ready.', dialog: null,
    workspace: { records: {}, sources: [], capabilities: [], workflows: [], databaseBytes: 0, reusableBytes: 0 },
  };
  const calls = [];
  const controller = {
    getState: () => state,
    load: () => calls.push('load'),
    requestExport: () => calls.push('export'),
    prepareRestore: () => calls.push('restore'),
    inspectDeletion: () => calls.push('inspect'),
    confirmExport: () => calls.push('confirm-export'),
    confirmRestore: () => calls.push('confirm-restore'),
    confirmDeletion: () => calls.push('confirm-deletion'),
    cancel: () => { calls.push('cancel'); state = { ...state, dialog: null }; },
  };
  const target = (selector, dataset = {}) => ({
    dataset,
    closest(candidate) { return candidate === selector ? this : null; },
  });
  const binding = bindWorkspaceView(fixture.root, controller, { getStorageMode: () => mode });
  const click = (selector, dataset) => fixture.listeners.get('click')?.({ target: target(selector, dataset) });

  click('#workspace-export');
  click('#workspace-restore');
  click('[data-workspace-inspect]', { workspaceInspect: 'source', workspaceId: 'source:' + '1'.repeat(64), workspaceIntent: 'delete' });
  assert.deepEqual(calls, ['export', 'restore', 'inspect']);

  state = { ...state, dialog: { kind: 'export-warning' } };
  binding.render();
  mode = 'managed_encrypted';
  binding.render();
  assert.equal(calls.at(-1), 'cancel');
  click('#workspace-export');
  assert.equal(calls.includes('confirm-export'), false);

  mode = 'browser';
  click('#workspace-restore');
  click('[data-workspace-inspect]', { workspaceInspect: 'source', workspaceId: 'source:' + '1'.repeat(64), workspaceIntent: 'delete' });
  state = { ...state, dialog: { kind: 'delete', mode: 'confirm' } };
  click('[data-workspace-confirm]');
  assert.deepEqual(calls, ['export', 'restore', 'inspect', 'cancel']);

  binding.purgeForWorkspaceLock();
  mode = 'legacy_plaintext';
  click('#workspace-export');
  assert.deepEqual(calls, ['export', 'restore', 'inspect', 'cancel']);
});

test('workspace binding routes supported encrypted recovery actions and rejects them after capability loss', () => {
  const fixture = rootFixture();
  let state = {
    status: 'ready', pending: null, message: 'Ready.', dialog: null, encryptedRecoverySupported: true,
    workspace: { records: {}, sources: [], capabilities: [], workflows: [], databaseBytes: 0, reusableBytes: 0 },
  };
  const calls = [];
  const controller = {
    getState: () => state,
    load: () => calls.push('load'),
    requestEncryptedExport: () => calls.push('request-encrypted-export'),
    prepareEncryptedRestore: () => calls.push('prepare-encrypted-restore'),
    createRecoveryKey: () => calls.push('create-recovery-key'),
    confirmEncryptedExport: () => calls.push('confirm-encrypted-export'),
    prepareRestore: () => calls.push('prepare-legacy-restore'),
    inspectDeletion: () => calls.push('inspect'),
    confirmExport: () => calls.push('confirm-export'),
    confirmRestore: () => calls.push('confirm-restore'),
    confirmDeletion: () => calls.push('confirm-deletion'),
    cancel: () => { calls.push('cancel'); state = { ...state, dialog: null }; },
  };
  const target = selector => ({ closest(candidate) { return candidate === selector ? this : null; } });
  const binding = bindWorkspaceView(fixture.root, controller, { getStorageMode: () => 'managed_encrypted' });
  const click = selector => fixture.listeners.get('click')?.({ target: target(selector) });

  click('#workspace-encrypted-export');
  click('#workspace-encrypted-restore');
  state = { ...state, dialog: { kind: 'encrypted-export-warning' } };
  binding.render();
  click('[data-workspace-create-recovery-key]');
  click('[data-workspace-confirm-encrypted-export]');
  assert.deepEqual(calls, ['request-encrypted-export', 'prepare-encrypted-restore', 'create-recovery-key', 'confirm-encrypted-export']);

  state = { ...state, encryptedRecoverySupported: false, dialog: null };
  binding.render();
  click('#workspace-encrypted-export');
  click('#workspace-encrypted-restore');
  assert.deepEqual(calls, ['request-encrypted-export', 'prepare-encrypted-restore', 'create-recovery-key', 'confirm-encrypted-export']);
});
