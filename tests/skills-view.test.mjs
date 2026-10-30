import test from 'node:test';
import assert from 'node:assert/strict';
import { renderSkillsView, getSkillsEditorPresentation, skillsDiscardMatches, syncSkillsEditorChrome } from '../preview/skills-view.mjs';
import { EMPTY_SKILLS_STATE } from '../preview/skills-model.mjs';

const capabilityId = `capability:${'3'.repeat(64)}`;
const revisionId = `revision:${'4'.repeat(64)}`;
const sourceId = `source:${'1'.repeat(64)}`;
const fragmentId = `fragment:${'2'.repeat(64)}`;
const compositionId = `composition:${'5'.repeat(64)}`;
const source = { id: capabilityId, origin: { kind: 'source', sourceId, fragmentId }, latestRevisionId: revisionId, title: '<Unsafe & title>', reviewed: false, revisionCount: 1 };
const composition = { id: `capability:${'6'.repeat(64)}`, origin: { kind: 'composition', compositionId, operation: 'merge', outputIndex: 0 }, latestRevisionId: `revision:${'7'.repeat(64)}`, title: 'Reviewed composition', reviewed: true, revisionCount: 2 };
const selectedComposition = { id: composition.id, origin: composition.origin, latestRevisionId: composition.latestRevisionId, revision: { id: composition.latestRevisionId, title: composition.title, content: 'Saved text\n', review: { reviewer: 'local_operator', reviewedAtMs: 1 }, provenance: { kind: 'ordinary' } }, history: [] };

function state(changes = {}) {
  return { ...EMPTY_SKILLS_STATE, bridgeAvailable: true, listStatus: 'ready', capabilities: [source, composition], message: 'Ready.', ...changes };
}

test('renders local summary-only controls, escaped values, counts and a hidden selected notice', () => {
  const html = renderSkillsView(state({ query: '<Unsafe & title>', reviewFilter: 'unreviewed', selected: selectedComposition, viewedRevisionId: composition.latestRevisionId }));
  assert.match(html, /Search saved skills/);
  assert.match(html, /Loaded summary titles and IDs only\./);
  assert.match(html, /Review[\s\S]*Reviewed[\s\S]*Unreviewed/);
  assert.match(html, /Origin[\s\S]*Source section[\s\S]*Composition/);
  assert.match(html, /1 match \/ 2 loaded/);
  assert.match(html, /Selected skill is hidden by these filters\. It remains open in the editor\./);
  assert.match(html, /value="&lt;Unsafe &amp; title&gt;"/);
  assert.doesNotMatch(html, /<Unsafe & title>/);
  assert.doesNotMatch(html, /Find files|Categories/);
});

test('keeps unavailable, loading, stale failure, empty, and no-match states distinct', () => {
  assert.match(renderSkillsView(state({ bridgeAvailable: false, listStatus: 'unavailable', capabilities: [] })), /Open this page in the Rangoon desktop app/);
  assert.match(renderSkillsView(state({ listStatus: 'loading', capabilities: [] })), /Loading local skills…/);
  const loadingWithPrior = renderSkillsView(state({ listStatus: 'loading', query: 'unsafe' }));
  assert.match(loadingWithPrior, /1 match \/ 2 loaded/);
  assert.match(loadingWithPrior, /Loading local skills\. Prior results remain visible\./);
  assert.match(loadingWithPrior, /&lt;Unsafe &amp; title&gt;/);
  const failed = renderSkillsView(state({ listStatus: 'failed', query: 'missing' }));
  assert.match(failed, /Library unavailable\. Prior results may be stale\./);
  assert.match(failed, /0 matches \/ 2 loaded/);
  const empty = renderSkillsView(state({ capabilities: [] }));
  assert.match(empty, /0 matches \/ 0 loaded/);
  assert.match(empty, /No local capabilities yet\./);
  const noMatch = renderSkillsView(state({ query: 'missing' }));
  assert.match(noMatch, /No saved skills match these filters\./);
  const idle = renderSkillsView(state({ listStatus: 'idle', capabilities: [] }));
  assert.match(idle, /Local skill library has not loaded yet\./);
  assert.doesNotMatch(idle, /No saved skills match these filters\./);
});

test('gives search, facets, and Clear filters 44px control hooks without changing editor data', () => {
  const html = renderSkillsView(state({ query: 'reviewed', reviewFilter: 'reviewed', originFilter: 'composition' }));
  assert.match(html, /id="skills-library-query"/);
  assert.match(html, /id="skills-review-filter"/);
  assert.match(html, /id="skills-origin-filter"/);
  assert.match(html, /id="skills-clear-library-filters"/);
  assert.doesNotMatch(html, /skills-clear-library-filters[^>]*disabled/);
});

function editorState(changes = {}) {
  const revision = { ...selectedComposition.revision, review: null, createdAtMs: 1793030400000 };
  const selected = { ...selectedComposition, revision, history: [revision] };
  return state({ selected, viewedRevisionId: selected.latestRevisionId, draft: { capabilityId: selected.id, baseRevisionId: selected.latestRevisionId, title: revision.title, content: revision.content, dirty: false }, ...changes });
}

