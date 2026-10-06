import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorkspaceController, validateWorkspaceData } from '../preview/workspace-model.mjs';

const sourceId = `source:${'1'.repeat(64)}`;
const fragmentId = `fragment:${'2'.repeat(64)}`;
const capabilityId = `capability:${'3'.repeat(64)}`;
const revisionId = `revision:${'4'.repeat(64)}`;
const stateId = `workspace:${'5'.repeat(64)}`;
const backupId = `backup:${'6'.repeat(64)}`;
const compositionId = `composition:${'7'.repeat(64)}`;
const count = (overrides = {}) => ({ sources: 1, capabilities: 1, revisions: 1, reviews: 0, recipes: 0, applications: 0, derivations: 0, ...overrides });
const summary = (overrides = {}) => ({ id: capabilityId, origin: { kind: 'source', sourceId, fragmentId }, latestRevisionId: revisionId, title: 'Bounded rules', reviewed: false, revisionCount: 1, ...overrides });
const composedSummary = (overrides = {}) => summary({ origin: { kind: 'composition', compositionId, operation: 'merge', outputIndex: 0 }, ...overrides });
const workspace = (overrides = {}) => ({ schemaVersion: 'rangoon.workspace-data.v1', stateId, records: count(), databaseBytes: 4096, reusableBytes: 0, sources: [{ sourceId, displayName: 'AGENTS.md', sha256: 'a'.repeat(64), byteLength: 22, savedAtMs: 1 }], capabilities: [summary()], ...overrides });
const plan = (overrides = {}) => ({ schemaVersion: 'rangoon.restore-plan.v1', backupId, expectedStateId: stateId, byteLength: 2048, add: count(), keptSources: 0, keptCapabilities: 0, ...overrides });
const deletion = (overrides = {}) => ({ schemaVersion: 'rangoon.deletion-plan.v1', kind: 'capability', id: capabilityId, title: 'Bounded rules', expectedStateId: stateId, remove: count({ sources: 0 }), dependencies: [], ...overrides });

test('strict v1 workspace DTO rejects v0, unknown keys and origin/count inconsistencies', () => {
  assert.ok(validateWorkspaceData(workspace()));
  assert.equal(validateWorkspaceData(workspace({ capabilities: [composedSummary({ origin: { ...composedSummary().origin, operation: 'merge', outputIndex: 1 } })] })), null);
  assert.equal(validateWorkspaceData(workspace({ schemaVersion: 'rangoon.workspace-data.v0' })), null);
  assert.equal(validateWorkspaceData(workspace({ records: count({ sources: 2 }) })), null);
  assert.equal(validateWorkspaceData({ ...workspace(), extra: true }), null);
  assert.equal(validateWorkspaceData(workspace({ capabilities: [summary({ origin: { kind: 'source', sourceId: `source:${'f'.repeat(64)}`, fragmentId } })] })), null);
  assert.equal(validateWorkspaceData(workspace({ records: count({ recipes: 1, applications: 0 }) })), null);
  assert.ok(validateWorkspaceData(workspace({ capabilities: [composedSummary()] })));
});

test('restore preview rejects totals that would exceed local source or skill limits', async () => {
  const controller = createWorkspaceController({ invoke: async command => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    return { outcome: 'restore_ready', plan: plan({ keptSources: 128 }) };
  } });
  await controller.load();
  await controller.prepareRestore();
  assert.equal(controller.getState().restorePlan, null);
  assert.equal(controller.getState().error.message, 'The local workspace action could not finish.');
});

test('controller rejects closed v1 recovery plans before confirmation', async () => {
  const restore = createWorkspaceController({ invoke: async command => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    return { outcome: 'restore_ready', plan: plan({ add: { ...count(), recipes: 1 } }) };
  } });
  await restore.load();
  await restore.prepareRestore();
  assert.equal(restore.getState().restorePlan, null);
  assert.equal(restore.getState().error.message, 'The local workspace action could not finish.');

  const deletionController = createWorkspaceController({ invoke: async command => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    return { outcome: 'deletion_ready', plan: deletion({ remove: { ...count({ sources: 0 }), applications: 1, derivations: 0 } }) };
  } });
  await deletionController.load();
  await deletionController.inspectDeletion('capability', capabilityId, true);
  assert.equal(deletionController.getState().deletionPlan, null);
  assert.equal(deletionController.getState().error.message, 'The local workspace action could not finish.');
});

