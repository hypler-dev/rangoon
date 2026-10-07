import test from 'node:test';
import assert from 'node:assert/strict';
import { createWorkflowController } from '../preview/workflow-model.mjs';
import { validateWorkflowDetail, validateWorkflowSaveReady } from '../preview/workflow-contract.mjs';

const hex = char => char.repeat(64);
const workflowId = `workflow:${hex('1')}`;
const draftId = `workflow-draft:${hex('2')}`;
const previewId = `workflow-preview:${hex('3')}`;
const revisionId = `workflow-revision:${hex('4')}`;
const stateId = `workspace:${hex('5')}`;
const capabilityId = `capability:${hex('6')}`;
const capabilityRevisionId = `revision:${hex('7')}`;
const historicalCapabilityRevisionId = `revision:${hex('a')}`;
const childRevisionId = `workflow-revision:${hex('8')}`;
const historicalRevisionId = `workflow-revision:${hex('9')}`;
const definition = title => ({ schemaVersion: 'rangoon.workflow-definition.v1', title, nodes: [
  { id: 'input', title: 'Input', operation: { kind: 'input' }, inputs: [], outputs: [{ name: 'text', dataType: 'text' }] },
  { id: 'output', title: 'Output', operation: { kind: 'output' }, inputs: [{ name: 'text', dataType: 'text' }], outputs: [] },
], controlEdges: [{ fromNode: 'input', outlet: 'next', toNode: 'output' }], dataEdges: [{ fromNode: 'input', fromPort: 'text', toNode: 'output', toPort: 'text' }] });
const layout = (count = 2) => ({ positions: Array.from({ length: count }, (_, nodeIndex) => ({ nodeIndex, x: nodeIndex * 300, y: 0 })) });
function revision(def = definition('Local workflow')) { return { schemaVersion: 'rangoon.workflow-revision.v1', id: revisionId, workflowId, parentRevisionId: null, intent: 'draft', definition: def, layout: layout(def.nodes.length) }; }
function detail(def = definition('Local workflow')) {
  const rev = revision(def); const summary = { id: workflowId, latestRevisionId: revisionId, label: def.title || 'Untitled workflow', labelAdjusted: false, intent: 'draft', structurallyValid: false, revisionCount: 1, savedAtMs: 1, unresolvedReferences: 0 };
  return { schemaVersion: 'rangoon.workflow-detail.v1', head: summary, revision: rev, savedAtMs: 1, history: [{ id: revisionId, parentRevisionId: null, intent: 'draft', structurallyValid: false, savedAtMs: 1 }], references: [] };
}
function report(valid = false, definitionValue = definition('Local workflow')) { return { schemaVersion: 'rangoon.workflow-validation.v1', structurallyValid: valid, definitionId: valid ? `workflow-definition:${hex('8')}` : null, diagnostics: valid ? [] : [{ code: 'invalid_node_count', nodeIndex: null, controlEdgeIndex: null, dataEdgeIndex: null, portIndex: null }], diagnosticsTruncated: false, topologicalOrder: valid ? definitionValue.nodes.map(node => node.id) : [], dependencies: [], referenceStatus: 'unverified', executionStatus: 'unavailable', authority: 'none' }; }
function saveReady(request, valid = false) {
  const rev = revision(request.definition); rev.parentRevisionId = null; rev.intent = request.intent; rev.layout = structuredClone(request.layout);
  return { outcome: 'save_ready', schemaVersion: 'rangoon.workflow-save-session.v1', draftId, previewId, revision: rev, report: report(valid, request.definition), plan: { schemaVersion: 'rangoon.workflow-save-plan.v1', workflowId, revisionId, expectedStateId: stateId, expectedHeadId: null, alreadySaved: false, unresolvedReferences: 0 } };
}
function receipt(candidate) { const saved = detail(candidate.revision.definition); saved.revision = structuredClone(candidate.revision); saved.head = { ...saved.head, latestRevisionId: candidate.revision.id, intent: candidate.revision.intent, structurallyValid: candidate.report.structurallyValid, unresolvedReferences: candidate.plan.unresolvedReferences }; saved.history = [{ id: candidate.revision.id, parentRevisionId: candidate.revision.parentRevisionId, intent: candidate.revision.intent, structurallyValid: candidate.report.structurallyValid, savedAtMs: 1 }]; return { schemaVersion: 'rangoon.workflow-save-receipt.v1', workflow: saved, alreadySaved: false, stateId }; }
function capabilityDetail() {
  const root = { id: historicalCapabilityRevisionId, parentRevisionId: null, title: 'Pinned capability', sha256: hex('b'), createdAtMs: 1, review: null, provenance: { kind: 'ordinary' } };
  const current = { id: capabilityRevisionId, parentRevisionId: historicalCapabilityRevisionId, title: 'Pinned capability', sha256: hex('c'), createdAtMs: 2, review: null, provenance: { kind: 'ordinary' } };
  return {
    schemaVersion: 'rangoon.capability.v1', id: capabilityId,
    origin: { kind: 'source', sourceId: `source:${hex('d')}`, fragmentId: `fragment:${hex('e')}`, sourceName: 'rules.md', span: { startByte: 0, endByte: 1, startLine: 1, endLine: 1 }, originalText: 'x' },
    latestRevisionId: capabilityRevisionId,
    revision: { ...current, content: 'Use this local capability.' },
    history: [root, current], authority: 'none',
  };
}
function bridge(overrides = {}) {
  const calls = []; let lastCandidate;
  const invoke = async (command, args) => {
    calls.push([command, args]);
    if (command === 'list_workflows') return { outcome: 'listed', workflows: [] };
    if (command === 'list_capabilities') return { outcome: 'listed', capabilities: [] };
    if (command === 'begin_workflow_draft') return { outcome: 'draft_ready', schemaVersion: 'rangoon.workflow-draft-session.v1', draftId, workflowId, parentRevisionId: null, workflow: null };
    if (command === 'inspect_workflow_save') { const request = JSON.parse(new TextDecoder().decode(args)); lastCandidate = saveReady(request, request.intent === 'validated'); return lastCandidate; }
    if (command === 'commit_workflow_save') return { outcome: 'saved', receipt: receipt(lastCandidate) };
    if (command === 'clear_workflow_draft') return { outcome: 'cleared' };
    if (command === 'open_workflow') return { outcome: 'opened', workflow: detail() };
    if (command === 'open_capability') return { outcome: 'opened', capability: capabilityDetail(), alreadyApplied: false };
    return { outcome: 'failed', error: { code: 'unexpected', message: 'Unexpected command.' } };
  };
  return { calls, invoke: overrides.invoke ?? invoke, candidate: () => lastCandidate };
}

