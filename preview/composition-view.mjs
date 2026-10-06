import { escapeText } from './analysis-model.mjs';
import { icon } from './icons.mjs';
import { byteRangeText, textareaSelectionRange } from './composition-model.mjs';

export { byteRangeText, textareaSelectionRange };

const text = escapeText;
const encoder = new TextEncoder();
const bytes = value => encoder.encode(String(value ?? '')).length;
const short = value => {
  const valueText = String(value ?? 'Unavailable');
  return valueText.length > 22 ? `${valueText.slice(0, 10)}…${valueText.slice(-8)}` : valueText;
};
const label = operation => operation === 'decompose' ? 'Decompose' : operation === 'merge' ? 'Merge' : 'Split';
const rangeLabel = range => range ? `Input ${Number(range.inputIndex) + 1}, bytes ${range.startByte}–${range.endByte}` : 'Range unavailable';
const pending = state => Boolean(state.pending);
const disabled = state => pending(state) || !state.bridgeAvailable;
const targetLabel = (target, detail) => target?.kind === 'append'
  ? `Update ${detail?.revision?.title ?? detail?.title ?? short(target.capabilityId)} · current head ${short(target.expectedRevisionId)}`
  : 'Create new skill';
const inputKind = input => input?.reference?.kind ?? input?.kind;
const inputTitle = input => input?.title ?? (inputKind(input) === 'source' ? 'Saved source' : 'Saved skill revision');
const inputIdentity = input => input?.reference?.sourceId ?? input?.reference?.revisionId ?? input?.sourceId ?? input?.revisionId ?? 'Unavailable';
const bindingState = new WeakMap();
const localFormSelector = '[data-composition-range-reason],[data-composition-range-content],[data-composition-add-authored],[data-composition-add-authored-reason],[data-composition-piece-content],[data-composition-piece-reason],[data-composition-annotation-reason],[data-composition-conflict-title],[data-composition-conflict-context],[data-composition-conflict-resolution]';
const compositionDataKey = node => {
  const name = node.getAttributeNames().find(attribute => attribute.startsWith('data-composition-'));
  return name ? `${name}:${node.getAttribute(name) ?? ''}` : null;
};

function option(value, selected, title) {
  return `<option value="${text(value)}" ${String(value) === String(selected) ? 'selected' : ''}>${text(title)}</option>`;
}

function capabilityOptions(state, target) {
  return ['<option value="">Create new skill</option>', ...(state.capabilities ?? []).map(item => option(item.id, target?.kind === 'append' ? target.capabilityId : '', `${item.title} · current ${short(item.latestRevisionId)}`))].join('');
}

const previewCore = state => state.preview?.preview?.core ?? null;
const previewOutput = (state, outputIndex) => previewCore(state)?.outputs?.find(item => item.outputIndex === outputIndex) ?? null;
const previewCoverage = state => previewCore(state)?.coverage ?? [];

function inputPicker(state) {
  const sourceRows = (state.sources ?? []).map(item => `<button class="composition-picker__item" type="button" data-composition-add-source="${text(item.id ?? item.sourceId)}" ${disabled(state) || state.draft ? 'disabled' : ''}><strong>${icon('source', { size: 16 })}${text(item.displayName ?? item.title ?? 'Saved source')}</strong><small>${text(short(item.id ?? item.sourceId))} · ${Number(item.byteLength ?? 0).toLocaleString()} bytes</small></button>`).join('');
  const capabilityRows = (state.capabilities ?? []).map(item => {
    const history = item.history ?? [];
    const revisions = history.length ? history : [{ id: item.latestRevisionId, title: item.title, current: true }];
    return `<details class="composition-capability"><summary>${icon('skill', { size: 15 })}${text(item.title)} <small>${text(short(item.latestRevisionId))}</small></summary><div>${revisions.map(revision => `<button class="composition-picker__revision" type="button" data-composition-add-revision="${text(item.id)}" data-composition-revision="${text(revision.id)}" ${disabled(state) || state.draft ? 'disabled' : ''}>${text(revision.title ?? item.title)}${revision.id === item.latestRevisionId ? ' · current' : ' · historical'}<small>${text(short(revision.id))}</small></button>`).join('')}</div></details>`;
  }).join('');
  const empty = state.status === 'unavailable' ? 'Open this route in Rangoon desktop app. Browser preview cannot invent saved records.' : state.status === 'loading' ? 'Loading saved workspace records…' : state.status === 'failed' ? 'Saved records unavailable. Existing draft stays in memory.' : 'No saved records available yet.';
  return `<aside class="composition-picker" aria-label="Composition input picker"><header><p class="analysis-kicker">INPUTS</p><h2>Saved records</h2></header>${state.status === 'ready' && !sourceRows && !capabilityRows ? `<p class="composition-note">${empty}</p>` : state.status !== 'ready' ? `<p class="composition-note">${empty}</p>` : ''}${state.operation === 'decompose' ? sourceRows : capabilityRows}</aside>`;
}

