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
const whole = value => Number.isSafeInteger(value) && value >= 0;
const overlap = (left, right) => left.inputIndex === right.inputIndex && left.startByte < right.endByte && right.startByte < left.endByte;
const sameRange = (left, right) => left.inputIndex === right.inputIndex && left.startByte === right.startByte && left.endByte === right.endByte;
const safeRange = (value, inputIndex = 0) => {
  const range = value?.range ?? value;
  return range && whole(range.inputIndex ?? inputIndex) && whole(range.startByte) && whole(range.endByte) && range.endByte > range.startByte
    ? { inputIndex: range.inputIndex ?? inputIndex, startByte: range.startByte, endByte: range.endByte }
    : null;
};
const pieceRange = piece => (piece?.kind === 'copy' || piece?.kind === 'replace') ? safeRange(piece.range) : null;
const sourceExcerpt = (state, range) => {
  const value = byteRangeText(state.inputs?.[range?.inputIndex]?.content ?? '', range)?.replace(/\s+/g, ' ').trim();
  return value ? (value.length > 72 ? `${value.slice(0, 69)}…` : value) : 'Source text unavailable';
};
const structuralNotice = 'Apply or cancel text edits before moving, reordering, undoing, or removing graph records.';
const rangeIdentity = range => range ? `${range.inputIndex}:${range.startByte}:${range.endByte}` : '';
const cacheLayout = state => {
  const draft = state?.draft;
  if (!draft) return null;
  return {
    inputs: (draft.inputs ?? []).map(input => JSON.stringify(input)),
    outputs: (draft.outputs ?? []).map(output => ({
      title: output.title,
      pieces: (output.pieces ?? []).map(piece => ({ kind: piece.kind, range: rangeIdentity(pieceRange(piece)), content: piece.content ?? '', reason: piece.reason ?? '' })),
    })),
    annotations: ['exclusions', 'duplications'].map(kind => (draft[kind] ?? []).map(item => rangeIdentity(item.range))),
    conflicts: (draft.conflicts ?? []).map(item => `${item.id}:${(item.ranges ?? []).map(rangeIdentity).join(',')}`),
  };
};
const sameValues = (left, right) => JSON.stringify(left) === JSON.stringify(right);
function cacheLayoutChanged(previous, next, previousVersion, nextVersion) {
  if (previous === null || next === null) return previous !== next;
  if (nextVersion - previousVersion > 1 || !sameValues(previous.inputs, next.inputs) || !sameValues(previous.annotations, next.annotations) || !sameValues(previous.conflicts, next.conflicts) || previous.outputs.length !== next.outputs.length) return true;
  let changedValues = 0;
  for (let outputIndex = 0; outputIndex < previous.outputs.length; outputIndex += 1) {
    const left = previous.outputs[outputIndex]; const right = next.outputs[outputIndex];
    if (left.pieces.length !== right.pieces.length) return true;
    if (left.title !== right.title) changedValues += 1;
    for (let pieceIndex = 0; pieceIndex < left.pieces.length; pieceIndex += 1) {
      const before = left.pieces[pieceIndex]; const after = right.pieces[pieceIndex];
      if (before.kind !== after.kind || before.range !== after.range) return true;
      if (before.content !== after.content || before.reason !== after.reason) changedValues += 1;
    }
  }
  return changedValues > 1;
}

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
  return `<section class="composition-range-actions"><div><p class="analysis-kicker">RANGE ACTION</p><h3>Selected bytes</h3>${selection}</div><label>Output <select data-composition-range-output ${disabled(state) ? 'disabled' : ''}>${outputs.map((item, index) => option(index, 0, `Output ${index + 1}: ${item.title || 'Untitled'}`)).join('')}</select></label><label>Reason <input data-composition-range-reason maxlength="1024" placeholder="Required for duplicate, replace, exclude" ${disabled(state) ? 'disabled' : ''}></label><label class="composition-range-content">Replacement text <textarea data-composition-range-content maxlength="262144" placeholder="Required for Replace" ${disabled(state) ? 'disabled' : ''}></textarea></label><div class="composition-range-buttons"><button type="button" class="analysis-button" data-composition-apply="assign" ${disabled(state) ? 'disabled' : ''}>Assign to output</button><button type="button" class="analysis-button" data-composition-apply="duplicate" ${disabled(state) ? 'disabled' : ''}>Duplicate deliberately</button><button type="button" class="analysis-button" data-composition-apply="replace" ${disabled(state) ? 'disabled' : ''}>Replace</button><button type="button" class="analysis-button" data-composition-apply="exclude" ${disabled(state) ? 'disabled' : ''}>Exclude</button><button type="button" class="analysis-button analysis-button--small" data-composition-conflict-range ${disabled(state) ? 'disabled' : ''}>Add range to conflict</button><button type="button" class="analysis-button analysis-button--small" data-composition-clear-range ${disabled(state) ? 'disabled' : ''}>Cancel</button></div></section>`;
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
  const snippet = kind === 'copy' || kind === 'replace' ? sourceExcerpt(state, piece.range) : 'New authored text';
  return `<li class="composition-piece composition-piece--${text(kind)}"><header><strong>${text(kind === 'copy' ? 'Copy · source-backed' : kind === 'replace' ? 'Replace · source-backed range' : 'Authored')}</strong><span>${text(snippet)}</span>${pieceRange(piece) ? `<small>${text(rangeLabel(piece.range))}</small>` : ''}</header>${mutable ? `<label>Text <textarea data-composition-piece-content="${outputIndex}:${pieceIndex}" maxlength="262144" ${disabled(state) ? 'disabled' : ''}>${text(piece.content ?? '')}</textarea></label><label>Reason <input data-composition-piece-reason="${outputIndex}:${pieceIndex}" maxlength="1024" value="${text(piece.reason ?? '')}" ${disabled(state) ? 'disabled' : ''}></label><div class="composition-row-actions"><button type="button" class="analysis-button analysis-button--small" data-composition-update-piece="${outputIndex}:${pieceIndex}" ${disabled(state) ? 'disabled' : ''}>Apply piece</button><button type="button" class="analysis-button analysis-button--small" data-composition-clear-piece="${outputIndex}:${pieceIndex}" ${disabled(state) ? 'disabled' : ''}>Cancel</button></div>` : `<p>Read-only reference. Select a source range to replace it deliberately.</p>`}<div class="composition-row-actions"><button type="button" class="analysis-button analysis-button--small" data-composition-move-piece="${outputIndex}:${pieceIndex}" data-composition-delta="-1" ${disabled(state) || pieceIndex === 0 ? 'disabled' : ''}>Move up</button><button type="button" class="analysis-button analysis-button--small" data-composition-move-piece="${outputIndex}:${pieceIndex}" data-composition-delta="1" ${disabled(state) ? 'disabled' : ''}>Move down</button><button type="button" class="analysis-button analysis-button--small" data-composition-remove-piece="${outputIndex}:${pieceIndex}" ${disabled(state) ? 'disabled' : ''}>Remove</button></div></li>`;
}