test('loads exact inventory and sends exact camel-case restore/deletion arguments', async () => {
  const calls = [];
  const mutations = [];
  const controller = createWorkspaceController({ invoke: async (command, args) => {
    calls.push([command, args]);
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'prepare_workspace_restore') return { outcome: 'restore_ready', plan: plan() };
    if (command === 'restore_workspace_backup') return { outcome: 'restored', plan: plan() };
    if (command === 'inspect_workspace_deletion') return { outcome: 'deletion_ready', plan: deletion() };
    return { outcome: 'deleted', kind: 'capability', id: capabilityId };
  }, onMutation: async event => mutations.push(event) });
  await controller.load();
  await controller.prepareRestore();
  await controller.confirmRestore();
  assert.equal(controller.getState().message, 'Restore completed. Local data refreshed.');
  await controller.inspectDeletion('capability', capabilityId, true);
  await controller.confirmDeletion();
  assert.deepEqual(calls.find(([command]) => command === 'restore_workspace_backup'), ['restore_workspace_backup', { backupId, expectedStateId: stateId }]);
  assert.deepEqual(calls.find(([command]) => command === 'delete_workspace_record'), ['delete_workspace_record', { kind: 'capability', id: capabilityId, expectedStateId: stateId }]);
  assert.deepEqual(mutations, [{ kind: 'restore' }, { kind: 'delete', recordKind: 'capability', id: capabilityId }]);
  assert.equal(controller.getState().message, 'Local record removed. Local data refreshed.');
});

test('pending guard blocks repeated export clicks and export never receives renderer arguments', async () => {
  let resolveExport;
  const calls = [];
  const controller = createWorkspaceController({ invoke: (command, args) => {
    calls.push([command, args]);
    if (command === 'get_workspace_data') return Promise.resolve({ outcome: 'loaded', workspace: workspace() });
    return new Promise(resolve => { resolveExport = resolve; });
  } });
  await controller.load();
  controller.requestExport();
  const first = controller.confirmExport();
  const second = controller.confirmExport();
  assert.equal(calls.filter(([command]) => command === 'export_workspace_backup').length, 1);
  resolveExport({ outcome: 'exported', backupId, byteLength: 2048 });
  await Promise.all([first, second]);
  assert.deepEqual(calls.find(([command]) => command === 'export_workspace_backup'), ['export_workspace_backup', undefined]);
});

test('restore cancellation retains the prior usable preview and failure preserves inventory', async () => {
  let restorePicker = 0;
  const controller = createWorkspaceController({ invoke: async command => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'prepare_workspace_restore') return ++restorePicker === 1 ? { outcome: 'restore_ready', plan: plan() } : { outcome: 'cancelled' };
    return { outcome: 'failed', error: { code: 'workspace_changed', message: 'Changed' } };
  } });
  await controller.load();
  await controller.prepareRestore();
  controller.cancel();
  await controller.prepareRestore();
  assert.equal(controller.getState().restorePlan.backupId, backupId);
  controller.requestExport();
  await controller.confirmExport();
  assert.equal(controller.getState().workspace.stateId, stateId);
  assert.equal(controller.getState().error.code, 'workspace_changed');
});

test('cancelled second picker reopens the retained restore preview and can still confirm it', async () => {
  let picker = 0;
  const calls = [];
  const controller = createWorkspaceController({ invoke: async command => {
    calls.push(command);
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'prepare_workspace_restore') return ++picker === 1 ? { outcome: 'restore_ready', plan: plan() } : { outcome: 'cancelled' };
    return { outcome: 'restored', plan: plan() };
  } });
  await controller.load();
  await controller.prepareRestore();
  controller.cancel();
  await controller.prepareRestore();
  assert.equal(controller.getState().dialog.kind, 'restore');
  await controller.confirmRestore();
  assert.equal(calls.filter(command => command === 'restore_workspace_backup').length, 1);
});

test('verified mutation remains busy through related refresh and callback failures retain the receipt', async () => {
  let releaseMutation;
  let releaseLoad;
  let loaded = 0;
  const calls = [];
  const controller = createWorkspaceController({ invoke: command => {
    calls.push(command);
    if (command === 'get_workspace_data') {
      if (++loaded === 1) return Promise.resolve({ outcome: 'loaded', workspace: workspace() });
      return new Promise(resolve => { releaseLoad = resolve; });
    }
    if (command === 'prepare_workspace_restore') return Promise.resolve({ outcome: 'restore_ready', plan: plan() });
    if (command === 'restore_workspace_backup') return new Promise(resolve => { releaseMutation = resolve; });
    return Promise.resolve({ outcome: 'cancelled' });
  }, onMutation: async () => { throw new Error('related view failed'); } });
  await controller.load();
  await controller.prepareRestore();
  const restoring = controller.confirmRestore();
  releaseMutation({ outcome: 'restored', plan: plan() });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(controller.getState().pending, 'refresh');
  await controller.requestExport();
  assert.equal(calls.includes('export_workspace_backup'), false);
  releaseLoad({ outcome: 'loaded', workspace: workspace() });
  await restoring;
  assert.equal(controller.getState().pending, null);
  assert.equal(controller.getState().message, 'Restore completed. Related local views could not refresh.');
});

