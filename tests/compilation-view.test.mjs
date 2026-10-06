import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { renderCompilationPage } from '../preview/compilation-view.mjs';

const vectors = JSON.parse(readFileSync(new URL('../fixtures/compilation/instruction-v1.json', import.meta.url), 'utf8'));
const candidate = vectors.cases.find(item => item.name === 'agents_exact_bom_crlf_unicode').expected;
const capabilityId = candidate.selectedRevision.capabilityId;
const revisionId = candidate.selectedRevision.revisionId;
const historicalId = `revision:${'b'.repeat(64)}`;
const detail = {
  id: capabilityId, title: '<Selected & hostile>', latestRevisionId: revisionId, authority: 'none',
  revision: { id: historicalId, title: '<Older & hostile>', review: null, provenance: { kind: 'ordinary' } },
  history: [
    { id: historicalId, title: '<Older & hostile>', review: null, provenance: { kind: 'ordinary' } },
    { id: revisionId, title: 'Review instructions', review: candidate.reviewObservation, provenance: { kind: 'ordinary' } },
  ],
};
const state = (overrides = {}) => ({
  bridgeAvailable: true, listStatus: 'ready', capabilities: [{ id: capabilityId, title: '<Library & hostile>', reviewed: true, revisionCount: 2 }],
  selected: detail, profile: 'agents_md_v1', pending: null, needsRefresh: false, report: null, reportStale: false,
  error: null, message: 'Saved records ready.', tab: 'text', ...overrides,
});
const nativeReport = (overrides = {}) => ({ outcome: 'compiled', schemaVersion: 'rangoon.compilation-inspection.v1', observedCurrentHead: revisionId, compilation: candidate, candidateManifestJson: JSON.stringify(candidate.candidate.manifest), ...overrides });

test('renders exact compiled fixture with noneditable escaped source and canonical manifest', () => {
  const html = renderCompilationPage(state({ report: nativeReport() }));
  assert.match(html, /id="compile-title" tabindex="-1"/);
  assert.match(html, /id="compile-artifact-title" tabindex="-1"/);
  assert.match(html, /&lt;Library &amp; hostile&gt;/);
  assert.doesNotMatch(html, /<Library & hostile>/);
  assert.match(html, /<pre id="compile-artifact-text" class="compile-artifact"/);
  assert.doesNotMatch(html, /textarea/);
  assert.match(html, /Inspect canonical candidate manifest/);
  assert.match(html, /id="compile-artifact-text"/);
  assert.match(html, /id="compile-manifest-details"/);
  assert.match(html, /id="compile-manifest-text"/);
  assert.match(html, /Byte length and SHA-256 come from native report/);
  assert.match(html, /Runtime: untested/);
  assert.match(html, /Target budget: unknown/);
  assert.match(html, /Semantic equivalence: unverified/);
  assert.match(html, /Authority: none/);
  assert.match(html, /Documentation retrieved/);
  assert.match(html, /Capability ID/);
  assert.match(html, /Selected title/);
  assert.match(html, /Content SHA-256/);
  assert.doesNotMatch(html, /compile-status" role="status"/);
  assert.match(html, /target budget.*unknown/i);
  assert.match(html, /semantic equivalence.*unverified/i);
  assert.match(html, /authority none/i);
});

test('uses real tab semantics and exposes fixed controller hooks', () => {
  const html = renderCompilationPage(state({ report: nativeReport(), tab: 'diagnostics' }));
  assert.match(html, /role="tablist"/);
  assert.match(html, /id="compile-tab-text"[^>]*aria-selected="false"[^>]*tabindex="-1"[^>]*data-compile-tab="text"/);
  assert.match(html, /id="compile-tab-diagnostics"[^>]*aria-selected="true"[^>]*tabindex="0"[^>]*data-compile-tab="diagnostics"/);
  assert.match(html, /data-compile-capability=/);
  assert.match(html, /data-compile-revision=/);
  assert.match(html, /data-compile-profile="agents_md_v1"/);
  assert.match(html, /id="compile-refresh"/);
  assert.match(html, /id="compile-generate"/);
  assert.match(html, /id="compile-open-skills"/);
  assert.match(html, /id="compile-selection"/);
  assert.match(html, /id="compile-capability-list"/);
  assert.match(html, /id="compile-revision-list"/);
});

test('disables unavailable, busy, stale and skills-busy actions without inventing workspace records', () => {
  const unavailable = renderCompilationPage(state({ bridgeAvailable: false, listStatus: 'unavailable', selected: null, capabilities: [], profile: null }));
  assert.match(unavailable, /Browser preview has no native workspace bridge/);
  assert.match(unavailable.match(/<button id="compile-generate"[^>]*>/)[0], /disabled/);
  const busy = renderCompilationPage(state({ pending: 'generate' }), { busy: true, skillsBusy: true });
  assert.match(busy.match(/<button id="compile-refresh"[^>]*>/)[0], /disabled/);
  assert.match(busy.match(/<button id="compile-open-skills"[^>]*>/)[0], /disabled/);
  const stale = renderCompilationPage(state({ reportStale: true }));
  assert.doesNotMatch(stale.match(/<button id="compile-generate"[^>]*>/)[0], /disabled/);
  assert.match(stale, /Refresh before another compile/);
});

test('shows blocked and review-required reports, ordered diagnostics, historical head distinction and escaped hostile detail', () => {
  const blockedCompilation = {
    ...candidate, readiness: 'blocked', candidate: null,
    diagnostics: [
      { code: 'instruction_semantics_unverified', severity: 'warning', detail: null },
      { code: 'unsupported_requirement', severity: 'error', detail: '<needs & hostile>' },
      { code: 'possible_include_syntax', severity: 'error', detail: null },
    ],
  };
  const html = renderCompilationPage(state({ selected: detail, tab: 'diagnostics', report: nativeReport({ observedCurrentHead: revisionId, compilation: blockedCompilation, candidateManifestJson: null }) }));
  assert.match(html, /Blocked by diagnostics/);
  assert.match(html, /1\. instruction_semantics_unverified[\s\S]*2\. unsupported_requirement[\s\S]*3\. possible_include_syntax/);
  assert.match(html, /&lt;needs &amp; hostile&gt;/);
  assert.match(html, /Historical revision/);
  assert.match(html, /Observed current head/);
  const reviewRequired = renderCompilationPage(state({ report: nativeReport({ compilation: { ...candidate, readiness: 'review_required', reviewObservation: null, candidate: null }, candidateManifestJson: null }) }));
  assert.match(reviewRequired, /Local review required/);
  assert.match(reviewRequired, /No candidate manifest/);
  const staleCandidate = renderCompilationPage(state({ reportStale: true, report: nativeReport() }));
  assert.match(staleCandidate, /Retained\/stale · Static candidate/);
});

test('compilation css keeps wide columns, stacks evidence, then one column and no visual claims', () => {
  const css = readFileSync(new URL('../preview/compilation.css', import.meta.url), 'utf8');
  assert.match(css, /grid-template-columns:minmax\(225px,.7fr\) minmax\(0,1.65fr\) minmax\(225px,.72fr\)/);
  assert.match(css, /@media \(max-width:1120px\)[\s\S]*compile-evidence-rail\{grid-column:1 \/ -1/);
  assert.match(css, /@media \(max-width:760px\)[\s\S]*compile-workbench\{grid-template-columns:minmax\(0,1fr\)/);
  assert.doesNotMatch(css, /screenshot|visual regression|pixel/i);
});
