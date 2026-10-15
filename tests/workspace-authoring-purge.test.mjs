import test from 'node:test';
import assert from 'node:assert/strict';
import { createAnalysisController } from '../preview/analysis-model.mjs';
import { createSkillsController } from '../preview/skills-model.mjs';
import { createCompositionController } from '../preview/composition-model.mjs';
import { createWorkflowController } from '../preview/workflow-model.mjs';

const hex = value => value.repeat(64);
const sourceId = `source:${hex('1')}`;
const fragmentId = `fragment:${hex('2')}`;
const capabilityId = `capability:${hex('3')}`;
const revisionId = `revision:${hex('4')}`;
const workspaceId = `workspace:${hex('5')}`;
const workflowId = `workflow:${hex('6')}`;
const workflowRevisionId = `workflow-revision:${hex('7')}`;
const workflowDraftId = `workflow-draft:${hex('8')}`;
const sha = hex('a');
const content = 'retained marker content\n';
const bytes = new TextEncoder();
const summary = revision => ({ id: revision.id, parentRevisionId: revision.parentRevisionId, title: revision.title, sha256: revision.sha256, createdAtMs: revision.createdAtMs, review: revision.review, provenance: revision.provenance });
const capabilityRevision = () => ({ id: revisionId, parentRevisionId: null, title: 'Retained marker skill', content, sha256: sha, createdAtMs: 1, review: null, provenance: { kind: 'ordinary' } });
const capabilityDetail = () => {
  const revision = capabilityRevision();
  return { schemaVersion: 'rangoon.capability.v1', id: capabilityId, origin: { kind: 'source', sourceId, fragmentId, sourceName: 'marker.md', span: { startByte: 0, endByte: bytes.encode(content).length, startLine: 1, endLine: 1 }, originalText: content }, latestRevisionId: revisionId, revision, history: [summary(revision)], authority: 'none' };
};
const capabilitySummary = () => ({ id: capabilityId, origin: { kind: 'source', sourceId, fragmentId }, latestRevisionId: revisionId, title: 'Retained marker skill', reviewed: false, revisionCount: 1 });
const analysisReport = () => ({ source: { id: sourceId, content: 'analysis retained marker' }, fragments: [{ id: fragmentId, text: 'analysis retained marker' }] });
const workspace = () => ({ schemaVersion: 'rangoon.workspace-data.v2', stateId: workspaceId, records: { sources: 1, capabilities: 1, revisions: 1, reviews: 0, recipes: 0, applications: 0, derivations: 0, workflows: 0, workflowRevisions: 0 }, databaseBytes: 1, reusableBytes: 0, sources: [{ sourceId, displayName: 'marker.md', sha256: sha, byteLength: 1, savedAtMs: 1 }], capabilities: [capabilitySummary()], workflows: [] });
const workflowSummary = () => ({ id: workflowId, latestRevisionId: workflowRevisionId, label: 'Retained marker workflow', labelAdjusted: false, intent: 'draft', structurallyValid: false, revisionCount: 1, savedAtMs: 1, unresolvedReferences: 0 });

test('analysis purge clears retained report and snapshots, then discards held rejection', async () => {
  let rejectChoose;
  const calls = [];
  const controller = createAnalysisController({ invoke(command) {
    calls.push(command);
    if (command === 'list_snapshots') return Promise.resolve({ outcome: 'listed', snapshots: [{ sourceId, marker: 'retained snapshot' }] });
    if (command === 'select_and_analyze') return new Promise((resolve, reject) => { rejectChoose = reject; });
    throw new Error(`unexpected ${command}`);
  } });
  await Promise.resolve();
  const pending = controller.choose();
  assert.equal(controller.getState().busyAction, 'choose');
  controller.purgeForWorkspaceLock();
  controller.purgeForWorkspaceLock();
  assert.deepEqual(controller.getState().snapshots, []);
  assert.equal(controller.getState().report, null);
  assert.equal(controller.getState().selectedFragmentId, null);
  assert.equal(controller.getState().busyAction, null);
  const before = calls.length;
  rejectChoose(new Error('held analysis marker'));
  await pending;
  assert.equal(calls.length, before);
  assert.equal(controller.getState().errorCode, null);
  assert.equal(controller.getState().report, null);
});