test('state starts with the frozen public shape and unavailable bridge state', () => {
  const state = createWorkflowController().getState();
  assert.deepEqual(Object.keys(state), Object.keys(createWorkflowController().getState()));
  assert.equal(state.bridgeAvailable, false); assert.equal(state.definition, null); assert.equal(state.session, null);
});

test('native lifecycle uses raw UTF-8 requests and accepts an exact draft receipt once', async () => {
  const native = bridge(); const model = createWorkflowController({ bridge: native.invoke });
  assert.ok(await model.load()); assert.ok(await model.beginNew());
  assert.ok(model.addNode('input', { x: 0, y: 0 })); assert.ok(model.addNode('output', { x: 300, y: 0 }));
  assert.ok(model.connectControl(0, 'next', 1)); assert.ok(model.connectData(0, 'text', 1, 'text'));
  assert.ok(await model.inspect('draft')); assert.ok(model.acknowledge(true)); assert.ok(await model.commit());
  const inspect = native.calls.find(([command]) => command === 'inspect_workflow_save'); const commit = native.calls.find(([command]) => command === 'commit_workflow_save');
  assert.ok(inspect[1] instanceof Uint8Array); assert.ok(commit[1] instanceof Uint8Array); assert.equal(model.getState().receipt.stateId, stateId);
});

test('graph decoder retains an incomplete capability as a draft and native report decides candidate status', async () => {
  const native = bridge(); const model = createWorkflowController({ bridge: native.invoke }); await model.beginNew();
  assert.ok(model.addNode('capability', { x: 1, y: 2 })); assert.equal(model.getState().definition.nodes[0].operation.capabilityId, '');
  assert.ok(await model.inspect('draft')); assert.equal(model.getState().status, 'draft');
});

test('malformed candidate response fails closed without replacing local authored graph', async () => {
  const native = bridge({ invoke: async (command, args) => command === 'begin_workflow_draft' ? { outcome: 'draft_ready', schemaVersion: 'rangoon.workflow-draft-session.v1', draftId, workflowId, parentRevisionId: null, workflow: null } : command === 'inspect_workflow_save' ? { ...saveReady(JSON.parse(new TextDecoder().decode(args))), plan: { bad: true } } : { outcome: 'listed', workflows: [], capabilities: [] } });
  const model = createWorkflowController({ bridge: native.invoke }); await model.beginNew(); model.addNode('input', { x: 0, y: 0 }); const before = model.getState().definition;
  assert.equal(await model.inspect('draft'), false); assert.deepEqual(model.getState().definition, before); assert.equal(model.getState().candidate, null);
});