function outputCards(state) {
  const outputs = state.draft?.outputs ?? [];
  return `<section class="composition-output-list" aria-label="Composition output recipes"><header><div><p class="analysis-kicker">RECIPES</p><h2>Output skills</h2></div><button type="button" class="analysis-button analysis-button--small" id="composition-add-output" ${disabled(state) || state.operation === 'merge' || outputs.length >= 16 ? 'disabled' : ''}>Add output</button></header>${outputs.map((output, index) => { const after = previewOutput(state, index); const before = state.targetDetails?.[index]?.revision?.content; return `<article class="composition-output" data-composition-output="${index}"><header><span>${icon('skill', { size: 16 })} Output ${index + 1}</span><div class="composition-row-actions"><button type="button" class="analysis-button analysis-button--small" data-composition-move-output="${index}" data-composition-delta="-1" ${disabled(state) || index === 0 ? 'disabled' : ''}>Move up</button><button type="button" class="analysis-button analysis-button--small" data-composition-move-output="${index}" data-composition-delta="1" ${disabled(state) || index === outputs.length - 1 ? 'disabled' : ''}>Move down</button><button type="button" class="analysis-button analysis-button--small" data-composition-remove-output="${index}" ${disabled(state) ? 'disabled' : ''}>Remove</button></div></header><label>Output title <input data-composition-output-title="${index}" maxlength="160" value="${text(output.title ?? '')}" ${disabled(state) ? 'disabled' : ''}></label><label>Destination <select data-composition-target="${index}" ${disabled(state) ? 'disabled' : ''}>${capabilityOptions(state, state.targets?.[index])}</select></label><p class="composition-target-state">${text(targetLabel(state.targets?.[index], state.targetDetails?.[index]))}${state.targets?.[index]?.kind === 'append' ? ` <button type="button" class="composition-link-button" data-composition-refresh-target="${index}" ${disabled(state) ? 'disabled' : ''}>Refresh current head</button>` : ''}</p>${state.targets?.[index]?.kind === 'append' ? `<details class="composition-before-after"><summary>Target before / proposed after</summary><h4>Current pinned text</h4><pre>${text(before ?? 'Current target detail unavailable.')}</pre><h4>Native preview output</h4><pre>${text(after?.content ?? 'Prepare a native preview to inspect exact after text.')}</pre></details>` : ''}<label>New authored piece <textarea data-composition-add-authored="${index}" maxlength="262144" placeholder="New prose or an explicit merge separator" ${disabled(state) ? 'disabled' : ''}></textarea></label><label>Reason <input data-composition-add-authored-reason="${index}" maxlength="1024" placeholder="Why this new text belongs here" ${disabled(state) ? 'disabled' : ''}></label><div class="composition-row-actions"><button type="button" class="analysis-button analysis-button--small" data-composition-add-authored-button="${index}" ${disabled(state) ? 'disabled' : ''}>Add authored piece</button><button type="button" class="analysis-button analysis-button--small" data-composition-clear-authored="${index}" ${disabled(state) ? 'disabled' : ''}>Cancel</button></div><ol class="composition-pieces">${(output.pieces ?? []).map((value, pieceIndex) => piece(value, index, pieceIndex, state)).join('') || '<li class="composition-note">No pieces. This output cannot be saved empty.</li>'}</ol></article>`; }).join('')}</section>`;
}

function graphSourceSections(state, inputIndex) {
  const input = state.inputs?.[inputIndex];
  return Array.isArray(input?.sections) ? input.sections.map((section, index) => {
    const range = safeRange(section, inputIndex);
    return range ? { range, title: String(section?.title ?? section?.heading ?? section?.label ?? `Section ${index + 1}`) } : null;
  }).filter(Boolean) : [];
}
function graphSourceRanges(state, inputIndex) {
  return graphSourceSections(state, inputIndex).map(section => section.range);
}

function graphMenu(state, ui) {
  const menu = ui.menu;
  if (!menu) return '';
  const outputs = state.draft?.outputs ?? [];
  const outputChoices = outputs.map((output, index) => `<button type="button" role="menuitem" data-composition-menu-action="${menu.kind === 'source' ? 'assign-source' : 'move-piece'}" data-composition-menu-output="${index}">${menu.kind === 'source' ? 'Assign to' : 'Move to'} Output ${index + 1}: ${text(output.title || 'Untitled')}</button>`).join('');
  const actions = menu.kind === 'piece'
    ? `<button type="button" role="menuitem" data-composition-menu-action="reveal-piece">Inspect source</button>${outputChoices}<button type="button" role="menuitem" data-composition-menu-action="remove-piece">Remove piece</button>`
    : menu.kind === 'source'
      ? `<button type="button" role="menuitem" data-composition-menu-action="reveal-source">Reveal source</button>${menu.range ? outputChoices : ''}<button type="button" role="menuitem" data-composition-menu-action="source-tab">Use Source actions</button>`
      : `<button type="button" role="menuitem" data-composition-menu-action="add-text">Add text block</button>${ui.connectFrom?.kind === 'piece' ? '<button type="button" role="menuitem" data-composition-menu-action="move-here">Move selected piece here</button>' : ''}`;
  return `<div class="composition-context" data-composition-context role="menu" aria-label="Graph actions" tabindex="-1">${actions}</div>`;
}