test('skills purge clears drafts, source cache, library state, and held save result', async () => {
  let resolveSave;
  const calls = [];
  const controller = createSkillsController({ invoke(command) {
    calls.push(command);
    if (command === 'list_capabilities') return Promise.resolve({ outcome: 'listed', capabilities: [capabilitySummary()] });
    if (command === 'create_capability' || command === 'open_capability') return Promise.resolve({ outcome: 'opened', capability: capabilityDetail(), alreadyApplied: false });
    if (command === 'revise_capability') return new Promise(resolve => { resolveSave = resolve; });
    throw new Error(`unexpected ${command}`);
  } });
  await controller.list();
  controller.setSourceContext({ report: { source: { id: sourceId }, selectedFragmentId: fragmentId, fragments: [{ id: fragmentId, heading: { title: 'Default title' }, span: { startByte: 0, endByte: 1, startLine: 1, endLine: 1 } }] }, snapshots: [{ sourceId }], snapshotsStatus: 'ready' });
  controller.updateCreateTitle('retained creation title');
  await controller.create('Retained marker skill');
  controller.updateDraft({ content: 'changed retained draft' });
  const saving = controller.save();
  assert.equal(controller.getState().pending, 'save');
  controller.purgeForWorkspaceLock();
  controller.purgeForWorkspaceLock();
  const cleared = controller.getState();
  assert.deepEqual(cleared.capabilities, []);
  assert.equal(cleared.selected, null);
  assert.equal(cleared.draft, null);
  assert.equal(cleared.source, null);
  assert.equal(cleared.createTitle, '');
  assert.equal(controller.hasUnsavedDrafts(), false);
  controller.setSourceContext({ report: { source: { id: sourceId }, fragments: [{ id: fragmentId, heading: { title: 'Default title' }, span: { startByte: 0, endByte: 1, startLine: 1, endLine: 1 } }] }, snapshots: [{ sourceId }], snapshotsStatus: 'ready' });
  assert.equal(controller.getState().createTitle, 'Default title');
  const before = calls.length;
  resolveSave({ outcome: 'failed', error: { code: 'marker_error', message: 'held skill marker' } });
  await saving;
  assert.equal(calls.length, before);
  assert.equal(controller.getState().error, null);
  assert.equal(controller.getState().selected, null);
});

function compositionPreview(draft, targets, inputs) {
  const mappings = [];
  const outputs = draft.outputs.map((output, outputIndex) => {
    let materialized = ''; let start = 0;
    output.pieces.forEach((piece, pieceIndex) => {
      const text = piece.kind === 'copy' ? inputs[piece.range.inputIndex].content.slice(piece.range.startByte, piece.range.endByte) : piece.content;
      mappings.push({ outputIndex, pieceIndex, outputStartByte: start, outputEndByte: start + bytes.encode(text).length, kind: piece.kind, inputRange: piece.kind === 'authored' ? null : piece.range });
      materialized += text;
      start += bytes.encode(text).length;
    });
    return { outputIndex, title: output.title, content: materialized, sha256: hex(`${9 - outputIndex}`), capabilityId: `capability:${hex(`${9 - outputIndex}`)}`, revisionId: `revision:${hex(`${9 - outputIndex}`)}` };
  });
  const coverage = mappings.map(mapping => ({ range: mapping.inputRange, disposition: 'copied', references: [{ outputIndex: mapping.outputIndex, pieceIndex: mapping.pieceIndex, kind: mapping.kind }], duplicationAcknowledged: false }));
  return { outcome: 'ready', schemaVersion: 'rangoon.composition-session.v0', previewId: `preview:${hex('b')}`, expectedStateId: workspaceId, preview: { schemaVersion: 'rangoon.composition-application-preview.v0', core: { schemaVersion: 'rangoon.composition-core-preview.v0', compositionId: `composition:${hex('c')}`, saveable: true, outputs, mappings, coverage, diagnostics: [], authority: 'none' }, applicationId: `composition-application:${hex('d')}`, appliedOutputs: targets.map((target, outputIndex) => ({ outputIndex, kind: target.kind, capabilityId: outputs[outputIndex].capabilityId, parentRevisionId: null, revisionId: outputs[outputIndex].revisionId })), saveable: true, authority: 'none' } };
}

