import { icon } from './icons.mjs';
import { skillCompileSelection, skillCompileHint } from './skill-compile-handoff.mjs';
import { filterSavedSkills, safeText, validCapabilityContent, validCapabilityTitle } from './skills-model.mjs';
import { renderTermHelp } from './term-help.mjs';

const text = safeText;
const short = value => String(value ?? '').length > 18 ? `${String(value).slice(0, 9)}…${String(value).slice(-7)}` : String(value ?? 'Unavailable');
const validTime = value => Number.isSafeInteger(value) && value >= 0 && value <= 8_640_000_000_000_000;
const stamp = value => validTime(value) ? new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'medium' }).format(new Date(value)) : 'Date unavailable';
const time = value => validTime(value) ? `<time datetime="${new Date(value).toISOString()}" title="${new Date(value).toISOString()} · ${value} ms since Unix epoch">${text(stamp(value))}</time>` : 'Date unavailable';
const exactTime = value => validTime(value) ? `${new Date(value).toISOString()} · ${value} ms since Unix epoch` : 'Date unavailable';

const span = value => value ? `bytes ${value.startByte}–${value.endByte} · lines ${value.startLine}–${value.endLine}` : 'Span unavailable';
const originLabel = origin => origin?.kind === 'source' ? 'Source section' : origin?.kind === 'composition' ? `Composition · ${origin.operation} · output ${origin.outputIndex + 1}` : 'Origin unavailable';
const birthOrigin = origin => origin?.kind === 'source'
  ? `<div><dt>Birth origin</dt><dd>Source section</dd></div><div><dt>Source</dt><dd class="analysis-mono">${text(origin.sourceName || short(origin.sourceId))}</dd></div><div><dt>Source ID</dt><dd class="analysis-mono">${text(origin.sourceId)}</dd></div><div><dt>Fragment</dt><dd class="analysis-mono">${text(origin.fragmentId)}</dd></div><div><dt>Span</dt><dd>${text(span(origin.span))}</dd></div>`
  : `<div><dt>Birth origin</dt><dd>Composition birth</dd></div><div><dt>Composition</dt><dd class="analysis-mono">${text(origin?.compositionId)}</dd></div><div><dt>Operation</dt><dd>${text(origin?.operation)}</dd></div><div><dt>Output</dt><dd>${text(Number.isInteger(origin?.outputIndex) ? origin.outputIndex + 1 : 'Unavailable')}</dd></div><div><dt>Pinned inputs</dt><dd>${text(origin?.inputs?.length ?? 'Unavailable')}</dd></div>`;
const revisionProvenance = provenance => provenance?.kind === 'ordinary'
  ? 'Ordinary revision'
  : `Composed revision · ${short(provenance?.compositionId)} · output ${Number.isInteger(provenance?.outputIndex) ? provenance.outputIndex + 1 : 'Unavailable'}`;

export function getSkillsEditorPresentation(state) {
  const selected = state.selected;
  const current = Boolean(selected && selected.revision.id === selected.latestRevisionId && state.viewedRevisionId === selected.latestRevisionId);
  const draft = current ? state.draft : selected?.revision;
  const readonly = !current || Boolean(state.pending) || Boolean(state.needsReload);
  const titleOK = validCapabilityTitle(draft?.title);
  const contentOK = validCapabilityContent(draft?.content);
  let kind = !selected ? 'empty' : state.pending ? 'pending' : state.needsReload ? 'conflict' : !current ? 'history' : draft?.dirty ? 'dirty' : 'saved';
  const label = { empty: 'Choose a saved skill', pending: 'Working…', conflict: 'Revision conflict', history: 'Viewing history', dirty: 'Unsaved draft', saved: 'Saved revision' }[kind];
  const hint = kind === 'conflict' ? 'Draft retained. Reload current to compare before saving or reviewing.'
    : kind === 'history' ? 'Read-only historical revision. Your current draft is retained.'
      : kind === 'dirty' ? 'Unsaved edits. Save a revision before review.'
        : kind === 'pending' ? 'Waiting for the local host. Your draft remains available.'
          : 'Local content review does not validate, publish or authorize execution.';
  return { current, draft, readonly, kind, label, hint, titleOK, contentOK,
    titleBytes: new TextEncoder().encode(draft?.title ?? '').length,
    contentBytes: new TextEncoder().encode(draft?.content ?? '').length,
    canSave: Boolean(state.bridgeAvailable && draft?.dirty && titleOK && contentOK && !readonly),
    canReview: Boolean(state.bridgeAvailable && !draft?.dirty && titleOK && contentOK && !selected?.revision?.review && !readonly),
    canDiscard: Boolean(state.bridgeAvailable && current && state.draft?.dirty && !state.pending && state.draft?.capabilityId === selected?.id) };
}

