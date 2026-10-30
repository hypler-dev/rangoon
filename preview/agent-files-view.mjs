import { renderMarkdownReview } from './markdown-review.mjs';
import { escapeText } from './analysis-model.mjs';
import { getAgentFilesProjection } from './agent-files-model.mjs';

const text = value => escapeText(value ?? '');
const pageSize = 50;

function formatLabel(format) {
  if (format === 'agents_markdown') return 'AGENTS.md';
  if (format === 'claude_markdown') return 'CLAUDE.md';
  if (format === 'skill_markdown') return 'SKILL.md';
  if (format === 'generic_markdown') return 'Markdown';
  return 'Unclassified';
}

function stateLabel(record) {
  if (record?.origin === 'folder') return 'Metadata only';
  if (record?.state === 'checked') return 'Checked';
  if (record?.state === 'failed') return 'Unavailable';
  return 'Unchecked';
}

function formatBytes(value) {
  if (!Number.isFinite(value) || value < 0) return 'Size unavailable';
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(value < 10240 ? 1 : 0)} KiB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
}

function sourceKind(record) {
  if (record?.origin === 'folder') return 'Folder file';
  if (record?.origin === 'saved') return 'Saved file';
  const match = typeof record?.key === 'string' ? /^selected:\d+:(\d+)$/.exec(record.key) : null;
  return `Selected file${match ? ` · item ${Number(match[1]) + 1}` : ''}`;
}

function timestamp(value, fallback = 'Not recorded', compact = false) {
  if (!Number.isSafeInteger(value) || value < 0 || value > 8_640_000_000_000_000) return text(fallback);
  const date = new Date(value);
  const label = new Intl.DateTimeFormat(undefined, compact ? { dateStyle: 'medium' } : { dateStyle: 'medium', timeStyle: 'medium' }).format(date);
  return `<time datetime="${date.toISOString()}" title="${date.toISOString()} · ${value} ms since Unix epoch">${text(label)}</time>`;
}
function exactTimestamp(value) {
  if (!Number.isSafeInteger(value) || value < 0 || value > 8_640_000_000_000_000) return 'Not recorded';
  return `${new Date(value).toISOString()} · ${value} ms since Unix epoch`;
}
function rowTimestamp(record) {
  if (record.origin === 'folder') return `Modified ${timestamp(record.modifiedAtMs, 'Unknown', true)}`;
  const saved = record.origin === 'saved';
  return `${saved ? 'Saved' : 'Selected'} ${timestamp(saved ? record.savedAtMs : record.selectedAtMs, 'Not recorded', true)}`;
}

function fileName(record) {
  const name = typeof record?.name === 'string' && record.name.trim() ? record.name : 'Unnamed file';
  return text(name);
}

function findingSummary(record) {
  const findings = Array.isArray(record?.findings) ? record.findings : [];
  const review = findings.filter(finding => finding?.severity === 'review').length;
  if (record?.origin === 'folder') return 'Contents not read';
  if (record?.state === 'failed') return 'Could not read';
  if (review) return `${review} review item${review === 1 ? '' : 's'}`;
  if (record?.state === 'checked') return findings.length ? `${findings.length} note${findings.length === 1 ? '' : 's'}` : 'No local matches';
  return 'Not checked';
}

function recordRow(record, selected, pending) {
  const key = text(record?.key);
  const marked = Boolean(selected.markedKeys?.includes?.(record?.key));
  const opened = record?.key === selected.selectedKey;
  return `<div class="agent-files-row${opened ? ' agent-files-row--open' : ''}" data-agent-files-row="${key}" role="listitem">
    <label class="agent-files-mark"><input type="checkbox" data-agent-files-mark="${key}" ${marked ? 'checked' : ''} ${pending || record.origin === 'folder' ? 'disabled' : ''}><span class="agent-files-sr">Mark ${fileName(record)}</span></label>
    <button type="button" class="agent-files-open" data-agent-files-open="${key}" aria-pressed="${opened}" ${pending ? 'disabled' : ''}>
      <span class="agent-files-file-icon" aria-hidden="true"></span><span class="agent-files-row__name">${fileName(record)}${record.origin==='folder' ? `<small class="agent-files-row__path" title="${text(record.relativePath)}">${text(record.relativePath)}</small>` : ''}<small class="agent-files-row__time">${rowTimestamp(record)}</small></span>
      <span class="agent-files-row__type">${text(formatLabel(record?.format))}</span>
      <span class="agent-files-row__origin">${text(sourceKind(record))}</span>
      <span class="agent-files-row__size">${text(formatBytes(record?.byteLength))}</span>
      <span class="agent-files-row__state agent-files-row__state--${text(record?.state ?? 'unchecked')}">${text(findingSummary(record))}</span>
    </button>
  </div>`;
}