function selectedInputs(state) {
  const values = state.inputs ?? [];
  const locked = Boolean(state.draft);
  const countText = state.operation === 'decompose' ? 'Choose one saved source.' : state.operation === 'merge' ? 'Choose 2–16 pinned revisions.' : 'Choose one pinned revision.';
  const validInputCount = state.operation === 'merge' ? values.length >= 2 && values.length <= 16 : values.length === 1;
  const action = locked ? '<p class="composition-note">Inputs locked after draft start. Discard draft to change them.</p>' : `<button id="composition-start" class="analysis-button analysis-button--primary" type="button" ${validInputCount && !disabled(state) ? '' : 'disabled'}>${pending(state) ? 'Working…' : 'Start draft'}</button>`;
  return `<section class="composition-inputs" aria-label="Chosen composition inputs"><header><div><p class="analysis-kicker">PINNED INPUTS</p><h2>${text(countText)}</h2></div>${action}</header>${values.length ? `<ol>${values.map((item, index) => `<li><div><strong>${text(inputTitle(item))}</strong><small>${inputKind(item) === 'revision' && item.latestRevisionId && item.reference?.revisionId !== item.latestRevisionId ? 'Historical · ' : ''}${text(short(inputIdentity(item)))} · ${Number(item.content ? bytes(item.content) : item.byteLength ?? 0).toLocaleString()} bytes</small>${inputKind(item) === 'revision' && item.history?.length ? `<label class="composition-revision-choice">Revision <select data-composition-replace-revision="${index}" ${disabled(state) || locked ? 'disabled' : ''}>${item.history.map(revision => option(revision.id, item.reference?.revisionId, `${revision.title ?? item.title}${revision.id === item.latestRevisionId ? ' · current' : ' · historical'}`)).join('')}</select></label>` : ''}</div>${locked ? '' : `<div class="composition-row-actions"><button type="button" class="analysis-button analysis-button--small" data-composition-move-input="${index}" data-composition-delta="-1" ${disabled(state) || index === 0 ? 'disabled' : ''}>Move up</button><button type="button" class="analysis-button analysis-button--small" data-composition-move-input="${index}" data-composition-delta="1" ${disabled(state) || index === values.length - 1 ? 'disabled' : ''}>Move down</button><button type="button" class="analysis-button analysis-button--small" data-composition-remove-input="${index}" ${disabled(state) ? 'disabled' : ''}>Remove</button></div>`}</li>`).join('')}</ol>` : `<p class="composition-note">${text(countText)} Exact selected revisions remain pinned; no latest revision is substituted silently.</p>`}</section>`;
}

function rangeActions(state) {
  const outputs = state.draft?.outputs ?? [];
  const selection = `<p class="composition-selection" data-composition-selection>No source span selected. Select text in a read-only source pane, then choose an action.</p>`;
  return `<section class="composition-range-actions"><div><p class="analysis-kicker">RANGE ACTION</p><h3>Selected bytes</h3>${selection}</div><label>Output <select data-composition-range-output ${disabled(state) ? 'disabled' : ''}>${outputs.map((item, index) => option(index, 0, `Output ${index + 1}: ${item.title || 'Untitled'}`)).join('')}</select></label><label>Reason <input data-composition-range-reason maxlength="1024" placeholder="Required for duplicate, replace, exclude" ${disabled(state) ? 'disabled' : ''}></label><label class="composition-range-content">Replacement text <textarea data-composition-range-content maxlength="262144" placeholder="Required for Replace" ${disabled(state) ? 'disabled' : ''}></textarea></label><div class="composition-range-buttons"><button type="button" class="analysis-button" data-composition-apply="assign" ${disabled(state) ? 'disabled' : ''}>Assign to output</button><button type="button" class="analysis-button" data-composition-apply="duplicate" ${disabled(state) ? 'disabled' : ''}>Duplicate deliberately</button><button type="button" class="analysis-button" data-composition-apply="replace" ${disabled(state) ? 'disabled' : ''}>Replace</button><button type="button" class="analysis-button" data-composition-apply="exclude" ${disabled(state) ? 'disabled' : ''}>Exclude</button><button type="button" class="analysis-button analysis-button--small" data-composition-conflict-range ${disabled(state) ? 'disabled' : ''}>Add range to conflict</button></div></section>`;
}