test('edge refusal does not replace occupied data mapping and remove node remaps layout', async () => {
  const model = createWorkflowController({ bridge: bridge().invoke }); await model.beginNew();
  for (const kind of ['input', 'input', 'output']) assert.ok(model.addNode(kind, { x: 10, y: 10 }));
  assert.ok(model.connectData(0, 'text', 2, 'text')); assert.equal(model.connectData(1, 'text', 2, 'text'), false);
  assert.ok(model.removeNode(1, true)); assert.deepEqual(model.getState().layout.positions.map(item => item.nodeIndex), [0, 1]);
});

test('pending field text survives rerenders, cancel, and a failed native inspection', async () => {
  const native = bridge({ invoke: async (command, args) => {
    if (command === 'begin_workflow_draft') return { outcome: 'draft_ready', schemaVersion: 'rangoon.workflow-draft-session.v1', draftId, workflowId, parentRevisionId: null, workflow: null };
    if (command === 'inspect_workflow_save') return { outcome: 'failed', error: { message: 'Inspection unavailable.' } };
    return { outcome: 'listed', workflows: [], capabilities: [] };
  } });
  const model = createWorkflowController({ bridge: native.invoke }); await model.beginNew(); model.addNode('input', { x: 0, y: 0 });
  assert.ok(model.editFields({ kind: 'node', index: 0 }, { title: '' })); assert.equal(model.getState().fieldEdit.patch.title, '');
  assert.ok(model.cancelFields()); assert.equal(model.getState().definition.nodes[0].title, 'Input');
  assert.ok(model.editFields({ kind: 'node', index: 0 }, { title: '' })); assert.ok(model.applyFields());
  assert.equal(await model.inspect('draft'), false); assert.equal(model.getState().definition.nodes[0].title, '');
});

test('undo is bounded, restores layout, and new edits clear redo', async () => {
  const model = createWorkflowController({ bridge: bridge().invoke }); await model.beginNew();
  assert.ok(model.addNode('input', { x: 5, y: 6 })); assert.ok(model.moveNode(0, 8, 9)); assert.ok(model.undo());
  assert.deepEqual(model.getState().layout.positions[0], { nodeIndex: 0, x: 5, y: 6 }); assert.ok(model.redo());
  for (let index = 0; index < 51; index += 1) model.moveNode(0, index, 0);
  let count = 0; while (model.undo()) count += 1; assert.equal(count, 50);
});

test('pending commit locks edits, consumes candidate on failed result, and keeps draft', async () => {
  let resolveCommit; const native = bridge({ invoke: async (command, args) => {
    if (command === 'begin_workflow_draft') return { outcome: 'draft_ready', schemaVersion: 'rangoon.workflow-draft-session.v1', draftId, workflowId, parentRevisionId: null, workflow: null };
    if (command === 'inspect_workflow_save') return saveReady(JSON.parse(new TextDecoder().decode(args)));
    if (command === 'commit_workflow_save') return new Promise(resolve => { resolveCommit = () => resolve({ outcome: 'failed', error: { message: 'Unknown result.' } }); });
    return { outcome: 'listed', workflows: [], capabilities: [] };
  } });
  const model = createWorkflowController({ bridge: native.invoke }); await model.beginNew(); model.addNode('input', { x: 0, y: 0 }); await model.inspect('draft'); model.acknowledge(true);
  const saving = model.commit(); assert.equal(model.addNode('output', { x: 0, y: 0 }), false); resolveCommit(); assert.equal(await saving, false);
  assert.ok(model.getState().definition); assert.equal(model.getState().candidate, null); assert.equal(await model.commit(), false);
});

