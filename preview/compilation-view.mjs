import { escapeText } from './analysis-model.mjs';
import { icon } from './icons.mjs';

const text = escapeText;
const short = value => {
  const source = String(value ?? 'Unavailable');
  return source.length > 24 ? `${source.slice(0, 11)}…${source.slice(-9)}` : source;
};
const number = value => Number.isFinite(Number(value)) ? Number(value).toLocaleString() : 'Unavailable';
const busyState = (state, busy) => Boolean(busy || state.pending);
const selectedRevision = state => state.selected?.revision ?? null;
const selectedCapabilityId = state => state.selected?.id ?? null;
const report = state => state.report?.outcome === 'compiled' ? state.report : null;
const compilation = state => report(state)?.compilation ?? null;
const profileLabel = profile => profile === 'agents_md_v1' ? 'AGENTS.md · profile v1' : profile === 'claude_md_v1' ? 'CLAUDE.md · profile v1' : 'Choose text profile';
const provenanceLabel = value => value?.kind === 'ordinary'
  ? 'Ordinary saved revision'
  : value?.kind === 'composition'
    ? `Composition ${short(value.compositionId)} · output ${Number.isInteger(value.outputIndex) ? value.outputIndex + 1 : 'Unavailable'}`
    : 'Provenance unavailable';
const readinessLabel = (value, { stale = false } = {}) => {
  const label = value === 'candidate' ? 'Static candidate' : value === 'review_required' ? 'Local review required' : value === 'blocked' ? 'Blocked by diagnostics' : 'Readiness unavailable';
  return stale ? `Retained/stale · ${label}` : label;
};
const diagnosticExplanation = diagnostic => {
  switch (diagnostic?.code) {
    case 'instruction_semantics_unverified': return 'Exact bytes remain inspectable. Meaning, dependency closure and enforcement remain unverified.';
    case 'target_environment_unqualified': return 'Runtime discovery, combined instruction files, transformations and budget remain unknown.';
    case 'unsupported_requirement': return 'This closed text profile cannot satisfy declared requirement. Candidate stays blocked.';
    case 'possible_include_syntax': return 'Conservative byte scan found @. This can flag email or code examples; Rangoon does not parse or follow includes.';
    case 'possible_comment_elision': return 'Conservative byte scan found <!--. Target loading can omit comments; no content is removed here.';
    default: return 'No explanation catalog exists for this native diagnostic.';
  }
};

function disabled(state, busy) {
  return busyState(state, busy) || !state.bridgeAvailable;
}

function capabilityList(state, busy) {
  const selected = selectedCapabilityId(state);
  const rows = (state.capabilities ?? []).map(item => `<button class="compile-capability${item.id === selected ? ' compile-capability--active' : ''}" type="button" data-compile-capability="${text(item.id)}" aria-pressed="${item.id === selected}" ${disabled(state, busy) ? 'disabled' : ''}><strong>${text(item.title ?? 'Untitled skill')}</strong><small>${text(short(item.id))} · ${Number(item.revisionCount ?? 0)} revision${Number(item.revisionCount ?? 0) === 1 ? '' : 's'}</small><span>${item.reviewed ? 'Head locally reviewed' : 'Head unreviewed'}</span></button>`).join('');
  if (state.listStatus === 'unavailable') return '<p class="compile-note">Browser preview has no native workspace bridge. Saved skills cannot be invented.</p>';
  if (state.listStatus === 'loading') return rows || '<p class="compile-note">Loading saved skills…</p>';
  if (state.listStatus === 'failed') return `${rows}<p class="compile-note">Saved-skill list unavailable. Existing selection may be stale.</p>`;
  return rows || '<p class="compile-note">No saved skills yet. Create and save one in Skills first.</p>';
}

function revisions(state, busy) {
  const detail = state.selected;
  if (!detail) return '<p class="compile-note">Select saved skill. Exact revision required; current head never substitutes silently.</p>';
  const selected = selectedRevision(state)?.id;
  return `<div class="compile-selected"><strong>${text(detail.revision?.title ?? detail.title ?? 'Selected skill')}</strong><small>${text(short(detail.id))}</small></div><ol id="compile-revision-list" class="compile-revisions">${(detail.history ?? []).map(item => `<li><button class="compile-revision${item.id === selected ? ' compile-revision--active' : ''}" type="button" data-compile-revision="${text(item.id)}" aria-pressed="${item.id === selected}" ${disabled(state, busy) ? 'disabled' : ''}><span>${text(item.title ?? 'Untitled revision')}</span><small>${item.id === detail.latestRevisionId ? 'Observed current head' : 'Historical revision'} · ${item.review ? 'locally reviewed' : 'unreviewed'} · ${text(short(item.id))}</small></button></li>`).join('')}</ol>`;
}