function findings(record) {
  const items = Array.isArray(record?.findings) ? record.findings : [];
  if (!items.length) return `<p class="agent-files-muted">${record?.state === 'checked' ? 'No local rule matches.' : 'Run a local check to list advisory findings.'}</p>`;
  return `<ul class="agent-files-findings">${items.map(finding => `<li class="agent-files-finding agent-files-finding--${text(finding?.severity)}"><div><strong>${text(finding?.title || 'Untitled finding')}</strong><span>${text(finding?.severity === 'review' ? 'Review' : 'Note')}${Number.isInteger(finding?.line) && record.report ? ` · <button type="button" data-md-jump="${finding.line}" class="agent-files-finding-line">line ${finding.line}</button>` : Number.isInteger(finding?.line) ? ` · line ${finding.line}` : ''}</span></div><p>${text(finding?.explanation || 'No explanation available.')}</p><small>${text(finding?.rulePack || 'rangoon.agent-file-checks.v1')} · ${text(finding?.ruleId || 'Rule unavailable')}</small></li>`).join('')}</ul>`;
}

function sourceHandoff(record, state) {
  const saved = record?.origin === 'saved';
  const enabled = saved && state.sourceHandoffAvailable && !state.pending;
  return `<div class="agent-files-handoff"><button type="button" class="analysis-button" data-agent-files-action="open-source" aria-describedby="agent-files-handoff-help" ${enabled ? '' : 'disabled'}>${state.pending === 'handoff' ? 'Opening source…' : 'Open saved source'}</button><small id="agent-files-handoff-help">${saved ? state.sourceHandoffAvailable ? 'Review source sections and create reusable skills.' : 'Source-workbench access is unavailable in this view.' : 'Selected-file results are temporary. Open a saved file, or use Import &amp; Analyze to select and save this one.'}</small></div>`;
}