test('composition purge clears inputs, draft, preview, history, and held save receipt', async () => {
  let resolveCommit; let preview; let committed = 0;
  const calls = [];
  const controller = createCompositionController({ operation: 'split', invoke: async (command, args) => {
    calls.push(command);
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'open_capability') return { outcome: 'opened', capability: capabilityDetail() };
    if (command === 'preview_composition') {
      const request = JSON.parse(new TextDecoder().decode(args));
      preview = compositionPreview(request.draft, request.targets, [{ content }]);
      return preview;
    }
    if (command === 'commit_composition') return new Promise(resolve => { resolveCommit = resolve; });
    throw new Error(`unexpected ${command}`);
  }, onCommitted: () => { committed += 1; } });
  assert.equal(await controller.load(), true);
  assert.equal(await controller.addRevision(capabilityId), true);
  assert.equal(controller.startDraft(), true);
  assert.equal(controller.addAuthored(0, 'retained edit', 'retained reason'), true);
  assert.equal(controller.undo(), true);
  assert.equal(await controller.requestPreview(), true);
  assert.equal(controller.acknowledge(true), true);
  const saving = controller.save();
  assert.equal(controller.getState().pending, 'save');
  controller.purgeForWorkspaceLock();
  controller.purgeForWorkspaceLock();
  const cleared = controller.getState();
  assert.deepEqual(cleared.sources, []);
  assert.deepEqual(cleared.capabilities, []);
  assert.deepEqual(cleared.inputs, []);
  assert.equal(cleared.draft, null);
  assert.equal(cleared.preview, null);
  assert.equal(cleared.results.length, 0);
  assert.equal(cleared.canUndo, false);
  assert.equal(cleared.canRedo, false);
  const before = calls.length;
  resolveCommit({ outcome: 'committed', receipt: { marker: 'held receipt' } });
  assert.equal(await saving, false);
  assert.equal(calls.length, before);
  assert.equal(committed, 0);
  assert.equal(controller.getState().error, null);
  assert.equal(controller.getState().preview, null);
});

test('workflow purge clears library, authored draft, undo state, and held inspection error', async () => {
  let resolveInspect;
  const calls = [];
  const controller = createWorkflowController({ bridge: async (command) => {
    calls.push(command);
    if (command === 'list_workflows') return { outcome: 'listed', workflows: [workflowSummary()] };
    if (command === 'list_capabilities') return { outcome: 'listed', capabilities: [capabilitySummary()] };
    if (command === 'begin_workflow_draft') return { outcome: 'draft_ready', schemaVersion: 'rangoon.workflow-draft-session.v1', draftId: workflowDraftId, workflowId, parentRevisionId: null, workflow: null };
    if (command === 'inspect_workflow_save') return new Promise(resolve => { resolveInspect = resolve; });
    throw new Error(`unexpected ${command}`);
  } });
  assert.equal(await controller.load(), true);
  assert.equal(await controller.beginNew(), true);
  assert.equal(controller.addNode('input', { x: 2, y: 3 }), true);
  assert.equal(controller.undo(), true);
  assert.equal(controller.redo(), true);
  controller.setViewport({ x: 3, y: 4, zoom: 1.2 });
  const inspecting = controller.inspect('draft');
  assert.equal(controller.getState().pending, 'inspect');
  controller.purgeForWorkspaceLock();
  controller.purgeForWorkspaceLock();
  const cleared = controller.getState();
  assert.deepEqual(cleared.workflows, []);
  assert.deepEqual(cleared.capabilities, []);
  assert.equal(cleared.opened, null);
  assert.equal(cleared.session, null);
  assert.equal(cleared.definition, null);
  assert.equal(cleared.selection, null);
  assert.equal(cleared.fieldEdit, null);
  assert.equal(cleared.canUndo, false);
  assert.equal(cleared.canRedo, false);
  const before = calls.length;
  resolveInspect({ outcome: 'failed', error: { code: 'marker_error', message: 'held workflow marker' } });
  assert.equal(await inspecting, false);
  assert.equal(calls.length, before);
  assert.equal(controller.getState().definition, null);
  assert.equal(controller.getState().candidate, null);
});

