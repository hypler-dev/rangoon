import test from 'node:test';
import assert from 'node:assert/strict';
import { byteRangeText, createCompositionController, textareaSelectionRange } from '../preview/composition-model.mjs';
import { validatePreviewEnvelope } from '../preview/composition-contract.mjs';

const sourceId = `source:${'1'.repeat(64)}`;
const capabilityId = `capability:${'2'.repeat(64)}`;
const revisionId = `revision:${'3'.repeat(64)}`;
const nextRevisionId = `revision:${'4'.repeat(64)}`;
const sha = 'a'.repeat(64);
const workspaceId = `workspace:${'5'.repeat(64)}`;
const compositionId = `composition:${'6'.repeat(64)}`;
const applicationId = `composition-application:${'7'.repeat(64)}`;
const previewId = `preview:${'8'.repeat(64)}`;
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
const span = (content, startByte = 0, endByte = encoder.encode(content).length) => ({ startByte, endByte, startLine: 1, endLine: 1 });

function detail({ id = capabilityId, revision = revisionId, content = 'alpha\r\n😀 beta\n', latest = revision } = {}) {
  const rev = { id: revision, parentRevisionId: null, title: 'Bounded skill', content, sha256: sha, createdAtMs: 1, review: null, provenance: { kind: 'ordinary' } };
  const { content: ignored, ...summary } = rev;
  const history = latest === revision ? [summary] : [summary, { ...summary, id: latest, parentRevisionId: revision, createdAtMs: 2 }];
  return { schemaVersion: 'rangoon.capability.v1', id, origin: { kind: 'source', sourceId, fragmentId: `fragment:${'b'.repeat(64)}`, sourceName: 'rules.md', span: span(content), originalText: content }, latestRevisionId: latest, revision: rev, history, authority: 'none' };
}
function workspace() {
  return { schemaVersion: 'rangoon.workspace-data.v2', stateId: workspaceId, records: { sources: 1, capabilities: 1, revisions: 1, reviews: 0, recipes: 0, applications: 0, derivations: 0, workflows: 0, workflowRevisions: 0 }, databaseBytes: 1, reusableBytes: 0, sources: [{ sourceId, displayName: 'rules.md', sha256: sha, byteLength: 20, savedAtMs: 1 }], capabilities: [{ id: capabilityId, origin: { kind: 'source', sourceId, fragmentId: `fragment:${'b'.repeat(64)}` }, latestRevisionId: revisionId, title: 'Bounded skill', reviewed: false, revisionCount: 1 }], workflows: [] };
}
function report(content = 'one\r\n😀two\n') {
  const all = encoder.encode(content).length;
  const first = encoder.encode('one\r\n').length;
  return { schemaVersion: 'rangoon.source-analysis.v0', analyzerVersion: '0.1.0', source: { id: sourceId, displayName: 'rules.md', format: 'generic_markdown', sha256: sha, byteLength: all, lineCount: 2, content }, fragments: [
    { id: `fragment:${'c'.repeat(64)}`, kind: 'preamble', heading: null, span: { startByte: 0, endByte: first, startLine: 1, endLine: 1 }, text: 'one\r\n', reviewState: 'unreviewed' },
    { id: `fragment:${'d'.repeat(64)}`, kind: 'section', heading: { level: 1, title: 'Emoji' }, span: { startByte: first, endByte: all, startLine: 2, endLine: 2 }, text: '😀two\n', reviewState: 'unreviewed' },
  ], diagnostics: [], authority: 'none' };
}
function ready(draft, targets) {
  const source = 'alpha\r\n😀 beta\n';
  const mappings = []; const outputs = draft.outputs.map((recipe, outputIndex) => {
    let content = ''; let outputByte = 0;
    recipe.pieces.forEach((piece, pieceIndex) => {
      const original = piece.kind === 'copy' ? decoder.decode(encoder.encode(source).slice(piece.range.startByte, piece.range.endByte)) : piece.content;
      mappings.push({ outputIndex, pieceIndex, outputStartByte: outputByte, outputEndByte: outputByte + encoder.encode(original).length, kind: piece.kind, inputRange: piece.kind === 'authored' ? null : piece.range });
      content += original; outputByte += encoder.encode(original).length;
    });
    const digest = String(9 - outputIndex).repeat(64);
    return { outputIndex, title: recipe.title, content, sha256: digest, capabilityId: `capability:${digest}`, revisionId: `revision:${digest}` };
  });
  const coverage = mappings.filter(mapping => mapping.inputRange).map(mapping => ({ range: mapping.inputRange, disposition: 'copied', references: [{ outputIndex: mapping.outputIndex, pieceIndex: mapping.pieceIndex, kind: mapping.kind }], duplicationAcknowledged: false }));
  return { outcome: 'ready', schemaVersion: 'rangoon.composition-session.v0', previewId, expectedStateId: workspaceId, preview: { schemaVersion: 'rangoon.composition-application-preview.v0', core: { schemaVersion: 'rangoon.composition-core-preview.v0', compositionId, saveable: true, outputs, mappings, coverage, diagnostics: [], authority: 'none' }, applicationId, appliedOutputs: targets.map((target, outputIndex) => { const output = outputs[outputIndex]; return { outputIndex, kind: target.kind, capabilityId: target.kind === 'append' ? target.capabilityId : output.capabilityId, parentRevisionId: target.kind === 'append' ? target.expectedRevisionId : null, revisionId: output.revisionId }; }), saveable: true, authority: 'none' } };
}
function receiptFor(preview, draft) {
  return { schemaVersion: 'rangoon.composition-receipt.v0', compositionId, applicationId, committedStateId: workspaceId, authority: 'none', capabilities: preview.preview.appliedOutputs.map((applied, outputIndex) => {
    const output = preview.preview.core.outputs[outputIndex];
    const revision = { id: applied.revisionId, parentRevisionId: applied.parentRevisionId, title: output.title, content: output.content, sha256: output.sha256, createdAtMs: 1, review: null, provenance: { kind: 'composition', applicationId, compositionId, outputIndex } };
    const { content, ...summary } = revision;
    return { schemaVersion: 'rangoon.capability.v1', id: applied.capabilityId, origin: { kind: 'composition', compositionId, operation: draft.operation, outputIndex, inputs: draft.inputs }, latestRevisionId: applied.revisionId, revision, history: [summary], authority: 'none' };
  }) };
}
function controller(operation, overrides = {}) {
  const calls = [];
  const invoke = async (command, args) => {
    calls.push([command, args]);
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'read_composition_source') return { outcome: 'analyzed', report: report() };
    if (command === 'open_capability') return { outcome: 'opened', capability: detail({ revision: args.revisionId ?? revisionId, latest: revisionId }) };
    if (command === 'preview_composition') { const request = JSON.parse(new TextDecoder().decode(args)); return ready(request.draft, request.targets); }
    return { outcome: 'failed', error: { code: 'unexpected', message: 'unexpected' } };
  };
  return { controller: createCompositionController({ operation, invoke: overrides.invoke ?? invoke, onCommitted: overrides.onCommitted }), calls };
}

