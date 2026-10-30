import test from 'node:test';
import assert from 'node:assert/strict';
import { bindCompositionView, byteRangeText, compositionSelectionExcerpt, purgeCompositionViewCache, renderCompositionView, textareaSelectionRange } from '../preview/composition-view.mjs';

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

function filterRoot(attribute) {
  const listeners = new Map();
  const document = { activeElement: null, addEventListener() {}, removeEventListener() {} };
  const root = {
    ownerDocument: document, renders: 0,
    input: null,
    contains: node => node === document.activeElement,
    set innerHTML(html) {
      const match = html.match(new RegExp(`<input[^>]*${attribute}[^>]*value="([^"]*)"`));
      this.renders += 1;
      if (!match) { this.input = null; return; }
      const input = {
        value: match[1], selectionStart: 0, selectionEnd: 0,
        matches: selector => selector === `[${attribute}]`,
        getAttributeNames: () => [attribute], getAttribute: name => name === attribute ? '' : null,
        focus: () => { document.activeElement = input; },
        setSelectionRange: (start, end) => { input.selectionStart = start; input.selectionEnd = end; },
      };
      this.input = input;
    },
    querySelector: selector => selector === `[${attribute}]` ? root.input : null,
    querySelectorAll: () => [],
    addEventListener: (type, listener) => listeners.set(type, listener),
    removeEventListener: type => listeners.delete(type),
    fire: (type, event) => listeners.get(type)?.(event),
  };
  return root;
}

function compositionMenuRoot() {
  const listeners = new Map();
  const receiver = { closest: () => null };
  const popup = { removed: false, remove() { this.removed = true; } };
  const trigger = {
    tagName: 'BUTTON', dataset: { compositionGraphOutputActions: '0' },
    closest: () => trigger, matches: () => false, hasAttribute: () => false,
  };
  const menuItem = { closest: selector => selector === '[data-composition-context]' ? popup : null, focus() {} };
  class Element {}
  const document = {
    activeElement: null, addEventListener() {}, removeEventListener() {}, createTextNode: value => ({ value }),
    defaultView: { Element, addEventListener() {}, removeEventListener() {} },
  };
  const root = {
    ownerDocument: document, html: '', renders: 0,
    contains: node => node === document.activeElement && node !== receiver,
    set innerHTML(value) { this.html = value; this.renders += 1; }, get innerHTML() { return this.html; },
    addEventListener(type, listener) { listeners.set(type, listener); },
    removeEventListener(type, listener) { if (listeners.get(type) === listener) listeners.delete(type); },
    querySelector(selector) {
      if (selector === '[data-composition-context]') return this.html.includes('data-composition-context') ? popup : null;
      if (selector === '[data-composition-context] button') return this.html.includes('data-composition-context') ? menuItem : null;
      return null;
    },
    querySelectorAll() { return []; },
  };
  return { root, document, listeners, receiver, popup, trigger, menuItem };
}

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