test('refresh failure does not overwrite a verified deletion receipt', async () => {
  let loads = 0;
  const controller = createWorkspaceController({ invoke: async command => {
    if (command === 'get_workspace_data') return ++loads === 1 ? { outcome: 'loaded', workspace: workspace() } : { outcome: 'failed', error: { code: 'workspace_unavailable', message: 'Unavailable' } };
    if (command === 'inspect_workspace_deletion') return { outcome: 'deletion_ready', plan: deletion() };
    return { outcome: 'deleted', kind: 'capability', id: capabilityId };
  } });
  await controller.load();
  await controller.inspectDeletion('capability', capabilityId, true);
  await controller.confirmDeletion();
  assert.match(controller.getState().message, /^Local record removed\. Refreshing saved data… Inventory refresh failed\./);
  assert.equal(controller.getState().error.code, 'workspace_unavailable');
});

test('stale or malformed receipts never invoke mutation callbacks', async () => {
  const events = [];
  const controller = createWorkspaceController({ invoke: async command => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'prepare_workspace_restore') return { outcome: 'restore_ready', plan: plan() };
    if (command === 'restore_workspace_backup') return { outcome: 'restored', plan: plan({ expectedStateId: `workspace:${'7'.repeat(64)}` }) };
    if (command === 'inspect_workspace_deletion') return { outcome: 'deletion_ready', plan: deletion() };
    return { outcome: 'deleted', kind: 'source', id: sourceId };
  }, onMutation: async event => events.push(event) });
  await controller.load();
  await controller.prepareRestore();
  await controller.confirmRestore();
  assert.equal(events.length, 0);
  await controller.inspectDeletion('capability', capabilityId, true);
  await controller.confirmDeletion();
  assert.equal(events.length, 0);
  assert.equal(controller.getState().error.message, 'The local workspace action could not finish.');
});

test('source inspection rejects delete when dependencies exist and draft warning is carried only for skills', async () => {
  const dependent = composedSummary({ id: `capability:${'8'.repeat(64)}` });
  const controller = createWorkspaceController({ invoke: async command => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    return { outcome: 'deletion_ready', plan: deletion({ kind: 'source', id: sourceId, title: 'AGENTS.md', remove: count({ capabilities: 0, revisions: 0 }), dependencies: [dependent] }) };
  }, getDraftWarning: () => true });
  await controller.load();
  await controller.inspectDeletion('source', sourceId);
  assert.equal(controller.getState().dialog.draftWarning, null);
  await controller.confirmDeletion();
  assert.equal(controller.getState().pending, null);
});

test('blocked source deletion says no records will be removed', async () => {
  const { renderWorkspaceView } = await import('../preview/workspace-view.mjs');
  const dependent = composedSummary({ id: `capability:${'8'.repeat(64)}` });
  const html = renderWorkspaceView({
    bridgeAvailable: true, status: 'ready', pending: null, error: null, message: 'Deletion blocked.', workspace: workspace(),
    dialog: { kind: 'delete', mode: 'confirm', draftWarning: null }, restorePlan: null,
    deletionPlan: deletion({ kind: 'source', id: sourceId, title: 'AGENTS.md', remove: count({ capabilities: 0, revisions: 0 }), dependencies: [dependent] }),
  });
  assert.match(html, /No records will be removed while dependent skills remain\./);
  assert.doesNotMatch(html, /Sources (removed|to remove)/);
  assert.doesNotMatch(html, /Reviews (removed|to remove)/);
});

test('restore and deletion views show v1 composition impact from host plans', async () => {
  const { renderWorkspaceView } = await import('../preview/workspace-view.mjs');
  const html = renderWorkspaceView({
    bridgeAvailable: true, status: 'ready', pending: null, error: null, message: 'Restore preview ready.', workspace: workspace(),
    dialog: { kind: 'restore' }, restorePlan: plan({ add: count({ recipes: 1, applications: 1, derivations: 1 }) }), deletionPlan: null,
  });
  assert.match(html, /Add 1 recipe/);
  assert.match(html, /Add 1 application/);
  assert.match(html, /Add 1 derivation/);
  const deletionHtml = renderWorkspaceView({
    bridgeAvailable: true, status: 'ready', pending: null, error: null, message: 'Deletion preview ready.', workspace: workspace(),
    dialog: { kind: 'delete', mode: 'confirm' }, restorePlan: null,
    deletionPlan: deletion({ remove: count({ sources: 0, recipes: 1, applications: 1, derivations: 1 }) }),
  });
  assert.match(deletionHtml, /Remove 1 recipe/);
  assert.match(deletionHtml, /Remove 1 application/);
  assert.match(deletionHtml, /Remove 1 derivation/);
});
