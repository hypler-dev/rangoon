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
  bundleExport: { status: 'idle', receipt: null, error: null }, externalBundle: { status: 'idle', report: null, previous: null, error: null },
  error: null, message: 'Saved records ready.', tab: 'text', ...overrides,
});
const nativeReport = (overrides = {}) => ({ outcome: 'compiled', schemaVersion: 'rangoon.compilation-inspection.v1', observedCurrentHead: revisionId, compilation: candidate, candidateManifestJson: JSON.stringify(candidate.candidate.manifest), ...overrides });
const externalReport = (overrides = {}) => ({ outcome: 'inspected', schemaVersion: 'rangoon.instruction-bundle-file.v1', inspection: { schemaVersion: 'rangoon.instruction-bundle-inspection.v1', bundleId: `instruction-bundle:${'a'.repeat(64)}`, sha256: 'b'.repeat(64), byteLength: 512, compilation: candidate, verification: 'internal_consistency_only', authority: 'none' }, candidateManifestJson: JSON.stringify(candidate.candidate.manifest), ...overrides });

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
  assert.match(html, /id="compile-tab-external"[^>]*aria-controls="compile-panel-external"/);
  assert.match(html, /id="compile-panel-external"[^>]*role="tabpanel"/);
  assert.match(html, /id="compile-inspect"/);
  assert.match(html, /id="compile-inspect-external"/);
  assert.match(html, /id="compile-selection"/);
  assert.match(html, /id="compile-capability-list"/);
  assert.match(html, /id="compile-revision-list"/);
  assert.match(html, /read-only compilation. Separate file actions can export or inspect/);
  assert.doesNotMatch(html, /cannot edit, review, export, install or run/);
});

test('renders portable export only for a fresh candidate and escapes receipt facts', () => {
  const ready = renderCompilationPage(state({ report: nativeReport() }));
  assert.doesNotMatch(ready.match(/<button id="compile-export"[^>]*>/)[0], /disabled/);
  assert.match(ready, /Plaintext bundle/);
  assert.match(ready, /Expected candidate/);
  assert.doesNotMatch(ready, /No export, installation/);
  const receipt = renderCompilationPage(state({ report: nativeReport(), bundleExport: { status: 'exported', receipt: { bundleId: '<bundle & hostile>', candidateId: candidate.candidate.candidateId, sha256: candidate.artifact.sha256, byteLength: candidate.artifact.byteLength, authority: 'none' }, error: null } }));
  assert.match(receipt, /id="compile-export-title" tabindex="-1"/);
  assert.match(receipt, /Write completed/);
  assert.match(receipt, /&lt;bundle &amp; hostile&gt;/);
  assert.doesNotMatch(receipt, /<bundle & hostile>/);
  assert.match(receipt, /write and file sync only/);
});

test('gates export for stale, noncandidate, busy, and unavailable state', () => {
  const stale = renderCompilationPage(state({ report: nativeReport(), reportStale: true }));
  assert.match(stale.match(/<button id="compile-export"[^>]*>/)[0], /disabled/);
  assert.match(stale, /Refresh and compile again before export/);
  const blocked = renderCompilationPage(state({ report: nativeReport({ compilation: { ...candidate, readiness: 'blocked', candidate: null }, candidateManifestJson: null }) }));
  assert.match(blocked.match(/<button id="compile-export"[^>]*>/)[0], /disabled/);
  assert.match(blocked, /Static diagnostics block export/);
  const busy = renderCompilationPage(state({ report: nativeReport(), pending: 'export', bundleExport: { status: 'pending', receipt: null, error: null } }));
  assert.match(busy.match(/<button id="compile-export"[^>]*>/)[0], /disabled/);
  assert.match(busy.match(/<button id="compile-inspect"[^>]*>/)[0], /disabled/);
  const unavailable = renderCompilationPage(state({ bridgeAvailable: false, report: nativeReport() }));
  assert.match(unavailable.match(/<button id="compile-export"[^>]*>/)[0], /disabled/);
  assert.match(unavailable, /Native export is unavailable/);
});