function sourcePane(state) {
  const inputs = state.inputs ?? [];
  const sourceOptions = inputs.map((item, index) => option(index, 0, `Input ${index + 1}: ${inputTitle(item)}${item.revisionId && item.revisionId !== item.latestRevisionId ? ' · historical' : ''}`)).join('');
  const content = inputs[0]?.content ?? '';
  return `<section class="composition-source-pane" aria-label="Original pinned input"><header><div><p class="analysis-kicker">SOURCE / SELECTABLE ORIGINAL</p><h2>Exact saved text</h2></div><label>Input <select data-composition-source-input ${disabled(state) ? 'disabled' : ''}>${sourceOptions}</select></label></header><textarea class="composition-source-text" data-composition-source data-composition-scroll="source" readonly spellcheck="false" aria-label="Select source text for composition actions">${text(content)}</textarea>${rangeActions(state)}</section>`;
}

function piece(piece, outputIndex, pieceIndex, state) {
  const kind = piece.kind ?? 'unknown';
  const mutable = kind === 'authored' || kind === 'replace';
  const snippet = kind === 'copy' ? rangeLabel(piece.range) : kind === 'replace' ? `${rangeLabel(piece.range)} → replacement` : 'New authored text';
  return `<li class="composition-piece composition-piece--${text(kind)}"><header><strong>${text(kind === 'copy' ? 'Copy · source-backed' : kind === 'replace' ? 'Replace · source-backed range' : 'Authored')}</strong><span>${text(snippet)}</span></header>${mutable ? `<label>Text <textarea data-composition-piece-content="${outputIndex}:${pieceIndex}" maxlength="262144" ${disabled(state) ? 'disabled' : ''}>${text(piece.content ?? '')}</textarea></label><label>Reason <input data-composition-piece-reason="${outputIndex}:${pieceIndex}" maxlength="1024" value="${text(piece.reason ?? '')}" ${disabled(state) ? 'disabled' : ''}></label><button type="button" class="analysis-button analysis-button--small" data-composition-update-piece="${outputIndex}:${pieceIndex}" ${disabled(state) ? 'disabled' : ''}>Apply piece</button>` : `<p>Read-only reference. Select a source range to replace it deliberately.</p>`}<div class="composition-row-actions"><button type="button" class="analysis-button analysis-button--small" data-composition-move-piece="${outputIndex}:${pieceIndex}" data-composition-delta="-1" ${disabled(state) || pieceIndex === 0 ? 'disabled' : ''}>Move up</button><button type="button" class="analysis-button analysis-button--small" data-composition-move-piece="${outputIndex}:${pieceIndex}" data-composition-delta="1" ${disabled(state) ? 'disabled' : ''}>Move down</button><button type="button" class="analysis-button analysis-button--small" data-composition-remove-piece="${outputIndex}:${pieceIndex}" ${disabled(state) ? 'disabled' : ''}>Remove</button></div></li>`;
}

function outputCards(state) {
  const outputs = state.draft?.outputs ?? [];
  return `<section class="composition-output-list" aria-label="Composition output recipes"><header><div><p class="analysis-kicker">RECIPES</p><h2>Output skills</h2></div><button type="button" class="analysis-button analysis-button--small" id="composition-add-output" ${disabled(state) || state.operation === 'merge' ? 'disabled' : ''}>Add output</button></header>${outputs.map((output, index) => { const after = previewOutput(state, index); const before = state.targetDetails?.[index]?.revision?.content; return `<article class="composition-output" data-composition-output="${index}"><header><span>${icon('skill', { size: 16 })} Output ${index + 1}</span><div class="composition-row-actions"><button type="button" class="analysis-button analysis-button--small" data-composition-move-output="${index}" data-composition-delta="-1" ${disabled(state) || index === 0 ? 'disabled' : ''}>Move up</button><button type="button" class="analysis-button analysis-button--small" data-composition-move-output="${index}" data-composition-delta="1" ${disabled(state) || index === outputs.length - 1 ? 'disabled' : ''}>Move down</button><button type="button" class="analysis-button analysis-button--small" data-composition-remove-output="${index}" ${disabled(state) ? 'disabled' : ''}>Remove</button></div></header><label>Output title <input data-composition-output-title="${index}" maxlength="160" value="${text(output.title ?? '')}" ${disabled(state) ? 'disabled' : ''}></label><label>Destination <select data-composition-target="${index}" ${disabled(state) ? 'disabled' : ''}>${capabilityOptions(state, state.targets?.[index])}</select></label><p class="composition-target-state">${text(targetLabel(state.targets?.[index], state.targetDetails?.[index]))}${state.targets?.[index]?.kind === 'append' ? ` <button type="button" class="composition-link-button" data-composition-refresh-target="${index}" ${disabled(state) ? 'disabled' : ''}>Refresh current head</button>` : ''}</p>${state.targets?.[index]?.kind === 'append' ? `<details class="composition-before-after"><summary>Target before / proposed after</summary><h4>Current pinned text</h4><pre>${text(before ?? 'Current target detail unavailable.')}</pre><h4>Native preview output</h4><pre>${text(after?.content ?? 'Prepare a native preview to inspect exact after text.')}</pre></details>` : ''}<label>New authored piece <textarea data-composition-add-authored="${index}" maxlength="262144" placeholder="New prose or an explicit merge separator" ${disabled(state) ? 'disabled' : ''}></textarea></label><label>Reason <input data-composition-add-authored-reason="${index}" maxlength="1024" placeholder="Why this new text belongs here" ${disabled(state) ? 'disabled' : ''}></label><button type="button" class="analysis-button analysis-button--small" data-composition-add-authored-button="${index}" ${disabled(state) ? 'disabled' : ''}>Add authored piece</button><ol class="composition-pieces">${(output.pieces ?? []).map((value, pieceIndex) => piece(value, index, pieceIndex, state)).join('') || '<li class="composition-note">No pieces. This output cannot be saved empty.</li>'}</ol></article>`; }).join('')}</section>`;
}

