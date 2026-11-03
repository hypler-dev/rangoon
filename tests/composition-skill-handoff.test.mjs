import test from 'node:test';
import assert from 'node:assert/strict';
import { createSkillsController } from '../preview/skills-model.mjs';
import { compositionResultSelection, compositionResultIssue, openCompositionResultInSkills, compositionResultFocus } from '../preview/composition-skill-handoff.mjs';

const cap = `capability:${'a'.repeat(64)}`, rev = `revision:${'b'.repeat(64)}`, newer = `revision:${'c'.repeat(64)}`;
const other = `capability:${'d'.repeat(64)}`;
const revision = id => ({ id, parentRevisionId: id === rev ? null : rev, title: 'Saved output', content: 'Original saved output\n', sha256: 'e'.repeat(64), createdAtMs: id === rev ? 1 : 2, review: null, provenance: { kind: 'ordinary' } });
function detail(capabilityId = cap, revisionId = rev, latest = revisionId) {
  const selected = revision(revisionId), current = revision(latest);
  const summary = ({ content: ignored, ...value }) => value;
  return { schemaVersion: 'rangoon.capability.v1', id: capabilityId,
    origin: { kind: 'source', sourceId: `source:${'1'.repeat(64)}`, fragmentId: `fragment:${'2'.repeat(64)}`, sourceName: 'synthetic.md', span: { startByte: 0, endByte: 22, startLine: 1, endLine: 1 }, originalText: 'Original saved output\n' },
    latestRevisionId: latest, revision: selected, history: latest === revisionId ? [summary(selected)] : [summary(selected), summary(current)], authority: 'none' };
}
const opened = value => ({ outcome: 'opened', capability: value, alreadyApplied: false });
function origin() { const state = { bridgeAvailable: true, pending: null, results: [detail()] }; return { state, getState: () => state }; }

test('saved result uses exact native revision even when the current head has advanced', async () => {
  const composition = origin(), calls = [];
  const skills = createSkillsController({ invoke: async (command, args) => { calls.push([command, args]); return opened(detail(cap, rev, newer)); } });
  assert.equal(await openCompositionResultInSkills({ composition, skills, capabilityId: cap, revisionId: rev, isCurrent: () => true }), true);
  assert.deepEqual(calls, [['open_capability', { capabilityId: cap, revisionId: rev }]]);
  assert.equal(skills.getState().viewedRevisionId, rev);
  assert.equal(skills.getState().selected.latestRevisionId, newer);
  assert.equal(compositionResultFocus(skills.getState(), { capabilityId: cap, revisionId: rev }), '#skills-editor-title');
});

test('result handoff retains unrelated dirty drafts and blocks same-skill edits', async () => {
  const composition = origin(); let reads = 0;
  const skills = createSkillsController({ invoke: async (_cmd, args) => { reads++; return opened(detail(args.capabilityId)); } });
  await skills.open(other); skills.updateDraft({ content: 'Keep these edits\n' });
  assert.equal(await openCompositionResultInSkills({ composition, skills, capabilityId: cap, revisionId: rev, isCurrent: () => true }), true);
  assert.equal(skills.hasUnsavedDraft(other), true);
  skills.updateDraft({ content: 'Unsaved output edits\n' }); const before = skills.getState(); const count = reads;
  assert.match(compositionResultIssue({ composition, skills, capabilityId: cap, revisionId: rev }), /Save or discard/);
  assert.equal(await openCompositionResultInSkills({ composition, skills, capabilityId: cap, revisionId: rev, isCurrent: () => true }), false);
  assert.equal(reads, count); assert.deepEqual(skills.getState(), before);
  await skills.open(other); assert.equal(skills.getState().draft.content, 'Keep these edits\n');
});

test('only a pinned member of current saved results can initiate a read', async () => {
  for (const change of [s => { s.results = []; }, s => { s.pending = 'save'; }, s => { s.bridgeAvailable = false; }, s => { s.results[0].revision.id = newer; }]) {
    const composition = origin(); change(composition.state);
    const skills = createSkillsController({ invoke: () => assert.fail('no native call') });
    assert.equal(compositionResultSelection(composition.state, cap, rev), null);
    assert.equal(await openCompositionResultInSkills({ composition, skills, capabilityId: cap, revisionId: rev, isCurrent: () => true }), false);
  }
  for (const value of ['', null, 'latest', rev.toUpperCase(), `revision:${'a'.repeat(65)}`]) assert.equal(compositionResultSelection(origin().state, cap, value), null);
});