test('maps normalized textarea selection to original UTF-8 bytes and rejects half surrogate points', () => {
  const content = '\ufeffA\r\n😀\rZ\n';
  const range = textareaSelectionRange(content, 3, 5, 3);
  assert.deepEqual(range, { inputIndex: 3, startByte: 6, endByte: 10 });
  assert.equal(byteRangeText(content, range), '😀');
  assert.equal(textareaSelectionRange(content, 3, 4, 0), null);
  assert.equal(byteRangeText(content, { inputIndex: 0, startByte: 5, endByte: 8 }), null);
});

test('decompose starts a complete deterministic source partition', async () => {
  const { controller: model } = controller('decompose');
  await model.load(); await model.addSource(sourceId); assert.ok(model.startDraft());
  const state = model.getState();
  assert.equal(state.draft.outputs.length, 2);
  const ranges = state.draft.outputs.flatMap(output => output.pieces.map(piece => piece.range));
  assert.deepEqual(ranges, [{ inputIndex: 0, startByte: 0, endByte: 5 }, { inputIndex: 0, startByte: 5, endByte: 13 }]);
});

test('BOM source stays byte-exact through source selection, draft, and native preview', async () => {
  const content = '\ufeffHi\r\n'; const byteLength = encoder.encode(content).length;
  const sourceReport = { schemaVersion: 'rangoon.source-analysis.v0', analyzerVersion: '0.1.0', source: { id: sourceId, displayName: 'bom.md', format: 'generic_markdown', sha256: sha, byteLength, lineCount: 1, content }, fragments: [{ id: `fragment:${'e'.repeat(64)}`, kind: 'preamble', heading: null, span: { startByte: 0, endByte: byteLength, startLine: 1, endLine: 1 }, text: content, reviewState: 'unreviewed' }], diagnostics: [], authority: 'none' };
  const { controller: model } = controller('decompose', { invoke: async (command, args) => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'read_composition_source') return { outcome: 'analyzed', report: sourceReport };
    if (command === 'preview_composition') {
      const request = JSON.parse(new TextDecoder().decode(args)); const output = request.draft.outputs[0];
      return { outcome: 'ready', schemaVersion: 'rangoon.composition-session.v0', previewId, expectedStateId: workspaceId, preview: { schemaVersion: 'rangoon.composition-application-preview.v0', core: { schemaVersion: 'rangoon.composition-core-preview.v0', compositionId, saveable: true, outputs: [{ outputIndex: 0, title: output.title, content, sha256: sha, capabilityId: capabilityId, revisionId }], mappings: [{ outputIndex: 0, pieceIndex: 0, outputStartByte: 0, outputEndByte: byteLength, kind: 'copy', inputRange: output.pieces[0].range }], coverage: [{ range: output.pieces[0].range, disposition: 'copied', references: [{ outputIndex: 0, pieceIndex: 0, kind: 'copy' }], duplicationAcknowledged: false }], diagnostics: [], authority: 'none' }, applicationId, appliedOutputs: [{ outputIndex: 0, kind: 'new', capabilityId, parentRevisionId: null, revisionId }], saveable: true, authority: 'none' } };
    }
    return { outcome: 'failed' };
  } });
  await model.load(); assert.ok(await model.addSource(sourceId)); assert.ok(model.startDraft());
  assert.equal(byteRangeText(content, model.getState().draft.outputs[0].pieces[0].range), content);
  assert.ok(await model.requestPreview());
});