function inspector(record, state) {
  if (!record) return `<aside class="agent-files-inspector" aria-label="File inspector"><div class="agent-files-panel-head"><div><p class="analysis-kicker">INSPECTOR</p><h2>No file open</h2></div></div><div class="agent-files-inspector__empty"><p>Select a file to inspect its local text and advisory findings.</p><p>Compatibility not assessed.</p></div></aside>`;
  if (record.origin === 'folder') return `<aside class="agent-files-inspector" aria-label="File inspector"><div class="agent-files-panel-head"><div><p class="analysis-kicker">FOLDER FILE</p><h2 tabindex="-1">${fileName(record)}</h2><p>Metadata only</p></div></div><section class="agent-files-inspector__section"><dl class="agent-files-activity"><div><dt>Folder</dt><dd>${text(record.rootLabel)}</dd></div><div><dt>Relative path</dt><dd>${text(record.relativePath)}</dd></div><div><dt>Size</dt><dd>${text(formatBytes(record.byteLength))}</dd></div><div><dt>Modified</dt><dd>${timestamp(record.modifiedAtMs, 'Unknown')}</dd></div><div><dt>Listed locally</dt><dd>${timestamp(record.listedAtMs)}</dd></div></dl><p class="agent-files-muted">Modified is observed filesystem time, at whole-second precision. It does not establish a revision or freshness.</p></section><section class="agent-files-inspector__section"><h3>Inspect source</h3><p class="agent-files-muted">Contents have not been read. Choose individual files in the native picker to review text and findings.</p><button type="button" class="analysis-button" data-agent-files-action="choose" ${state.pending ? 'disabled' : ''}>Choose files to inspect</button></section></aside>`;
  const report = record.report;
  const content = typeof report?.source?.content === 'string' ? report.source.content : null;
  const error = record?.error?.message;
  return `<aside class="agent-files-inspector" aria-label="File inspector"><div class="agent-files-panel-head"><div><p class="analysis-kicker">INSPECTOR</p><h2 tabindex="-1">${fileName(record)}</h2>${content === null ? '<button type="button" class="analysis-button" data-agent-files-action="back-to-list">Back to files</button>' : ''}<p class="agent-files-inspector__subhead">${text(sourceKind(record))} · ${text(stateLabel(record))}</p></div></div>
    ${content === null ? sourceHandoff(record, state) : ''}
    <section class="agent-files-inspector__section"><h3>Record activity</h3><dl class="agent-files-activity"><div><dt>Saved</dt><dd>${timestamp(record.savedAtMs, 'Not saved locally')}</dd></div><div><dt>Selected</dt><dd>${timestamp(record.selectedAtMs)}</dd></div><div><dt>Checked</dt><dd>${timestamp(record.checkedAtMs, 'Not checked in this session')}</dd></div></dl><p class="agent-files-muted">Local times · hover a date for UTC. Selection and checks last only this session.</p></section>
    <section class="agent-files-inspector__section"><h3>Revision history</h3><p class="agent-files-muted">Not linked to this file. Saved snapshots are content records, not an ordered revision history.</p></section>
    <section class="agent-files-inspector__section"><h3>Advisory findings</h3>${findings(record)}</section>
    ${content === null ? `<section class="agent-files-inspector__section"><h3>Source text</h3><p class="agent-files-muted">${text(error || 'Source text is unavailable for this file.')}</p></section>` : ''}
    <section class="agent-files-inspector__section"><h3>Compatibility</h3><p class="agent-files-muted">Compatibility not assessed.</p></section>
    <details class="agent-files-details"><summary>Technical details</summary><dl><div><dt>Source ID</dt><dd>${text(record.sourceId || 'Unavailable')}</dd></div><div><dt>SHA-256</dt><dd>${text(record.sha256 || 'Unavailable')}</dd></div><div><dt>Bytes</dt><dd>${text(Number.isFinite(record.byteLength) ? record.byteLength : 'Unavailable')}</dd></div><div><dt>Saved at</dt><dd>${timestamp(record.savedAtMs, 'Not saved locally')}</dd></div><div><dt>Saved UTC</dt><dd>${text(exactTimestamp(record.savedAtMs))}</dd></div><div><dt>Selected UTC</dt><dd>${text(exactTimestamp(record.selectedAtMs))}</dd></div><div><dt>Checked UTC</dt><dd>${text(exactTimestamp(record.checkedAtMs))}</dd></div><div><dt>File modified</dt><dd>Not collected</dd></div><div><dt>Lines</dt><dd>${text(report?.source?.lineCount ?? 'Unavailable')}</dd></div><div><dt>Sections</dt><dd>${text(report?.fragments?.length ?? 'Unavailable')}</dd></div><div><dt>Analyzer</dt><dd>${text(report?.analyzerVersion ?? 'Unavailable')}</dd></div><div><dt>Time source</dt><dd>Saved: workspace record. Selected/checked: local renderer wall clock; not trusted audit time.</dd></div><div><dt>Format</dt><dd>${text(formatLabel(record.format))}</dd></div><div><dt>Rule pack</dt><dd>rangoon.agent-file-checks.v1</dd></div></dl></details>
  </aside>`;
}

function noRecords(status, hasFilters, checkCancelled) {
  if (checkCancelled) return 'Local check cancelled. Existing results remain unchanged.';
  if (hasFilters) return 'No files match these filters.';
  if (status === 'unavailable') return 'Open this route in Rangoon desktop to choose local files.';
  if (status === 'failed') return 'Saved-file refresh could not finish. Prior results remain when available.';
  return 'Choose Markdown files or reload saved files to begin.';
}

function groupRows(rows, state, pending) {
  const selected = rows.filter(record => record.origin === 'selected');
  const saved = rows.filter(record => record.origin === 'saved');
  const folder = rows.filter(record => record.origin === 'folder');
  const section = (label, records) => records.length ? `<section class="agent-files-group" aria-label="${label}"><h3>${label}<span>${records.length}</span></h3>${records.map(record => recordRow(record, state, pending)).join('')}</section>` : '';
  return `${section('Selected files', selected)}${section('Saved files', saved)}${section('Folder listing', folder)}`;
}