function graph(state) {
  const outputs = state.draft?.outputs ?? [];
  return `<section class="composition-graph" aria-label="Composition graph and list"><header><p class="analysis-kicker">GRAPH / SAME RECORDS</p><h2>Input and output links</h2></header><div class="composition-graph__list">${(state.inputs ?? []).map((item, index) => `<div class="composition-graph__input"><strong>Input ${index + 1}</strong><span>${text(inputTitle(item))}</span></div>`).join('') || '<p class="composition-note">Choose saved inputs first.</p>'}${outputs.map((output, index) => `<div class="composition-graph__edge" aria-hidden="true">→</div><div class="composition-graph__output"><strong>Output ${index + 1}</strong><span>${text(output.title || 'Untitled')}</span><small>${(output.pieces ?? []).length} recipe piece${(output.pieces ?? []).length === 1 ? '' : 's'} · ${text(targetLabel(state.targets?.[index], state.targetDetails?.[index]))}</small></div>`).join('')}</div><p>Graph is a visual form of the editable input and recipe lists. It creates no separate records.</p></section>`;
}

function annotationList(state) {
  const draft = state.draft;
  const annotations = [['exclusions', 'Excluded ranges'], ['duplications', 'Deliberate duplicates']];
  return `<section class="composition-annotations"><header><p class="analysis-kicker">COVERAGE RECORDS</p><h2>Exceptions and conflicts</h2></header>${annotations.map(([kind, title]) => `<div><h3>${title}</h3>${(draft?.[kind] ?? []).map((value, index) => `<article><strong>${text(rangeLabel(value.range))}</strong><label>Reason <input data-composition-annotation-reason="${kind}:${index}" maxlength="1024" value="${text(value.reason ?? '')}" ${disabled(state) ? 'disabled' : ''}></label><button type="button" class="analysis-button analysis-button--small" data-composition-annotation-apply="${kind}:${index}" ${disabled(state) ? 'disabled' : ''}>Apply</button><button type="button" class="analysis-button analysis-button--small" data-composition-remove-annotation="${kind}:${index}" ${disabled(state) ? 'disabled' : ''}>Remove</button></article>`).join('') || '<p class="composition-note">None.</p>'}</div>`).join('')}<div class="composition-conflicts"><h3>Manual conflicts</h3><p>Declarations document local context. A resolution is not proof of semantic safety.</p>${(draft?.conflicts ?? []).map((value, index) => `<article><strong>${text(value.title)}</strong><small>${(value.ranges ?? []).map(rangeLabel).join(' · ')}</small><label>Resolution <textarea data-composition-conflict-resolution="${index}" maxlength="1024" placeholder="Unresolved blocks saving">${text(value.resolution ?? '')}</textarea></label><button type="button" class="analysis-button analysis-button--small" data-composition-resolve-conflict="${index}" ${disabled(state) ? 'disabled' : ''}>Apply resolution</button><button type="button" class="analysis-button analysis-button--small" data-composition-remove-conflict="${index}" ${disabled(state) ? 'disabled' : ''}>Remove</button></article>`).join('') || '<p class="composition-note">No declared conflicts.</p>'}<label>Conflict title <input data-composition-conflict-title maxlength="160" ${disabled(state) ? 'disabled' : ''}></label><label>Context <textarea data-composition-conflict-context maxlength="1024" ${disabled(state) ? 'disabled' : ''}></textarea></label><p class="composition-note" data-composition-conflict-basket>Select at least two source ranges with “Add range to conflict” in inspector.</p><button type="button" class="analysis-button analysis-button--small" data-composition-add-conflict ${disabled(state) ? 'disabled' : ''}>Add manual conflict</button></div></section>`;
}

