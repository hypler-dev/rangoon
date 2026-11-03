// Inert display fixtures only. These are not native preview or persistence receipts.
export function compositionInspectorCases() {
  const id = (kind, digit) => `${kind}:${digit.repeat(64)}`;
  const content = 'Disposable inspector source. '.repeat(4);
  const byteLength = new TextEncoder().encode(content).length;
  const title = '<Output & review> ' + 'LongTitleWithoutSpaces'.repeat(6);
  const outputs = Array.from({ length: 16 }, (_, index) => ({
    title: `${index + 1} ${title}`,
    pieces: [{ kind: 'authored', content: `Disposable output ${index + 1}.`, reason: 'Synthetic display case.' }],
  }));
  const input = { kind: 'revision', title: '<Saved & source> ' + 'UnbrokenTitle'.repeat(10), content, byteLength, reference: { kind: 'revision', capabilityId: id('capability', '2'), revisionId: id('revision', '3') }, history: [] };
  const base = {
    operation: 'split', bridgeAvailable: true, status: 'ready', pending: null, error: null,
    message: 'Synthetic inspector stress case; no native data or acceptance.', dirty: true,
    sources: [], capabilities: [], inputs: [input], draftVersion: 1,
    draft: { schemaVersion: 'rangoon.composition-draft.v0', operation: 'split', inputs: [input.reference], outputs, exclusions: [], duplications: [], conflicts: [] },
    targets: outputs.map(() => ({ kind: 'new' })), targetDetails: outputs.map(() => null), preview: null, previewStale: false, acknowledged: false, results: [], canUndo: false, canRedo: false,
  };
  const failure = structuredClone(base);
  failure.error = { code: 'fixture_preview_failed', message: 'Synthetic preview unavailable.' };
  failure.message = 'Synthetic preview unavailable. Draft retained.';
  const stale = structuredClone(base);
  stale.preview = { previewId: 'fixture-only', expectedStateId: 'fixture-only', preview: { saveable: false, core: { diagnostics: [{ code: 'fixture_stale', message: 'Synthetic stale preview; inspect again.' }], outputs: [] } } };
  stale.previewStale = true;
  const conflict = structuredClone(base);
  conflict.draft.conflicts = [{ id: 'fixture-conflict', title: '<Unresolved & declaration> ' + 'LongContext'.repeat(10), ranges: [{ inputIndex: 0, startByte: 0, endByte: 8 }, { inputIndex: 0, startByte: 8, endByte: 16 }], resolution: '' }];
  const busy = structuredClone(base); busy.pending = 'preview';
  return [{ name: 'many-outputs', state: base }, { name: 'preview-failed', state: failure }, { name: 'preview-stale', state: stale }, { name: 'unresolved-conflict', state: conflict }, { name: 'busy', state: busy }];
}