test('filters loaded saved sources and revisions without changing inputs or unavailable states', () => {
  const sources = [{ id: sourceId, displayName: 'alpha.rules', byteLength }, { id: `source:${'5'.repeat(64)}`, displayName: 'beta.rules', byteLength }];
  const sourceState = initial({ sources });
  const sourceMatch = renderCompositionView(sourceState, { inputFilter: 'beta' });
  assert.match(sourceMatch, /beta\.rules/);
  assert.doesNotMatch(sourceMatch, /alpha\.rules/);
  const hostile = renderCompositionView(sourceState, { inputFilter: '"<none & missing> 🦀' });
  assert.match(hostile, /No saved source matches “&quot;&lt;none &amp; missing&gt; 🦀”/);
  assert.doesNotMatch(hostile, /"<none & missing> 🦀/);
  const revision = renderCompositionView(initial({ operation: 'merge' }), { inputFilter: 'Older skill' });
  assert.match(revision, /Older skill · historical/);
  assert.doesNotMatch(revision, /Existing skill · latest saved revision/);
  assert.match(renderCompositionView(initial({ status: 'loading', sources: [] }), { inputFilter: 'missing' }), /Loading saved workspace records/);
  assert.match(renderCompositionView(initial({ status: 'ready', sources: [] }), { inputFilter: 'missing' }), /No saved records available yet/);
  assert.match(renderCompositionView(initial({ bridgeAvailable: false, status: 'unavailable', sources: [] }), { inputFilter: 'missing' }), /Browser preview cannot invent saved records/);
  assert.equal(sourceState.inputs.length, 1);
  assert.equal(sourceState.sources.length, 2);
});

test('preserves composition picker query across pure renderer updates without selecting records', () => {
  const source = initial();
  const first = renderCompositionView(source, { inputFilter: 'rules' });
  const rerender = renderCompositionView(source, { inputFilter: 'rules' });
  assert.match(first, /data-composition-picker-filter value="rules"/);
  assert.match(rerender, /data-composition-picker-filter value="rules"/);
  assert.equal(source.inputs.length, 1);
  assert.equal(source.draft, null);
});

test('bound composition picker keeps focus and caret through controller rerenders without changing selected inputs', () => {
  const snapshot = initial();
  const root = filterRoot('data-composition-picker-filter');
  const binding = bindCompositionView(root, { getState: () => snapshot });
  root.input.value = 'rules'; root.input.focus(); root.input.setSelectionRange(1, 4);
  root.fire('input', { target: root.input });
  assert.equal(root.input.value, 'rules');
  assert.equal(root.ownerDocument.activeElement, root.input);
  assert.deepEqual([root.input.selectionStart, root.input.selectionEnd], [1, 4]);
  binding.render();
  assert.equal(root.input.value, 'rules');
  assert.equal(root.ownerDocument.activeElement, root.input);
  assert.deepEqual([root.input.selectionStart, root.input.selectionEnd], [1, 4]);
  let stopped = false;
  root.fire('keydown', { key: 'Escape', target: root.input, preventDefault() {}, stopPropagation() { stopped = true; } });
  assert.equal(root.input.value, '');
  assert.equal(stopped, true);
  assert.equal(snapshot.inputs.length, 1);
  assert.equal(snapshot.draft, null);
  binding.dispose();
});

test('composition route cache survives ordinary disposal, lock purge clears inactive cache, and old render cannot revive it', () => {
  const snapshot = initial();
  const controller = { getState: () => snapshot };
  const first = filterRoot('data-composition-picker-filter');
  const oldBinding = bindCompositionView(first, controller);
  first.input.value = 'rules'; first.fire('input', { target: first.input });
  oldBinding.dispose();

  const resumed = filterRoot('data-composition-picker-filter');
  const resumedBinding = bindCompositionView(resumed, controller);
  assert.equal(resumed.input.value, 'rules');
  oldBinding.render();
  assert.equal(resumed.input.value, 'rules');
  resumedBinding.dispose();

  purgeCompositionViewCache(controller);
  const afterLock = filterRoot('data-composition-picker-filter');
  const afterLockBinding = bindCompositionView(afterLock, controller);
  assert.equal(afterLock.input.value, '');
  afterLockBinding.purgeForWorkspaceLock();
  afterLockBinding.render();
  assert.equal(afterLock.input.value, '');
});

test('a late composition action completion cannot redraw an old disposed binding', async () => {
  const snapshot = initial();
  let resolveStart;
  const controller = {
    getState: () => snapshot,
    startDraft: () => new Promise(resolve => { resolveStart = resolve; }),
  };
  const root = filterRoot('data-composition-picker-filter');
  const binding = bindCompositionView(root, controller);
  const target = {
    tagName: 'BUTTON', dataset: {}, closest: () => target,
    matches: selector => selector === '#composition-start', hasAttribute: () => false,
  };
  root.fire('click', { target });
  assert.equal(typeof resolveStart, 'function');
  binding.dispose();
  const renderedBeforeLateCompletion = root.renders;
  resolveStart(true);
  await Promise.resolve(); await Promise.resolve();
  assert.equal(root.renders, renderedBeforeLateCompletion);
});

test('composition menu focus exit removes only its popup and retains the receiving control', () => {
  const fixture = compositionMenuRoot();
  const snapshot = initial({ draft, dirty: true, draftVersion: 1 });
  const binding = bindCompositionView(fixture.root, { getState: () => snapshot });
  fixture.listeners.get('click')({ target: fixture.trigger });
  const rendersBeforeExit = fixture.root.renders;
  fixture.document.activeElement = fixture.receiver;
  fixture.listeners.get('focusout')({ target: fixture.menuItem, relatedTarget: fixture.receiver });
  assert.equal(fixture.popup.removed, true);
  assert.equal(fixture.root.renders, rendersBeforeExit);
  assert.equal(fixture.document.activeElement, fixture.receiver);
  binding.dispose();
  assert.equal(fixture.listeners.has('focusout'), false);
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
  assert.match(html, /Update Existing skill · latest saved revision/);
  assert.match(html, /data-term-help-group="composition"/);
  assert.match(html, /data-term-help="recipe"/);
  assert.match(html, /aria-label="Select source text for composition actions"/);
  assert.match(html, /data-composition-apply="duplicate"/);
  assert.match(html, /data-composition-add-conflict/);
  const picker = renderCompositionView(initial({ operation: 'merge', draft: null, sources: [] }));
  assert.match(picker, /Older skill · historical/);
});

test('composition views use pressed ordinary buttons and keep panels controlled', () => {
  const html = renderCompositionView(initial({ draft, dirty: true }));
  assert.match(html, /data-composition-tab="source" aria-pressed="true"/);
  assert.match(html, /data-composition-tab="recipe" aria-pressed="false"/);
  assert.match(html, /data-composition-tab="graph" aria-pressed="false"/);
  assert.doesNotMatch(html, /data-composition-tab="[^"]+" aria-selected/);
  assert.match(html, /data-composition-panel="source"/);
  assert.match(html, /data-composition-panel="recipe" hidden/);
  assert.match(html, /aria-label="Composition editor views"/);
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

test('derived canvas exposes escaped source links, guarded ports and history controls', () => {
  const canvasInput = { ...input, sections: [{ title: '<Setup & safety>', range: { inputIndex: 0, startByte: 0, endByte: byteLength } }] };
  const canvasDraft = { ...draft, outputs: [...draft.outputs, { title: 'Second output', pieces: [{ kind: 'authored', content: 'Typed text', reason: 'Separate material' }] }] };
  const html = renderCompositionView(initial({ inputs: [canvasInput], draft: canvasDraft, targets: [{ kind: 'new' }, { kind: 'new' }], canUndo: true, draftVersion: 7 }));
  assert.match(html, /data-composition-graph-section="0:0"/);
  assert.match(html, /composition-port--source/);
  assert.match(html, /composition-port--piece/);
  assert.match(html, /data-composition-wires/);
  assert.match(html, /&lt;Setup &amp; safety&gt;/);
  assert.match(html, /draggable="true"/);
  assert.match(html, /data-composition-draft-version="7"/);
  assert.match(html, /Input 1, bytes 0–/);
  assert.match(html, /data-composition-undo/);
  assert.match(html, /data-composition-redo[^>]*disabled/);
  const pending = renderCompositionView(initial({ inputs: [canvasInput], draft: canvasDraft, targets: [{ kind: 'new' }, { kind: 'new' }], pending: 'preview' }));
  assert.match(pending, /data-composition-graph-piece="0:0"[^>]*disabled/);
  assert.match(pending, /draggable="false"/);
});

test('keeps the picker unchanged before drafting and adds local dock controls after draft start', () => {
  const picker = renderCompositionView(initial());
  assert.doesNotMatch(picker, /data-workspace-workbench/);
  const html = renderCompositionView(initial({ draft, dirty: true }), { workspaceLayout: { desktop: true, left: 240, right: 320, leftMax: 360, rightMax: 440, leftCollapsed: false, rightCollapsed: false } });
  assert.match(html, /data-workspace-workbench/);
  assert.match(html, /data-workspace-resize="right"[^>]*role="separator"[^>]*aria-valuenow="320"/);
  assert.match(html, /data-workspace-collapse="right"/);
  assert.doesNotMatch(html, /Fit graph/);
});

test('contextual composition starts with disabled selection actions and closed advanced operations', () => {
  const html = renderCompositionView(initial({ draft }));
  assert.match(html, /aria-label="Add source selection to recipe"/);
  assert.match(html, /data-composition-apply="assign" disabled>Assign selection/);
  assert.match(html, /data-composition-range-options >/);
  assert.doesNotMatch(html, /data-composition-range-options open/);
  assert.match(html, /Working selection/);
  assert.match(html, /Recipe edits leave original files unchanged/);
});

test('section quick selection retains exact range context, escapes provenance and makes reassignment explicit', () => {
  const section = { title: '<Review & evidence>', startByte: 0, endByte: byteLength };
  const html = renderCompositionView(initial({ draft, inputs: [{ ...input, sections: [section] }] }), { selectedRange: { inputIndex: 0, startByte: 0, endByte: byteLength }, sourceInputIndex: 0 });
  assert.match(html, /data-composition-section="0:0" aria-pressed="true"/);
  assert.match(html, /&lt;Review &amp; evidence&gt;/);
  assert.match(html, /Existing placements of this range will be reassigned/);
  assert.match(html, /Duplicate keeps both/);
  assert.doesNotMatch(html, /data-composition-apply="assign" disabled/);
  assert.match(html, /data-composition-working-provenance/);
});

test('invalid, mismatched-input and pending selections cannot enable contextual actions', () => {
  const range = { inputIndex: 0, startByte: 0, endByte: byteLength };
  for (const [state, ui] of [
    [initial({ draft }), { selectedRange: { ...range, endByte: byteLength + 1 } }],
    [initial({ draft }), { selectedRange: range, sourceInputIndex: 1 }],
    [initial({ draft, pending: 'preview' }), { selectedRange: range }],
    [initial({ draft, bridgeAvailable: false }), { selectedRange: range }],
  ]) assert.match(renderCompositionView(state, ui), /data-composition-apply="assign" disabled/);
});

test('working selection stays bounded and is independent of native preview saveability', () => {
  const value = 'x'.repeat(1024);
  const state = initial({ draft, inputs: [{ ...input, content: value }] });
  const html = renderCompositionView(state, { selectedRange: { inputIndex: 0, startByte: 0, endByte: 1024 }, rangeOptionsOpen: true });
  const excerpt = html.match(/<pre data-composition-working-excerpt[^>]*>([^<]*)<\/pre>/)[1];
  assert.equal(excerpt.length, 161);
  assert.match(html, /data-composition-range-options open/);
  assert.match(html, /Preview required/);
  assert.match(html.match(/<button id="composition-save"[^>]*>/)[0], /disabled/);
});

function selectionRoot() {
  const listeners = new Map();
  const document = { activeElement: null, addEventListener() {}, removeEventListener() {}, createTextNode: value => ({ value }), defaultView: { addEventListener() {}, removeEventListener() {} } };
  let nodes = new Map();
  const create = (attribute, value = '', dataset = {}) => {
    const node = { value: '', dataset, selectionStart: 0, selectionEnd: 0, disabled: false, hidden: false, textContent: '', open: false, tagName: 'BUTTON',
      getAttributeNames: () => [attribute], getAttribute: name => name === attribute ? value : null,
      hasAttribute: name => name === attribute, setAttribute(name, value) { this[name] = value; },
      matches: selector => selector.split(',').some(s => s.trim() === `[${attribute}]`), closest: () => node,
      focus: () => { document.activeElement = node; }, setSelectionRange(start, end) { this.selectionStart = start; this.selectionEnd = end; },
    };
    nodes.set(`[${attribute}]`, node); return node;
  };
  const root = { ownerDocument: document, renders: 0, contains: node => [...nodes.values()].includes(node),
    set innerHTML(html) {
      this.html = html; this.renders++; nodes = new Map();
      create('data-composition-source').tagName = 'TEXTAREA';
      create('data-composition-range-output').tagName = 'SELECT';
      create('data-composition-range-reason').tagName = 'INPUT';
      create('data-composition-range-content').tagName = 'TEXTAREA';
      create('data-composition-range-options').open = /data-composition-range-options open/.test(html);
      create('data-composition-range-unsaved'); create('data-composition-range-binding'); create('data-composition-return-range');
      for (const attribute of ['data-composition-selection','data-composition-selection-effect','data-composition-working-title','data-composition-working-state','data-composition-working-excerpt','data-composition-working-destination','data-composition-working-target','data-composition-working-provenance']) create(attribute);
      create('data-composition-apply','assign',{ compositionApply: 'assign' });
      create('data-composition-clear-range','',{ compositionClearRange: '' });
      create('data-composition-range-summary');
    },
    querySelector(selector) { return selector === '[data-composition-range-options] summary' ? nodes.get('[data-composition-range-summary]') : nodes.get(selector) ?? null; },
    querySelectorAll(selector) {
      if (selector.includes('[data-composition-range-reason]')) return [nodes.get('[data-composition-range-reason]'),nodes.get('[data-composition-range-content]')].filter(Boolean);
      if (selector === '[data-composition-apply],[data-composition-conflict-range]') return [nodes.get('[data-composition-apply]')].filter(Boolean);
      return [];
    },
    addEventListener: (name, fn) => listeners.set(name, fn), removeEventListener: name => listeners.delete(name),
    fire: (name, target, extra = {}) => listeners.get(name)?.({ target, ...extra }),
  };
  return root;
}
const settleView = () => new Promise(resolve => setImmediate(resolve));

test('bound selection updates only contextual DOM, keeps destination and clears after successful assign', async () => {
  const root = selectionRoot(); const calls = []; const outputs = [draft.outputs[0], { title: 'Second', pieces: [] }];
  const state = initial({ draft: { ...draft, outputs }, targets: [{kind:'new'},{kind:'new'}] });
  const binding = bindCompositionView(root, { getState: () => state, applyRange(range, options) { calls.push({ range, options }); return true; } });
  const destination = root.querySelector('[data-composition-range-output]'); destination.value = '1'; root.fire('change', destination);
  const area = root.querySelector('[data-composition-source]'); area.selectionStart = 0; area.selectionEnd = 8;
  const renders = root.renders; root.fire('select', area);
  assert.equal(root.renders, renders); assert.equal(root.querySelector('[data-composition-apply]').disabled, false);
  assert.equal(root.querySelector('[data-composition-working-destination]').textContent, 'Second');
  root.fire('click', root.querySelector('[data-composition-apply]')); await settleView();
  assert.equal(calls.length, 1); assert.equal(calls[0].options.outputIndex, 1); assert.equal(calls[0].options.kind, 'assign');
  assert.equal(root.querySelector('[data-composition-range-output]').value, '1');
  assert.equal(root.querySelector('[data-composition-apply]').disabled, true);
  assert.equal(root.ownerDocument.activeElement, root.querySelector('[data-composition-source]'));
  binding.dispose();
});

test('unapplied advanced text is preserved and blocks assignment instead of being silently discarded', async () => {
  const root = selectionRoot(); let calls = 0;
  const binding = bindCompositionView(root, { getState: () => initial({ draft }), applyRange() { calls++; return true; } });
  const area = root.querySelector('[data-composition-source]'); area.selectionStart = 0; area.selectionEnd = 8; root.fire('select', area);
  const replacement = root.querySelector('[data-composition-range-content]'); replacement.value = 'Unapplied replacement'; root.fire('input', replacement);
  root.fire('click', root.querySelector('[data-composition-apply]')); await settleView();
  assert.equal(calls, 0); assert.equal(root.querySelector('[data-composition-range-content]').value, 'Unapplied replacement');
  assert.equal(root.querySelector('[data-composition-range-options]').open, true); assert.match(root.html, /Finish or cancel unapplied selection edits/);
  root.fire('click', root.querySelector('[data-composition-clear-range]')); await settleView();
  assert.equal(root.querySelector('[data-composition-range-content]').value, ''); assert.equal(root.querySelector('[data-composition-apply]').disabled, true);
  binding.dispose();
});

test('ordinary route disposal retains live fields and output/disclosure; lock purge does not', () => {
  const state = initial({ draft: { ...draft, outputs: [draft.outputs[0], {title:'Second',pieces:[]}] } });
  const controller = { getState: () => state }; const first = selectionRoot(); const binding = bindCompositionView(first, controller);
  const output = first.querySelector('[data-composition-range-output]'); output.value = '1'; first.fire('change', output);
  first.querySelector('[data-composition-range-options]').open = true;
  first.querySelector('[data-composition-range-reason]').value = 'Live unsubmitted reason';
  binding.dispose(); const second = selectionRoot(); const restored = bindCompositionView(second, controller);
  assert.equal(second.querySelector('[data-composition-range-output]').value, '1'); assert.equal(second.querySelector('[data-composition-range-options]').open, true);
  assert.equal(second.querySelector('[data-composition-range-reason]').value, 'Live unsubmitted reason');
  restored.purgeForWorkspaceLock(); const third = selectionRoot(); const clean = bindCompositionView(third, controller);
  assert.equal(third.querySelector('[data-composition-range-reason]').value, ''); assert.equal(third.querySelector('[data-composition-range-options]').open, false);
  assert.equal(third.querySelector('[data-composition-range-output]').value, '0'); clean.dispose();
});

test('missing advanced-operation reason keeps the selection and focuses an actionable validation field', async () => {
  const root = selectionRoot(); let calls = 0;
  const binding = bindCompositionView(root, { getState: () => initial({ draft }), applyRange() { calls++; return true; } });
  const area = root.querySelector('[data-composition-source]'); area.selectionStart = 0; area.selectionEnd = 8; root.fire('select', area);
  const action = root.querySelector('[data-composition-apply]'); action.dataset.compositionApply = 'replace'; root.fire('click', action);
  assert.equal(calls, 0); assert.match(root.html, /Add a reason before/);
  assert.equal(root.ownerDocument.activeElement, root.querySelector('[data-composition-range-reason]'));
  assert.equal(root.querySelector('[data-composition-apply]').disabled, false);
  const reason = root.querySelector('[data-composition-range-reason]'); reason.value = 'Explicit reason'; root.fire('input', reason);
  const retry = root.querySelector('[data-composition-apply]'); retry.dataset.compositionApply = 'replace'; root.fire('click', retry); await settleView();
  assert.equal(calls, 1); binding.dispose();
});

test('quick section selection and returning to range-bound edits update DOM without rerender or wrong-range mutation', async () => {
  const value = 'Alpha\nBeta\n'; const calls = [];
  const state = initial({ draft: { ...draft, outputs: [{title:'Output',pieces:[]}] }, inputs: [{...input,content:value,byteLength:11,sections:[{title:'Alpha',startByte:0,endByte:6},{title:'Beta',startByte:6,endByte:11}]}] });
  const root = selectionRoot(); const binding = bindCompositionView(root, {getState:()=>state,applyRange(range,options){calls.push({range,options});return true;}});
  const section = index => { const node={tagName:'BUTTON',dataset:{compositionSection:`0:${index}`},hasAttribute:()=>false,matches:()=>false};node.closest=()=>node;return node; };
  const initialRenders=root.renders;const firstSection=section(0);root.ownerDocument.activeElement=firstSection;root.fire('click',firstSection);assert.equal(root.renders,initialRenders);
  assert.equal(root.ownerDocument.activeElement,firstSection);
  assert.equal(root.querySelector('[data-composition-source]').selectionEnd,6);
  const reason=root.querySelector('[data-composition-range-reason]');reason.value='Alpha only';root.fire('input',reason);
  const replacement=root.querySelector('[data-composition-range-content]');replacement.value='New Alpha';root.fire('input',replacement);
  root.fire('click',section(1));assert.equal(root.renders,initialRenders);
  assert.equal(root.querySelector('[data-composition-apply]').disabled,true);
  assert.equal(root.querySelector('[data-composition-range-content]').disabled,true);
  assert.equal(root.querySelector('[data-composition-return-range]').hidden,false);
  assert.match(root.querySelector('[data-composition-range-binding]').textContent,/bytes 0–6/);
  const bad=root.querySelector('[data-composition-apply]');bad.dataset.compositionApply='replace';root.fire('click',bad);await settleView();assert.equal(calls.length,0);
  assert.equal(root.querySelector('[data-composition-range-content]').value,'New Alpha');
  const beforeReturn=root.renders;root.fire('click',root.querySelector('[data-composition-return-range]'));assert.equal(root.renders,beforeReturn);
  assert.equal(root.querySelector('[data-composition-source]').selectionStart,0);assert.equal(root.querySelector('[data-composition-source]').selectionEnd,6);
  const good=root.querySelector('[data-composition-apply]');good.dataset.compositionApply='replace';root.fire('click',good);await settleView();
  assert.equal(calls.length,1);assert.deepEqual(calls[0].range,{inputIndex:0,startByte:0,endByte:6});assert.equal(calls[0].options.content,'New Alpha');binding.dispose();
});

test('selection excerpt bounds large spans, preserves astral code points, and rejects invalid byte boundaries',()=>{
 const large='x'.repeat(4*1024*1024);
 assert.equal(compositionSelectionExcerpt(large,{inputIndex:0,startByte:0,endByte:large.length}),'x'.repeat(160)+'…');
 const value='a'.repeat(159)+'🦀Z';const range={inputIndex:0,startByte:0,endByte:164};
 assert.equal(compositionSelectionExcerpt(value,range),'a'.repeat(159)+'🦀…');
 assert.equal(compositionSelectionExcerpt('🦀',{inputIndex:0,startByte:1,endByte:4}),null);
 assert.equal(compositionSelectionExcerpt('abc',{inputIndex:0,startByte:0,endByte:4}),null);
 assert.equal(compositionSelectionExcerpt('\ud800',{inputIndex:0,startByte:0,endByte:3}),null);
});
