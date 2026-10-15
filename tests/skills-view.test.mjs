import test from 'node:test';
import assert from 'node:assert/strict';
import { renderSkillsView } from '../preview/skills-view.mjs';
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