test('late inspect response cannot replace an invalidated candidate', async () => {
  let respond; const native = bridge({ invoke: async (command, args) => {
    if (command === 'begin_workflow_draft') return { outcome: 'draft_ready', schemaVersion: 'rangoon.workflow-draft-session.v1', draftId, workflowId, parentRevisionId: null, workflow: null };
    if (command === 'inspect_workflow_save') return new Promise(resolve => { respond = () => resolve(saveReady(JSON.parse(new TextDecoder().decode(args)))); });
    return { outcome: 'listed', workflows: [], capabilities: [] };
  } });
  const model = createWorkflowController({ bridge: native.invoke }); await model.beginNew(); model.addNode('input', { x: 0, y: 0 }); const pending = model.inspect('draft'); model.invalidate('Workspace changed.'); respond(); assert.equal(await pending, false); assert.equal(model.getState().candidate, null);
});

test('contract rejects missing history and forged candidate relationships', () => {
  const good = detail(); assert.ok(validateWorkflowDetail(good)); const noHistory = structuredClone(good); noHistory.history = []; assert.equal(validateWorkflowDetail(noHistory), null);
  const request = { definition: definition('Local workflow'), layout: layout(), intent: 'draft' }; const candidate = saveReady(request); assert.ok(validateWorkflowSaveReady(candidate, { draftId, workflowId, parentRevisionId: null, definition: request.definition, layout: layout(), intent: 'draft' })); candidate.plan.revisionId = `workflow-revision:${hex('f')}`; assert.equal(validateWorkflowSaveReady(candidate, { draftId, workflowId, parentRevisionId: null, definition: request.definition, layout: layout(), intent: 'draft' }), null);
});

test('definition decoder accepts bounded invalid Draft strings while native envelopes stay exact', () => {
  const long = 'x'.repeat(8_000);
  const incomplete = {
    schemaVersion: 'rangoon.workflow-definition.v1',
    title: long,
    nodes: [{ id: '', title: long, operation: { kind: 'capability', capabilityId: long, revisionId: long }, inputs: [{ name: '', dataType: 'text' }], outputs: [{ name: '', dataType: 'text' }] }],
    controlEdges: [], dataEdges: [],
  };
  const opened = detail(incomplete);
  opened.head.label = 'Saved workflow label';
  opened.head.unresolvedReferences = 1;
  opened.references = [{ nodeIndex: 0, capabilityId: long, revisionId: long, status: 'invalid_reference' }];
  assert.ok(validateWorkflowDetail(opened));
  const request = { definition: incomplete, layout: layout(1), intent: 'draft' };
  const candidate = saveReady(request);
  assert.ok(validateWorkflowSaveReady(candidate, { draftId, workflowId, parentRevisionId: null, ...request }));
  const extraPort = structuredClone(incomplete);
  extraPort.nodes[0].inputs[0].extra = true;
  const extraDetail = detail(extraPort);
  extraDetail.head.label = 'Saved workflow label';
  extraDetail.head.unresolvedReferences = 1;
  extraDetail.references = opened.references;
  assert.equal(validateWorkflowDetail(extraDetail), null);
  const missingTitle = structuredClone(opened);
  delete missingTitle.revision.definition.title;
  assert.equal(validateWorkflowDetail(missingTitle), null);
  const extraCandidate = structuredClone(candidate);
  extraCandidate.untrusted = true;
  assert.equal(validateWorkflowSaveReady(extraCandidate, { draftId, workflowId, parentRevisionId: null, ...request }), null);
});

test('detail validator requires an ordered exact history chain and current-head coherence', () => {
  const chained = detail();
  chained.revision.id = childRevisionId;
  chained.revision.parentRevisionId = revisionId;
  chained.head.latestRevisionId = childRevisionId;
  chained.head.revisionCount = 2;
  chained.head.savedAtMs = 2;
  chained.savedAtMs = 2;
  chained.history = [
    { id: revisionId, parentRevisionId: null, intent: 'draft', structurallyValid: false, savedAtMs: 1 },
    { id: childRevisionId, parentRevisionId: revisionId, intent: 'draft', structurallyValid: false, savedAtMs: 2 },
  ];
  assert.ok(validateWorkflowDetail(chained));
  const reordered = structuredClone(chained);
  reordered.history.reverse();
  assert.equal(validateWorkflowDetail(reordered), null);
  const brokenParent = structuredClone(chained);
  brokenParent.history[1].parentRevisionId = null;
  assert.equal(validateWorkflowDetail(brokenParent), null);
  const summaryDrift = structuredClone(chained);
  summaryDrift.head.savedAtMs = 3;
  assert.equal(validateWorkflowDetail(summaryDrift), null);
  const wrongCount = structuredClone(chained);
  wrongCount.head.revisionCount = 1;
  assert.equal(validateWorkflowDetail(wrongCount), null);
  const invalidValidated = structuredClone(chained);
  invalidValidated.history[1].intent = 'validated';
  assert.equal(validateWorkflowDetail(invalidValidated), null);
});

