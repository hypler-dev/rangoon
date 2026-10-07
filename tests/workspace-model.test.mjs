import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorkspaceController, validateWorkspaceData } from '../preview/workspace-model.mjs';

const id = (prefix, digit) => `${prefix}:${digit.repeat(64)}`;
const sourceId = id('source', '1');
const capabilityId = id('capability', '3');
const revisionId = id('revision', '4');
const workflowId = id('workflow', '5');
const workflowRevisionId = id('workflow-revision', '6');
const stateId = id('workspace', '7');
const backupId = id('backup', '8');
const counts = (overrides = {}) => ({ sources: 1, capabilities: 1, revisions: 1, reviews: 0, recipes: 0, applications: 0, derivations: 0, workflows: 1, workflowRevisions: 1, ...overrides });
const capability = (overrides = {}) => ({ id: capabilityId, origin: { kind: 'source', sourceId, fragmentId: id('fragment', '2') }, latestRevisionId: revisionId, title: 'Bounded skill', reviewed: false, revisionCount: 1, ...overrides });
const workflow = (overrides = {}) => ({ id: workflowId, latestRevisionId: workflowRevisionId, label: 'Review workflow', labelAdjusted: false, intent: 'draft', structurallyValid: false, revisionCount: 1, savedAtMs: 1, unresolvedReferences: 1, ...overrides });
const workspace = (overrides = {}) => ({ schemaVersion: 'rangoon.workspace-data.v2', stateId, records: counts(), databaseBytes: 4096, reusableBytes: 0, sources: [{ sourceId, displayName: 'AGENTS.md', sha256: 'a'.repeat(64), byteLength: 22, savedAtMs: 1 }], capabilities: [capability()], workflows: [workflow()], ...overrides });
const plan = (overrides = {}) => ({ schemaVersion: 'rangoon.restore-plan.v2', backupId, expectedStateId: stateId, byteLength: 2048, add: counts(), keptSources: 0, keptCapabilities: 0, keptWorkflows: 0, ...overrides });
const deletion = (overrides = {}) => ({ schemaVersion: 'rangoon.deletion-plan.v2', kind: 'workflow', id: workflowId, label: 'Review workflow', expectedStateId: stateId, remove: counts({ sources: 0, capabilities: 0, revisions: 0, workflows: 1, workflowRevisions: 1 }), capabilityDependencies: [], workflowDependencies: [], ...overrides });

test('closed v2 inventory rejects v1, unknown fields, invalid summaries and count drift', () => {
  assert.ok(validateWorkspaceData(workspace()));
  assert.equal(validateWorkspaceData(workspace({ schemaVersion: 'rangoon.workspace-data.v1' })), null);
  assert.equal(validateWorkspaceData({ ...workspace(), unknown: true }), null);
  assert.equal(validateWorkspaceData(workspace({ records: counts({ workflowRevisions: 2 }) })), null);
  assert.equal(validateWorkspaceData(workspace({ workflows: [workflow({ label: 'unsafe\nlabel' })] })), null);
  assert.equal(validateWorkspaceData(workspace({ workflows: [workflow({ intent: 'validated', structurallyValid: false, unresolvedReferences: 0 })] })), null);
  assert.equal(validateWorkspaceData(workspace({ workflows: [workflow({ intent: 'validated', structurallyValid: true, unresolvedReferences: 1 })] })), null);
  assert.ok(validateWorkspaceData(workspace({ workflows: [workflow({ intent: 'validated', structurallyValid: true, unresolvedReferences: 0 })] })));
});

test('workflow-only restore is actionable and sends closed selector arguments', async () => {
  const calls = []; const mutations = [];
  const workflowOnly = plan({ add: counts({ sources: 0, capabilities: 0, revisions: 0, workflows: 1, workflowRevisions: 1 }) });
  const controller = createWorkspaceController({ invoke: async (command, args) => {
    calls.push([command, args]);
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'prepare_workspace_restore') return { outcome: 'restore_ready', plan: workflowOnly };
    return { outcome: 'restored', plan: workflowOnly };
  }, onMutation: async event => mutations.push(event) });
  await controller.load(); await controller.prepareRestore(); await controller.confirmRestore();
  assert.deepEqual(calls.find(([command]) => command === 'restore_workspace_backup'), ['restore_workspace_backup', { backupId, expectedStateId: stateId }]);
  assert.deepEqual(mutations, [{ kind: 'restore' }]);
});