function previewPanel(state) {
  const envelope = state.preview;
  const preview = envelope?.preview ?? envelope;
  const core = preview?.core ?? preview;
  const diagnostics = core?.diagnostics ?? preview?.diagnostics ?? [];
  const outputs = core?.outputs ?? preview?.outputs ?? preview?.outputDetails ?? [];
  const previewReady = Boolean(envelope?.previewId && envelope?.expectedStateId);
  const saveable = Boolean(preview?.saveable);
  const stale = Boolean(state.previewStale);
  const saved = Array.isArray(state.results) && state.results.length > 0;
  return `<section class="composition-preview" aria-label="Native composition preview"><header><div><p class="analysis-kicker">NATIVE PREVIEW</p><h2>${saved ? 'Composition saved' : stale ? 'Preview stale' : previewReady ? saveable ? 'Preview ready' : 'Preview blocked' : 'Preview required'}</h2></div><button id="composition-request-preview" type="button" class="analysis-button" ${disabled(state) || !state.draft ? 'disabled' : ''}>${state.pending === 'preview' ? 'Checking…' : 'Preview composition'}</button></header><p role="status" aria-live="polite">${saved ? 'Saved local composition. Open returned skills below.' : stale ? 'Recipe changed or preview failed. Inspect a new native preview before saving.' : previewReady ? 'Host returned this exact preview; it is not an approval or execution authority.' : 'Native host computes output content, coverage and saveability.'}</p>${diagnostics.length ? `<ul class="composition-diagnostics">${diagnostics.map(item => `<li>${text(item.message ?? item.code ?? 'Blocked by local validation.')}</li>`).join('')}</ul>` : ''}${outputs.length ? `<div class="composition-preview-outputs">${outputs.map((output, index) => `<details><summary>Output ${Number(output.outputIndex ?? index) + 1}: ${text(output.title ?? state.draft?.outputs?.[index]?.title ?? 'Untitled')} · ${Number(output.byteLength ?? bytes(output.content ?? '')).toLocaleString()} bytes · ${text(preview?.appliedOutputs?.[index]?.kind === 'append' ? 'update existing' : 'create new')}</summary><pre>${text(output.content ?? '')}</pre></details>`).join('')}</div>` : ''}${saved ? `<div class="composition-results"><h3>Actual saved skills</h3>${state.results.map(item => `<button type="button" class="composition-result" data-composition-open-skill="${text(item.capability?.id ?? item.id ?? item.capabilityId ?? '')}">${icon('skill', { size: 16 })}${text(item.capability?.revision?.title ?? item.revision?.title ?? item.title ?? 'Saved skill')}<small>${text(short(item.capability?.id ?? item.id ?? item.capabilityId))} · unreviewed · authority none</small></button>`).join('')}</div>` : `<label class="composition-ack"><input type="checkbox" data-composition-ack ${state.acknowledged ? 'checked' : ''} ${!previewReady || stale || !saveable || disabled(state) ? 'disabled' : ''}> I reviewed exact host preview content, destinations and coverage. This does not approve execution.</label><button id="composition-save" type="button" class="analysis-button analysis-button--primary" ${!previewReady || stale || !saveable || !state.acknowledged || disabled(state) ? 'disabled' : ''}>${state.pending === 'save' ? 'Saving locally…' : 'Save composition'}</button>`}</section>`;
}

function coveragePanel(state) {
  const coverage = previewCoverage(state);
  return `<section class="composition-coverage"><p class="analysis-kicker">COVERAGE</p><h2>Actual ledger</h2>${coverage.length ? `<ol>${coverage.map(item => `<li><strong>${text(item.disposition ?? 'unknown')}</strong><span>${text(rangeLabel(item.range))}${item.duplicationAcknowledged ? ' · acknowledged duplicate' : ''}</span></li>`).join('')}</ol>` : '<p>Prepare a native preview to inspect copied, duplicated, replaced, excluded and unassigned byte spans.</p>'}<button type="button" data-composition-jump-unassigned class="analysis-button analysis-button--small" ${coverage.some(item => item.disposition === 'unassigned') ? '' : 'disabled'}>Reveal first unassigned span</button></section>`;
}

