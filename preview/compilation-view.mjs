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
const exportState = state => state.bundleExport ?? { status: 'idle', receipt: null, error: null };
const externalState = state => state.externalBundle ?? { status: 'idle', report: null, previous: null, error: null };
const externalReport = value => value?.inspection ? value : value?.report?.inspection ? value.report : null;
const exportBlocker = (state, busy) => {
  const value = compilation(state);
  if (!state.bridgeAvailable) return 'Native export is unavailable in this preview.';
  if (busyState(state, busy)) return 'Another local action is in progress.';
  if (state.needsRefresh || state.reportStale) return 'Refresh and compile again before export.';
  if (!value) return 'Compile an exact saved revision first.';
  if (value.readiness !== 'candidate' || !value.candidate?.candidateId) return value.readiness === 'blocked' ? 'Static diagnostics block export.' : 'Local content review is required before export.';
  return null;
};
const fileActionMessage = (action, state) => {
  if (action === 'export') {
    const value = exportState(state);
    if (value.status === 'cancelled') return 'Export cancelled. No new bundle was created.';
    if (value.status === 'uncertain') return 'Export outcome is uncertain. A file may remain; choose a new name before another attempt.';
    if (value.status === 'failed') return value.error?.message ?? 'The bundle export did not complete.';
    return '';
  }
  const value = externalState(state);
  if (value.status === 'cancelled') return 'Inspection cancelled. No external evidence was changed.';
  if (value.status === 'failed') return value.error?.message ?? 'The external bundle inspection did not complete.';
  return '';
};
const fileActionRole = (action, state) => (action === 'export' ? exportState(state).status : externalState(state).status) === 'cancelled' ? 'status' : 'alert';
const fileActionErrorCode = (action, state) => {
  const value = action === 'export' ? exportState(state) : externalState(state);
  return ['failed', 'uncertain'].includes(value.status) ? value.error?.code : null;
};
const fileActionNotice = (action, state) => {
  const message = fileActionMessage(action, state);
  const code = fileActionErrorCode(action, state);
  return code ? `${message} Error code: ${code}.` : message;
};
const isOwnedFileError = state => {
  const error = state.error;
  if (!error) return false;
  const matches = item => ['failed', 'uncertain'].includes(item.status)
    && item.error && (item.error === error || (item.error.code === error.code && item.error.message === error.message));
  return matches(exportState(state)) || (state.tab === 'external' && matches(externalState(state)));
};
const isDuplicateErrorStatusMessage = state => Boolean(state.error && state.message === state.error.message);
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
  return `<aside id="compile-selection" class="compile-selection" aria-label="Compilation selection"><div class="compile-panel-head"><div><p class="analysis-kicker">SAVED SKILLS</p><h2>Exact inputs</h2></div><button id="compile-refresh" class="analysis-button analysis-button--small" type="button" ${disabled(state, busy) ? 'disabled' : ''}>${state.pending === 'refresh' ? 'Refreshing…' : 'Refresh'}</button></div><p class="compile-note">Native reads resolve saved data for read-only compilation. Separate file actions can export or inspect; no instruction is installed or run.</p><div id="compile-capability-list" class="compile-capability-list">${capabilityList(state, busy)}</div><section class="compile-history" aria-label="Revision history"><p class="analysis-kicker">REVISION HISTORY</p><h3>Pin one revision</h3>${revisions(state, busy)}</section>${profiles(state, busy)}<div class="compile-actions"><button id="compile-generate" class="analysis-button analysis-button--primary" type="button" ${canGenerate ? '' : 'disabled'}>${state.pending === 'generate' ? 'Compiling…' : `${icon('bundle', { size: 16 })} Compile selected revision`}</button><button id="compile-open-skills" class="analysis-button" type="button" ${skillsBusy || busyState(state, busy) || !hasSelection ? 'disabled' : ''}>Open selected revision in Skills</button></div>${state.needsRefresh ? '<p class="compile-stale" role="alert">Workspace changed. Refresh this exact selection before compiling again.</p>' : ''}</aside>`;
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