test('recipe range operations retain visible duplicate, exclusion, replacement and conflict declarations', async () => {
  const { controller: model } = controller('split');
  await model.load(); await model.addRevision(capabilityId); model.startDraft();
  const range = { inputIndex: 0, startByte: 0, endByte: 5 };
  assert.ok(model.applyRange(range, { kind: 'duplicate', outputIndex: 1, reason: 'Shared heading.' }));
  assert.equal(model.getState().draft.duplications.length, 1);
  assert.ok(model.applyRange(range, { kind: 'replace', outputIndex: 0, content: 'ALPHA', reason: 'Short title.' }));
  assert.equal(model.getState().draft.outputs[0].pieces.at(-1).kind, 'replace');
  assert.ok(model.applyRange({ inputIndex: 0, startByte: 5, endByte: 6 }, { kind: 'exclude', outputIndex: 0, reason: 'Separator not needed.' }));
  assert.equal(model.getState().draft.exclusions.length, 1);
  assert.ok(model.addConflict({ title: 'Overlap', context: 'Manual comparison.', ranges: [range, { inputIndex: 0, startByte: 7, endByte: 11 }] }));
  assert.equal(model.getState().draft.conflicts[0].resolution, null);
  assert.ok(model.resolveConflict(0, 'Keep both visible.'));
});

test('range replacement retains non-overlapping annotation remainder with its reason', async () => {
  const { controller: model } = controller('split');
  await model.load(); await model.addRevision(capabilityId); model.startDraft();
  assert.ok(model.applyRange({ inputIndex: 0, startByte: 0, endByte: 5 }, { kind: 'duplicate', outputIndex: 1, reason: 'Shared prefix.' }));
  assert.ok(model.applyRange({ inputIndex: 0, startByte: 2, endByte: 3 }, { kind: 'assign', outputIndex: 0 }));
  assert.deepEqual(model.getState().draft.duplications, [
    { range: { inputIndex: 0, startByte: 0, endByte: 2 }, reason: 'Shared prefix.' },
    { range: { inputIndex: 0, startByte: 3, endByte: 5 }, reason: 'Shared prefix.' },
  ]);
});