test('native failure, malformed details and changed revision cannot open a substitute', async () => {
  for (const reply of [{ outcome: 'failed', error: { code: 'missing', message: 'Revision unavailable' } }, opened({}), opened(detail(cap, newer)), opened(detail(other))]) {
    const skills = createSkillsController({ invoke: async () => reply });
    assert.equal(await openCompositionResultInSkills({ composition: origin(), skills, capabilityId: cap, revisionId: rev, isCurrent: () => true }), false);
    assert.equal(skills.getState().selected, null); assert.equal(skills.getState().pending, null); assert.ok(skills.getState().error);
  }
});

test('late result reads are discarded after navigation, receipt invalidation or Lock', async () => {
  for (const kind of ['route', 'receipt', 'lock']) {
    const composition = origin(); let active = true, release;
    const skills = createSkillsController({ invoke: () => new Promise(resolve => { release = resolve; }) });
    const before = skills.getState();
    const waiting = openCompositionResultInSkills({ composition, skills, capabilityId: cap, revisionId: rev, isCurrent: () => active });
    if (kind === 'route') active = false;
    if (kind === 'receipt') composition.state.results = [];
    if (kind === 'lock') skills.purgeForWorkspaceLock();
    const locked = skills.getState(); release(opened(detail()));
    assert.equal(await waiting, false); assert.deepEqual(skills.getState(), kind === 'lock' ? locked : before);
  }
});

test('focus only targets the matching successfully loaded saved revision', () => {
  const state = { pending: null, selected: detail(), viewedRevisionId: rev };
  assert.equal(compositionResultFocus(state, { capabilityId: cap, revisionId: rev }), '#skills-editor-title');
  assert.equal(compositionResultFocus(state, { capabilityId: other, revisionId: rev }), '#skills-title');
  assert.equal(compositionResultFocus(state, { capabilityId: cap, revisionId: newer }), '#skills-title');
  assert.equal(compositionResultFocus({ ...state, pending: 'open' }, { capabilityId: cap, revisionId: rev }), null);
});

test('opt-in handoff fixture preserves exact display contracts and memory-only receipts', async () => {
  const { handoffPreview, handoffReceipt } = await import('../scripts/ui-composition-handoff-fixture.mjs');
  const { validatePreviewEnvelope, validateReceipt } = await import('../preview/composition-contract.mjs');
  const reads = [detail(cap), detail(other)];
  const inputs = reads.map(item => ({ reference: { kind: 'revision', capabilityId: item.id, revisionId: item.revision.id, sha256: item.revision.sha256 }, content: item.revision.content }));
  const draft = { schemaVersion: 'rangoon.composition-draft.v0', operation: 'merge', inputs: inputs.map(item => item.reference), outputs: [{ title: 'Synthetic saved result', pieces: inputs.map((item, inputIndex) => ({ kind: 'copy', range: { inputIndex, startByte: 0, endByte: new TextEncoder().encode(item.content).length } })) }], exclusions: [], duplications: [], conflicts: [] };
  const request = { schemaVersion: 'rangoon.composition-application-request.v0', draft, targets: [{ kind: 'new' }] };
  const ready = await handoffPreview(request, reads);
  assert.equal(validatePreviewEnvelope(ready, draft, request.targets, inputs), ready);
  assert.equal(ready.preview.core.outputs[0].content, inputs.map(item => item.content).join(''));
  const receipt = handoffReceipt(ready, request);
  assert.equal(validateReceipt(receipt, ready.preview), receipt);
  assert.equal(receipt.authority, 'none');
  assert.equal(receipt.capabilities[0].revision.id, `revision:${'9'.repeat(64)}`);
  await assert.rejects(handoffPreview({ ...request, targets: [{ kind: 'append' }] }, reads), /Unsupported/);
  await assert.rejects(handoffPreview(request, []), /Unknown exact/);
});

test('handoff fixture does nothing on ordinary preview URLs', async () => {
  const { installHandoffFixture } = await import('../scripts/ui-composition-handoff-fixture.mjs');
  const saved = globalThis.location;
  try {
    globalThis.location = new URL('http://127.0.0.1/analyze.html?qa=open#skills');
    assert.doesNotThrow(() => installHandoffFixture(() => assert.fail('inactive'), () => assert.fail('inactive')));
  } finally {
    if (saved === undefined) delete globalThis.location;
    else globalThis.location = saved;
  }
});