test('append rejects a forged historical response and keeps native node labels unique', async () => {
  const existing = definition('Saved workflow');
  existing.nodes = [{ id: 'node_1', title: 'Input', operation: { kind: 'input' }, inputs: [], outputs: [{ name: 'text', dataType: 'text' }] }];
  existing.controlEdges = [];
  existing.dataEdges = [];
  const current = detail(existing);
  current.revision.layout = layout(1);
  const historical = structuredClone(current);
  historical.revision.id = historicalRevisionId;
  historical.revision.parentRevisionId = null;
  historical.savedAtMs = 1;
  historical.history = [
    { id: historicalRevisionId, parentRevisionId: null, intent: 'draft', structurallyValid: false, savedAtMs: 1 },
    { id: revisionId, parentRevisionId: historicalRevisionId, intent: 'draft', structurallyValid: false, savedAtMs: 2 },
  ];
  historical.head.latestRevisionId = revisionId;
  historical.head.revisionCount = 2;
  historical.head.savedAtMs = 2;
  const forgedOpen = createWorkflowController({ bridge: async command => command === 'open_workflow' ? { outcome: 'opened', workflow: historical } : { outcome: 'listed', workflows: [], capabilities: [] } });
  assert.equal(await forgedOpen.open(workflowId), false);
  const invoke = async (command, args) => {
    if (command === 'open_workflow') return { outcome: 'opened', workflow: current };
    if (command === 'begin_workflow_draft') return { outcome: 'draft_ready', schemaVersion: 'rangoon.workflow-draft-session.v1', draftId, workflowId, parentRevisionId: revisionId, workflow: historical };
    return { outcome: 'listed', workflows: [], capabilities: [] };
  };
  const model = createWorkflowController({ bridge: invoke });
  assert.ok(await model.open(workflowId));
  assert.equal(await model.beginAppend(), false);
  assert.equal(model.getState().session, null);

  const currentInvoke = async (command, args) => {
    if (command === 'open_workflow') return { outcome: 'opened', workflow: current };
    if (command === 'begin_workflow_draft') return { outcome: 'draft_ready', schemaVersion: 'rangoon.workflow-draft-session.v1', draftId, workflowId, parentRevisionId: revisionId, workflow: current };
    return { outcome: 'listed', workflows: [], capabilities: [] };
  };
  const currentModel = createWorkflowController({ bridge: currentInvoke });
  assert.ok(await currentModel.open(workflowId));
  assert.ok(await currentModel.beginAppend());
  assert.ok(currentModel.addNode('input', { x: 1, y: 1 }));
  const added = currentModel.getState().definition.nodes.at(-1);
  assert.equal(added.id, 'node_2');
  assert.equal(added.title, 'Input 2');

  const wrongCompare = createWorkflowController({ bridge: async command => {
    if (command === 'open_workflow') return { outcome: 'opened', workflow: current };
    return { outcome: 'listed', workflows: [], capabilities: [] };
  } });
  assert.ok(await wrongCompare.open(workflowId));
  assert.equal(await wrongCompare.compare(revisionId, childRevisionId), false);
});

test('valid Draft inspection remains Draft and gates validated inspection on the same generation', async () => {
  const native = bridge({ invoke: async (command, args) => {
    if (command === 'begin_workflow_draft') return { outcome: 'draft_ready', schemaVersion: 'rangoon.workflow-draft-session.v1', draftId, workflowId, parentRevisionId: null, workflow: null };
    if (command === 'inspect_workflow_save') return saveReady(JSON.parse(new TextDecoder().decode(args)), true);
    return { outcome: 'listed', workflows: [], capabilities: [] };
  } });
  const model = createWorkflowController({ bridge: native.invoke });
  await model.beginNew();
  model.addNode('input', { x: 0, y: 0 });
  model.addNode('output', { x: 1, y: 0 });
  model.connectControl(0, 'next', 1);
  model.connectData(0, 'text', 1, 'text');
  assert.equal(await model.inspect('validated'), false);
  assert.ok(await model.inspect('draft'));
  assert.equal(model.getState().status, 'draft');
  assert.equal(model.getState().candidate.revision.intent, 'draft');
  assert.equal(model.getState().candidate.report.structurallyValid, true);
  assert.ok(await model.inspect('validated'));
  assert.equal(model.getState().status, 'validated');
  assert.equal(model.getState().candidate.revision.intent, 'validated');
});