test('editing marks prior preview stale and removes exact acknowledgment', async () => {
  const { controller: model } = controller('merge');
  await model.load(); await model.addRevision(capabilityId); await model.addRevision(capabilityId, nextRevisionId); model.startDraft();
  await model.requestPreview(); assert.ok(model.acknowledge(true));
  assert.ok(model.addAuthored(0, 'note', 'Operator note.'));
  const state = model.getState();
  assert.equal(state.previewStale, true); assert.equal(state.acknowledged, false);
});

test('replacement loads a historical revision directly, including when input list is full', async () => {
  const ids = Array.from({ length: 16 }, (_, index) => `revision:${index.toString(16).repeat(64)}`);
  const replacement = `revision:${'ab'.repeat(32)}`;
  const { controller: model } = controller('merge', { invoke: async (command, args) => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'open_capability') return { outcome: 'opened', capability: detail({ revision: args.revisionId, latest: args.revisionId }) };
    return { outcome: 'failed' };
  } });
  await model.load();
  for (const id of ids) assert.ok(await model.addRevision(capabilityId, id));
  assert.equal(model.getState().inputs.length, 16);
  assert.ok(await model.replaceRevision(0, replacement));
  assert.equal(model.getState().inputs[0].reference.revisionId, replacement);
});

test('preview validation rejects forged append destination, materialized content, and piece mapping', async () => {
  const { controller: model } = controller('split');
  await model.load(); await model.addRevision(capabilityId); model.startDraft(); await model.setTarget(0, capabilityId);
  const state = model.getState(); const good = ready(state.draft, state.targets);
  assert.ok(validatePreviewEnvelope(good, state.draft, state.targets, state.inputs));
  const forgedTarget = structuredClone(good); forgedTarget.preview.appliedOutputs[0].capabilityId = `capability:${'f'.repeat(64)}`;
  assert.equal(validatePreviewEnvelope(forgedTarget, state.draft, state.targets, state.inputs), null);
  const forgedContent = structuredClone(good); forgedContent.preview.core.outputs[0].content = 'forged';
  assert.equal(validatePreviewEnvelope(forgedContent, state.draft, state.targets, state.inputs), null);
  const forgedMapping = structuredClone(good); forgedMapping.preview.core.mappings[0].outputEndByte += 1;
  assert.equal(validatePreviewEnvelope(forgedMapping, state.draft, state.targets, state.inputs), null);
  const missingCoverage = structuredClone(good); missingCoverage.preview.core.coverage = [];
  assert.equal(validatePreviewEnvelope(missingCoverage, state.draft, state.targets, state.inputs), null);
});

test('malformed native coverage fails closed, clears pending, and permits a new preview', async () => {
  let previews = 0;
  const { controller: model } = controller('split', { invoke: async (command, args) => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'open_capability') return { outcome: 'opened', capability: detail() };
    if (command === 'preview_composition') {
      const request = JSON.parse(new TextDecoder().decode(args)); const result = ready(request.draft, request.targets);
      if (++previews === 1) result.preview.core.coverage = [null];
      return result;
    }
    return { outcome: 'failed' };
  } });
  await model.load(); await model.addRevision(capabilityId); model.startDraft();
  assert.equal(await model.requestPreview(), false);
  assert.equal(model.getState().pending, null);
  assert.equal(await model.requestPreview(), true);
  assert.equal(model.getState().pending, null);
});

test('late native preview cannot replace an invalidated preview slot', async () => {
  let respond;
  const { controller: model } = controller('split', { invoke: async (command, args) => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'open_capability') return { outcome: 'opened', capability: detail() };
    if (command === 'preview_composition') return new Promise(resolve => { respond = () => { const request = JSON.parse(new TextDecoder().decode(args)); resolve(ready(request.draft, request.targets)); }; });
    return { outcome: 'failed' };
  } });
  await model.load(); await model.addRevision(capabilityId); model.startDraft();
  const pending = model.requestPreview(); model.invalidatePreview('Another operation prepared a preview.'); respond();
  await pending; assert.equal(model.getState().preview, null); assert.equal(model.getState().pending, null);
  const fresh = model.requestPreview(); respond(); assert.ok(await fresh);
});