export function renderAgentFilesView(state) {
  const projection = getAgentFilesProjection(state);
  const pending = Boolean(state.pending);
  const unavailable = state.status === 'unavailable';
  const hasFilters = Boolean(state.query) || state.typeFilter !== 'all' || state.findingFilter !== 'all';
  const canChoose = !pending && !unavailable;
  const canRefresh = !pending && !unavailable;
  const markedSavedCount = state.records.filter(record => state.markedKeys.includes(record.key) && record.origin === 'saved').length;
  const canCheck = !pending && markedSavedCount > 0;
  const canOpenImportWorkbench = !pending && !unavailable;
  const canClearSelectedFiles = !pending && state.records.some(record => record.origin === 'selected');
  const showing = projection.pageRows.length;
  const selectedHidden = projection.selected && !projection.filtered.some(record => record.key === projection.selected.key);
  const selectedOutsidePage = projection.selected && !selectedHidden && !projection.pageRows.some(record => record.key === projection.selected.key);
  const filters = `<section class="agent-files-filters" aria-label="File filters"><label>Search files<input type="search" data-agent-files-query maxlength="256" value="${text(state.query)}" autocomplete="off" placeholder="Name or source ID"></label><label>Type<select data-agent-files-type>${['all', 'agents_markdown', 'claude_markdown', 'skill_markdown', 'generic_markdown'].map(value => `<option value="${value}" ${state.typeFilter === value ? 'selected' : ''}>${text(value === 'all' ? 'All types' : formatLabel(value))}</option>`).join('')}</select></label><label>Findings<select data-agent-files-findings>${[['all', 'All files'], ['review', 'Review items'], ['unchecked', 'Unchecked'], ['metadata', 'Metadata only'], ['no_matches', 'No local matches']].map(([value, label]) => `<option value="${value}" ${state.findingFilter === value ? 'selected' : ''}>${label}</option>`).join('')}</select></label></section>`;
  const selectedContent = projection.selected?.report?.source?.content;
  const hasSource = typeof selectedContent === 'string';
  const sourceReview = hasSource ? `<section class="agent-files-source" aria-label="Source editor"><div class="agent-files-panel-head"><div><p class="analysis-kicker">SOURCE REVIEW</p><h2 tabindex="-1">${fileName(projection.selected)}</h2></div><button type="button" class="analysis-button" data-agent-files-action="back-to-list">Back to files</button></div>${sourceHandoff(projection.selected, state)}${renderMarkdownReview(selectedContent)}</section>` : '';
  const status = state.error ? state.error.message : state.message;
  const rows = showing ? groupRows(projection.pageRows, state, pending) : `<div class="agent-files-empty"><p>${text(noRecords(state.status, hasFilters, state.checkCancelled))}</p></div>`;
  return `<section class="agent-files-page" aria-labelledby="agent-files-title" aria-busy="${pending}">
    <header class="agent-files-header"><div><p class="analysis-kicker">LOCAL / READ ONLY</p><h1 id="agent-files-title" tabindex="-1">Agent <em>files</em></h1><p class="agent-files-status${state.error ? ' agent-files-status--error' : ''}" role="status">${text(status || 'Choose files to inspect local instructions.')}</p></div><div class="agent-files-header__actions"><button type="button" class="analysis-button analysis-button--primary" data-agent-files-action="choose" ${canChoose ? '' : 'disabled'}>${state.pending === 'choose' ? 'Choosing…' : 'Choose files'}</button><button type="button" class="analysis-button" data-agent-files-action="choose-folder" ${canChoose ? '' : 'disabled'}>${state.pending === 'folder' ? 'Listing folder…' : 'Choose folder'}</button><button type="button" class="analysis-button" data-agent-files-action="refresh" ${canRefresh ? '' : 'disabled'}>${state.pending === 'refresh' ? 'Reloading…' : 'Reload saved'}</button><details class="agent-files-limits"><summary>Inspection limits</summary><div><p>Manual selection: up to 32 Markdown files, held in memory up to 4 MiB. Saved inventory: up to 128 files.</p><p>Folder discovery: up to 1000 metadata rows, 10,000 visited entries and 2 MiB per listing. Linked and excluded folders are skipped. Contents are opened separately.</p><p>Selected files remain unsaved. Saving uses the current one-file Import &amp; Analyze path.</p><button type="button" class="analysis-button" data-agent-files-action="import" ${canOpenImportWorkbench ? '' : 'disabled'}>Open Import &amp; Analyze</button></div></details></div></header>

    ${state.folder ? `<section class="agent-files-folder-summary" aria-label="Folder listing status"><p><strong>${text(state.folder.rootLabel)}</strong> · ${state.records.filter(r=>r.origin==='folder').length} listed · ${state.folder.partialReasons.length ? 'Partial listing' : 'Metadata only'}</p><button type="button" class="analysis-button" data-agent-files-action="clear-folder" ${pending ? 'disabled' : ''}>Clear folder</button><details><summary>Listing details</summary><p>${state.folder.visitedEntries} entries visited · ${state.folder.excludedEntries} excluded by discovery policy. Listing is an observation, not a synchronized snapshot.</p>${state.folder.partialReasons.length ? `<ul>${state.folder.partialReasons.map(reason=>`<li>${text(({read_error:'Some entries could not be listed.',changed_directory:'A directory changed during listing.',other_device:'A different filesystem was skipped.',repeated_directory:'A repeated directory was skipped.',repeated_entry:'A repeated file entry was skipped.',depth_limit:'Folder depth limit reached.',entry_limit:'Visited-entry limit reached.',record_limit:'1000-file window reached.',unsupported_name:'Some names could not be represented safely.'})[reason] ?? 'Some entries are unavailable.')}</li>`).join('')}</ul>` : ''}</details></section>` : ''}
    <div class="agent-files-toolbar">${filters}
    <div class="agent-files-toolbar-info"><section class="agent-files-summary" aria-label="File summary"><span><strong>${projection.counts.all}</strong> files</span><span><strong>${projection.counts.review}</strong> files to review</span><span><strong>${projection.counts.unchecked}</strong> unchecked</span>${projection.counts.metadata ? `<span><strong>${projection.counts.metadata}</strong> metadata only</span>` : ''}${projection.counts.failed ? `<span><strong>${projection.counts.failed}</strong> unavailable</span>` : ''}</section>
    <p class="agent-files-brief">Checks advisory; original files unchanged.</p></div></div>
    <div class="agent-files-workbench${hasSource ? ' agent-files-workbench--review' : ''}"><section class="agent-files-library" aria-label="Agent file library"><div class="agent-files-panel-head"><div><p class="analysis-kicker">FILE LIBRARY</p><h2 tabindex="-1">${projection.filtered.length} match${projection.filtered.length === 1 ? '' : 'es'}</h2></div><span>${projection.markedCount} marked</span></div><div class="agent-files-bulk"><button type="button" class="analysis-button" data-agent-files-action="select-page" ${pending || !showing || !projection.pageRows.some(r=>r.origin!=='folder') ? 'disabled' : ''}>Mark page</button>${projection.markedCount ? `<button type="button" class="analysis-button" data-agent-files-action="clear-selection" ${pending ? 'disabled' : ''}>Clear marks</button><button type="button" class="analysis-button" data-agent-files-action="check-selected" ${canCheck ? '' : 'disabled'}>${state.pending === 'check' ? 'Checking…' : `Check ${markedSavedCount} marked saved ${markedSavedCount === 1 ? 'file' : 'files'}`}</button>` : ''}${state.pending === 'check' ? '<button type="button" class="analysis-button" data-agent-files-action="cancel-check">Cancel check</button>' : ''}${canClearSelectedFiles ? '<button type="button" class="analysis-button" data-agent-files-action="clear-selected-files">Clear selected files</button>' : ''}</div>${selectedHidden ? '<p class="agent-files-hidden">Open file remains in inspector outside these filters.</p>' : selectedOutsidePage ? '<p class="agent-files-hidden">Open file remains in inspector on another page.</p>' : ''}<div class="agent-files-table" role="list">${rows}</div><nav class="agent-files-pagination" aria-label="Agent file pages"><button type="button" class="analysis-button" data-agent-files-page="${projection.page - 1}" ${projection.page === 0 ? 'disabled' : ''}>Previous</button><p data-agent-files-page-status tabindex="-1">Page ${projection.page + 1} of ${projection.pageCount} · ${showing ? projection.page * pageSize + 1 : 0}–${Math.min((projection.page + 1) * pageSize, projection.filtered.length)} of ${projection.filtered.length}</p><button type="button" class="analysis-button" data-agent-files-page="${projection.page + 1}" ${projection.page + 1 >= projection.pageCount ? 'disabled' : ''}>Next</button></nav></section>${sourceReview}${inspector(projection.selected, state)}</div>
  </section>`;
}