test('restore receipt must equal every issued v2 field before mutation callback', async () => {
  const events = [];
  const controller = createWorkspaceController({ invoke: async command => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'prepare_workspace_restore') return { outcome: 'restore_ready', plan: plan() };
    return { outcome: 'restored', plan: plan({ keptWorkflows: 1 }) };
  }, onMutation: async event => events.push(event) });
  await controller.load(); await controller.prepareRestore(); await controller.confirmRestore();
  assert.equal(events.length, 0);
  assert.equal(controller.getState().restorePlan, null);
  assert.equal(controller.getState().error.message, 'The local workspace action could not finish.');
});

test('restore preview rejects corrupt limits and all fields outside v2 shape', async () => {
  const controller = createWorkspaceController({ invoke: async command => command === 'get_workspace_data' ? { outcome: 'loaded', workspace: workspace() } : { outcome: 'restore_ready', plan: plan({ keptWorkflows: 128 }) } });
  await controller.load(); await controller.prepareRestore();
  assert.equal(controller.getState().restorePlan, null);
  assert.equal(controller.getState().error.message, 'The local workspace action could not finish.');
});

test('confirmed workflow deletion consumes preview, calls callback, and preserves source-specific draft behavior', async () => {
  const calls = []; const events = [];
  const controller = createWorkspaceController({ invoke: async (command, args) => {
    calls.push([command, args]);
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'inspect_workspace_deletion') return { outcome: 'deletion_ready', plan: deletion() };
    return { outcome: 'deleted', kind: 'workflow', id: workflowId };
  }, getDraftWarning: () => { throw new Error('workflow must not ask skill draft warning'); }, onMutation: async event => events.push(event) });
  await controller.load(); await controller.inspectDeletion('workflow', workflowId, true); await controller.confirmDeletion();
  assert.deepEqual(calls.find(([command]) => command === 'delete_workspace_record'), ['delete_workspace_record', { kind: 'workflow', id: workflowId, expectedStateId: stateId }]);
  assert.deepEqual(events, [{ kind: 'delete', recordKind: 'workflow', id: workflowId }]);
  assert.equal(controller.getState().deletionPlan, null);
});

test('failed write consumes candidate and needs fresh preview before another confirmation', async () => {
  let inspections = 0; let deletes = 0;
  const controller = createWorkspaceController({ invoke: async command => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'inspect_workspace_deletion') { inspections += 1; return { outcome: 'deletion_ready', plan: deletion() }; }
    deletes += 1; return { outcome: 'failed', error: { code: 'workspace_unavailable', message: 'Uncertain' } };
  } });
  await controller.load(); await controller.inspectDeletion('workflow', workflowId, true); await controller.confirmDeletion(); await controller.confirmDeletion();
  assert.equal(deletes, 1); assert.equal(controller.getState().deletionPlan, null);
  await controller.inspectDeletion('workflow', workflowId, true); assert.equal(inspections, 2);
});

test('historical workflow blockers prevent deletion and render escaped blocker sections', async () => {
  const blocker = workflow({ id: id('workflow', '9'), label: '<draft & history>', intent: 'draft' });
  const controller = createWorkspaceController({ invoke: async command => command === 'get_workspace_data' ? { outcome: 'loaded', workspace: workspace() } : { outcome: 'deletion_ready', plan: deletion({ kind: 'capability', id: capabilityId, label: 'Bounded skill', remove: counts({ sources: 0, capabilities: 1, revisions: 1, workflows: 0, workflowRevisions: 0 }), workflowDependencies: [blocker] }) } });
  await controller.load(); await controller.inspectDeletion('capability', capabilityId, true); await controller.confirmDeletion();
  assert.equal(controller.getState().pending, null);
  const { renderWorkspaceView } = await import('../preview/workspace-view.mjs');
  const html = renderWorkspaceView({ ...controller.getState() });
  assert.match(html, /Blocking workflows/);
  assert.match(html, /&lt;draft &amp; history&gt;/);
  assert.match(html, /No records will be removed while blockers remain\./);
});