test('invalidating an old preview does not cancel an in-flight target read', async () => {
  let resolveTarget;
  const { controller: model } = controller('split', { invoke: async (command, args) => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'open_capability') return resolveTarget ? new Promise(resolve => { resolveTarget = resolve; }) : { outcome: 'opened', capability: detail() };
    if (command === 'preview_composition') { const request = JSON.parse(new TextDecoder().decode(args)); return ready(request.draft, request.targets); }
    return { outcome: 'failed' };
  } });
  await model.load(); await model.addRevision(capabilityId); model.startDraft(); await model.requestPreview();
  resolveTarget = () => {};
  const targeting = model.setTarget(0, capabilityId); model.invalidatePreview('Workspace changed.');
  resolveTarget({ outcome: 'opened', capability: detail() });
  assert.ok(await targeting); assert.equal(model.getState().targets[0].kind, 'append');
});

test('targets bind exact current head and uncertain save keeps draft stale', async () => {
  const commits = [];
  const { controller: model, calls } = controller('split', { invoke: async (command, args) => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'open_capability') return { outcome: 'opened', capability: detail({ revision: nextRevisionId, latest: nextRevisionId }) };
    if (command === 'preview_composition') { const request = JSON.parse(new TextDecoder().decode(args)); return ready(request.draft, request.targets); }
    if (command === 'commit_composition') { commits.push(args); return { outcome: 'failed', error: { code: 'workspace_unavailable', message: 'Unknown result' } }; }
  } });
  await model.load(); await model.addRevision(capabilityId); model.startDraft();
  assert.ok(await model.setTarget(0, capabilityId));
  assert.equal(model.getState().targets[0].expectedRevisionId, nextRevisionId);
  assert.ok(await model.requestPreview()); assert.ok(model.acknowledge(true));
  assert.equal(await model.save(), false);
  const state = model.getState(); assert.equal(state.previewStale, true); assert.equal(state.acknowledged, false); assert.ok(state.draft);
  assert.equal(commits.length, 1); assert.ok(commits[0] instanceof Uint8Array);
});

test('target selection rejects a returned historical revision when a newer head exists', async () => {
  let opened = 0;
  const { controller: model } = controller('split', { invoke: async command => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'open_capability') return { outcome: 'opened', capability: ++opened === 1 ? detail({ revision: nextRevisionId, latest: nextRevisionId }) : detail({ revision: revisionId, latest: nextRevisionId }) };
    return { outcome: 'failed' };
  } });
  await model.load(); assert.ok(await model.addRevision(capabilityId)); model.startDraft();
  assert.equal(await model.setTarget(0, capabilityId), false);
  assert.deepEqual(model.getState().targets[0], { kind: 'new' });
});

test('successful save receipt clears when a later draft edit begins a new save lifecycle', async () => {
  let latestPreview; let latestDraft;
  const { controller: model } = controller('split', { invoke: async (command, args) => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'open_capability') return { outcome: 'opened', capability: detail() };
    if (command === 'preview_composition') { const request = JSON.parse(new TextDecoder().decode(args)); latestDraft = request.draft; latestPreview = ready(request.draft, request.targets); return latestPreview; }
    if (command === 'commit_composition') return { outcome: 'committed', receipt: receiptFor(latestPreview, latestDraft) };
    return { outcome: 'failed' };
  } });
  await model.load(); await model.addRevision(capabilityId); model.startDraft(); await model.requestPreview(); model.acknowledge(true);
  assert.ok(await model.save()); assert.equal(model.getState().results.length, 2);
  assert.ok(model.addAuthored(0, 'follow-up', 'New local note.'));
  assert.equal(model.getState().results.length, 0);
  assert.ok(await model.requestPreview()); assert.ok(model.acknowledge(true));
});