export function skillsDiscardMatches(state, expected) {
  return Boolean(expected && state.selected && state.draft?.dirty
    && state.selected.revision.id === state.selected.latestRevisionId && state.viewedRevisionId === state.selected.latestRevisionId
    && state.draft.capabilityId === state.selected.id
    && ['capabilityId', 'baseRevisionId', 'title', 'content'].every(key => expected[key] === state.draft[key]));
}

// Local UI hints only. The controller and native host still validate operations.
export function syncSkillsEditorChrome(root, state, { compileHandoff = {} } = {}) {
  const view = getSkillsEditorPresentation(state);
  const setText = (selector, value) => { const node = root.querySelector(selector); if (node && node.textContent !== value) node.textContent = value; };
  setText('#skills-title-count', `${view.titleBytes} / 160 UTF-8 bytes${view.titleOK ? '' : ' · use one nonblank line'}`);
  setText('#skills-content-count', `${view.contentBytes} / 262144 UTF-8 bytes${view.contentOK ? '' : ' · content cannot be blank or contain NUL'}`);
  setText('#skills-editor-state', view.label);
  setText('#skills-draft-status', view.hint);
  setText('#skills-editor-title', view.current ? view.draft?.title || state.selected?.revision?.title || 'Untitled draft' : state.selected?.revision?.title || 'Select a skill');
  const badge = root.querySelector('#skills-editor-state'); if (badge) badge.dataset.kind = view.kind;
  for (const [id, enabled] of [['#skills-save', view.canSave], ['#skills-review', view.canReview], ['#skills-discard', view.canDiscard]]) {
    const node = root.querySelector(id); if (node) node.disabled = !enabled;
  }
  const compile = root.querySelector('#skills-open-compile');
  if (compile) compile.disabled = Boolean(compileHandoff.pending) || !skillCompileSelection(state);
  setText('#skills-compile-help', compileHandoff.pending ? 'Opening this saved revision…' : compileHandoff.error || skillCompileHint(state));
  return view;
}

function revisionLabel(state) {
  const history = state.selected?.history ?? [];
  const index = history.findIndex(item => item.id === state.selected?.revision?.id);
  return index < 0 ? 'Saved revision' : `Revision ${index + 1} of ${history.length}${state.selected.revision.id === state.selected.latestRevisionId ? ' · current' : ' · historical'}`;
}