test('keeps external inspection available without workspace and separates current and previous evidence', () => {
  const empty = renderCompilationPage(state({ bridgeAvailable: false, listStatus: 'unavailable', selected: null, capabilities: [], profile: null, tab: 'external' }));
  assert.match(empty, /id="compile-tab-external"[^>]*aria-selected="true"[^>]*tabindex="0"/);
  assert.match(empty, /No saved skill or compilation is required/);
  assert.match(empty.match(/<button id="compile-inspect-external"[^>]*>/)[0], /disabled/);
  const report = externalReport({ candidateManifestJson: '<manifest & hostile>', inspection: { ...externalReport().inspection, compilation: { ...candidate, artifact: { ...candidate.artifact, content: '<external & hostile>' } } } });
  const html = renderCompilationPage(state({ tab: 'external', externalBundle: { status: 'failed', report: null, previous: report, error: { message: 'Fixed failure copy.' } } }));
  assert.match(html, /Previous external inspection/);
  assert.match(html, /Fixed failure copy/);
  assert.match(html, /id="compile-external-title" tabindex="-1"/);
  assert.match(html, /id="compile-external-manifest"/);
  assert.match(html, /id="compile-external-text"/);
  assert.match(html, /&lt;manifest &amp; hostile&gt;/);
  assert.match(html, /&lt;external &amp; hostile&gt;/);
  assert.doesNotMatch(html, /<external & hostile>/);
  assert.match(html, /Internal consistency only/);
  const external = html.slice(html.indexOf('id="compile-panel-external"'));
  assert.match(external, /Ordinary revision reference/);
  assert.equal((html.match(/role="alert"/g) ?? []).length, 1);
  const hero = html.slice(0, html.indexOf('compile-workbench'));
  assert.doesNotMatch(hero, /Fixed failure copy/);
});

test('keeps file-action messages in their own surface and announces cancellation without an alert', () => {
  const externalError = { code: 'bundle_invalid', message: 'External fixed failure.' };
  const idleExport = renderCompilationPage(state({ report: nativeReport(), error: externalError, externalBundle: { status: 'failed', report: null, previous: null, error: externalError } }));
  const portable = idleExport.slice(idleExport.indexOf('compile-portable-action'), idleExport.indexOf('compile-export-receipt'));
  assert.doesNotMatch(portable, /External fixed failure/);
  assert.equal((idleExport.match(/role="alert"/g) ?? []).length, 1);
  assert.match(idleExport.slice(0, idleExport.indexOf('compile-workbench')), /External fixed failure/);
  assert.doesNotMatch(idleExport.slice(idleExport.indexOf('id="compile-panel-external"'), idleExport.indexOf('compile-portable-action')), /External fixed failure/);
  const pendingPrior = renderCompilationPage(state({ tab: 'external', externalBundle: { status: 'pending', report: externalReport(), previous: externalReport(), error: null } }));
  assert.match(pendingPrior, /Previous external inspection/);
  const exportCancelled = renderCompilationPage(state({ report: nativeReport(), bundleExport: { status: 'cancelled', receipt: null, error: null } }));
  assert.match(exportCancelled, /Export cancelled. No new bundle was created/);
  assert.match(exportCancelled, /role="status">Export cancelled/);
  assert.doesNotMatch(exportCancelled, /role="alert">Export cancelled/);
  const inspectionCancelled = renderCompilationPage(state({ tab: 'external', externalBundle: { status: 'cancelled', report: null, previous: null, error: null } }));
  assert.match(inspectionCancelled, /role="status">Inspection cancelled/);
  assert.doesNotMatch(inspectionCancelled, /role="alert">Inspection cancelled/);
});

test('shows a native external failure once when controller message and error match', () => {
  const issue = { code: 'bundle_invalid', message: 'Instruction bundle failed consistency validation.' };
  const externalBundle = { status: 'failed', report: null, previous: null, error: issue };
  const textTab = renderCompilationPage(state({ tab: 'text', message: issue.message, error: issue, externalBundle }));
  assert.equal((textTab.match(/Instruction bundle failed consistency validation\./g) ?? []).length, 1);
  assert.match(textTab, /Error code: bundle_invalid/);
  assert.equal((textTab.match(/role="alert"/g) ?? []).length, 1);
  const externalTab = renderCompilationPage(state({ tab: 'external', message: issue.message, error: issue, externalBundle }));
  assert.equal((externalTab.match(/Instruction bundle failed consistency validation\./g) ?? []).length, 1);
  assert.match(externalTab, /Error code: bundle_invalid/);
  assert.equal((externalTab.match(/role="alert"/g) ?? []).length, 1);
  assert.doesNotMatch(externalTab.slice(0, externalTab.indexOf('compile-workbench')), /Instruction bundle failed consistency validation/);
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
  assert.match(css, /compile-portable-action/);
  assert.match(css, /@media \(max-width:820px\)\{\.compile-tabs\{display:grid;grid-template-columns:repeat\(2,minmax\(0,1fr\)\)/);
  assert.match(css, /prefers-reduced-motion/);
  assert.doesNotMatch(css, /screenshot|visual regression|pixel/i);
});
