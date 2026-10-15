import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createCompilationController } from '../preview/compilation-model.mjs';
import { createWorkspaceController } from '../preview/workspace-model.mjs';
import { createModelAssistanceController, DEFAULT_LOCAL_MODEL_PROFILE } from '../preview/model-assistance-model.mjs';
import { createCloudCredentialController } from '../preview/cloud-credential-model.mjs';

const id = (prefix, digit) => `${prefix}:${digit.repeat(64)}`;
const sourceId = id('source', '1');
const capabilityId = id('capability', '3');
const revisionId = id('revision', '4');
const stateId = id('workspace', '7');
const backupId = id('backup', '8');
const counts = (overrides = {}) => ({ sources: 1, capabilities: 1, revisions: 1, reviews: 0, recipes: 0, applications: 0, derivations: 0, workflows: 1, workflowRevisions: 1, ...overrides });
const workspace = () => ({ schemaVersion: 'rangoon.workspace-data.v2', stateId, records: counts(), databaseBytes: 4096, reusableBytes: 0, sources: [{ sourceId, displayName: 'AGENTS.md', sha256: 'a'.repeat(64), byteLength: 22, savedAtMs: 1 }], capabilities: [{ id: capabilityId, origin: { kind: 'source', sourceId, fragmentId: id('fragment', '2') }, latestRevisionId: revisionId, title: 'Bounded skill', reviewed: false, revisionCount: 1 }], workflows: [{ id: id('workflow', '5'), latestRevisionId: id('workflow-revision', '6'), label: 'Review workflow', labelAdjusted: false, intent: 'draft', structurallyValid: false, revisionCount: 1, savedAtMs: 1, unresolvedReferences: 1 }] });
const restorePlan = () => ({ schemaVersion: 'rangoon.restore-plan.v2', backupId, expectedStateId: stateId, byteLength: 2048, add: counts(), keptSources: 0, keptCapabilities: 0, keptWorkflows: 0 });
const profile = { ...DEFAULT_LOCAL_MODEL_PROFILE, model: 'synthetic:v1' };
const profileResult = (outcome, extra = {}) => ({ schemaVersion: 'rangoon.local-session-result.v1', generation: '1', runId: null, authority: 'none', outcome, ...extra });
const session = (configured = false) => ({ generation: '1', profile: configured ? { config: { schemaVersion: 'rangoon.local-profile.v1', adapter: 'ollama-loopback.v1', ...profile }, profileSha256: 'b'.repeat(64) } : null, active: null, persistence: 'session_only', processingLocation: 'unknown', retention: 'unknown', authority: 'none' });
const custody = outcome => ({ schemaVersion: 'rangoon.cloud-custody.v1', provider: 'openai', authority: 'none', backend: 'macos_keychain', outcome, ...(outcome === 'stored' ? { revision: 'c'.repeat(64) } : {}) });

const compilationFixture = JSON.parse(await readFile(new URL('../fixtures/compilation/instruction-v1.json', import.meta.url), 'utf8'));
const compilationCase = compilationFixture.cases.find(item => item.name === 'agents_exact_bom_crlf_unicode');
const compilationDetail = () => {
  const revision = structuredClone(compilationCase.revision);
  const { content, ...history } = revision;
  return {
    schemaVersion: 'rangoon.capability.v1', id: compilationCase.capabilityId,
    origin: { kind: 'source', sourceId, fragmentId: id('fragment', '2'), sourceName: 'fixture.md', span: { startByte: 0, endByte: new TextEncoder().encode(revision.content).length, startLine: 1, endLine: 1 }, originalText: revision.content },
    latestRevisionId: revision.id, revision, history: [history], authority: 'none',
  };
};
const compilationSummary = detail => ({ id: detail.id, origin: { kind: 'source', sourceId, fragmentId: id('fragment', '2') }, latestRevisionId: detail.latestRevisionId, title: detail.revision.title, reviewed: Boolean(detail.revision.review), revisionCount: detail.history.length });

test('compilation purge clears retained selection and rejects a held open result', async () => {
  let rejectOpen;
  const calls = [];
  const detail = compilationDetail();
  const controller = createCompilationController({ invoke: command => {
    calls.push(command);
    if (command === 'list_capabilities') return Promise.resolve({ outcome: 'listed', capabilities: [compilationSummary(detail)] });
    return new Promise((resolve, reject) => { rejectOpen = reject; });
  } });
  await controller.refresh();
  const opening = controller.open(compilationCase.capabilityId);
  controller.purgeForWorkspaceLock();
  const afterPurge = controller.getState();
  assert.deepEqual(afterPurge.capabilities, []);
  assert.equal(afterPurge.selected, null);
  assert.equal(afterPurge.pending, null);
  rejectOpen(new Error('late native failure'));
  await opening;
  assert.deepEqual(controller.getState().capabilities, []);
  assert.equal(controller.getState().selected, null);
  controller.purgeForWorkspaceLock();
  assert.equal(calls.length, 2);
});