test('no bridge controllers keep unavailable state across repeated purge without invoking', () => {
  const controllers = [
    createAnalysisController(),
    createSkillsController(),
    createCompositionController({ operation: 'split' }),
    createWorkflowController(),
  ];
  for (const controller of controllers) {
    controller.purgeForWorkspaceLock();
    controller.purgeForWorkspaceLock();
    const state = controller.getState();
    assert.equal(state.bridgeAvailable ?? false, false);
    assert.equal(state.status ?? state.listStatus, 'unavailable');
    if (state.operation) assert.equal(state.operation, 'split');
  }
});

test('reentrant observer purge prevents every authoring controller native invoke', async () => {
  let purgeCount = 0;
  let purgeNext = false;
  let analysis;
  const analysisCalls = [];
  analysis = createAnalysisController({
    invoke(command) {
      analysisCalls.push(command);
      if (command === 'list_snapshots') return Promise.resolve({ outcome: 'listed', snapshots: [] });
      throw new Error(`unexpected ${command}`);
    },
    onChange() { if (purgeNext) { purgeNext = false; purgeCount += 1; analysis.purgeForWorkspaceLock(); } },
  });
  await Promise.resolve();
  analysisCalls.length = 0;
  purgeNext = true;
  await analysis.choose();
  assert.deepEqual(analysisCalls, []);
  assert.equal(analysis.getState().report, null);
  assert.equal(analysis.getState().busyAction, null);

  let skills;
  const skillCalls = [];
  skills = createSkillsController({
    invoke(command) { skillCalls.push(command); throw new Error(`unexpected ${command}`); },
    onChange() { if (purgeNext) { purgeNext = false; purgeCount += 1; skills.purgeForWorkspaceLock(); } },
  });
  skills.setSourceContext({ report: { source: { id: sourceId }, fragments: [{ id: fragmentId, heading: { title: 'Marker' }, span: { startByte: 0, endByte: 1, startLine: 1, endLine: 1 } }] }, snapshots: [{ sourceId }], snapshotsStatus: 'ready' });
  purgeNext = true;
  await skills.create('Retained marker skill');
  assert.deepEqual(skillCalls, []);
  assert.equal(skills.getState().source, null);
  assert.equal(skills.getState().pending, null);

  let composition;
  const compositionCalls = [];
  composition = createCompositionController({
    operation: 'split',
    invoke: async (command, args) => {
      compositionCalls.push(command);
      if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
      if (command === 'open_capability') return { outcome: 'opened', capability: capabilityDetail() };
      if (command === 'preview_composition') {
        const request = JSON.parse(new TextDecoder().decode(args));
        return compositionPreview(request.draft, request.targets, [{ content }]);
      }
      throw new Error(`unexpected ${command}`);
    },
    onChange() { if (purgeNext) { purgeNext = false; purgeCount += 1; composition.purgeForWorkspaceLock(); } },
  });
  await composition.load();
  await composition.addRevision(capabilityId);
  composition.startDraft();
  await composition.requestPreview();
  composition.acknowledge(true);
  compositionCalls.length = 0;
  purgeNext = true;
  assert.equal(await composition.save(), false);
  assert.deepEqual(compositionCalls, []);
  assert.equal(composition.getState().preview, null);
  assert.equal(composition.getState().pending, null);

  let workflow;
  const workflowCalls = [];
  workflow = createWorkflowController({
    bridge: async command => { workflowCalls.push(command); throw new Error(`unexpected ${command}`); },
    onChange() { if (purgeNext) { purgeNext = false; purgeCount += 1; workflow.purgeForWorkspaceLock(); } },
  });
  purgeNext = true;
  assert.equal(await workflow.beginNew(), false);
  assert.deepEqual(workflowCalls, []);
  assert.equal(workflow.getState().session, null);
  assert.equal(workflow.getState().pending, null);
  assert.equal(purgeCount, 4);
});