test('workspace view names workflow history and no workflow creation action', async () => {
  const { renderWorkspaceView } = await import('../preview/workspace-view.mjs');
  const html = renderWorkspaceView({ bridgeAvailable: true, status: 'ready', pending: null, error: null, message: 'Ready.', workspace: workspace(), dialog: null, restorePlan: null, deletionPlan: null });
  assert.match(html, /SAVED WORKFLOWS/);
  assert.match(html, /Workflow revisions/);
  assert.match(html, /Authority: none/);
  assert.doesNotMatch(html, /Create workflow/);
});

test('source deletion accepts original 255-byte display label with spaces and Unicode separator', async () => {
  const sourceLabel = ' ' + 'a'.repeat(247) + ' \u2028.md';
  const sourcePlan = deletion({
    kind: 'source',
    id: sourceId,
    label: sourceLabel,
    remove: counts({
      capabilities: 0,
      revisions: 0,
      workflows: 0,
      workflowRevisions: 0,
    }),
  });
  const controller = createWorkspaceController({
    invoke: async command => command === 'get_workspace_data'
      ? { outcome: 'loaded', workspace: workspace() }
      : { outcome: 'deletion_ready', plan: sourcePlan },
  });
  await controller.load();
  await controller.inspectDeletion('source', sourceId);
  assert.equal(controller.getState().deletionPlan.label, sourceLabel);
});

test('pending export blocks duplicate confirmation and supplies no renderer arguments', async () => {
  let resolveExport;
  const calls = [];
  const controller = createWorkspaceController({
    invoke: (command, args) => {
      calls.push([command, args]);
      if (command === 'get_workspace_data') return Promise.resolve({ outcome: 'loaded', workspace: workspace() });
      return new Promise(resolve => { resolveExport = resolve; });
    },
  });
  await controller.load();
  controller.requestExport();
  const first = controller.confirmExport();
  const second = controller.confirmExport();
  assert.equal(calls.filter(([command]) => command === 'export_workspace_backup').length, 1);
  resolveExport({ outcome: 'exported', backupId, byteLength: 2048 });
  await Promise.all([first, second]);
  assert.deepEqual(calls.find(([command]) => command === 'export_workspace_backup'), ['export_workspace_backup', undefined]);
});

test('cancelled second restore picker preserves retained candidate for confirmation', async () => {
  let picker = 0;
  const calls = [];
  const controller = createWorkspaceController({
    invoke: async command => {
      calls.push(command);
      if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
      if (command === 'prepare_workspace_restore') return ++picker === 1 ? { outcome: 'restore_ready', plan: plan() } : { outcome: 'cancelled' };
      return { outcome: 'restored', plan: plan() };
    },
  });
  await controller.load();
  await controller.prepareRestore();
  controller.cancel();
  await controller.prepareRestore();
  assert.equal(controller.getState().dialog.kind, 'restore');
  await controller.confirmRestore();
  assert.equal(calls.filter(command => command === 'restore_workspace_backup').length, 1);
});

test('reordered complete receipt succeeds, but workflow-count drift is refused', async () => {
  const events = [];
  const base = plan();
  const reordered = {
    keptWorkflows: base.keptWorkflows,
    add: { workflowRevisions: base.add.workflowRevisions, workflows: base.add.workflows, derivations: base.add.derivations, applications: base.add.applications, recipes: base.add.recipes, reviews: base.add.reviews, revisions: base.add.revisions, capabilities: base.add.capabilities, sources: base.add.sources },
    expectedStateId: base.expectedStateId,
    backupId: base.backupId,
    byteLength: base.byteLength,
    schemaVersion: base.schemaVersion,
    keptCapabilities: base.keptCapabilities,
    keptSources: base.keptSources,
  };
  const controller = createWorkspaceController({
    invoke: async command => {
      if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
      if (command === 'prepare_workspace_restore') return { outcome: 'restore_ready', plan: base };
      return { outcome: 'restored', plan: reordered };
    },
    onMutation: async event => events.push(event),
  });
  await controller.load();
  await controller.prepareRestore();
  await controller.confirmRestore();
  assert.deepEqual(events, [{ kind: 'restore' }]);

  const drift = createWorkspaceController({
    invoke: async command => command === 'get_workspace_data'
      ? { outcome: 'loaded', workspace: workspace() }
      : { outcome: 'restore_ready', plan: plan({ add: counts({ workflows: 2, workflowRevisions: 1 }) }) },
  });
  await drift.load();
  await drift.prepareRestore();
  assert.equal(drift.getState().restorePlan, null);
});

