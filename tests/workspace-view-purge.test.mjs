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