export function renderCompositionView(state) {
  const operation = state.operation ?? 'decompose';
  const phase = state.draft ? 'recipe' : 'inputs';
  return `<section class="composition-page" aria-labelledby="composition-title" aria-busy="${pending(state)}"><header class="composition-hero"><p class="analysis-kicker">${icon(operation === 'merge' ? 'merge' : 'decompose', { size: 16 })} LOCAL COMPOSITION / ${text(operation.toUpperCase())}</p><h1 id="composition-title" tabindex="-1">${text(label(operation))} <em>saved content.</em></h1><p>${operation === 'decompose' ? 'Turn one saved source into explicitly covered local skills.' : operation === 'merge' ? 'Combine pinned saved revisions with explicit provenance and separators.' : 'Divide one pinned saved revision into explicit output recipes.'} No provider, engine, file scan or execution runs here.</p><p class="composition-status${state.error ? ' composition-status--error' : ''}" role="status">${text(state.message ?? '')}</p></header>${!state.draft ? `<div class="composition-start-grid">${inputPicker(state)}${selectedInputs(state)}</div>` : `<nav class="composition-tabs" aria-label="Composition editor tabs"><button type="button" data-composition-tab="source" aria-selected="true">Source</button><button type="button" data-composition-tab="recipe" aria-selected="false">Recipe</button><button type="button" data-composition-tab="graph" aria-selected="false">Graph</button><span>${state.dirty ? 'Unsaved draft' : 'Saved local result'}</span><button type="button" id="composition-reset" class="analysis-button analysis-button--small" ${disabled(state) ? 'disabled' : ''}>Discard draft</button></nav><div class="composition-workbench"><aside class="composition-left">${selectedInputs(state)}${coveragePanel(state)}</aside><main class="composition-main"><div data-composition-panel="source">${sourcePane(state)}</div><div data-composition-panel="recipe" hidden>${outputCards(state)}</div><div data-composition-panel="graph" hidden>${graph(state)}</div></main><aside class="composition-inspector"><header><p class="analysis-kicker">INSPECTOR</p><h2>Recipe details</h2></header><p>Original input remains read-only. Reasons and annotations remain visible with every preview.</p>${annotationList(state)}</aside></div>${previewPanel(state)}`}</section>`;
}