test('same field target merges pending text while cross-target actions remain locked', async () => {
  const model = createWorkflowController({ bridge: bridge().invoke });
  await model.beginNew();
  model.addNode('input', { x: 0, y: 0 });
  assert.ok(model.editFields({ kind: 'node', index: 0 }, { title: '' }));
  assert.ok(model.editFields({ kind: 'node', index: 0 }, { id: '' }));
  assert.deepEqual(model.getState().fieldEdit.patch, { title: '', id: '' });
  assert.equal(model.editFields({ kind: 'workflow' }, { title: 'Other' }), false);
  assert.equal(model.select({ kind: 'node', index: 0 }), false);
  assert.equal(await model.load(), false);
  assert.ok(model.applyFields());
  assert.equal(model.getState().definition.nodes[0].id, '');
});

test('capability selection requires a strict native detail and permits its historical exact pins', async () => {
  const native = bridge();
  const model = createWorkflowController({ bridge: native.invoke });
  await model.beginNew();
  model.addNode('capability', { x: 0, y: 0 });
  assert.ok(await model.chooseCapability(capabilityId));
  assert.ok(model.pinCapability(0, capabilityId, historicalCapabilityRevisionId));
  assert.equal(model.getState().definition.nodes[0].operation.revisionId, historicalCapabilityRevisionId);
  const hostile = capabilityDetail();
  hostile.origin.extra = true;
  const bad = createWorkflowController({ bridge: async command => command === 'open_capability' ? { outcome: 'opened', capability: hostile, alreadyApplied: false } : { outcome: 'listed', workflows: [], capabilities: [] } });
  await bad.beginNew();
  assert.equal(await bad.chooseCapability(capabilityId), false);
});

test('invalidation retains the pending lock until its callback returns and preserves local state', async () => {
  let respond;
  const native = bridge({ invoke: async (command, args) => {
    if (command === 'begin_workflow_draft') return { outcome: 'draft_ready', schemaVersion: 'rangoon.workflow-draft-session.v1', draftId, workflowId, parentRevisionId: null, workflow: null };
    if (command === 'inspect_workflow_save') return new Promise(resolve => { respond = () => resolve(saveReady(JSON.parse(new TextDecoder().decode(args)))); });
    return { outcome: 'listed', workflows: [], capabilities: [] };
  } });
  const model = createWorkflowController({ bridge: native.invoke });
  await model.beginNew();
  model.addNode('input', { x: 2, y: 3 });
  model.select({ kind: 'node', index: 0 });
  model.setViewport({ x: 3, y: 4, zoom: 1.5 });
  const inspect = model.inspect('draft');
  model.invalidate('Workspace changed.');
  assert.equal(model.getState().pending, 'inspect');
  assert.equal(model.addNode('output', { x: 0, y: 0 }), false);
  assert.deepEqual(model.getState().selection, { kind: 'node', index: 0 });
  assert.deepEqual(model.getState().viewport, { x: 3, y: 4, zoom: 1.5 });
  respond();
  assert.equal(await inspect, false);
  assert.equal(model.getState().pending, null);
  assert.equal(model.getState().candidate, null);
  assert.equal(model.getState().definition.nodes.length, 1);
});

test('stale begin quarantines its native session and clean sessions block open until explicit clear', async () => {
  let respond;
  const delayed = createWorkflowController({ bridge: async command => {
    if (command === 'begin_workflow_draft') return new Promise(resolve => { respond = () => resolve({ outcome: 'draft_ready', schemaVersion: 'rangoon.workflow-draft-session.v1', draftId, workflowId, parentRevisionId: null, workflow: null }); });
    return { outcome: 'listed', workflows: [] };
  } });
  const beginning = delayed.beginNew();
  delayed.invalidate('Workspace changed.');
  assert.equal(delayed.getState().pending, 'begin');
  assert.equal(delayed.getState().session, null);
  respond();
  assert.equal(await beginning, false);
  assert.equal(delayed.getState().pending, null);
  assert.equal(delayed.getState().session, null);

  const clean = createWorkflowController({ bridge: bridge().invoke });
  await clean.beginNew();
  assert.equal(await clean.open(workflowId), false);
  assert.ok(await clean.clear(true));
});