test('workspace purge clears an in-flight restore preview and cannot revive its plan', async () => {
  let resolvePreview;
  const calls = [];
  const controller = createWorkspaceController({ invoke: command => {
    calls.push(command);
    if (command === 'get_workspace_data') return Promise.resolve({ outcome: 'loaded', workspace: workspace() });
    return new Promise(resolve => { resolvePreview = resolve; });
  } });
  await controller.load();
  const preview = controller.prepareRestore();
  controller.purgeForWorkspaceLock();
  assert.equal(controller.getState().workspace, null);
  assert.equal(controller.getState().restorePlan, null);
  assert.equal(controller.getState().dialog, null);
  resolvePreview({ outcome: 'restore_ready', plan: restorePlan() });
  await preview;
  assert.equal(controller.getState().restorePlan, null);
  assert.equal(controller.getState().pending, null);
  controller.purgeForWorkspaceLock();
  assert.deepEqual(calls, ['get_workspace_data', 'prepare_workspace_restore']);
});

test('workspace purge discards a late restore write before callbacks or refresh', async () => {
  let resolveRestore;
  const calls = [];
  const mutations = [];
  const controller = createWorkspaceController({
    invoke: command => {
      calls.push(command);
      if (command === 'get_workspace_data') return Promise.resolve({ outcome: 'loaded', workspace: workspace() });
      if (command === 'prepare_workspace_restore') return Promise.resolve({ outcome: 'restore_ready', plan: restorePlan() });
      return new Promise(resolve => { resolveRestore = resolve; });
    },
    onMutation: async event => mutations.push(event),
  });
  await controller.load();
  await controller.prepareRestore();
  const restoring = controller.confirmRestore();
  controller.purgeForWorkspaceLock();
  resolveRestore({ outcome: 'restored', plan: restorePlan() });
  await restoring;
  assert.deepEqual(mutations, []);
  assert.deepEqual(calls, ['get_workspace_data', 'prepare_workspace_restore', 'restore_workspace_backup']);
  assert.equal(controller.getState().workspace, null);
  assert.equal(controller.getState().pending, null);
});

test('model purge clears input inventory, discards a held check, and stops polling', async () => {
  let resolveCheck;
  const calls = [];
  const controller = createModelAssistanceController({ invoke: command => {
    calls.push(command);
    if (command === 'configure_local_model' || command === 'get_local_model_profile') return Promise.resolve(profileResult('configured', { session: session(true) }));
    if (command === 'check_local_model') return new Promise(resolve => { resolveCheck = resolve; });
    if (command === 'list_snapshots') return Promise.resolve({ outcome: 'listed', snapshots: [{ sourceId, displayName: 'rules.md', sha256: 'a'.repeat(64), byteLength: 8 }] });
    return Promise.resolve({ outcome: 'listed', capabilities: [] });
  } });
  await controller.load();
  await controller.configure(profile);
  const checking = controller.check();
  await new Promise(resolve => setImmediate(resolve));
  controller.purgeForWorkspaceLock();
  const callsAtPurge = calls.length;
  const state = controller.getState();
  assert.deepEqual(state.snapshots, []);
  assert.deepEqual(state.capabilities, []);
  assert.deepEqual(state.inputs, []);
  assert.equal(state.session, null);
  assert.equal(state.check, null);
  resolveCheck(profileResult('failed', { code: 'connection_failed' }));
  await checking;
  await new Promise(resolve => setTimeout(resolve, 300));
  assert.equal(calls.length, callsAtPurge);
  assert.equal(controller.getState().error, null);
  controller.purgeForWorkspaceLock();
});

test('credential purge invalidates a held edit and browser purges make no native calls', async () => {
  let resolveEdit;
  const calls = [];
  const controller = createCloudCredentialController({ invoke: command => {
    calls.push(command);
    return new Promise(resolve => { resolveEdit = resolve; });
  } });
  const edit = controller.edit();
  controller.purgeForWorkspaceLock();
  assert.equal(controller.getState().revision, null);
  assert.equal(controller.getState().pending, null);
  resolveEdit(custody('stored'));
  await edit;
  assert.equal(controller.getState().status, 'unknown');
  assert.equal(controller.getState().revision, null);
  controller.purgeForWorkspaceLock();
  assert.deepEqual(calls, ['edit_cloud_credential']);

  for (const Controller of [createCompilationController, createWorkspaceController, createModelAssistanceController, createCloudCredentialController]) {
    const browser = Controller();
    browser.purgeForWorkspaceLock();
    assert.equal(browser.getState().bridgeAvailable, false);
  }
});