function provenancePanel(state) {
  if (!state.selected) return '<aside class="skills-provenance" aria-label="Skill provenance"><p class="analysis-note">Provenance appears after opening a local capability.</p></aside>';
  const selected = state.selected;
  return `<aside class="skills-provenance" aria-label="Skill provenance"><div class="skills-panel-head"><div><p class="analysis-kicker">SAVED RECORD</p><h2>Origin and history</h2></div></div><div class="skills-record-summary"><strong>${text(revisionLabel(state))}</strong><span>Saved ${time(selected.revision.createdAtMs)}</span><span>${selected.revision.review ? `Locally reviewed ${time(selected.revision.review.reviewedAtMs)}` : 'Not locally reviewed'}</span><p>Local review is not execution approval.</p></div><details class="skills-advanced" data-skills-disclosure="advanced"><summary>Advanced provenance</summary><dl class="skills-fields"><div><dt>Authority</dt><dd>none</dd></div>${birthOrigin(selected.origin)}<div><dt>Selected revision provenance</dt><dd>${text(revisionProvenance(selected.revision.provenance))}</dd></div><div><dt>Revision ID</dt><dd>${text(selected.revision.id)}</dd></div><div><dt>Parent revision</dt><dd>${text(selected.revision.parentRevisionId ?? 'Initial revision')}</dd></div><div><dt>SHA-256</dt><dd>${text(selected.revision.sha256 ?? 'Unavailable')}</dd></div><div><dt>Saved UTC</dt><dd>${text(exactTime(selected.revision.createdAtMs))}</dd></div><div><dt>Reviewed UTC</dt><dd>${text(exactTime(selected.revision.review?.reviewedAtMs))}</dd></div></dl>${selected.origin.kind === 'source' ? `<details class="skills-original"><summary>Original saved section</summary><pre>${text(selected.origin.originalText)}</pre></details>` : ''}</details><div class="skills-history"><h3>Revision history</h3>${(selected.history ?? []).map((item, index) => `<button type="button" class="skills-history__item${item.id === state.viewedRevisionId ? ' skills-history__item--active' : ''}" data-revision="${text(item.id)}" aria-pressed="${item.id === state.viewedRevisionId}" ${state.pending ? 'disabled' : ''}><span>Revision ${index + 1} · ${text(item.title)}</span><small>${time(item.createdAtMs)} · ${item.review ? 'Reviewed' : 'Unreviewed'} · ${text(revisionProvenance(item.provenance))}</small></button>`).join('')}</div></aside>`;
}