test('moves complete pieces across outputs without changing BOM, Unicode, ranges, or reasons', async () => {
  const { controller: model } = controller('split');
  await model.load(); await model.addRevision(capabilityId); model.startDraft();
  const range = { inputIndex: 0, startByte: 0, endByte: 5 };
  assert.ok(model.applyRange(range, { kind: 'replace', outputIndex: 0, content: '\ufeff😀', reason: 'Preserve exact source replacement.' }));
  assert.ok(model.addAuthored(0, 'notes', 'Keep authored context.'));
  const replacementIndex = model.getState().draft.outputs[0].pieces.findIndex(piece => piece.kind === 'replace');
  const replacement = structuredClone(model.getState().draft.outputs[0].pieces[replacementIndex]);
  const beforeMove = model.getState().draft;
  assert.ok(model.movePieceTo(0, replacementIndex, 1, 0));
  const state = model.getState();
  assert.deepEqual(state.draft.outputs[1].pieces[0], replacement);
  assert.ok(state.draft.outputs[0].pieces.some(piece => piece.kind === 'authored'));
  assert.equal(state.canUndo, true);
  const moved = state.draft;
  assert.ok(model.undo());
  assert.deepEqual(model.getState().draft, beforeMove);
  assert.ok(model.redo());
  assert.deepEqual(model.getState().draft, moved);
});

test('piece transfer rejects invalid positions and same-output no-ops without creating history', async () => {
  const { controller: model } = controller('split');
  await model.load(); await model.addRevision(capabilityId); model.startDraft();
  const initial = model.getState();
  assert.equal(model.movePieceTo(0, 0, 0, 0), false);
  assert.equal(model.movePieceTo(0, 0, 0, 1), false);
  assert.equal(model.movePieceTo(0, 1, 1), false);
  assert.equal(model.movePieceTo(0, 0, 1, 2), false);
  assert.equal(model.movePieceTo(-1, 0, 1), false);
  const state = model.getState();
  assert.deepEqual(state.draft, initial.draft);
  assert.equal(state.draftVersion, initial.draftVersion);
  assert.equal(state.canUndo, false);
});

test('same-output piece transfer respects pre-move before positions and append order', async () => {
  const { controller: model } = controller('split');
  await model.load(); await model.addRevision(capabilityId); model.startDraft();
  assert.ok(model.addAuthored(0, 'A', 'Order A.'));
  assert.ok(model.addAuthored(0, 'B', 'Order B.'));
  assert.ok(model.addAuthored(0, 'C', 'Order C.'));
  const contents = () => model.getState().draft.outputs[0].pieces.map(piece => piece.kind === 'copy' ? 'copy' : piece.content);
  assert.deepEqual(contents(), ['copy', 'A', 'B', 'C']);
  assert.ok(model.movePieceTo(0, 1, 0, 3));
  assert.deepEqual(contents(), ['copy', 'B', 'A', 'C']);
  assert.ok(model.movePieceTo(0, 2, 0, 0));
  assert.deepEqual(contents(), ['A', 'copy', 'B', 'C']);
  assert.ok(model.movePieceTo(0, 0, 0));
  assert.deepEqual(contents(), ['copy', 'B', 'C', 'A']);
});

test('stale drag versions reject after edit, undo, reset, and new draft', async () => {
  const { controller: model } = controller('split');
  await model.load(); await model.addRevision(capabilityId); model.startDraft();
  const initialVersion = model.getState().draftVersion;
  assert.ok(model.addAuthored(0, 'one', 'First edit.'));
  assert.equal(model.movePieceTo(0, 0, 1, null, initialVersion), false);
  const editedVersion = model.getState().draftVersion;
  assert.ok(model.undo());
  assert.equal(model.movePieceTo(0, 0, 1, null, editedVersion), false);
  const beforeReset = model.getState().draftVersion;
  assert.ok(model.resetDraft());
  assert.ok(model.startDraft());
  assert.equal(model.movePieceTo(0, 0, 1, null, beforeReset), false);
  assert.ok(model.movePieceTo(0, 0, 1, null, model.getState().draftVersion));
});