function externalEvidenceFacts(value) {
  const inspection = value?.inspection;
  const compiled = inspection?.compilation;
  const selected = compiled?.selectedRevision;
  const provenance = selected?.provenance;
  const artifact = compiled?.artifact;
  if (!inspection || !compiled) return '<p class="compile-empty">Choose a selected bundle file to inspect its internal consistency. This does not change saved workspace records.</p>';
  const provenanceFacts = provenance?.kind === 'composition'
    ? `<div><dt>Provenance</dt><dd>Composition-derived revision</dd></div><div><dt>Composition application ID</dt><dd class="analysis-mono">${text(provenance.applicationId ?? 'Unavailable')}</dd></div><div><dt>Composition ID</dt><dd class="analysis-mono">${text(provenance.compositionId ?? 'Unavailable')}</dd></div><div><dt>Composition output index</dt><dd>${Number.isInteger(provenance.outputIndex) ? provenance.outputIndex : 'Unavailable'}</dd></div>`
    : `<div><dt>Provenance</dt><dd>${provenance?.kind === 'ordinary' ? 'Ordinary revision reference' : text(provenanceLabel(provenance))}</dd></div>`;
  return `<dl class="compile-evidence compile-external-evidence"><div><dt>Evidence class</dt><dd>External selected file</dd></div><div><dt>Verification</dt><dd>${text(inspection.verification ?? 'internal_consistency_only')}</dd></div><div><dt>Authority</dt><dd>${text(inspection.authority ?? 'none')}</dd></div><div><dt>Bundle ID</dt><dd class="analysis-mono">${text(inspection.bundleId ?? 'Unavailable')}</dd></div><div><dt>Bundle SHA-256</dt><dd class="analysis-mono">${text(inspection.sha256 ?? 'Unavailable')}</dd></div><div><dt>Bundle bytes</dt><dd>${number(inspection.byteLength)}</dd></div><div><dt>Profile</dt><dd>${text(profileLabel(compiled.profile?.id))}</dd></div><div><dt>Profile digest</dt><dd class="analysis-mono">${text(compiled.profileDigest ?? 'Unavailable')}</dd></div><div><dt>Capability ID</dt><dd class="analysis-mono">${text(selected?.capabilityId ?? 'Unavailable')}</dd></div><div><dt>Selected revision</dt><dd class="analysis-mono">${text(selected?.revisionId ?? 'Unavailable')}</dd></div><div><dt>Selected title</dt><dd>${text(selected?.title ?? 'Unavailable')}</dd></div><div><dt>Content SHA-256</dt><dd class="analysis-mono">${text(selected?.sha256 ?? 'Unavailable')}</dd></div>${provenanceFacts}<div><dt>Candidate identity</dt><dd class="analysis-mono">${text(compiled.candidate?.candidateId ?? 'Unavailable')}</dd></div><div><dt>Artifact SHA-256</dt><dd class="analysis-mono">${text(artifact?.sha256 ?? 'Unavailable')}</dd></div></dl><p class="compile-inspection-note">Internal consistency only. This is not authentication, complete lineage validation, target qualification, or execution authority.</p><details class="compile-manifest"><summary>Inspect external canonical candidate manifest</summary><pre id="compile-external-manifest" tabindex="0">${text(value.candidateManifestJson ?? '')}</pre></details><details class="compile-manifest"><summary>Inspect external instruction text</summary><pre id="compile-external-text" class="compile-artifact" tabindex="0">${text(artifact?.content ?? '')}</pre></details>`;
}

function externalPanel(state, active, busy) {
  const external = externalState(state);
  const current = externalReport(external.report);
  const previous = externalReport(external.previous);
  const rendered = current ?? previous;
  const hasPrevious = Boolean(rendered) && external.status !== 'inspected';
  const pending = busyState(state, busy) || external.status === 'pending';
  const status = fileActionNotice('inspect', state);
  return `<section id="compile-panel-external" class="compile-report-panel compile-external-panel" role="tabpanel" aria-labelledby="compile-tab-external" ${active ? '' : 'hidden'}><header><div><p class="analysis-kicker">SELECTED EXTERNAL FILE</p><h2 id="compile-external-title" tabindex="-1">External bundle</h2></div><button id="compile-inspect-external" class="analysis-button analysis-button--small" type="button" ${pending || !state.bridgeAvailable ? 'disabled' : ''}>${external.status === 'pending' ? 'Inspecting…' : 'Inspect bundle'}</button></header><p class="compile-inspection-note">Inspect a selected plaintext bundle. It stays separate from saved workspace evidence.</p>${active && status ? `<p class="${fileActionRole('inspect', state) === 'alert' ? 'compile-alert' : 'compile-note'}" role="${fileActionRole('inspect', state)}">${text(status)}</p>` : ''}${hasPrevious ? '<p class="compile-previous-label">Previous external inspection</p>' : ''}${rendered ? externalEvidenceFacts(rendered) : '<p class="compile-empty">Select a bundle file to inspect its internal consistency. No saved skill or compilation is required.</p>'}</section>`;
}

function inspectionPanel(state, busy) {
  const value = compilation(state);
  const tab = ['text', 'diagnostics', 'evidence', 'external'].includes(state.tab) ? state.tab : 'text';
  return `<section class="compile-inspection" aria-label="Compilation inspection"><div class="compile-tabs" role="tablist" aria-label="Compilation report tabs">${tabButton('text', 'Generated text', tab === 'text')}${tabButton('diagnostics', 'Diagnostics', tab === 'diagnostics')}${tabButton('evidence', 'Evidence', tab === 'evidence')}${tabButton('external', 'External bundle', tab === 'external')}</div>${generatedPanel(state, value, tab === 'text')}${diagnosticsPanel(value, tab === 'diagnostics')}${evidencePanel(state, value, tab === 'evidence')}${externalPanel(state, tab === 'external', busy)}</section>`;
}