test('related refresh stays busy, callback failure retains receipt, and failed refresh preserves deletion receipt', async () => {
  let releaseMutation;
  let releaseLoad;
  let loads = 0;
  const calls = [];
  const controller = createWorkspaceController({
    invoke: command => {
      calls.push(command);
      if (command === 'get_workspace_data') {
        if (++loads === 1) return Promise.resolve({ outcome: 'loaded', workspace: workspace() });
        return new Promise(resolve => { releaseLoad = resolve; });
      }
      if (command === 'prepare_workspace_restore') return Promise.resolve({ outcome: 'restore_ready', plan: plan() });
      return new Promise(resolve => { releaseMutation = resolve; });
    },
    onMutation: async () => { throw new Error('related view failed'); },
  });
  await controller.load();
  await controller.prepareRestore();
  const restoring = controller.confirmRestore();
  releaseMutation({ outcome: 'restored', plan: plan() });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(controller.getState().pending, 'refresh');
  controller.requestExport();
  assert.equal(calls.includes('export_workspace_backup'), false);
  releaseLoad({ outcome: 'loaded', workspace: workspace() });
  await restoring;
  assert.equal(controller.getState().message, 'Restore completed. Related local views could not refresh.');

  let deletionLoads = 0;
  const deletionController = createWorkspaceController({
    invoke: async command => {
      if (command === 'get_workspace_data') return ++deletionLoads === 1
        ? { outcome: 'loaded', workspace: workspace() }
        : { outcome: 'failed', error: { code: 'workspace_unavailable', message: 'Unavailable' } };
      if (command === 'inspect_workspace_deletion') return { outcome: 'deletion_ready', plan: deletion() };
      return { outcome: 'deleted', kind: 'workflow', id: workflowId };
    },
  });
  await deletionController.load();
  await deletionController.inspectDeletion('workflow', workflowId, true);
  await deletionController.confirmDeletion();
  assert.match(deletionController.getState().message, /^Local record removed\. Refreshing saved data… Inventory refresh failed\./);
  assert.equal(deletionController.getState().error.code, 'workspace_unavailable');
});

test('malformed deletion receipt does not call mutation callback', async () => {
  const events = [];
  const controller = createWorkspaceController({
    invoke: async command => {
      if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
      if (command === 'inspect_workspace_deletion') return { outcome: 'deletion_ready', plan: deletion() };
      return { outcome: 'deleted', kind: 'source', id: sourceId };
    },
    onMutation: async event => events.push(event),
  });
  await controller.load();
  await controller.inspectDeletion('workflow', workflowId, true);
  await controller.confirmDeletion();
  assert.deepEqual(events, []);
  assert.equal(controller.getState().error.message, 'The local workspace action could not finish.');
});

test('unknown v2 deletion count fails closed', async () => {
  const controller = createWorkspaceController({
    invoke: async command => command === 'get_workspace_data'
      ? { outcome: 'loaded', workspace: workspace() }
      : { outcome: 'deletion_ready', plan: deletion({ remove: { ...deletion().remove, unexpected: 0 } }) },
  });
  await controller.load();
  await controller.inspectDeletion('workflow', workflowId, true);
  assert.equal(controller.getState().deletionPlan, null);
  assert.equal(controller.getState().error.message, 'The local workspace action could not finish.');
});


test('impact counts use singular nouns and correctly pluralize zero, one and two', async () => {
  const { renderWorkspaceView } = await import('../preview/workspace-view.mjs');
  for (const count of [0, 1, 2]) {
    const html = renderWorkspaceView({ status: 'ready', pending: null, error: null, message: 'Ready.', workspace: workspace(), dialog: { kind: 'delete', mode: 'delete' }, deletionPlan: deletion({ remove: counts({ sources: count, capabilities: count, revisions: count, workflows: count, workflowRevisions: count, reviews: count, recipes: count, applications: count, derivations: count }) }) });
    for (const noun of ['source', 'skill', 'workflow', 'capability revision', 'workflow revision', 'review', 'recipe', 'application', 'derivation']) {
      assert.ok(html.includes(`Remove ${count} ${noun}${count === 1 ? '' : 's'}</dd>`));
    }
    assert.ok(html.includes('Save intent and structural checks are observations, not review, approval or execution authority.'));
  }
});