function profiles(state, busy) {
  return `<fieldset class="compile-profiles"><legend>Closed text profile</legend><label><input type="radio" name="compile-profile" data-compile-profile="agents_md_v1" ${state.profile === 'agents_md_v1' ? 'checked' : ''} ${disabled(state, busy) ? 'disabled' : ''}> <span>AGENTS.md <small>Exact UTF-8 content</small></span></label><label><input type="radio" name="compile-profile" data-compile-profile="claude_md_v1" ${state.profile === 'claude_md_v1' ? 'checked' : ''} ${disabled(state, busy) ? 'disabled' : ''}> <span>CLAUDE.md <small>Conservative include/comment checks</small></span></label></fieldset>`;
}

function selectionPanel(state, busy, skillsBusy) {
  const hasSelection = Boolean(selectedCapabilityId(state) && selectedRevision(state));
  const canGenerate = Boolean(state.bridgeAvailable && hasSelection && state.profile && !state.needsRefresh && !busyState(state, busy));
  return `<aside id="compile-selection" class="compile-selection" aria-label="Compilation selection"><div class="compile-panel-head"><div><p class="analysis-kicker">SAVED SKILLS</p><h2>Exact inputs</h2></div><button id="compile-refresh" class="analysis-button analysis-button--small" type="button" ${disabled(state, busy) ? 'disabled' : ''}>${state.pending === 'refresh' ? 'Refreshing…' : 'Refresh'}</button></div><p class="compile-note">Native reads resolve saved data. This page cannot edit, review, export, install or run instructions.</p><div id="compile-capability-list" class="compile-capability-list">${capabilityList(state, busy)}</div><section class="compile-history" aria-label="Revision history"><p class="analysis-kicker">REVISION HISTORY</p><h3>Pin one revision</h3>${revisions(state, busy)}</section>${profiles(state, busy)}<div class="compile-actions"><button id="compile-generate" class="analysis-button analysis-button--primary" type="button" ${canGenerate ? '' : 'disabled'}>${state.pending === 'generate' ? 'Compiling…' : `${icon('bundle', { size: 16 })} Compile selected revision`}</button><button id="compile-open-skills" class="analysis-button" type="button" ${skillsBusy || busyState(state, busy) || !hasSelection ? 'disabled' : ''}>Open selected revision in Skills</button></div>${state.needsRefresh ? '<p class="compile-stale" role="alert">Workspace changed. Refresh this exact selection before compiling again.</p>' : ''}</aside>`;
}

function tabButton(tab, label, selected, disabledTab) {
  return `<button id="compile-tab-${tab}" type="button" role="tab" aria-selected="${selected}" aria-controls="compile-panel-${tab}" tabindex="${selected ? '0' : '-1'}" data-compile-tab="${tab}" ${disabledTab ? 'disabled' : ''}>${label}</button>`;
}

function generatedPanel(state, value, active) {
  const artifact = value?.artifact;
  if (!value || !artifact) return `<section id="compile-panel-text" class="compile-report-panel" role="tabpanel" aria-labelledby="compile-tab-text" ${active ? '' : 'hidden'}><p class="compile-empty">Compile an exact saved revision to inspect host-generated text. Runtime remains untested.</p></section>`;
  return `<section id="compile-panel-text" class="compile-report-panel" role="tabpanel" aria-labelledby="compile-tab-text" ${active ? '' : 'hidden'}><header><div><p class="analysis-kicker">HOST-GENERATED TEXT</p><h2 id="compile-artifact-title" tabindex="-1">${text(artifact.path ?? 'Artifact')}</h2></div><span>${number(artifact.byteLength)} bytes</span></header><dl class="compile-facts"><div><dt>SHA-256</dt><dd class="analysis-mono">${text(artifact.sha256 ?? 'Unavailable')}</dd></div><div><dt>Readiness</dt><dd>${text(readinessLabel(value.readiness, { stale: state.reportStale }))}</dd></div></dl><p class="compile-inspection-note">Byte length and SHA-256 come from native report. This renderer does not independently verify artifact bytes.</p><pre id="compile-artifact-text" class="compile-artifact" tabindex="0" aria-label="Generated instruction text">${text(artifact.content ?? '')}</pre></section>`;
}