function graph(state, ui = {}) {
  const outputs = state.draft?.outputs ?? [];
  const locked = disabled(state);
  const sources = (state.inputs ?? []).map((item, inputIndex) => {
    const sections = graphSourceSections(state, inputIndex);
    return `<section class="composition-graph__source"><header><button type="button" class="composition-graph__source-title" data-composition-graph-source="${inputIndex}" aria-label="Source ${inputIndex + 1} actions" ${locked ? 'disabled' : ''}><span class="composition-port" aria-hidden="true"></span><strong>Source ${inputIndex + 1}</strong><small>${text(inputTitle(item))}</small></button></header>${sections.length ? `<ol>${sections.map((section, sectionIndex) => `<li><button type="button" draggable="${locked ? 'false' : 'true'}" data-composition-graph-section="${inputIndex}:${sectionIndex}" data-composition-draft-version="${Number(state.draftVersion ?? 0)}" title="${text(rangeLabel(section.range))}" ${locked ? 'disabled' : ''}>${text(section.title)}<small>${text(rangeLabel(section.range))}</small><span class="composition-port composition-port--source" aria-hidden="true"></span></button></li>`).join('')}</ol>` : '<p>No analyzer sections. Select an exact source range in Source.</p>'}</section>`;
  }).join('') || '<p class="composition-note">Choose saved inputs first.</p>';
  const outputCards = outputs.map((output, outputIndex) => `<article class="composition-graph__output" data-composition-graph-output="${outputIndex}" data-composition-drop-output="${outputIndex}"><header><button type="button" class="composition-graph__output-title" data-composition-graph-output-button="${outputIndex}" aria-label="Output ${outputIndex + 1} actions" ${locked ? 'disabled' : ''}><span class="composition-port" aria-hidden="true"></span><strong>Output ${outputIndex + 1}</strong><small>${text(output.title || 'Untitled')}</small></button><button type="button" class="composition-actions" data-composition-graph-output-actions="${outputIndex}" aria-label="Output ${outputIndex + 1} actions" ${locked ? 'disabled' : ''}>Actions</button></header><p>${text(targetLabel(state.targets?.[outputIndex], state.targetDetails?.[outputIndex]))}</p><ol>${(output.pieces ?? []).map((value, pieceIndex) => { const range = pieceRange(value); const active = ui.connectFrom?.kind === 'piece' && ui.connectFrom.outputIndex === outputIndex && ui.connectFrom.pieceIndex === pieceIndex; return `<li class="composition-graph__piece${active ? ' is-connecting' : ''}" data-composition-drop-piece="${outputIndex}:${pieceIndex}"><button type="button" draggable="${locked ? 'false' : 'true'}" data-composition-graph-piece="${outputIndex}:${pieceIndex}" data-composition-draft-version="${Number(state.draftVersion ?? 0)}" aria-pressed="${active}" title="${text(range ? sourceExcerpt(state, range) : value.kind === 'authored' ? 'Authored text block' : 'Piece')}" ${locked ? 'disabled' : ''}><strong>${text(value.kind === 'copy' ? 'Copy' : value.kind === 'replace' ? 'Replace' : 'Authored')}</strong><small>${text(range ? sourceExcerpt(state, range) : value.reason || 'Authored text')}</small><small>${text(range ? rangeLabel(range) : 'No source range')}</small><span class="composition-port composition-port--piece" aria-hidden="true"></span></button><button type="button" class="composition-actions" data-composition-graph-piece-actions="${outputIndex}:${pieceIndex}" aria-label="Piece ${pieceIndex + 1} actions" ${locked ? 'disabled' : ''}>Actions</button></li>`; }).join('') || '<li class="composition-graph__empty">No pieces yet. Drop a section or add authored text.</li>'}</ol></article>`).join('');
  return `<section class="composition-graph" aria-label="Editable composition graph"><header><div><p class="analysis-kicker">GRAPH / SAME RECORDS</p><h2>Recipe connections</h2></div><button type="button" class="analysis-button analysis-button--small" data-composition-graph-add-output ${disabled(state) || state.operation === 'merge' || outputs.length >= 16 ? 'disabled' : ''}>Add output</button></header><p class="composition-graph__help">Lines label exact source ranges. Drag or connect a piece to move it. Source sections assign only when whole section remains unassigned; duplicate and replace stay in Source actions.</p>${ui.notice ? `<p class="composition-graph__notice" role="status">${text(ui.notice)}</p>` : ''}<div class="composition-canvas"><svg class="composition-canvas__wires" data-composition-wires aria-hidden="true"></svg><div class="composition-canvas__sources">${sources}</div><div class="composition-canvas__links" aria-label="Derived recipe links">${outputs.flatMap((output, outputIndex) => (output.pieces ?? []).map((value, pieceIndex) => { const range = pieceRange(value); return `<div class="composition-canvas__link"><span class="composition-canvas__line" aria-hidden="true"></span><span>${text(range ? `${rangeLabel(range)} → Output ${outputIndex + 1}` : `Authored → Output ${outputIndex + 1}`)}</span></div>`; })).join('') || '<p>No piece links yet.</p>'}</div><div class="composition-canvas__outputs">${outputCards}</div></div>${graphMenu(state, ui)}<p>Graph edits same ordered recipe records. It does not create executable workflow nodes.</p></section>`;
}