function portableAction(state, busy) {
  const value = compilation(state);
  const bundle = exportState(state);
  const blocker = exportBlocker(state, busy);
  const receipt = bundle.status === 'exported' ? bundle.receipt : null;
  const status = fileActionNotice('export', state);
  return `<section class="compile-portable-action" aria-label="Portable bundle actions"><p class="analysis-kicker">PORTABLE ACTION</p><h3>Plaintext bundle</h3><p>Export writes a selected plaintext <span class="analysis-mono">.rangoon-instructions</span> file. It does not install or run instructions.</p><dl><div><dt>Expected candidate</dt><dd class="analysis-mono">${text(value?.candidate?.candidateId ?? 'Compile first')}</dd></div></dl><button id="compile-export" class="analysis-button analysis-button--primary" type="button" ${blocker ? 'disabled' : ''}>${bundle.status === 'pending' ? 'Exporting…' : 'Export candidate bundle'}</button><button id="compile-inspect" class="analysis-button" type="button" ${busyState(state, busy) || !state.bridgeAvailable ? 'disabled' : ''}>${externalState(state).status === 'pending' ? 'Inspecting…' : 'Inspect external bundle'}</button>${blocker ? `<p class="compile-note">${text(blocker)}</p>` : ''}${status ? `<p class="${fileActionRole('export', state) === 'alert' ? 'compile-alert' : 'compile-note'}" role="${fileActionRole('export', state)}">${text(status)}</p>` : ''}${receipt ? `<section class="compile-export-receipt"><p class="analysis-kicker">EXPORT RECEIPT</p><h3 id="compile-export-title" tabindex="-1">Write completed</h3><p>Completion covers this write and file sync only. It does not establish future integrity, directory durability, or authentication.</p><dl><div><dt>Bundle ID</dt><dd class="analysis-mono">${text(receipt.bundleId ?? 'Unavailable')}</dd></div><div><dt>Candidate ID</dt><dd class="analysis-mono">${text(receipt.candidateId ?? 'Unavailable')}</dd></div><div><dt>SHA-256</dt><dd class="analysis-mono">${text(receipt.sha256 ?? 'Unavailable')}</dd></div><div><dt>Bytes</dt><dd>${number(receipt.byteLength)}</dd></div><div><dt>Authority</dt><dd>${text(receipt.authority ?? 'none')}</dd></div></dl></section>` : ''}</section>`;
}

function evidenceRail(state, busy) {
  const value = compilation(state);
  return `<aside class="compile-evidence-rail" aria-label="Compilation state"><p class="analysis-kicker">STATE</p><h2>${text(value ? readinessLabel(value.readiness, { stale: state.reportStale }) : state.listStatus === 'unavailable' ? 'Desktop bridge unavailable' : state.needsRefresh ? 'Refresh required' : 'No compilation yet')}</h2><dl><div><dt>Selection</dt><dd>${selectedRevision(state) ? `${text(short(selectedRevision(state).id))}${selectedRevision(state).id === state.selected?.latestRevisionId ? ' · current' : ' · historical'}` : 'None'}</dd></div><div><dt>Profile</dt><dd>${text(profileLabel(state.profile))}</dd></div><div><dt>Bridge</dt><dd>${state.bridgeAvailable ? 'Native bridge available' : 'Native bridge unavailable'}</dd></div><div><dt>Report</dt><dd>${value ? state.reportStale ? 'Retained/stale native report' : 'Native report retained' : 'No report'}</dd></div></dl><p>Historical selection remains valid. Observed head records workspace state; it never replaces pinned revision.</p>${portableAction(state, busy)}</aside>`;
}

export function renderCompilationPage(state, { busy = false, skillsBusy = false } = {}) {
  const error = state.error;
  const message = state.message ?? '';
  const value = compilation(state);
  return `<section class="compile-page" aria-labelledby="compile-title" aria-busy="${busyState(state, busy)}"><header class="compile-hero"><p class="analysis-kicker">${icon('bundle', { size: 16 })} LOCAL COMPILATION INSPECTION</p><h1 id="compile-title" tabindex="-1">Compile <em>saved text.</em></h1><p>Inspect one exact local skill revision under a closed text profile. No installation, provider, engine or runtime action occurs here.</p><div class="compile-truth" aria-label="Compilation limits"><span>Runtime: untested</span><span>Target budget: unknown</span><span>Semantic equivalence: unverified</span><span>Authority: none</span></div><p class="compile-status">${text(isDuplicateErrorStatusMessage(state) ? '' : message)}</p>${error && !isOwnedFileError(state) ? `<p class="compile-alert" role="alert">${text(error.message ?? 'Local compilation could not finish.')}${error.code ? ` Error code: ${text(error.code)}.` : ''}</p>` : ''}${state.reportStale ? '<p class="compile-stale">Retained report may not match current workspace state. Refresh before another compile.</p>' : ''}${value?.readiness === 'blocked' ? '<p class="compile-blocked">Static diagnostics block candidate creation. Exact text stays available for inspection.</p>' : ''}</header><div class="compile-workbench">${selectionPanel(state, busy, skillsBusy)}${inspectionPanel(state, busy)}${evidenceRail(state, busy)}</div></section>`;
}