export function renderSkillsView(state, ui = {}) {
  const compileHandoff = ui.compileHandoff ?? {};
  const view = getSkillsEditorPresentation(state);
  const { current, draft, readonly, titleOK, contentOK, canSave, canReview, titleBytes, contentBytes } = view;
  const createDisabled = !state.bridgeAvailable || !state.source?.saved || !validCapabilityTitle(state.createTitle) || state.pending || state.needsReload;
  const discardArmed = skillsDiscardMatches(state, ui.discardConfirmation);
  const selectedId = state.selected?.id;
  const libraryFilters = { query: state.query ?? '', reviewFilter: state.reviewFilter ?? 'all', originFilter: state.originFilter ?? 'all' };
  const visibleCapabilities = filterSavedSkills(state.capabilities, libraryFilters) ?? state.capabilities;
  const filtersActive = Boolean(libraryFilters.query) || libraryFilters.reviewFilter !== 'all' || libraryFilters.originFilter !== 'all';
  const library = visibleCapabilities.map(item => `<button class="skills-library__item${item.id === selectedId ? ' skills-library__item--active' : ''}" type="button" data-capability="${text(item.id)}" aria-pressed="${item.id === selectedId}" ${state.pending ? 'disabled' : ''}><strong>${text(item.title)}</strong><small>${text(originLabel(item.origin))} · ${item.revisionCount} revision${item.revisionCount === 1 ? '' : 's'}</small><span class="skills-badge${item.reviewed ? ' skills-badge--reviewed' : ''}">${item.reviewed ? 'Reviewed' : 'Unreviewed'}</span></button>`).join('');
  const history = state.selected?.history ?? [];
  const source = state.source;
  const derivedCount = source ? new Set(state.capabilities.filter(item => item.origin?.kind === 'source' && item.origin.sourceId === source.sourceId).map(item => item.origin.fragmentId)).size : 0;
  const hasKnownRecords = state.listStatus === 'ready' || state.capabilities.length > 0;
  const visibleCount = `${visibleCapabilities.length} match${visibleCapabilities.length === 1 ? '' : 'es'} / ${state.capabilities.length} loaded`;
  const selectedHidden = Boolean(selectedId && state.capabilities.some(item => item.id === selectedId) && !visibleCapabilities.some(item => item.id === selectedId));
  const libraryContents = state.listStatus === 'unavailable' ? '<p class="analysis-note">Open this page in the Rangoon desktop app to view local capabilities.</p>'
    : state.listStatus === 'idle' ? '<p class="analysis-note">Local skill library has not loaded yet.</p>'
      : state.listStatus === 'loading' ? `${library}<p class="analysis-note">Loading local skills${state.capabilities.length ? '. Prior results remain visible.' : '…'}</p>`
      : state.listStatus === 'failed' ? `${library}<p class="analysis-note">Library unavailable. Prior results may be stale. Your draft remains available.</p>`
        : state.listStatus === 'ready' && state.capabilities.length === 0 ? '<p class="analysis-note">No local capabilities yet.</p>'
          : library || '<p class="analysis-note">No saved skills match these filters.</p>';
  return `<section class="skills-page" aria-labelledby="skills-title" aria-busy="${Boolean(state.pending)}">
    <header class="skills-hero"><p class="analysis-kicker">${icon('skill',{size:16})} LOCAL CAPABILITIES</p><h1 id="skills-title" tabindex="-1">Skills <em>workbench</em></h1><p>Create reusable skills from saved source sections. Review content while keeping the original in view.</p><p class="skills-status${state.error ? ' skills-status--error' : ''}" role="status">${text(state.message)}${state.error?.code ? ` Error code: ${text(state.error.code)}.` : ''}</p></header>
    ${renderTermHelp('skills', ['skill', 'revision', 'provenance', 'review'])}
    <details class="skills-source-create" data-skills-disclosure="create" ${source?.saved && !state.selected ? 'open' : ''}><summary>Create from a source section<span>${text(source?.saved ? source.title : 'Select a saved section in Import sources')}</span></summary><section class="skills-source" aria-label="Saved source section"><div><p class="analysis-kicker">SOURCE → SELECTED SAVED SECTION</p><strong>${text(source?.title ?? 'No selected source section')}</strong><small>${text(source ? `${short(source.sourceId)} · ${span(source.span)} · ${derivedCount} derived section${derivedCount === 1 ? '' : 's'}` : 'Save a source, then select a section in Import & Analyze.')}</small></div><label>Title <input id="skills-create-title" maxlength="160" value="${text(state.createTitle ?? '')}" ${source?.saved && !state.pending ? '' : 'disabled'}><small id="skills-create-count">${new TextEncoder().encode(state.createTitle ?? '').length} / 160 UTF-8 bytes. Source stays unreviewed; count does not prove preservation.</small></label><button id="skills-create" class="analysis-button analysis-button--primary" type="button" ${createDisabled ? 'disabled' : ''}>${icon('skill',{size:16})}${state.pending === 'create' ? 'Creating…' : 'Create draft'}</button></section></details>
    <div class="skills-workbench">
      <aside class="skills-library" aria-label="Skills library"><div class="skills-panel-head"><div><p class="analysis-kicker">LIBRARY</p><h2>Saved skills</h2></div><button id="skills-reload-list" class="analysis-button analysis-button--small" type="button" ${state.pending || !state.bridgeAvailable ? 'disabled' : ''}>Reload</button></div><div class="skills-library__filters"><label for="skills-library-query">Search saved skills <input id="skills-library-query" type="search" maxlength="512" value="${text(libraryFilters.query)}" autocomplete="off" aria-describedby="skills-library-search-help"></label><small id="skills-library-search-help">Loaded summary titles and IDs only.</small><div class="skills-library__facets"><label>Review <select id="skills-review-filter"><option value="all" ${libraryFilters.reviewFilter === 'all' ? 'selected' : ''}>All</option><option value="reviewed" ${libraryFilters.reviewFilter === 'reviewed' ? 'selected' : ''}>Reviewed</option><option value="unreviewed" ${libraryFilters.reviewFilter === 'unreviewed' ? 'selected' : ''}>Unreviewed</option></select></label><label>Origin <select id="skills-origin-filter"><option value="all" ${libraryFilters.originFilter === 'all' ? 'selected' : ''}>All</option><option value="source" ${libraryFilters.originFilter === 'source' ? 'selected' : ''}>Source section</option><option value="composition" ${libraryFilters.originFilter === 'composition' ? 'selected' : ''}>Composition</option></select></label></div><button id="skills-clear-library-filters" class="analysis-button analysis-button--small" type="button" ${filtersActive ? '' : 'disabled'}>Clear filters</button></div><p class="skills-library__note" role="status" aria-live="polite">${state.listStatus === 'unavailable' ? 'Desktop app only.' : state.listStatus === 'idle' ? 'Library not loaded.' : state.listStatus === 'loading' && !hasKnownRecords ? 'Loading library…' : state.listStatus === 'failed' && !hasKnownRecords ? 'Library unavailable. Current draft remains.' : visibleCount}</p>${selectedHidden ? '<p class="skills-library__hidden" role="status">Selected skill is hidden by these filters. It remains open in the editor.</p>' : ''}<div class="skills-library__list">${libraryContents}</div></aside>
      <section class="skills-editor" aria-label="Skill editor">${state.selected ? `<div class="skills-panel-head"><div><p class="analysis-kicker">${current ? 'CURRENT REVISION' : 'HISTORICAL REVISION'}</p><h2 id="skills-editor-title" tabindex="-1">${text(current ? draft?.title || state.selected.revision.title : state.selected.revision.title)}</h2><p class="skills-revision-context">${text(revisionLabel(state))} · saved ${time(state.selected.revision.createdAtMs)}</p></div><span id="skills-editor-state" class="skills-editor-state" data-kind="${view.kind}" role="status" tabindex="-1">${text(view.label)}</span></div>${!current ? '<div class="skills-historical"><span>Read-only historical revision. Current draft stays retained.</span><button id="skills-return-current" class="analysis-button analysis-button--small" type="button">Return to current</button></div>' : ''}<label class="skills-field">Title <input id="skills-title-input" maxlength="160" value="${text(draft?.title ?? '')}" ${readonly ? 'readonly' : ''}><small id="skills-title-count">${titleBytes} / 160 UTF-8 bytes${titleOK ? '' : ' · use one nonblank line'}</small></label><label class="skills-field">Content <textarea id="skills-content-input" maxlength="262144" ${readonly ? 'readonly' : ''}>${text(draft?.content ?? '')}</textarea><small id="skills-content-count">${contentBytes} / 262144 UTF-8 bytes${contentOK ? '' : ' · content cannot be blank or contain NUL'}</small></label><p id="skills-draft-status" class="skills-edit-hint">${text(view.hint)}</p><div class="skills-actions"><button id="skills-save" class="analysis-button analysis-button--primary" type="button" ${canSave ? '' : 'disabled'}>${state.pending === 'save' ? 'Saving…' : 'Save revision'}</button><button id="skills-review" class="analysis-button" type="button" ${canReview ? '' : 'disabled'}>${state.pending === 'review' ? 'Reviewing…' : state.selected.revision.review ? 'Locally reviewed' : 'Mark reviewed'}</button>${current ? `<button id="skills-discard" class="analysis-button" type="button" ${view.canDiscard ? '' : 'disabled'}>Discard local edits</button>` : ''}<button id="skills-open-compile" class="analysis-button" type="button" aria-describedby="skills-compile-help" ${compileHandoff.pending || !skillCompileSelection(state) ? 'disabled' : ''}>${compileHandoff.pending ? 'Opening…' : 'Open in Compile'}</button></div><p id="skills-compile-help" class="skills-edit-hint" role="status">${text(compileHandoff.pending ? 'Opening this saved revision…' : compileHandoff.error || skillCompileHint(state))}</p>${discardArmed ? `<section class="skills-discard-confirmation" aria-label="Confirm discarding unsaved edits"><p>Discard this skill’s unsaved title and content? Saved revisions stay unchanged.</p><div><button type="button" id="skills-discard-cancel" class="analysis-button" ${state.pending ? 'disabled' : ''}>Keep editing</button><button type="button" id="skills-discard-confirm" class="analysis-button" ${state.pending ? 'disabled' : ''}>Discard edits</button></div></section>` : ''}<details class="skills-saved"><summary>Saved revision · compare with editor</summary><pre>${text(state.selected.revision.content)}</pre></details>${state.needsReload ? '<p class="skills-conflict" role="alert">Conflict preserved this draft. <button id="skills-reload-current" class="analysis-button analysis-button--small" type="button">Reload current</button></p>' : ''}` : '<div class="skills-empty"><p class="analysis-kicker">EDITOR</p><h2>Select a skill</h2><p>Or create one from the selected saved section.</p></div>'}</section>
      ${provenancePanel(state)}
    </div>
  </section>`;
}