function diagnosticsPanel(value, active) {
  const diagnostics = value?.diagnostics ?? [];
  return `<section id="compile-panel-diagnostics" class="compile-report-panel" role="tabpanel" aria-labelledby="compile-tab-diagnostics" ${active ? '' : 'hidden'}><header><div><p class="analysis-kicker">ORDERED STATIC FINDINGS</p><h2>Diagnostics</h2></div><span>${diagnostics.length}</span></header>${diagnostics.length ? `<ol class="compile-diagnostics">${diagnostics.map((item, index) => `<li class="compile-diagnostic compile-diagnostic--${text(item.severity ?? 'unknown')}"><strong>${index + 1}. ${text(item.code ?? 'unknown_diagnostic')}</strong><span>${text(item.severity ?? 'unknown')}</span><p>${text(diagnosticExplanation(item))}</p>${item.detail === null || item.detail === undefined ? '' : `<pre>${text(item.detail)}</pre>`}</li>`).join('')}</ol>` : '<p class="compile-empty">No native diagnostics returned. Outcome remains unknown until a valid report exists.</p>'}</section>`;
}

function evidencePanel(state, value, active) {
  const selected = value?.selectedRevision;
  const candidate = value?.candidate;
  const canonicalManifest = report(state)?.candidateManifestJson;
  const profile = value?.profile;
  const provenance = selected?.provenance;
  const provenanceFacts = provenance?.kind === 'composition' ? `<div><dt>Provenance</dt><dd>Composition-derived revision</dd></div><div><dt>Composition application ID</dt><dd class="analysis-mono">${text(provenance.applicationId ?? 'Unavailable')}</dd></div><div><dt>Composition ID</dt><dd class="analysis-mono">${text(provenance.compositionId ?? 'Unavailable')}</dd></div><div><dt>Composition output index</dt><dd>${Number.isInteger(provenance.outputIndex) ? provenance.outputIndex : 'Unavailable'}</dd></div>` : `<div><dt>Provenance</dt><dd>${text(provenanceLabel(provenance))}</dd></div>`;
  return `<section id="compile-panel-evidence" class="compile-report-panel" role="tabpanel" aria-labelledby="compile-tab-evidence" ${active ? '' : 'hidden'}><header><div><p class="analysis-kicker">LOCAL INSPECTION EVIDENCE</p><h2>Evidence</h2></div><span>authority none</span></header>${value ? `<dl class="compile-evidence"><div><dt>Profile</dt><dd>${text(profileLabel(profile?.id))}</dd></div><div><dt>Profile digest</dt><dd class="analysis-mono">${text(value.profileDigest ?? 'Unavailable')}</dd></div><div><dt>Documentation URL</dt><dd class="analysis-mono compile-url">${text(profile?.documentationUrl ?? 'Unavailable')}</dd></div><div><dt>Documentation retrieved</dt><dd>${text(profile?.documentationRetrievedOn ?? 'Unavailable')}</dd></div><div><dt>Runtime qualification</dt><dd>${text(profile?.runtimeQualification ?? 'unknown')}</dd></div><div><dt>Target budget</dt><dd>${text(profile?.targetBudget ?? 'unknown')}</dd></div><div><dt>Semantic equivalence</dt><dd>${text(profile?.semanticEquivalence ?? 'unverified')}</dd></div><div><dt>Capability ID</dt><dd class="analysis-mono">${text(selected?.capabilityId ?? 'Unavailable')}</dd></div><div><dt>Selected revision</dt><dd class="analysis-mono">${text(selected?.revisionId ?? 'Unavailable')}</dd></div><div><dt>Selected title</dt><dd>${text(selected?.title ?? 'Unavailable')}</dd></div><div><dt>Content SHA-256</dt><dd class="analysis-mono">${text(selected?.sha256 ?? 'Unavailable')}</dd></div><div><dt>Observed current head</dt><dd class="analysis-mono">${text(report(state)?.observedCurrentHead ?? 'Unavailable')}</dd></div><div><dt>Parent revision</dt><dd class="analysis-mono">${text(selected?.parentRevisionId ?? 'None')}</dd></div>${provenanceFacts}<div><dt>Local review</dt><dd>${value.reviewObservation ? `Observed ${text(value.reviewObservation.reviewer ?? 'local operator')} at ${number(value.reviewObservation.reviewedAtMs)}` : 'No local review observed'}</dd></div><div><dt>Compilation ID</dt><dd class="analysis-mono">${text(value.compilationId ?? 'Unavailable')}</dd></div><div><dt>Candidate identity</dt><dd class="analysis-mono">${text(candidate?.candidateId ?? 'Absent')}</dd></div><div><dt>Candidate manifest SHA-256</dt><dd class="analysis-mono">${text(candidate?.manifestSha256 ?? 'Absent')}</dd></div></dl>${candidate && canonicalManifest ? `<details id="compile-manifest-details" class="compile-manifest"><summary>Inspect canonical candidate manifest</summary><pre id="compile-manifest-text" tabindex="0">${text(canonicalManifest)}</pre></details>` : '<p class="compile-note">No candidate manifest. Blocked and unreviewed reports remain inspectable without candidate identity.</p>'}<p class="compile-note">Static candidate only. Runtime untested, target budget unknown, semantic equivalence unverified, authority none.</p>` : '<p class="compile-empty">Evidence appears only after native inspection returns a valid report.</p>'}</section>`;
}