function annotationList(state) {
  const draft = state.draft;
  const annotations = [['exclusions', 'Excluded ranges'], ['duplications', 'Deliberate duplicates']];
  return `<section class="composition-annotations"><header><p class="analysis-kicker">COVERAGE RECORDS</p><h2>Exceptions and conflicts</h2></header>${annotations.map(([kind, title]) => `<div><h3>${title}</h3>${(draft?.[kind] ?? []).map((value, index) => `<article><strong>${text(rangeLabel(value.range))}</strong><label>Reason <input data-composition-annotation-reason="${kind}:${index}" maxlength="1024" value="${text(value.reason ?? '')}" ${disabled(state) ? 'disabled' : ''}></label><button type="button" class="analysis-button analysis-button--small" data-composition-annotation-apply="${kind}:${index}" ${disabled(state) ? 'disabled' : ''}>Apply</button><button type="button" class="analysis-button analysis-button--small" data-composition-remove-annotation="${kind}:${index}" ${disabled(state) ? 'disabled' : ''}>Remove</button></article>`).join('') || '<p class="composition-note">None.</p>'}</div>`).join('')}<div class="composition-conflicts"><h3>Manual conflicts</h3><p>Declarations document local context. A resolution is not proof of semantic safety.</p>${(draft?.conflicts ?? []).map((value, index) => `<article><strong>${text(value.title)}</strong><small>${(value.ranges ?? []).map(rangeLabel).join(' · ')}</small><label>Resolution <textarea data-composition-conflict-resolution="${index}" maxlength="1024" placeholder="Unresolved blocks saving">${text(value.resolution ?? '')}</textarea></label><button type="button" class="analysis-button analysis-button--small" data-composition-resolve-conflict="${index}" ${disabled(state) ? 'disabled' : ''}>Apply resolution</button><button type="button" class="analysis-button analysis-button--small" data-composition-remove-conflict="${index}" ${disabled(state) ? 'disabled' : ''}>Remove</button></article>`).join('') || '<p class="composition-note">No declared conflicts.</p>'}<label>Conflict title <input data-composition-conflict-title maxlength="160" ${disabled(state) ? 'disabled' : ''}></label><label>Context <textarea data-composition-conflict-context maxlength="1024" ${disabled(state) ? 'disabled' : ''}></textarea></label><p class="composition-note" data-composition-conflict-basket>Select at least two source ranges with “Add range to conflict” in inspector.</p><div class="composition-row-actions"><button type="button" class="analysis-button analysis-button--small" data-composition-add-conflict ${disabled(state) ? 'disabled' : ''}>Add manual conflict</button><button type="button" class="analysis-button analysis-button--small" data-composition-clear-conflict-form ${disabled(state) ? 'disabled' : ''}>Cancel</button></div></div></section>`;
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

export function renderCompositionView(state, graphUi = {}) {
  const operation = state.operation ?? 'decompose';
  const phase = state.draft ? 'recipe' : 'inputs';
  return `<section class="composition-page" aria-labelledby="composition-title" aria-busy="${pending(state)}"><header class="composition-hero"><p class="analysis-kicker">${icon(operation === 'merge' ? 'merge' : 'decompose', { size: 16 })} LOCAL COMPOSITION / ${text(operation.toUpperCase())}</p><h1 id="composition-title" tabindex="-1">${text(label(operation))} <em>saved content.</em></h1><p>${operation === 'decompose' ? 'Turn one saved source into explicitly covered local skills.' : operation === 'merge' ? 'Combine pinned saved revisions with explicit provenance and separators.' : 'Divide one pinned saved revision into explicit output recipes.'} No provider, engine, file scan or execution runs here.</p><p class="composition-status${state.error ? ' composition-status--error' : ''}" role="status">${text(state.message ?? '')}</p></header>${!state.draft ? `<div class="composition-start-grid">${inputPicker(state)}${selectedInputs(state)}</div>` : `<nav class="composition-tabs" aria-label="Composition editor tabs"><button type="button" data-composition-tab="source" aria-selected="true">Source</button><button type="button" data-composition-tab="recipe" aria-selected="false">Recipe</button><button type="button" data-composition-tab="graph" aria-selected="false">Graph</button><span>${state.dirty ? 'Unsaved draft' : 'Saved local result'}</span><div class="composition-history"><button type="button" class="analysis-button analysis-button--small" data-composition-undo ${disabled(state) || !state.canUndo ? 'disabled' : ''}>Undo</button><button type="button" class="analysis-button analysis-button--small" data-composition-redo ${disabled(state) || !state.canRedo ? 'disabled' : ''}>Redo</button></div><button type="button" id="composition-reset" class="analysis-button analysis-button--small" ${disabled(state) ? 'disabled' : ''}>Discard draft</button></nav><div class="composition-workbench"><aside class="composition-left">${selectedInputs(state)}${coveragePanel(state)}</aside><main class="composition-main"><div data-composition-panel="source">${sourcePane(state)}</div><div data-composition-panel="recipe" hidden>${outputCards(state)}</div><div data-composition-panel="graph" hidden>${graph(state, graphUi)}</div></main><aside class="composition-inspector"><header><p class="analysis-kicker">INSPECTOR</p><h2>Recipe details</h2></header><p>Original input remains read-only. Reasons and annotations remain visible with every preview.</p>${annotationList(state)}</aside></div>${previewPanel(state)}`}</section>`;
}

export function bindCompositionView(root, controller, { onOpenSkill = () => {} } = {}) {
  const retained = bindingState.get(controller) ?? { tab: 'source', sourceInputIndex: 0, selectedRange: null, focus: null, nextFocus: null, conflictRanges: [], scroll: new Map(), formDrafts: new Map(), formBaseline: new Map(), reveal: null, menu: null, connectFrom: null, notice: '', drag: null, menuFocus: null, cacheLayout: null, draftVersion: null };
  let { tab, sourceInputIndex, selectedRange, conflictRanges, focus, nextFocus, scroll, formDrafts, formBaseline = new Map(), reveal, menu, connectFrom, notice, drag, menuFocus, cacheLayout: retainedCacheLayout = null, draftVersion: retainedDraftVersion = null } = retained;
  const clearDraftKeys = new Set();
  let discardFormDrafts = false;
  let graphResizeObserver = null;
  let lastCacheLayout = retainedCacheLayout;
  let lastDraftVersion = retainedDraftVersion;
  const invoke = action => { Promise.resolve(action()).finally(render); };
  const invokeClear = (action, keys) => { Promise.resolve(action()).then(result => { if (result) keys.forEach(key => clearDraftKeys.add(key)); }).finally(render); };
  const formValueChanged = node => {
    const key = compositionDataKey(node); const baseline = formBaseline.get(key);
    return baseline ? node.value !== baseline.value : Boolean(node.value.length);
  };
  const hasPendingFormEdits = (ignored = new Set()) => [...root.querySelectorAll(localFormSelector)].some(node => !ignored.has(compositionDataKey(node)) && formValueChanged(node));
  const invokeStructural = action => { Promise.resolve(action()).then(result => { if (result) discardFormDrafts = true; }).finally(render); };
  const guardStructural = action => {
    if (hasPendingFormEdits()) { notice = structuralNotice; tab = 'graph'; menu = null; render(); return; }
    notice = ''; invokeStructural(action);
  };
  const clearLocal = key => { clearDraftKeys.add(key); notice = ''; render(); };
  const remember = () => {
    const active = root.ownerDocument?.activeElement;
    if (active && root.contains(active)) {
      const name = active.getAttributeNames?.().find(attribute => attribute.startsWith('data-composition-'));
      if (name) focus = { selector: `[${name}${active.getAttribute(name) ? `="${CSS.escape(active.getAttribute(name))}"` : ''}]`, start: active.selectionStart, end: active.selectionEnd };
    }
    if (discardFormDrafts) formDrafts.clear();
    root.querySelectorAll(localFormSelector).forEach(node => {
      const key = compositionDataKey(node);
      if (discardFormDrafts || clearDraftKeys.has(key)) formDrafts.delete(key);
      else if (formValueChanged(node)) formDrafts.set(key, { value: node.value, start: node.selectionStart, end: node.selectionEnd });
      else formDrafts.delete(key);
    });
    discardFormDrafts = false;
    clearDraftKeys.clear();
    root.querySelectorAll('[data-composition-scroll]').forEach(node => scroll.set(node.dataset.compositionScroll, { top: node.scrollTop, left: node.scrollLeft }));
  };
  const drawGraphEdges = () => {
    const canvas = root.querySelector('.composition-canvas');
    const wires = root.querySelector('[data-composition-wires]');
    const sources = root.querySelector('.composition-canvas__sources');
    const outputs = root.querySelector('.composition-canvas__outputs');
    if (!canvas || !wires || !sources || !outputs) return;
    wires.replaceChildren();
    const canvasBox = canvas.getBoundingClientRect(); const sourcesBox = sources.getBoundingClientRect(); const outputsBox = outputs.getBoundingClientRect();
    if (!canvasBox.width || !canvasBox.height || outputsBox.left <= sourcesBox.left + 1) return;
    wires.setAttribute('viewBox', `0 0 ${canvasBox.width} ${canvasBox.height}`);
    wires.setAttribute('width', String(canvasBox.width)); wires.setAttribute('height', String(canvasBox.height));
    const state = controller.getState();
    const sectionRanges = new Map();
    const sourcePorts = [...root.querySelectorAll('[data-composition-graph-section]')].map(node => {
      const [inputIndex, sectionIndex] = node.dataset.compositionGraphSection.split(':').map(Number);
      if (!sectionRanges.has(inputIndex)) sectionRanges.set(inputIndex, graphSourceRanges(state, inputIndex));
      return { range: sectionRanges.get(inputIndex)?.[sectionIndex] ?? null, box: node.querySelector('.composition-port')?.getBoundingClientRect() ?? null };
    }).filter(item => item.range && item.box);
    const piecePorts = new Map();
    (state.draft?.outputs ?? []).forEach((output, outputIndex) => (output.pieces ?? []).forEach((_, pieceIndex) => {
      const node = root.querySelector(`[data-composition-graph-piece="${outputIndex}:${pieceIndex}"]`);
      const box = node?.querySelector('.composition-port')?.getBoundingClientRect();
      if (box) piecePorts.set(`${outputIndex}:${pieceIndex}`, box);
    }));
    (state.draft?.outputs ?? []).forEach((output, outputIndex) => (output.pieces ?? []).forEach((value, pieceIndex) => {
      const range = pieceRange(value);
      if (!range) return;
      const from = sourcePorts.find(item => item.range.inputIndex === range.inputIndex && item.range.startByte <= range.startByte && item.range.endByte >= range.endByte)?.box;
      const to = piecePorts.get(`${outputIndex}:${pieceIndex}`);
      if (!from || !to) return;
      const startX = from.right - canvasBox.left; const startY = from.top + from.height / 2 - canvasBox.top;
      const endX = to.left - canvasBox.left; const endY = to.top + to.height / 2 - canvasBox.top;
      const bend = Math.max(24, Math.abs(endX - startX) * .45);
      const path = root.ownerDocument.createElementNS('http://www.w3.org/2000/svg', 'path');
      path.setAttribute('d', `M ${startX} ${startY} C ${startX + bend} ${startY}, ${endX - bend} ${endY}, ${endX} ${endY}`);
      wires.append(path);
    }));
  };
  const observeGraphEdges = () => {
    graphResizeObserver?.disconnect(); graphResizeObserver = null;
    const canvas = root.querySelector('.composition-canvas');
    if (!canvas || tab !== 'graph') return;
    drawGraphEdges();
    const Observer = root.ownerDocument.defaultView?.ResizeObserver;
    if (Observer) { graphResizeObserver = new Observer(drawGraphEdges); graphResizeObserver.observe(canvas); }
  };
  const render = () => {
    graphResizeObserver?.disconnect(); graphResizeObserver = null;
    const renderState = controller.getState();
    const nextLayout = cacheLayout(renderState);
    if (lastDraftVersion !== null && cacheLayoutChanged(lastCacheLayout, nextLayout, lastDraftVersion, Number(renderState.draftVersion ?? 0))) {
      discardFormDrafts = true; formDrafts.clear(); clearDraftKeys.clear();
      if (!nextLayout) { selectedRange = null; conflictRanges = []; connectFrom = null; drag = null; menu = null; }
    }
    remember();
    const requestedFocus = nextFocus;
    nextFocus = null;
    root.innerHTML = renderCompositionView(renderState, { menu, connectFrom, notice });
    root.querySelectorAll('[data-composition-panel]').forEach(node => { node.hidden = node.dataset.compositionPanel !== tab; });
    root.querySelectorAll('[data-composition-tab]').forEach(node => node.setAttribute('aria-selected', String(node.dataset.compositionTab === tab)));
    const textarea = root.querySelector('[data-composition-source]');
    const item = renderState.inputs?.[sourceInputIndex];
    if (textarea && item) textarea.value = item.content ?? '';
    const inputSelect = root.querySelector('[data-composition-source-input]');
    if (inputSelect) inputSelect.value = String(sourceInputIndex);
    observeGraphEdges();
    formBaseline = new Map([...root.querySelectorAll(localFormSelector)].map(node => [compositionDataKey(node), { value: node.value }]));
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
    } else if (requestedFocus ?? focus) {
      const next = requestedFocus ?? focus;
      const node = root.querySelector(next.selector);
      node?.focus({ preventScroll: true });
      if (node && Number.isInteger(next.start) && Number.isInteger(next.end) && 'setSelectionRange' in node) node.setSelectionRange(next.start, next.end);
    }
    if (menuFocus) { root.querySelector('[data-composition-context] button')?.focus({ preventScroll: true }); menuFocus = null; }
    lastCacheLayout = nextLayout; lastDraftVersion = Number(renderState.draftVersion ?? 0);
  };
  const selection = () => {
    const area = root.querySelector('[data-composition-source]');
    const item = controller.getState().inputs?.[sourceInputIndex];
    if (!area || !item) return null;
    const range = textareaSelectionRange(item.content ?? '', area.selectionStart, area.selectionEnd, sourceInputIndex);
    if (range) selectedRange = range;
    return range;
  };
  const sourceSection = (inputIndex, sectionIndex) => graphSourceRanges(controller.getState(), inputIndex)[sectionIndex] ?? null;
  const rangeAssignable = range => {
    const draft = controller.getState().draft;
    if (!draft || !graphSourceRanges(controller.getState(), range.inputIndex).some(section => sameRange(section, range))) return false;
    const assigned = draft.outputs.flatMap(output => output.pieces.map(pieceRange).filter(Boolean));
    return !assigned.some(item => overlap(item, range)) && !(draft.exclusions ?? []).map(item => safeRange(item.range)).filter(Boolean).some(item => overlap(item, range));
  };
  const revealRange = range => { selectedRange = range; sourceInputIndex = range.inputIndex; tab = 'source'; reveal = range; nextFocus = { selector: '[data-composition-source]' }; menu = null; connectFrom = null; render(); };
  const openMenu = (next, origin) => { menu = { ...next, version: next.version ?? (next.range ? Number(controller.getState().draftVersion ?? 0) : undefined), origin }; menuFocus = true; render(); };
  const closeMenu = () => { const origin = menu?.origin; menu = null; if (origin) nextFocus = { selector: origin }; render(); };
  const focusSelector = node => {
    if (!node) return null;
    if (node.id) return `#${CSS.escape(node.id)}`;
    const name = node.getAttributeNames?.().find(attribute => attribute.startsWith('data-composition-'));
    return name ? `[${name}${node.getAttribute(name) ? `="${CSS.escape(node.getAttribute(name))}"` : ''}]` : null;
  };
  const tabExitTarget = backwards => {
    const context = root.querySelector('[data-composition-context]');
    const origin = menu?.origin ? root.querySelector(menu.origin) : null;
    if (backwards) return focusSelector(origin);
    const NodeType = root.ownerDocument.defaultView?.Node;
    const controls = [...root.querySelectorAll('button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[href],[tabindex]:not([tabindex="-1"])')].filter(node => !node.closest('[data-composition-context]') && node.getClientRects().length && !node.hidden);
    const next = context && NodeType ? controls.find(node => Boolean(context.compareDocumentPosition(node) & NodeType.DOCUMENT_POSITION_FOLLOWING)) : null;
    return focusSelector(next) ?? focusSelector(origin);
  };
  const movePieceTo = (fromOutputIndex, pieceIndex, toOutputIndex, beforeIndex = null, expectedVersion = null) => {
    if (hasPendingFormEdits()) { notice = structuralNotice; render(); return; }
    if (expectedVersion !== null && Number(controller.getState().draftVersion ?? 0) !== expectedVersion) { notice = 'Graph changed before drop. Start move again.'; drag = null; render(); return; }
    notice = ''; connectFrom = null; menu = null; nextFocus = { selector: `[data-composition-graph-output-button="${toOutputIndex}"]` };
    const version = expectedVersion ?? Number(controller.getState().draftVersion ?? 0);
    invokeStructural(() => typeof controller.movePieceTo === 'function' ? controller.movePieceTo(fromOutputIndex, pieceIndex, toOutputIndex, beforeIndex, version) : false);
  };
  const assignSource = (range, outputIndex, expectedVersion = null) => {
    if (hasPendingFormEdits()) { notice = structuralNotice; render(); return; }
    if (expectedVersion !== null && Number(controller.getState().draftVersion ?? 0) !== expectedVersion) { notice = 'Graph changed before assignment. Start assignment again.'; drag = null; render(); return; }
    if (!rangeAssignable(range)) { notice = 'Section already assigned, replaced, excluded, or not a complete analyzer section. Use Source actions for deliberate duplicate or replace.'; connectFrom = null; menu = null; render(); return; }
    notice = ''; connectFrom = null; menu = null; nextFocus = { selector: `[data-composition-graph-output-button="${outputIndex}"]` }; invokeStructural(() => controller.applyRange(range, { kind: 'assign', outputIndex }));
  };
  const clicked = event => {
    const target = event.target.closest('button,select');
    if (!target) return;
    if (target.tagName !== 'SELECT' && !target.closest('[data-composition-context]') && menu && !target.matches('[data-composition-graph-source], [data-composition-graph-section], [data-composition-graph-piece], [data-composition-graph-output-button], [data-composition-graph-output-actions], [data-composition-graph-piece-actions]')) menu = null;
    if (target.dataset.compositionTab) { tab = target.dataset.compositionTab; return render(); }
    if (target.hasAttribute('data-composition-undo')) return guardStructural(() => typeof controller.undo === 'function' ? controller.undo() : false);
    if (target.hasAttribute('data-composition-redo')) return guardStructural(() => typeof controller.redo === 'function' ? controller.redo() : false);
    if (target.matches('#composition-start')) return invoke(() => controller.startDraft());
    if (target.matches('#composition-reset')) {
      if (!root.ownerDocument.defaultView?.confirm('Discard this in-memory composition draft? Selected inputs remain.')) return;
      return Promise.resolve(controller.resetDraft()).then(result => {
        if (!result) return;
        discardFormDrafts = true; formDrafts.clear(); selectedRange = null; conflictRanges = []; connectFrom = null; drag = null; menu = null; notice = ''; nextFocus = null;
      }).finally(render);
    }
    if (target.dataset.compositionAddSource) return invoke(() => controller.addSource(target.dataset.compositionAddSource));
    if (target.dataset.compositionAddRevision) return invoke(() => controller.addRevision(target.dataset.compositionAddRevision, target.dataset.compositionRevision));
    if (target.dataset.compositionRemoveInput) return invoke(() => controller.removeInput(Number(target.dataset.compositionRemoveInput)));
    if (target.dataset.compositionMoveInput) return invoke(() => controller.moveInput(Number(target.dataset.compositionMoveInput), Number(target.dataset.compositionDelta)));
    if (target.dataset.compositionApply) {
      const range = selection(); const index = Number(root.querySelector('[data-composition-range-output]')?.value ?? 0);
      if (!range) return render();
      const reason = root.querySelector('[data-composition-range-reason]')?.value ?? '';
      const content = root.querySelector('[data-composition-range-content]')?.value ?? '';
      const currentRangeFields = new Set(['data-composition-range-reason:', 'data-composition-range-content:']);
      if (hasPendingFormEdits(currentRangeFields)) { notice = structuralNotice; return render(); }
      notice = ''; return invokeStructural(() => controller.applyRange(range, { kind: target.dataset.compositionApply, outputIndex: index, content, reason }));
    }
    if (target.hasAttribute('data-composition-clear-range')) { clearDraftKeys.add('data-composition-range-reason:'); clearDraftKeys.add('data-composition-range-content:'); return clearLocal(''); }
    if (target.matches('[data-composition-conflict-range]')) { const range = selection(); if (range && !conflictRanges.some(item => item.inputIndex === range.inputIndex && item.startByte === range.startByte && item.endByte === range.endByte)) conflictRanges = [...conflictRanges, range]; return render(); }
    if (target.dataset.compositionMoveOutput) return guardStructural(() => controller.moveOutput(Number(target.dataset.compositionMoveOutput), Number(target.dataset.compositionDelta)));
    if (target.dataset.compositionRemoveOutput) return guardStructural(() => controller.removeOutput(Number(target.dataset.compositionRemoveOutput)));
    if (target.matches('#composition-add-output')) return guardStructural(() => controller.addOutput('Untitled output'));
    if (target.dataset.compositionRefreshTarget) return invoke(() => controller.refreshTarget(Number(target.dataset.compositionRefreshTarget)));
    if (target.dataset.compositionAddAuthoredButton) { const index = Number(target.dataset.compositionAddAuthoredButton); return invokeClear(() => controller.addAuthored(index, root.querySelector(`[data-composition-add-authored="${index}"]`)?.value ?? '', root.querySelector(`[data-composition-add-authored-reason="${index}"]`)?.value ?? ''), [`data-composition-add-authored:${index}`, `data-composition-add-authored-reason:${index}`]); }
    if (target.dataset.compositionClearAuthored) { const index = Number(target.dataset.compositionClearAuthored); clearDraftKeys.add(`data-composition-add-authored:${index}`); clearDraftKeys.add(`data-composition-add-authored-reason:${index}`); return clearLocal(''); }
    if (target.dataset.compositionUpdatePiece) { const [outputIndex, pieceIndex] = target.dataset.compositionUpdatePiece.split(':').map(Number); return invokeClear(() => controller.updatePiece(outputIndex, pieceIndex, root.querySelector(`[data-composition-piece-content="${outputIndex}:${pieceIndex}"]`)?.value ?? '', root.querySelector(`[data-composition-piece-reason="${outputIndex}:${pieceIndex}"]`)?.value ?? ''), [`data-composition-piece-content:${outputIndex}:${pieceIndex}`, `data-composition-piece-reason:${outputIndex}:${pieceIndex}`]); }
    if (target.dataset.compositionClearPiece) { const [outputIndex, pieceIndex] = target.dataset.compositionClearPiece.split(':').map(Number); clearDraftKeys.add(`data-composition-piece-content:${outputIndex}:${pieceIndex}`); clearDraftKeys.add(`data-composition-piece-reason:${outputIndex}:${pieceIndex}`); return clearLocal(''); }
    if (target.dataset.compositionMovePiece) { const [outputIndex, pieceIndex] = target.dataset.compositionMovePiece.split(':').map(Number); return guardStructural(() => controller.movePiece(outputIndex, pieceIndex, Number(target.dataset.compositionDelta))); }
    if (target.dataset.compositionRemovePiece) { const [outputIndex, pieceIndex] = target.dataset.compositionRemovePiece.split(':').map(Number); return guardStructural(() => controller.removePiece(outputIndex, pieceIndex)); }
    if (target.dataset.compositionAnnotationApply) { const [kind, index] = target.dataset.compositionAnnotationApply.split(':'); return invokeClear(() => controller.setAnnotationReason(kind, Number(index), root.querySelector(`[data-composition-annotation-reason="${kind}:${index}"]`)?.value ?? ''), [`data-composition-annotation-reason:${kind}:${index}`]); }
    if (target.dataset.compositionRemoveAnnotation) { const [kind, index] = target.dataset.compositionRemoveAnnotation.split(':'); return guardStructural(() => controller.removeAnnotation(kind, Number(index))); }
    if (target.dataset.compositionResolveConflict) return invokeClear(() => controller.resolveConflict(Number(target.dataset.compositionResolveConflict), root.querySelector(`[data-composition-conflict-resolution="${target.dataset.compositionResolveConflict}"]`)?.value ?? ''), [`data-composition-conflict-resolution:${target.dataset.compositionResolveConflict}`]);
    if (target.dataset.compositionRemoveConflict) return guardStructural(() => controller.removeConflict(Number(target.dataset.compositionRemoveConflict)));
    if (target.matches('[data-composition-add-conflict]')) { const ranges = conflictRanges.length >= 2 ? conflictRanges : selectedRange ? [selectedRange] : []; return invokeClear(() => controller.addConflict({ title: root.querySelector('[data-composition-conflict-title]')?.value ?? '', context: root.querySelector('[data-composition-conflict-context]')?.value ?? '', ranges, resolution: null }), ['data-composition-conflict-title:', 'data-composition-conflict-context:']); }
    if (target.hasAttribute('data-composition-clear-conflict-form')) { clearDraftKeys.add('data-composition-conflict-title:'); clearDraftKeys.add('data-composition-conflict-context:'); return clearLocal(''); }
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
    if (target.hasAttribute('data-composition-graph-add-output')) return guardStructural(() => controller.addOutput('Untitled output'));
    if (target.dataset.compositionGraphSource !== undefined) return openMenu({ kind: 'source', inputIndex: Number(target.dataset.compositionGraphSource), range: null }, `[data-composition-graph-source="${target.dataset.compositionGraphSource}"]`);
    if (target.dataset.compositionGraphSection) {
      const [inputIndex, sectionIndex] = target.dataset.compositionGraphSection.split(':').map(Number); const range = sourceSection(inputIndex, sectionIndex);
      if (!range) return;
      connectFrom = { kind: 'source', range, version: Number(controller.getState().draftVersion ?? 0) }; notice = rangeAssignable(range) ? 'Section selected. Choose an output port to assign it.' : 'Section is already assigned, replaced, excluded, or not available for assignment.'; return render();
    }
    if (target.dataset.compositionGraphPiece) {
      const [outputIndex, pieceIndex] = target.dataset.compositionGraphPiece.split(':').map(Number);
      if (connectFrom?.kind === 'piece') return movePieceTo(connectFrom.outputIndex, connectFrom.pieceIndex, outputIndex, pieceIndex, connectFrom.version);
      connectFrom = { kind: 'piece', outputIndex, pieceIndex, version: Number(controller.getState().draftVersion ?? 0) }; notice = 'Piece selected. Choose an output or another piece as destination.'; return render();
    }
    if (target.dataset.compositionGraphOutputButton !== undefined) {
      const outputIndex = Number(target.dataset.compositionGraphOutputButton);
      if (connectFrom?.kind === 'piece') return movePieceTo(connectFrom.outputIndex, connectFrom.pieceIndex, outputIndex, null, connectFrom.version);
      if (connectFrom?.kind === 'source') return assignSource(connectFrom.range, outputIndex, connectFrom.version);
      return openMenu({ kind: 'output', outputIndex }, `[data-composition-graph-output-button="${outputIndex}"]`);
    }
    if (target.dataset.compositionGraphOutputActions !== undefined) return openMenu({ kind: 'output', outputIndex: Number(target.dataset.compositionGraphOutputActions) }, `[data-composition-graph-output-actions="${target.dataset.compositionGraphOutputActions}"]`);
    if (target.dataset.compositionGraphPieceActions) { const [outputIndex, pieceIndex] = target.dataset.compositionGraphPieceActions.split(':').map(Number); return openMenu({ kind: 'piece', outputIndex, pieceIndex }, `[data-composition-graph-piece-actions="${target.dataset.compositionGraphPieceActions}"]`); }
    if (target.dataset.compositionMenuAction) {
      const action = target.dataset.compositionMenuAction; const outputIndex = Number(target.dataset.compositionMenuOutput);
      if (action === 'reveal-piece') { const range = pieceRange(controller.getState().draft?.outputs?.[menu.outputIndex]?.pieces?.[menu.pieceIndex]); return range ? revealRange(range) : closeMenu(); }
      if (action === 'remove-piece') { const current = menu; return guardStructural(() => controller.removePiece(current.outputIndex, current.pieceIndex)); }
      if (action === 'move-piece') { const current = menu; return movePieceTo(current.outputIndex, current.pieceIndex, outputIndex); }
      if (action === 'assign-source') return menu?.range ? assignSource(menu.range, outputIndex, menu.version) : closeMenu();
      if (action === 'reveal-source') return menu?.range ? revealRange(menu.range) : closeMenu();
      if (action === 'source-tab') { tab = 'source'; menu = null; nextFocus = { selector: '[data-composition-source]' }; return render(); }
      if (action === 'add-text') { const output = menu?.outputIndex; tab = 'recipe'; menu = null; nextFocus = { selector: `[data-composition-add-authored="${output}"]` }; return render(); }
      if (action === 'move-here' && connectFrom?.kind === 'piece') return movePieceTo(connectFrom.outputIndex, connectFrom.pieceIndex, menu.outputIndex, null, connectFrom.version);
    }
  };
  const changed = event => {
    const target = event.target;
    if (target.matches('[data-composition-source-input]')) { sourceInputIndex = Number(target.value); selectedRange = null; return render(); }
    if (target.dataset.compositionReplaceRevision) return invoke(() => controller.replaceRevision(Number(target.dataset.compositionReplaceRevision), target.value));
    if (target.dataset.compositionOutputTitle) return invokeClear(() => controller.setOutputTitle(Number(target.dataset.compositionOutputTitle), target.value), [`data-composition-output-title:${target.dataset.compositionOutputTitle}`]);
    if (target.dataset.compositionTarget) return invoke(() => controller.setTarget(Number(target.dataset.compositionTarget), target.value || null));
    if (target.matches('[data-composition-ack]')) return invoke(() => controller.acknowledge(target.checked));
  };
  const key = event => {
    if ((event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey)) && event.target.matches?.('[data-composition-graph-piece], [data-composition-graph-section], [data-composition-graph-output-button], [data-composition-graph-source]')) {
      event.preventDefault();
      if (event.target.dataset.compositionGraphPiece) { const [outputIndex, pieceIndex] = event.target.dataset.compositionGraphPiece.split(':').map(Number); return openMenu({ kind: 'piece', outputIndex, pieceIndex }, `[data-composition-graph-piece="${event.target.dataset.compositionGraphPiece}"]`); }
      if (event.target.dataset.compositionGraphSection) { const [inputIndex, sectionIndex] = event.target.dataset.compositionGraphSection.split(':').map(Number); const range = sourceSection(inputIndex, sectionIndex); return range ? openMenu({ kind: 'source', range }, `[data-composition-graph-section="${event.target.dataset.compositionGraphSection}"]`) : undefined; }
      if (event.target.dataset.compositionGraphOutputButton !== undefined) return openMenu({ kind: 'output', outputIndex: Number(event.target.dataset.compositionGraphOutputButton) }, `[data-composition-graph-output-button="${event.target.dataset.compositionGraphOutputButton}"]`);
      return openMenu({ kind: 'source', inputIndex: Number(event.target.dataset.compositionGraphSource), range: null }, `[data-composition-graph-source="${event.target.dataset.compositionGraphSource}"]`);
    }
    if (event.key === 'Escape') { if (menu) return closeMenu(); connectFrom = null; selectedRange = null; notice = ''; return render(); }
    if (event.key === 'Tab' && menu) { event.preventDefault(); nextFocus = { selector: tabExitTarget(event.shiftKey) }; menu = null; return render(); }
    if (menu && ['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
      const items = [...root.querySelectorAll('[data-composition-context] button:not(:disabled)')]; const current = items.indexOf(root.ownerDocument.activeElement); if (!items.length) return;
      event.preventDefault(); const index = event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1 : (current + (event.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length; items[index].focus(); return;
    }
    if (event.key === 'Enter' && (event.metaKey || event.ctrlKey) && event.target.matches('[data-composition-source]')) selection();
  };
  const context = event => {
    const target = event.target.closest('[data-composition-graph-piece], [data-composition-graph-section], [data-composition-graph-output-button], [data-composition-graph-source]');
    if (!target) return;
    event.preventDefault();
    if (target.dataset.compositionGraphPiece) { const [outputIndex, pieceIndex] = target.dataset.compositionGraphPiece.split(':').map(Number); return openMenu({ kind: 'piece', outputIndex, pieceIndex }, `[data-composition-graph-piece="${target.dataset.compositionGraphPiece}"]`); }
    if (target.dataset.compositionGraphSection) { const [inputIndex, sectionIndex] = target.dataset.compositionGraphSection.split(':').map(Number); const range = sourceSection(inputIndex, sectionIndex); return range ? openMenu({ kind: 'source', range }, `[data-composition-graph-section="${target.dataset.compositionGraphSection}"]`) : undefined; }
    if (target.dataset.compositionGraphOutputButton !== undefined) return openMenu({ kind: 'output', outputIndex: Number(target.dataset.compositionGraphOutputButton) }, `[data-composition-graph-output-button="${target.dataset.compositionGraphOutputButton}"]`);
    return openMenu({ kind: 'source', inputIndex: Number(target.dataset.compositionGraphSource), range: null }, `[data-composition-graph-source="${target.dataset.compositionGraphSource}"]`);
  };
  const dragstart = event => {
    const target = event.target.closest('[data-composition-graph-piece], [data-composition-graph-section]');
    drag = null;
    if (!target || hasPendingFormEdits()) { if (hasPendingFormEdits()) notice = structuralNotice; return; }
    if (target.dataset.compositionGraphPiece) { const [outputIndex, pieceIndex] = target.dataset.compositionGraphPiece.split(':').map(Number); drag = { kind: 'piece', outputIndex, pieceIndex, version: Number(target.dataset.compositionDraftVersion ?? 0) }; }
    else { const [inputIndex, sectionIndex] = target.dataset.compositionGraphSection.split(':').map(Number); const range = sourceSection(inputIndex, sectionIndex); if (!range) return; drag = { kind: 'source', range, version: Number(target.dataset.compositionDraftVersion ?? 0) }; }
    event.dataTransfer?.setDragImage?.(target, 8, 8);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
  };
  const dragover = event => { if (drag && event.target.closest('[data-composition-drop-output], [data-composition-drop-piece]')) event.preventDefault(); };
  const drop = event => {
    const target = event.target.closest('[data-composition-drop-output], [data-composition-drop-piece]');
    if (!target) return;
    if (!drag || event.dataTransfer?.files?.length || [...(event.dataTransfer?.types ?? [])].includes('Files')) { drag = null; event.preventDefault(); return; }
    event.preventDefault(); const current = drag; drag = null;
    const outputIndex = target.dataset.compositionDropPiece ? Number(target.dataset.compositionDropPiece.split(':')[0]) : Number(target.dataset.compositionDropOutput);
    if (current.kind === 'source') {
      if (Number(controller.getState().draftVersion ?? 0) !== current.version) { notice = 'Graph changed before drop. Start assignment again.'; render(); return; }
      return assignSource(current.range, outputIndex);
    }
    if (target.dataset.compositionDropPiece) { const [, pieceIndex] = target.dataset.compositionDropPiece.split(':').map(Number); return movePieceTo(current.outputIndex, current.pieceIndex, outputIndex, pieceIndex, current.version); }
    return movePieceTo(current.outputIndex, current.pieceIndex, outputIndex, null, current.version);
  };
  const dragend = () => { drag = null; };
  const documentClick = event => {
    if (!menu) return;
    const target = event.target;
    if (!(target instanceof root.ownerDocument.defaultView.Element)) return;
    if (target.closest('[data-composition-context], [data-composition-graph-source], [data-composition-graph-section], [data-composition-graph-piece], [data-composition-graph-output-button], [data-composition-graph-output-actions], [data-composition-graph-piece-actions]')) return;
    menu = null; render();
  };
  root.addEventListener('click', clicked);
  root.addEventListener('change', changed);
  root.addEventListener('keyup', selection);
  root.addEventListener('keydown', key);
  root.addEventListener('contextmenu', context);
  root.addEventListener('dragstart', dragstart);
  root.addEventListener('dragover', dragover);
  root.addEventListener('drop', drop);
  root.addEventListener('dragend', dragend);
  root.ownerDocument.addEventListener('click', documentClick);
  render();
  return { render, dispose: () => { drag = null; graphResizeObserver?.disconnect(); graphResizeObserver = null; bindingState.set(controller, { tab, sourceInputIndex, selectedRange, conflictRanges, focus, nextFocus, scroll, formDrafts, formBaseline, reveal, menu, connectFrom, notice, drag, menuFocus, cacheLayout: lastCacheLayout, draftVersion: lastDraftVersion }); root.removeEventListener('click', clicked); root.removeEventListener('change', changed); root.removeEventListener('keyup', selection); root.removeEventListener('keydown', key); root.removeEventListener('contextmenu', context); root.removeEventListener('dragstart', dragstart); root.removeEventListener('dragover', dragover); root.removeEventListener('drop', drop); root.removeEventListener('dragend', dragend); root.ownerDocument.removeEventListener('click', documentClick); } };
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
