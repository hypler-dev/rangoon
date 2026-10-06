import test from 'node:test';
import assert from 'node:assert/strict';
import { byteRangeText, renderCompositionView, textareaSelectionRange } from '../preview/composition-view.mjs';

const sourceId = `source:${'1'.repeat(64)}`;
const capabilityId = `capability:${'2'.repeat(64)}`;
const revisionId = `revision:${'3'.repeat(64)}`;
const content = 'Preamble\r\nEmoji 👩‍💻\r\nFinal\n';
const byteLength = new TextEncoder().encode(content).length;
const input = { kind: 'source', sourceId, reference: sourceId, title: '<Imported & source>', content, byteLength, sections: [], history: [] };
const draft = {
  schemaVersion: 'rangoon.composition-draft.v0', operation: 'decompose', inputs: [{ kind: 'source', sourceId, sha256: 'a'.repeat(64) }],
  outputs: [{ title: 'Rules', pieces: [{ kind: 'copy', range: { inputIndex: 0, startByte: 0, endByte: byteLength } }] }], exclusions: [], duplications: [], conflicts: [],
};
const initial = (overrides = {}) => ({ operation: 'decompose', bridgeAvailable: true, status: 'ready', pending: null, error: null, message: 'Saved records loaded.', dirty: false, sources: [{ id: sourceId, displayName: 'rules.md', byteLength }], capabilities: [{ id: capabilityId, title: 'Existing skill', latestRevisionId: revisionId, history: [{ id: revisionId, title: 'Existing skill' }, { id: `revision:${'4'.repeat(64)}`, title: 'Older skill' }] }], inputs: [input], draft: null, targets: [], targetDetails: [], preview: null, previewStale: false, acknowledged: false, results: [], ...overrides });

test('selection mapping preserves original CRLF and rejects half-surrogates', () => {
  const start = 'Preamble\n'.length;
  const selected = textareaSelectionRange(content, start, start + 'Emoji 👩‍💻'.length, 0);
  assert.deepEqual(selected, { inputIndex: 0, startByte: new TextEncoder().encode('Preamble\r\n').length, endByte: new TextEncoder().encode('Preamble\r\nEmoji 👩‍💻').length });
  assert.equal(textareaSelectionRange(content, start + 'Emoji '.length + 1, start + 'Emoji '.length + 2), null);
  assert.equal(byteRangeText(content, selected), 'Emoji 👩‍💻');
  const emojiStart = new TextEncoder().encode('Preamble\r\nEmoji ').length;
  assert.equal(byteRangeText(content, { inputIndex: 0, startByte: emojiStart + 1, endByte: emojiStart + 2 }), null);
});

test('unavailable, loading and empty records stay distinct', () => {
  assert.match(renderCompositionView(initial({ bridgeAvailable: false, status: 'unavailable', sources: [], capabilities: [] })), /Browser preview cannot invent saved records/);
  assert.match(renderCompositionView(initial({ status: 'loading', sources: [], capabilities: [] })), /Loading saved workspace records/);
  assert.match(renderCompositionView(initial({ status: 'ready', sources: [], capabilities: [] })), /No saved records available yet/);
});

test('start draft follows exact operation input counts', () => {
  const revision = index => ({ reference: { kind: 'revision', capabilityId: `capability:${String(index).repeat(64)}`, revisionId: `revision:${String(index).repeat(64)}`, sha256: 'a'.repeat(64) }, title: `Revision ${index}`, content: 'Saved revision\n', latestRevisionId: `revision:${String(index).repeat(64)}`, history: [] });
  const start = state => renderCompositionView(state).match(/<button id="composition-start"[^>]*>/)[0];
  assert.match(start(initial({ operation: 'merge', inputs: [] })), /disabled/);
  assert.match(start(initial({ operation: 'merge', inputs: [revision(4)] })), /disabled/);
  assert.doesNotMatch(start(initial({ operation: 'merge', inputs: [revision(4), revision(5)] })), /disabled/);
  assert.doesNotMatch(start(initial({ operation: 'decompose', inputs: [input] })), /disabled/);
  assert.doesNotMatch(start(initial({ operation: 'split', inputs: [revision(4)] })), /disabled/);
});

test('real draft escapes imported text and exposes historical and target controls', () => {
  const targetDetails = [{ revision: { title: 'Existing skill' } }];
  const html = renderCompositionView(initial({ draft, dirty: true, targets: [{ kind: 'append', capabilityId, expectedRevisionId: revisionId }], targetDetails }));
  assert.match(html, /&lt;Imported &amp; source&gt;/);
  assert.doesNotMatch(html, /<Imported & source>/);
  assert.match(html, /Update Existing skill · current head/);
  assert.match(html, /aria-label="Select source text for composition actions"/);
  assert.match(html, /data-composition-apply="duplicate"/);
  assert.match(html, /data-composition-add-conflict/);
  const picker = renderCompositionView(initial({ operation: 'merge', draft: null, sources: [] }));
  assert.match(picker, /Older skill · historical/);
});

test('preview states show blocked, ready, saving, stale and actual saved skills', () => {
  const blockedEnvelope = { previewId: 'preview:a', expectedStateId: 'state:a', preview: { saveable: false, core: { diagnostics: [{ code: 'unassigned' }], outputs: [], coverage: [{ range: { inputIndex: 0, startByte: 0, endByte: byteLength }, disposition: 'unassigned' }] } } };
  const blocked = renderCompositionView(initial({ draft, preview: blockedEnvelope }));
  assert.match(blocked, /Preview blocked/);
  assert.match(blocked, /unassigned/);
  assert.match(blocked, /Reveal first unassigned span/);
  const readyEnvelope = { previewId: 'preview:a', expectedStateId: 'state:a', preview: { saveable: true, appliedOutputs: [{ kind: 'new' }], core: { diagnostics: [], coverage: [], outputs: [{ outputIndex: 0, title: 'Rules', content: 'Exact output\n' }] } } };
  const ready = renderCompositionView(initial({ draft, preview: readyEnvelope }));
  assert.match(ready, /Preview ready/);
  assert.match(ready, /I reviewed exact host preview content/);
  assert.match(ready, /Exact output/);
  const savingEnvelope = { previewId: 'preview:a', expectedStateId: 'state:a', preview: { saveable: true, core: { diagnostics: [], coverage: [], outputs: [] } } };
  const saving = renderCompositionView(initial({ draft, pending: 'save', preview: savingEnvelope, acknowledged: true }));
  assert.match(saving, /Saving locally/);
  assert.match(renderCompositionView(initial({ draft, previewStale: true })), /Preview stale/);
  const saved = renderCompositionView(initial({ draft, results: [{ id: capabilityId, title: 'Actual created skill' }] }));
  assert.match(saved, /Composition saved/);
  assert.match(saved, /Actual created skill/);
  assert.match(saved, /data-composition-open-skill/);
});