export function bindCompositionView(root, controller, { onOpenSkill = () => {} } = {}) {
  const retained = bindingState.get(controller) ?? { tab: 'source', sourceInputIndex: 0, selectedRange: null, conflictRanges: [], focus: null, scroll: new Map(), formDrafts: new Map(), reveal: null };
  let { tab, sourceInputIndex, selectedRange, conflictRanges, focus, scroll, formDrafts, reveal } = retained;
  const invoke = action => { Promise.resolve(action()).finally(render); };
  const remember = () => {
    const active = root.ownerDocument?.activeElement;
    if (active && root.contains(active)) {
      const name = active.getAttributeNames?.().find(attribute => attribute.startsWith('data-composition-'));
      if (name) focus = { selector: `[${name}${active.getAttribute(name) ? `="${CSS.escape(active.getAttribute(name))}"` : ''}]`, start: active.selectionStart, end: active.selectionEnd };
    }
    root.querySelectorAll(localFormSelector).forEach(node => formDrafts.set(compositionDataKey(node), { value: node.value, start: node.selectionStart, end: node.selectionEnd }));
    root.querySelectorAll('[data-composition-scroll]').forEach(node => scroll.set(node.dataset.compositionScroll, { top: node.scrollTop, left: node.scrollLeft }));
  };
  const render = () => {
    remember();
    root.innerHTML = renderCompositionView(controller.getState());
    root.querySelectorAll('[data-composition-panel]').forEach(node => { node.hidden = node.dataset.compositionPanel !== tab; });
    root.querySelectorAll('[data-composition-tab]').forEach(node => node.setAttribute('aria-selected', String(node.dataset.compositionTab === tab)));
    const textarea = root.querySelector('[data-composition-source]');
    const item = controller.getState().inputs?.[sourceInputIndex];
    if (textarea && item) textarea.value = item.content ?? '';
    const inputSelect = root.querySelector('[data-composition-source-input]');
    if (inputSelect) inputSelect.value = String(sourceInputIndex);
    root.querySelectorAll(localFormSelector).forEach(node => {
      const key = compositionDataKey(node);
      const value = formDrafts.get(key);
      if (value) { node.value = value.value; if (Number.isInteger(value.start) && Number.isInteger(value.end) && 'setSelectionRange' in node) node.setSelectionRange(value.start, value.end); }
    });
    root.querySelectorAll('[data-composition-scroll]').forEach(node => {
      const saved = scroll.get(node.dataset.compositionScroll);
      if (saved) { node.scrollTop = saved.top; node.scrollLeft = saved.left; }
    });
    const description = root.querySelector('[data-composition-selection]');
    if (description) description.textContent = selectedRange ? `${rangeLabel(selectedRange)} selected. ${byteRangeText(item?.content ?? '', selectedRange) ?? ''}` : 'No source span selected. Select text in a read-only source pane, then choose an action.';
    root.querySelector('[data-composition-conflict-basket]')?.replaceChildren(root.ownerDocument.createTextNode(conflictRanges.length ? `${conflictRanges.length} conflict range${conflictRanges.length === 1 ? '' : 's'} selected. Add at least two.` : 'Select at least two source ranges with “Add range to conflict” in inspector.'));
    if (reveal) {
      const revealItem = controller.getState().inputs?.[reveal.inputIndex];
      const start = normalizedOffsetForByte(revealItem?.content ?? '', reveal.startByte);
      const end = normalizedOffsetForByte(revealItem?.content ?? '', reveal.endByte);
      if (textarea && start !== null && end !== null) { textarea.focus({ preventScroll: true }); textarea.setSelectionRange(start, end); }
      reveal = null;
    } else if (focus) {
      const node = root.querySelector(focus.selector);
      node?.focus({ preventScroll: true });
      if (node && Number.isInteger(focus.start) && Number.isInteger(focus.end) && 'setSelectionRange' in node) node.setSelectionRange(focus.start, focus.end);
    }
  };
  const selection = () => {
    const area = root.querySelector('[data-composition-source]');
    const item = controller.getState().inputs?.[sourceInputIndex];
    if (!area || !item) return null;
    const range = textareaSelectionRange(item.content ?? '', area.selectionStart, area.selectionEnd, sourceInputIndex);
    if (range) selectedRange = range;
    return range;
  };
  const clicked = event => {
    const target = event.target.closest('button,select');
    if (!target) return;
    if (target.dataset.compositionTab) { tab = target.dataset.compositionTab; return render(); }
    if (target.matches('#composition-start')) return invoke(() => controller.startDraft());
    if (target.matches('#composition-reset')) { if (root.ownerDocument.defaultView?.confirm('Discard this in-memory composition draft? Selected inputs remain.')) invoke(() => controller.resetDraft()); return; }
    if (target.dataset.compositionAddSource) return invoke(() => controller.addSource(target.dataset.compositionAddSource));
    if (target.dataset.compositionAddRevision) return invoke(() => controller.addRevision(target.dataset.compositionAddRevision, target.dataset.compositionRevision));
    if (target.dataset.compositionRemoveInput) return invoke(() => controller.removeInput(Number(target.dataset.compositionRemoveInput)));
    if (target.dataset.compositionMoveInput) return invoke(() => controller.moveInput(Number(target.dataset.compositionMoveInput), Number(target.dataset.compositionDelta)));
    if (target.dataset.compositionApply) {
      const range = selection(); const index = Number(root.querySelector('[data-composition-range-output]')?.value ?? 0);
      if (!range) return render();
      const reason = root.querySelector('[data-composition-range-reason]')?.value ?? '';
      const content = root.querySelector('[data-composition-range-content]')?.value ?? '';
      return invoke(() => controller.applyRange(range, { kind: target.dataset.compositionApply, outputIndex: index, content, reason }));
    }
    if (target.matches('[data-composition-conflict-range]')) { const range = selection(); if (range && !conflictRanges.some(item => item.inputIndex === range.inputIndex && item.startByte === range.startByte && item.endByte === range.endByte)) conflictRanges = [...conflictRanges, range]; return render(); }
    if (target.dataset.compositionMoveOutput) return invoke(() => controller.moveOutput(Number(target.dataset.compositionMoveOutput), Number(target.dataset.compositionDelta)));
    if (target.dataset.compositionRemoveOutput) return invoke(() => controller.removeOutput(Number(target.dataset.compositionRemoveOutput)));
    if (target.matches('#composition-add-output')) return invoke(() => controller.addOutput('Untitled output'));
    if (target.dataset.compositionRefreshTarget) return invoke(() => controller.refreshTarget(Number(target.dataset.compositionRefreshTarget)));
    if (target.dataset.compositionAddAuthoredButton) { const index = Number(target.dataset.compositionAddAuthoredButton); return invoke(() => controller.addAuthored(index, root.querySelector(`[data-composition-add-authored="${index}"]`)?.value ?? '', root.querySelector(`[data-composition-add-authored-reason="${index}"]`)?.value ?? '')); }
    if (target.dataset.compositionUpdatePiece) { const [outputIndex, pieceIndex] = target.dataset.compositionUpdatePiece.split(':').map(Number); return invoke(() => controller.updatePiece(outputIndex, pieceIndex, root.querySelector(`[data-composition-piece-content="${outputIndex}:${pieceIndex}"]`)?.value ?? '', root.querySelector(`[data-composition-piece-reason="${outputIndex}:${pieceIndex}"]`)?.value ?? '')); }
    if (target.dataset.compositionMovePiece) { const [outputIndex, pieceIndex] = target.dataset.compositionMovePiece.split(':').map(Number); return invoke(() => controller.movePiece(outputIndex, pieceIndex, Number(target.dataset.compositionDelta))); }
    if (target.dataset.compositionRemovePiece) { const [outputIndex, pieceIndex] = target.dataset.compositionRemovePiece.split(':').map(Number); return invoke(() => controller.removePiece(outputIndex, pieceIndex)); }
    if (target.dataset.compositionAnnotationApply) { const [kind, index] = target.dataset.compositionAnnotationApply.split(':'); return invoke(() => controller.setAnnotationReason(kind, Number(index), root.querySelector(`[data-composition-annotation-reason="${kind}:${index}"]`)?.value ?? '')); }
    if (target.dataset.compositionRemoveAnnotation) { const [kind, index] = target.dataset.compositionRemoveAnnotation.split(':'); return invoke(() => controller.removeAnnotation(kind, Number(index))); }
    if (target.dataset.compositionResolveConflict) return invoke(() => controller.resolveConflict(Number(target.dataset.compositionResolveConflict), root.querySelector(`[data-composition-conflict-resolution="${target.dataset.compositionResolveConflict}"]`)?.value ?? ''));
    if (target.dataset.compositionRemoveConflict) return invoke(() => controller.removeConflict(Number(target.dataset.compositionRemoveConflict)));
    if (target.matches('[data-composition-add-conflict]')) { const ranges = conflictRanges.length >= 2 ? conflictRanges : selectedRange ? [selectedRange] : []; return invoke(() => controller.addConflict({ title: root.querySelector('[data-composition-conflict-title]')?.value ?? '', context: root.querySelector('[data-composition-conflict-context]')?.value ?? '', ranges, resolution: null })); }
    if (target.matches('[data-composition-jump-unassigned]')) {
      const current = controller.getState();
      const core = current.preview?.preview?.core ?? current.preview?.preview ?? current.preview;
      const range = core?.coverage?.find(item => item?.disposition === 'unassigned')?.range;
      if (range) { selectedRange = range; sourceInputIndex = range.inputIndex; tab = 'source'; reveal = range; }
      return render();
    }
    if (target.matches('#composition-request-preview')) return invoke(() => controller.requestPreview());
    if (target.matches('#composition-save')) return invoke(() => controller.save());
    if (target.dataset.compositionOpenSkill) return onOpenSkill(target.dataset.compositionOpenSkill);
  };
  const changed = event => {
    const target = event.target;
    if (target.matches('[data-composition-source-input]')) { sourceInputIndex = Number(target.value); selectedRange = null; return render(); }
    if (target.dataset.compositionReplaceRevision) return invoke(() => controller.replaceRevision(Number(target.dataset.compositionReplaceRevision), target.value));
    if (target.dataset.compositionOutputTitle) return invoke(() => controller.setOutputTitle(Number(target.dataset.compositionOutputTitle), target.value));
    if (target.dataset.compositionTarget) return invoke(() => controller.setTarget(Number(target.dataset.compositionTarget), target.value || null));
    if (target.matches('[data-composition-ack]')) return invoke(() => controller.acknowledge(target.checked));
  };
  const key = event => { if (event.key === 'Escape') { selectedRange = null; render(); } if (event.key === 'Enter' && (event.metaKey || event.ctrlKey) && event.target.matches('[data-composition-source]')) selection(); };
  root.addEventListener('click', clicked);
  root.addEventListener('change', changed);
  root.addEventListener('keyup', selection);
  root.addEventListener('keydown', key);
  render();
  return { render, dispose: () => { bindingState.set(controller, { tab, sourceInputIndex, selectedRange, conflictRanges, focus, scroll, formDrafts, reveal }); root.removeEventListener('click', clicked); root.removeEventListener('change', changed); root.removeEventListener('keyup', selection); root.removeEventListener('keydown', key); } };
}

function normalizedOffsetForByte(content, byte) {
  let original = 0; let normalized = 0; let total = 0;
  const source = String(content ?? '');
  while (original <= source.length) {
    if (total === byte) return normalized;
    if (original === source.length) break;
    const code = source.codePointAt(original);
    const width = code > 0xffff ? 2 : 1;
    if (source[original] === '\r') {
      const next = source[original + 1] === '\n' ? 2 : 1;
      total += bytes(source.slice(original, original + next)); original += next; normalized += 1;
    } else { total += bytes(String.fromCodePoint(code)); original += width; normalized += width; }
  }
  return null;
}