function inspectionPanel(state) {
  const value = compilation(state);
  const tab = ['text', 'diagnostics', 'evidence'].includes(state.tab) ? state.tab : 'text';
  return `<section class="compile-inspection" aria-label="Compilation inspection"><div class="compile-tabs" role="tablist" aria-label="Compilation report tabs">${tabButton('text', 'Generated text', tab === 'text')}${tabButton('diagnostics', 'Diagnostics', tab === 'diagnostics')}${tabButton('evidence', 'Evidence', tab === 'evidence')}</div>${generatedPanel(state, value, tab === 'text')}${diagnosticsPanel(value, tab === 'diagnostics')}${evidencePanel(state, value, tab === 'evidence')}</section>`;
}

function evidenceRail(state) {
  const value = compilation(state);
  return `<aside class="compile-evidence-rail" aria-label="Compilation state"><p class="analysis-kicker">STATE</p><h2>${text(value ? readinessLabel(value.readiness, { stale: state.reportStale }) : state.listStatus === 'unavailable' ? 'Desktop bridge unavailable' : state.needsRefresh ? 'Refresh required' : 'No compilation yet')}</h2><dl><div><dt>Selection</dt><dd>${selectedRevision(state) ? `${text(short(selectedRevision(state).id))}${selectedRevision(state).id === state.selected?.latestRevisionId ? ' · current' : ' · historical'}` : 'None'}</dd></div><div><dt>Profile</dt><dd>${text(profileLabel(state.profile))}</dd></div><div><dt>Bridge</dt><dd>${state.bridgeAvailable ? 'Native bridge available' : 'Native bridge unavailable'}</dd></div><div><dt>Report</dt><dd>${value ? state.reportStale ? 'Retained/stale native report' : 'Native report retained' : 'No report'}</dd></div></dl><p>Historical selection remains valid. Observed head records workspace state; it never replaces pinned revision.</p></aside>`;
}

export function renderCompilationPage(state, { busy = false, skillsBusy = false } = {}) {
  const error = state.error;
  const message = state.message ?? '';
  const value = compilation(state);
  return `<section class="compile-page" aria-labelledby="compile-title" aria-busy="${busyState(state, busy)}"><header class="compile-hero"><p class="analysis-kicker">${icon('bundle', { size: 16 })} LOCAL COMPILATION INSPECTION</p><h1 id="compile-title" tabindex="-1">Compile <em>saved text.</em></h1><p>Inspect one exact local skill revision under a closed text profile. No export, installation, provider, engine or runtime action occurs here.</p><div class="compile-truth" aria-label="Compilation limits"><span>Runtime: untested</span><span>Target budget: unknown</span><span>Semantic equivalence: unverified</span><span>Authority: none</span></div><p class="compile-status">${text(message)}</p>${error ? `<p class="compile-alert" role="alert">${text(error.message ?? 'Local compilation could not finish.')}${error.code ? ` Error code: ${text(error.code)}.` : ''}</p>` : ''}${state.reportStale ? '<p class="compile-stale">Retained report may not match current workspace state. Refresh before another compile.</p>' : ''}${value?.readiness === 'blocked' ? '<p class="compile-blocked">Static diagnostics block candidate creation. Exact text stays available for inspection.</p>' : ''}</header><div class="compile-workbench">${selectionPanel(state, busy, skillsBusy)}${inspectionPanel(state)}${evidenceRail(state)}</div></section>`;
}