test('restore and deletion receipt observers cannot publish a stale mutation callback or refresh', async () => {
  for (const action of ['restore', 'delete']) {
    const calls = [];
    const mutations = [];
    const deletion = { schemaVersion: 'rangoon.deletion-plan.v2', kind: 'workflow', id: id('workflow', '5'), label: 'Review workflow', expectedStateId: stateId, remove: counts({ sources: 0, capabilities: 0, revisions: 0, workflows: 1, workflowRevisions: 1 }), capabilityDependencies: [], workflowDependencies: [] };
    let controller;
    let purged = false;
    controller = createWorkspaceController({
      invoke: async command => {
        calls.push(command);
        if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
        if (command === 'prepare_workspace_restore') return { outcome: 'restore_ready', plan: restorePlan() };
        if (command === 'restore_workspace_backup') return { outcome: 'restored', plan: restorePlan() };
        if (command === 'inspect_workspace_deletion') return { outcome: 'deletion_ready', plan: deletion };
        if (command === 'delete_workspace_record') return { outcome: 'deleted', kind: deletion.kind, id: deletion.id };
        throw new Error('unexpected fixture operation');
      },
      onMutation: value => mutations.push(value),
      onChange: state => {
        if (!purged && state.pending === 'refresh') {
          purged = true;
          controller.purgeForWorkspaceLock();
        }
      },
    });
    await controller.load();
    if (action === 'restore') {
      await controller.prepareRestore();
      await controller.confirmRestore();
      assert.deepEqual(calls, ['get_workspace_data', 'prepare_workspace_restore', 'restore_workspace_backup']);
    } else {
      await controller.inspectDeletion('workflow', deletion.id, true);
      await controller.confirmDeletion();
      assert.deepEqual(calls, ['get_workspace_data', 'inspect_workspace_deletion', 'delete_workspace_record']);
    }
    assert.equal(purged, true);
    assert.deepEqual(mutations, []);
    assert.equal(controller.getState().workspace, null);
    assert.equal(controller.getState().pending, null);
  }
});


test('service pending observers purge before any native effect starts', async () => {
  for (const [Controller, start, pending] of [
    [createCompilationController, value => value.refresh(), 'refresh'],
    [createCompilationController, value => value.inspectBundle(), 'inspect-bundle'],
    [createWorkspaceController, value => value.load(), 'load'],
    [createWorkspaceController, value => value.prepareRestore(), 'restore-preview'],
    [createModelAssistanceController, value => value.load(), 'load'],
    [createModelAssistanceController, value => value.configure(profile), 'configure'],
    [createModelAssistanceController, value => value.check(), 'check'],
    [createCloudCredentialController, value => value.edit(), 'edit'],
  ]) {
    let controller;
    let purged = false;
    const calls = [];
    controller = Controller({ invoke: command => { calls.push(command); return Promise.resolve(null); }, onChange: state => {
      if (!purged && state.pending === pending) {
        purged = true;
        controller.purgeForWorkspaceLock();
      }
    } });
    await start(controller);
    assert.equal(purged, true, pending);
    assert.deepEqual(calls, [], pending);
    assert.equal(controller.getState().pending, null);
    controller.purgeForWorkspaceLock();
  }
});


test('draft-warning observer purge cannot revive an inspected deletion plan', async () => {
  let controller;
  let purged = false;
  const calls = [];
  const plan = { schemaVersion: 'rangoon.deletion-plan.v2', kind: 'capability', id: capabilityId, label: 'Bounded skill', expectedStateId: stateId, remove: counts({ sources: 0, workflows: 0, workflowRevisions: 0 }), capabilityDependencies: [], workflowDependencies: [] };
  controller = createWorkspaceController({
    invoke: async command => {
      calls.push(command);
      if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
      return { outcome: 'deletion_ready', plan };
    },
    getDraftWarning: () => { purged = true; controller.purgeForWorkspaceLock(); return 'private-draft-warning'; },
  });
  await controller.load();
  await controller.inspectDeletion('capability', capabilityId, true);
  assert.equal(purged, true);
  assert.equal(controller.getState().workspace, null);
  assert.equal(controller.getState().deletionPlan, null);
  assert.equal(controller.getState().dialog, null);
  assert.equal(JSON.stringify(controller.getState()).includes('private-draft-warning'), false);
  assert.deepEqual(calls, ['get_workspace_data', 'inspect_workspace_deletion']);
});