test('saved receipt survives false and thrown refresh callbacks', async () => {
  for (const onSaved of [async () => false, async () => { throw new Error('refresh unavailable'); }]) {
    const model = createWorkflowController({ bridge: bridge().invoke, onSaved });
    await model.beginNew();
    model.addNode('input', { x: 0, y: 0 });
    model.addNode('output', { x: 1, y: 0 });
    model.connectControl(0, 'next', 1);
    model.connectData(0, 'text', 1, 'text');
    assert.ok(await model.inspect('draft'));
    assert.ok(model.acknowledge(true));
    assert.ok(await model.commit());
    assert.equal(model.getState().receipt.stateId, stateId);
    assert.match(model.getState().message, /refresh did not finish/i);
  }
});

test('closed outer native success envelopes reject extras and duplicate inventory ids', async () => {
  const listed = createWorkflowController({ bridge: async command => {
    if (command === 'list_workflows') return { outcome: 'listed', workflows: [], extra: true };
    return { outcome: 'listed', capabilities: [] };
  } });
  assert.equal(await listed.load(), false);

  const duplicate = detail().head;
  const duplicated = createWorkflowController({ bridge: async command => command === 'list_workflows'
    ? { outcome: 'listed', workflows: [duplicate, structuredClone(duplicate)] }
    : { outcome: 'listed', capabilities: [] } });
  assert.equal(await duplicated.load(), false);

  const summary = { id: capabilityId, origin: { kind: 'source', sourceId: `source:${hex('d')}`, fragmentId: `fragment:${hex('e')}` }, latestRevisionId: capabilityRevisionId, title: 'Pinned capability', reviewed: false, revisionCount: 1 };
  const duplicateCapabilities = createWorkflowController({ bridge: async command => command === 'list_capabilities'
    ? { outcome: 'listed', capabilities: [summary, structuredClone(summary)] }
    : { outcome: 'listed', workflows: [] } });
  assert.equal(await duplicateCapabilities.load(), false);

  const outerOpen = createWorkflowController({ bridge: async command => command === 'open_workflow'
    ? { outcome: 'opened', workflow: detail(), extra: true }
    : { outcome: 'listed', workflows: [], capabilities: [] } });
  assert.equal(await outerOpen.open(workflowId), false);

  const outerBegin = createWorkflowController({ bridge: async command => command === 'begin_workflow_draft'
    ? { outcome: 'draft_ready', schemaVersion: 'rangoon.workflow-draft-session.v1', draftId, workflowId, parentRevisionId: null, workflow: null, extra: true }
    : { outcome: 'listed', workflows: [], capabilities: [] } });
  assert.equal(await outerBegin.beginNew(), false);

  const clearBridge = bridge();
  const outerClear = createWorkflowController({ bridge: async (command, args) => {
    const result = await clearBridge.invoke(command, args);
    return command === 'clear_workflow_draft' ? { ...result, extra: true } : result;
  } });
  await outerClear.beginNew();
  assert.equal(await outerClear.clear(true), false);
  assert.ok(outerClear.getState().session);

  const commitBridge = bridge();
  const outerCommit = createWorkflowController({ bridge: async (command, args) => {
    const result = await commitBridge.invoke(command, args);
    return command === 'commit_workflow_save' ? { ...result, extra: true } : result;
  } });
  await outerCommit.beginNew();
  outerCommit.addNode('input', { x: 0, y: 0 });
  outerCommit.addNode('output', { x: 1, y: 0 });
  outerCommit.connectControl(0, 'next', 1);
  outerCommit.connectData(0, 'text', 1, 'text');
  await outerCommit.inspect('draft');
  outerCommit.acknowledge(true);
  assert.equal(await outerCommit.commit(), false);

  const capabilityBridge = bridge();
  const outerCapability = createWorkflowController({ bridge: async (command, args) => {
    const result = await capabilityBridge.invoke(command, args);
    return command === 'open_capability' ? { ...result, extra: true } : result;
  } });
  assert.equal(await outerCapability.chooseCapability(capabilityId), false);
});