test('piece transfer, undo, and redo invalidate acknowledged previews while restoring exact targets', async () => {
  const { controller: model } = controller('split');
  await model.load(); await model.addRevision(capabilityId); model.startDraft();
  await model.requestPreview(); assert.ok(model.acknowledge(true));
  assert.ok(await model.setTarget(0, capabilityId));
  const targeted = model.getState();
  assert.deepEqual(targeted.targets[0], { kind: 'append', capabilityId, expectedRevisionId: revisionId });
  assert.ok(targeted.targetDetails[0]);
  assert.equal(targeted.acknowledged, false);
  assert.ok(model.undo());
  const undone = model.getState();
  assert.deepEqual(undone.targets[0], { kind: 'new' });
  assert.equal(undone.targetDetails[0], null);
  assert.equal(undone.acknowledged, false);
  assert.equal(undone.previewStale, true);
  assert.deepEqual(undone.results, []);
  assert.ok(model.redo());
  const redone = model.getState();
  assert.deepEqual(redone.targets[0], targeted.targets[0]);
  assert.deepEqual(redone.targetDetails[0], targeted.targetDetails[0]);
  assert.equal(redone.acknowledged, false);
});

test('pending native preview and save reject undo and piece transfer', async () => {
  let resolvePreview;
  let resolveSave;
  const { controller: model } = controller('split', { invoke: async (command, args) => {
    if (command === 'get_workspace_data') return { outcome: 'loaded', workspace: workspace() };
    if (command === 'open_capability') return { outcome: 'opened', capability: detail() };
    if (command === 'preview_composition') return new Promise(resolve => { resolvePreview = () => { const request = JSON.parse(new TextDecoder().decode(args)); resolve(ready(request.draft, request.targets)); }; });
    if (command === 'commit_composition') return new Promise(resolve => { resolveSave = () => resolve({ outcome: 'failed', error: { code: 'unavailable', message: 'Save did not complete.' } }); });
    return { outcome: 'failed' };
  } });
  await model.load(); await model.addRevision(capabilityId); model.startDraft();
  assert.ok(model.addAuthored(0, 'first', 'First edit.'));
  const preview = model.requestPreview();
  assert.equal(model.getState().canUndo, false);
  assert.equal(model.undo(), false);
  assert.equal(model.movePieceTo(0, 0, 1), false);
  resolvePreview(); assert.ok(await preview); assert.ok(model.acknowledge(true));
  const saving = model.save();
  assert.equal(model.getState().canUndo, false);
  assert.equal(model.undo(), false);
  resolveSave(); assert.equal(await saving, false);
  assert.equal(model.getState().canUndo, true);
});

test('new edits clear redo and history retains only the latest 32 reversible edits', async () => {
  const { controller: model } = controller('split');
  await model.load(); await model.addRevision(capabilityId); model.startDraft();
  assert.ok(model.addAuthored(0, 'first', 'First edit.'));
  assert.ok(model.addAuthored(0, 'second', 'Second edit.'));
  assert.ok(model.undo());
  assert.equal(model.getState().canRedo, true);
  assert.equal(model.setOutputTitle(0, model.getState().draft.outputs[0].title), false);
  assert.equal(model.getState().canRedo, true);
  assert.ok(model.addAuthored(0, 'branch', 'Branch edit.'));
  assert.equal(model.redo(), false);
  for (let index = 0; index < 33; index += 1) assert.ok(model.addAuthored(0, `entry-${index}`, `Entry ${index}.`));
  let undos = 0;
  while (model.undo()) undos += 1;
  assert.equal(undos, 32);
  assert.equal(model.getState().canUndo, false);
});

test('serialized history budget keeps only independent recent snapshots', async () => {
  const { controller: model } = controller('split');
  await model.load(); await model.addRevision(capabilityId); model.startDraft();
  const large = 'x'.repeat(2_200_000);
  assert.ok(model.addAuthored(0, `${large}1`, 'Large draft text.'));
  assert.ok(model.updatePiece(0, 1, `${large}2`, 'Large draft text.'));
  assert.ok(model.updatePiece(0, 1, `${large}3`, 'Large draft text.'));
  const exposed = model.getState();
  exposed.draft.outputs[0].pieces[1].content = 'mutated outside state';
  assert.ok(model.undo());
  assert.equal(model.getState().draft.outputs[0].pieces[1].content.at(-1), '2');
  assert.equal(model.undo(), false);
  assert.ok(model.redo());
  assert.equal(model.getState().draft.outputs[0].pieces[1].content.at(-1), '3');
});