test('editor presents saved, dirty, history, pending and conflict with distinct action gates', () => {
  const saved = editorState(); assert.equal(getSkillsEditorPresentation(saved).kind, 'saved'); assert.equal(getSkillsEditorPresentation(saved).canReview, true);
  const dirty = { ...saved, draft: { ...saved.draft, content: 'Changed', dirty: true } };
  assert.equal(getSkillsEditorPresentation(dirty).canSave, true); assert.equal(getSkillsEditorPresentation(dirty).canReview, false);
  assert.equal(getSkillsEditorPresentation({ ...dirty, pending: 'save' }).kind, 'pending');
  const conflict = getSkillsEditorPresentation({ ...dirty, needsReload: true }); assert.equal(conflict.kind, 'conflict'); assert.equal(conflict.canSave, false); assert.equal(conflict.canDiscard, true);
  const historical = { ...dirty, viewedRevisionId: revisionId, selected: { ...saved.selected, revision: { ...saved.selected.revision, id: revisionId, content: 'Historical' } } };
  const view = getSkillsEditorPresentation(historical); assert.equal(view.kind, 'history'); assert.equal(view.draft.content, 'Historical'); assert.equal(view.canDiscard, false);
});

test('discard confirmation binds exact local text and cannot follow another revision or skill', () => {
  const saved = editorState(); const dirty = { ...saved, draft: { ...saved.draft, dirty: true, content: '<draft>' } }; const expected = { ...dirty.draft };
  assert.equal(skillsDiscardMatches(dirty, expected), true);
  assert.match(renderSkillsView(dirty, { discardConfirmation: expected }), /id="skills-discard-confirm"/);
  for (const key of ['capabilityId', 'baseRevisionId', 'title', 'content']) assert.equal(skillsDiscardMatches(dirty, { ...expected, [key]: 'changed' }), false);
  assert.doesNotMatch(renderSkillsView(dirty, { discardConfirmation: { ...expected, content: 'old' } }), /id="skills-discard-confirm"/);
  assert.match(renderSkillsView({ ...dirty, pending: 'open' }, { discardConfirmation: expected }), /id="skills-discard-confirm"[^>]*disabled/);
  assert.doesNotMatch(renderSkillsView(dirty), /<draft>/);
});

test('native revision position and timestamps stay visible while technical provenance is collapsed', () => {
  const html = renderSkillsView(editorState());
  assert.match(html, /Revision 1 of 1 · current/); assert.match(html, /datetime="2026-10-26T16:00:00.000Z"/);
  assert.match(html, /data-skills-disclosure="advanced"><summary>Advanced provenance/);
  assert.match(html, /data-skills-disclosure="create" ><summary>/);
  assert.match(html, /Local review is not execution approval/);
  assert.match(renderSkillsView(editorState({ source: { saved: true, title: 'Saved section' }, selected: null })), /data-skills-disclosure="create" open/);
  assert.match(renderSkillsView(state({ selected: selectedComposition })), /Date unavailable/);
});

test('typing updates editor chrome in place without assigning HTML or changing fields', () => {
  const saved = editorState(); const draft = { ...saved.draft, title: 'Changed title', content: 'Changed', dirty: true };
  const nodes = new Map(['#skills-title-count','#skills-content-count','#skills-editor-state','#skills-draft-status','#skills-editor-title','#skills-save','#skills-review','#skills-discard'].map(id => [id, { textContent: '', dataset: {}, disabled: false }]));
  syncSkillsEditorChrome({ querySelector: id => nodes.get(id) }, { ...saved, draft });
  assert.equal(nodes.get('#skills-editor-title').textContent, 'Changed title'); assert.equal(nodes.get('#skills-editor-state').dataset.kind, 'dirty'); assert.equal(nodes.get('#skills-save').disabled, false); assert.equal(nodes.get('#skills-review').disabled, true);
});

test('saved revision continuation is contextual, escaped and disabled for dirty or conflicting state', () => {
  const saved = editorState();
  assert.match(renderSkillsView(saved), /id="skills-open-compile"[^>]*>Open in Compile/);
  assert.doesNotMatch(renderSkillsView(saved), /id="skills-open-compile"[^>]*disabled/);
  for (const state of [{ ...saved, draft: { ...saved.draft, dirty: true } }, { ...saved, pending: 'save' }, { ...saved, needsReload: true }, { ...saved, bridgeAvailable: false }]) {
    assert.match(renderSkillsView(state), /id="skills-open-compile"[^>]*disabled/);
  }
  assert.match(renderSkillsView(saved, { compileHandoff: { pending: true } }), /id="skills-open-compile"[^>]*disabled[^>]*>Opening/);
  assert.match(renderSkillsView(saved, { compileHandoff: { error: '<bad>' } }), /&lt;bad&gt;/);
  const nodes = new Map([['#skills-open-compile', { disabled: false }], ['#skills-compile-help', { textContent: '' }]]);
  const root = { querySelector: selector => nodes.get(selector) };
  syncSkillsEditorChrome(root, { ...saved, draft: { ...saved.draft, dirty: true } });
  assert.equal(nodes.get('#skills-open-compile').disabled, true); assert.match(nodes.get('#skills-compile-help').textContent, /Save or discard/);
  syncSkillsEditorChrome(root, saved, { compileHandoff: { pending: true } });
  assert.equal(nodes.get('#skills-open-compile').disabled, true);
});