test('SaveReady report relations reject unrelated dependencies, bad topology, and bad diagnostic bounds', () => {
  const request = { definition: definition('Local workflow'), layout: layout(), intent: 'draft' };
  const valid = saveReady(request, true);
  assert.ok(validateWorkflowSaveReady(valid, { draftId, workflowId, parentRevisionId: null, ...request }));
  const wrongTopology = structuredClone(valid);
  wrongTopology.report.topologicalOrder.reverse();
  assert.equal(validateWorkflowSaveReady(wrongTopology, { draftId, workflowId, parentRevisionId: null, ...request }), null);
  const malformedTopology = structuredClone(valid);
  malformedTopology.report.topologicalOrder = {};
  assert.equal(validateWorkflowSaveReady(malformedTopology, { draftId, workflowId, parentRevisionId: null, ...request }), null);
  const invalidTie = saveReady(request, false);
  invalidTie.report.definitionId = `workflow-definition:${hex('f')}`;
  assert.equal(validateWorkflowSaveReady(invalidTie, { draftId, workflowId, parentRevisionId: null, ...request }), null);
  const noCapabilityCount = saveReady(request, false);
  noCapabilityCount.plan.unresolvedReferences = 1;
  assert.equal(validateWorkflowSaveReady(noCapabilityCount, { draftId, workflowId, parentRevisionId: null, ...request }), null);
  const outOfBounds = saveReady(request, false);
  outOfBounds.report.diagnostics[0].nodeIndex = 9;
  assert.equal(validateWorkflowSaveReady(outOfBounds, { draftId, workflowId, parentRevisionId: null, ...request }), null);

  const incomplete = definition('Local workflow');
  incomplete.dataEdges = [];
  const cardinalityRequest = { definition: incomplete, layout: layout(), intent: 'draft' };
  const cardinalityReady = saveReady(cardinalityRequest, false);
  cardinalityReady.report.diagnostics = [{ code: 'input_cardinality', nodeIndex: 1, controlEdgeIndex: null, dataEdgeIndex: null, portIndex: 0 }];
  assert.ok(validateWorkflowSaveReady(cardinalityReady, { draftId, workflowId, parentRevisionId: null, ...cardinalityRequest }));

  const withCapability = definition('Local workflow');
  withCapability.nodes.push({ id: 'capability', title: 'Capability', operation: { kind: 'capability', capabilityId, revisionId: capabilityRevisionId }, inputs: [{ name: 'text', dataType: 'text' }], outputs: [{ name: 'text', dataType: 'text' }] });
  const dependencyRequest = { definition: withCapability, layout: layout(3), intent: 'draft' };
  const dependencyReady = saveReady(dependencyRequest, false);
  dependencyReady.report.dependencies = [{ capabilityId, revisionId: capabilityRevisionId }];
  dependencyReady.plan.unresolvedReferences = 1;
  assert.ok(validateWorkflowSaveReady(dependencyReady, { draftId, workflowId, parentRevisionId: null, ...dependencyRequest }));
  dependencyReady.report.dependencies = [{ capabilityId: `capability:${hex('f')}`, revisionId: capabilityRevisionId }];
  assert.equal(validateWorkflowSaveReady(dependencyReady, { draftId, workflowId, parentRevisionId: null, ...dependencyRequest }), null);
});

test('connection refusals explain invalid targets without changing the inspected graph', async () => {
  const model = createWorkflowController({ bridge: bridge().invoke });
  await model.beginNew();
  model.addNode('input', { x: 0, y: 0 });
  model.addNode('output', { x: 1, y: 0 });
  await model.inspect('draft');
  const unchanged = () => ({ definition: model.getState().definition, layout: model.getState().layout, candidate: model.getState().candidate, canUndo: model.getState().canUndo });
  const beforeOutlet = unchanged();
  assert.equal(model.connectControl(0, 'true', 1), false);
  assert.match(model.getState().message, /outlet/i);
  assert.deepEqual(unchanged(), beforeOutlet);
  assert.equal(model.connectControl(0, 'next', 0), false);
  assert.match(model.getState().message, /itself/i);
  assert.equal(model.connectData(0, 'missing', 1, 'text'), false);
  assert.match(model.getState().message, /source output/i);
  assert.equal(model.connectData(9, 'text', 1, 'text'), false);
  assert.match(model.getState().message, /existing nodes/i);

  assert.ok(model.connectData(0, 'text', 1, 'text'));
  await model.inspect('draft');
  const beforeOccupied = unchanged();
  assert.equal(model.connectData(0, 'text', 1, 'text'), false);
  assert.match(model.getState().message, /already has a mapping/i);
  assert.deepEqual(unchanged(), beforeOccupied);

  model.addNode('check', { x: 2, y: 0 });
  await model.inspect('draft');
  const beforeType = unchanged();
  assert.equal(model.connectData(2, 'passed', 1, 'text'), false);
  assert.match(model.getState().message, /matching port types/i);
  assert.deepEqual(unchanged(), beforeType);
});
