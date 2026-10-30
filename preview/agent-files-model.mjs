// Local view/advisory state only. Native Rust owns file reads and trust boundaries.
export const AGENT_FILE_RULE_PACK = 'rangoon.agent-file-checks.v1';
export const AGENT_FILES_PAGE_SIZE = 50;
const MAX_BYTES = 256 * 1024;
const MAX_SELECTED_BYTES = 4 * 1024 * 1024;
const formats = new Set(['agents_markdown', 'claude_markdown', 'skill_markdown', 'generic_markdown']);
const hex = value => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
const sourceId = value => typeof value === 'string' && value.startsWith('source:') && hex(value.slice(7));
const record = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const exact = (value, keys) => record(value) && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
const utf8 = value => new TextEncoder().encode(value).length;
const safeName = value => typeof value === 'string' && value.length > 0 && utf8(value) <= 255 && !/[\p{Cc}\/\\]/u.test(value);
const fixedError = (code = 'agent_files_failed', message = 'File inspection could not finish. Previous results remain available.') => ({ code, message });
const publicError = value => typeof value?.code === 'string' && value.code.length <= 128 && typeof value.message === 'string' && value.message.length <= 2048 ? { code: value.code, message: value.message } : fixedError();
function formatFromName(name) { return { 'agents.md': 'agents_markdown', 'claude.md': 'claude_markdown', 'skill.md': 'skill_markdown' }[name.toLowerCase()] ?? 'generic_markdown'; }
function boundedFragment(f, source) {
  if (!exact(f, ['id','kind','heading','span','text','reviewState']) || typeof f.id !== 'string' || !/^fragment:[a-f0-9]{64}$/.test(f.id) || !['preamble','section'].includes(f.kind) || f.reviewState !== 'unreviewed' || typeof f.text !== 'string' || utf8(f.text) > MAX_BYTES) return false;
  const span=f.span;
  if (!exact(span,['startByte','endByte','startLine','endLine']) || !Object.values(span).every(Number.isSafeInteger) || span.startByte < 0 || span.endByte < span.startByte || span.endByte > source.byteLength || utf8(f.text)!==span.endByte-span.startByte || span.startLine < 1 || span.endLine<span.startLine || span.endLine>source.lineCount) return false;
  return f.heading===null || exact(f.heading,['level','title']) && Number.isSafeInteger(f.heading.level) && f.heading.level>=1 && f.heading.level<=6 && typeof f.heading.title==='string' && utf8(f.heading.title)<=16384;
}
function validReport(value) {
  const s = value?.source;
  return exact(value, ['schemaVersion', 'analyzerVersion', 'source', 'fragments', 'diagnostics', 'authority'])
    && value.schemaVersion === 'rangoon.source-analysis.v0' && value.analyzerVersion === '0.1.0' && value.authority === 'none'
    && exact(s, ['id', 'displayName', 'format', 'sha256', 'byteLength', 'lineCount', 'content'])
    && sourceId(s.id) && safeName(s.displayName) && formats.has(s.format) && hex(s.sha256)
    && typeof s.content === 'string' && !s.content.includes('\0') && utf8(s.content) === s.byteLength && s.byteLength <= MAX_BYTES
    && Number.isSafeInteger(s.lineCount) && s.lineCount >= 0 && s.lineCount <= 10000
    && Array.isArray(value.fragments) && value.fragments.length <= 256 && value.fragments.every(f=>boundedFragment(f,s))
    && Array.isArray(value.diagnostics) && value.diagnostics.length <= 512
    && value.diagnostics.every(d => record(d) && typeof d.code === 'string' && d.code.length <= 128 && typeof d.message === 'string' && d.message.length <= 2048 && (d.line === null || Number.isSafeInteger(d.line) && d.line >= 1 && d.line <= 10000));
}

function retainReport(report) {
  // Preserve the received original as immutable view data; this is not a claim
  // that renderer freezing authenticates native bytes or acquires file custody.
  Object.freeze(report.source);
  for (const fragment of report.fragments) if (record(fragment)) {
    if (record(fragment.span)) Object.freeze(fragment.span);
    if (record(fragment.heading)) Object.freeze(fragment.heading);
    Object.freeze(fragment);
  }
  for (const diagnostic of report.diagnostics) Object.freeze(diagnostic);
  Object.freeze(report.fragments); Object.freeze(report.diagnostics);
  return Object.freeze(report);
}

// These matches are review hints, not a security verdict, parsed command,
// deprecated-version assertion, source authentication, or authority decision.
export function inspectAgentFileReport(report) {
  if (!validReport(report)) return null;
  const findings = [];
  const add = (ruleId, title, severity, line, explanation) => findings.push(Object.freeze({ ruleId, title, severity, line, explanation, rulePack: AGENT_FILE_RULE_PACK }));
  if (!report.source.content.trim()) add('content.empty', 'Empty instructions', 'review', null, 'No non-whitespace instruction text was found. Verify whether this file is intentional.');
  const lines = report.source.content.split('\n');
  let secretShown = false;
  let scriptShown = false;
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (!secretShown && /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/.test(line)) {
      add('content.private-key-marker', 'Possible private key', 'review', index + 1, 'A private-key marker appears here. It may be an example; inspect locally before sharing this file. Matched content is not copied into this finding.'); secretShown = true;
    }
    if (!scriptShown && /\b(?:curl|wget)\b/.test(line) && /\|\s*(?:sh|bash|zsh)\b/.test(line)) {
      add('instructions.remote-script-pipe', 'Review remote script instruction', 'review', index + 1, 'This line combines a download command and a shell pipe. Check its source and context. Nothing was executed.'); scriptShown = true;
    }
    if (secretShown && scriptShown) break;
  }
  for (const diagnostic of report.diagnostics.slice(0, 32)) {
    add(`parser.${diagnostic.code}`, 'Parser note', 'note', diagnostic.line, diagnostic.message);
  }
  return Object.freeze(findings);
}
function baseState(available) {
  return { status: available ? 'idle' : 'unavailable', pending: null, records: [], folder: null, selectedKey: null, markedKeys: [], query: '', typeFilter: 'all', findingFilter: 'all', page: 0, error: null, message: available ? 'Choose files or load saved sources.' : 'Open Rangoon desktop to inspect local files.', checkCancelled: false, checkCompleted: 0, checkTotal: 0 };
}
function nativeSaved(values) {
  if (!Array.isArray(values) || values.length > 128) return null;
  const ids = new Set();
  const result = [];
  for (const s of values) {
    if (!exact(s, ['sourceId', 'displayName', 'sha256', 'byteLength', 'savedAtMs']) || !sourceId(s.sourceId) || ids.has(s.sourceId) || !safeName(s.displayName) || !hex(s.sha256) || !Number.isSafeInteger(s.byteLength) || s.byteLength < 0 || s.byteLength > MAX_BYTES || !Number.isSafeInteger(s.savedAtMs) || s.savedAtMs < 0 || s.savedAtMs > 8_640_000_000_000_000) return null;
    ids.add(s.sourceId); result.push({ key: `saved:${s.sourceId}`, name: s.displayName, sourceId: s.sourceId, sha256: s.sha256, byteLength: s.byteLength, savedAtMs: s.savedAtMs, selectedAtMs: null, checkedAtMs: null, origin: 'saved', format: formatFromName(s.displayName), state: 'unchecked', findings: [], report: null, error: null });
  }
  return result;
}
function nativeCollection(value, serial, observedAtMs) {
  if (!exact(value, ['outcome', 'schemaVersion', 'selectedCount', 'retainedBytes', 'items']) || value.outcome !== 'inspected' || value.schemaVersion !== 'rangoon.source-collection.v1' || !Array.isArray(value.items) || value.items.length < 1 || value.items.length > 32 || value.selectedCount !== value.items.length || !Number.isSafeInteger(value.retainedBytes) || value.retainedBytes < 0 || value.retainedBytes > MAX_SELECTED_BYTES) return null;
  let total = 0;
  const rows = [];
  for (let index = 0; index < value.items.length; index += 1) {
    const item = value.items[index];
    if (!exact(item, ['selectionIndex', 'displayName', 'result']) || item.selectionIndex !== index || !safeName(item.displayName)) return null;
    const result = item.result;
    if (exact(result, ['outcome', 'report']) && result.outcome === 'analyzed') {
      const findings = inspectAgentFileReport(result.report);
      if (findings === null || result.report.source.displayName !== item.displayName) return null;
      total += result.report.source.byteLength; if (total > MAX_SELECTED_BYTES) return null;
      const s = result.report.source;
      rows.push({ key: `selected:${serial}:${index}`, name: s.displayName, sourceId: s.id, sha256: s.sha256, byteLength: s.byteLength, format: s.format, origin: 'selected', savedAtMs: null, selectedAtMs: observedAtMs, checkedAtMs: observedAtMs, state: 'checked', findings, report: retainReport(result.report), error: null });
    } else if (exact(result, ['outcome', 'error']) && ['rejected', 'failed'].includes(result.outcome)) {
      rows.push({ key: `selected:${serial}:${index}`, name: item.displayName, sourceId: null, sha256: null, byteLength: 0, format: null, origin: 'selected', savedAtMs: null, selectedAtMs: observedAtMs, checkedAtMs: null, state: 'failed', findings: [], report: null, error: publicError(result.error) });
    } else return null;
  }
  return total === value.retainedBytes ? rows : null;
}
const folderReasons = new Set(['read_error','changed_directory','other_device','repeated_directory','repeated_entry','depth_limit','entry_limit','record_limit','unsupported_name']);
const nativeNameCompatible = value => [...value].every(c=>c.charCodeAt(0)<128 || /[\p{Alphabetic}\p{Number}]/u.test(c));
const folderComponent = part => typeof part==='string' && part.length>0 && part.length<=255 && part!=='.' && part!=='..' && nativeNameCompatible(part) && !/[\\/:%?#\p{Cc}\p{Cf}\p{Cs}]/u.test(part) && !/[. ]$/.test(part) && !/^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(part);
const folderExcluded = part => /^(?:node_modules|target|vendor|dist|build)$/i.test(part) || part.startsWith('.') && !/^(?:\.claude|\.cursor|\.windsurf|\.github|\.codex|\.agents)$/i.test(part);
function nativeFolder(value, serial, listedAtMs) {
  if (!exact(value, ['outcome','schemaVersion','rootLabel','entries','visitedEntries','emittedRecords','excludedEntries','exhausted','partialReasons']) || value.outcome !== 'listed' || value.schemaVersion !== 'rangoon.folder-collection.v1' || !safeName(value.rootLabel) || !folderComponent(value.rootLabel) || /[\p{Cf}\p{Cs}]/u.test(value.rootLabel) || !Array.isArray(value.entries) || value.entries.length > 1000 || value.emittedRecords !== value.entries.length || !Number.isSafeInteger(value.visitedEntries) || value.visitedEntries < value.entries.length || value.visitedEntries > 10000 || !Number.isSafeInteger(value.excludedEntries) || value.excludedEntries < 0 || value.excludedEntries > value.visitedEntries || value.exhausted !== true || !Array.isArray(value.partialReasons) || value.partialReasons.length > folderReasons.size || new Set(value.partialReasons).size !== value.partialReasons.length || value.partialReasons.some(reason => !folderReasons.has(reason))) return null;
  if (value.partialReasons.includes('record_limit') && value.entries.length!==1000 || value.partialReasons.includes('entry_limit') && value.visitedEntries!==10000) return null;
  const paths = new Set(); const rows = [];
  for (const [index, item] of value.entries.entries()) {
    if (!exact(item,['observationId','relativePath','byteLength','modifiedAtMs']) || item.observationId !== index + 1 || typeof item.relativePath !== 'string' || item.relativePath.length > 1024 || /[\\:%?#\p{Cc}\p{Cf}\p{Cs}]/u.test(item.relativePath) || !/\.md$/i.test(item.relativePath) || item.relativePath.split('/').some(part=>!folderComponent(part) || folderExcluded(part)) || paths.has(item.relativePath) || !Number.isSafeInteger(item.byteLength) || item.byteLength<0 || !(item.modifiedAtMs===null || Number.isSafeInteger(item.modifiedAtMs) && item.modifiedAtMs>=0 && item.modifiedAtMs<=8_640_000_000_000_000 && item.modifiedAtMs%1000===0)) return null;
    paths.add(item.relativePath); const name = item.relativePath.split('/').at(-1);
    rows.push({key:`folder:${serial}:${index}`, name, relativePath:item.relativePath, rootLabel:value.rootLabel, modifiedAtMs:item.modifiedAtMs, listedAtMs, sourceId:null, sha256:null, byteLength:item.byteLength, format:formatFromName(name), origin:'folder', savedAtMs:null, selectedAtMs:null, checkedAtMs:null, state:'metadata', findings:[], report:null, error:null});
  }
  if (utf8(JSON.stringify(value))>2*1024*1024) return null;
  return { rows, folder:{rootLabel:value.rootLabel,listedAtMs,visitedEntries:value.visitedEntries,excludedEntries:value.excludedEntries,partialReasons:[...value.partialReasons]} };
}
export function getAgentFilesProjection(state) {
  const marked = new Set(state.markedKeys);
  const query = state.query.trim().toLocaleLowerCase('en');
  const filtered = state.records.filter(row => (!query || `${row.name} ${row.relativePath ?? ''} ${row.sourceId ?? ''} ${row.findings.map(f => `${f.title} ${f.ruleId}`).join(' ')}`.toLocaleLowerCase('en').includes(query)) && (state.typeFilter === 'all' || row.format === state.typeFilter) && (state.findingFilter === 'all' || state.findingFilter === 'review' && row.findings.some(f => f.severity === 'review') || state.findingFilter === 'unchecked' && row.state === 'unchecked' || state.findingFilter === 'metadata' && row.origin === 'folder' || state.findingFilter === 'no_matches' && row.state === 'checked' && !row.findings.some(f => f.severity === 'review')));
  const pageCount = Math.max(1, Math.ceil(filtered.length / AGENT_FILES_PAGE_SIZE));
  const page = Math.max(0, Math.min(state.page, pageCount - 1));
  const shown = new Set(filtered.map(row => row.key));
  return { filtered, pageRows: filtered.slice(page * AGENT_FILES_PAGE_SIZE, (page + 1) * AGENT_FILES_PAGE_SIZE), page, pageCount, selected: state.records.find(row => row.key === state.selectedKey) ?? null, markedCount: marked.size, hiddenMarkedCount: [...marked].filter(key => !shown.has(key)).length, counts: { all: state.records.length, review: state.records.filter(r => r.findings.some(f => f.severity === 'review')).length, metadata: state.records.filter(r=>r.origin==='folder').length, unchecked: state.records.filter(r => r.state==='unchecked').length, failed: state.records.filter(r => r.state === 'failed').length } };
}
export function createAgentFilesController({ invoke, onChange = () => {}, now = Date.now, openSavedSource } = {}) {
  // UI observation times are local wall-clock metadata, never authoritative audit time.
  const observedTime = () => { try { const value = now(); return Number.isSafeInteger(value) && value >= 0 && value <= 8_640_000_000_000_000 ? value : null; } catch { return null; } };
  const available = typeof invoke === 'function';
  let state = baseState(available);
  let epoch = 0;
  let serial = 0;
  let disposed = false;
  const publish = changes => {
    if (disposed) return;
    state = { ...state, ...changes };
    state.page = getAgentFilesProjection(state).page;
    onChange(controller.getState());
  };
  const call = async (command, args) => { try { return await invoke(command, args); } catch { return { outcome: 'failed', error: fixedError() }; } };
  const start = pending => { if (!available || state.pending || disposed) return null; const token = ++epoch; publish({ pending, error: null, checkCancelled: false }); return token; };
  const current = token => !disposed && token === epoch;
  const replace = records => {
    const keys = new Set(records.map(r => r.key));
    return { records, markedKeys: state.markedKeys.filter(k => keys.has(k)), selectedKey: keys.has(state.selectedKey) ? state.selectedKey : null };
  };
  const controller = {
    getState: () => ({ ...state, sourceHandoffAvailable: available && typeof openSavedSource === 'function', folder: state.folder ? {...state.folder,partialReasons:[...state.folder.partialReasons]} : null, records: state.records.map(r => ({ ...r, findings: [...r.findings] })), markedKeys: [...state.markedKeys] }),
    purgeForWorkspaceLock() { epoch += 1; state = baseState(available); if (!disposed) onChange(controller.getState()); },
    dispose() { controller.purgeForWorkspaceLock(); disposed = true; },
    setQuery(query) { if (typeof query !== 'string' || query.length > 256 || query === state.query) return false; publish({ query, page: 0 }); return true; },
    setType(typeFilter) { if (typeFilter !== 'all' && !formats.has(typeFilter) || typeFilter === state.typeFilter) return false; publish({ typeFilter, page: 0 }); return true; },
    setFindingFilter(findingFilter) { if (!['all', 'review', 'unchecked', 'metadata', 'no_matches'].includes(findingFilter) || findingFilter === state.findingFilter) return false; publish({ findingFilter, page: 0 }); return true; },
    setPage(page) { if (!Number.isSafeInteger(page) || page < 0 || page >= getAgentFilesProjection(state).pageCount || page === state.page) return false; publish({ page }); return true; },
    mark(key) { if (state.pending || !state.records.some(r => r.key === key && r.origin !== 'folder')) return; const marked = new Set(state.markedKeys); if (marked.has(key)) marked.delete(key); else marked.add(key); publish({ markedKeys: [...marked] }); },
    selectPage() { if (state.pending) return; publish({ markedKeys: [...new Set([...state.markedKeys, ...getAgentFilesProjection(state).pageRows.filter(r=>r.origin!=='folder').map(r => r.key)])] }); },
    clearSelection() { if (!state.pending) publish({ markedKeys: [] }); },
    clearFolder() { if (!state.pending) publish({...replace(state.records.filter(r=>r.origin!=='folder')),folder:null,message:'Folder listing cleared. Original files are unchanged.'}); },
    clearSelectedFiles() { if (!state.pending) publish({ ...replace(state.records.filter(r => r.origin !== 'selected')), message: 'Selected-file results cleared from this view. Original files are unchanged.' }); },
    cancelCheck() { if (state.pending === 'check' && !state.checkCancelled) publish({ checkCancelled: true, message: 'Stopping after the current native read returns…' }); },
    acceptInitialSavedFailure(error) {
      if (!available || disposed || state.pending || state.status !== 'idle') return false;
      publish({ status: 'failed', error: publicError(error), message: 'Saved files could not load. Retry with Reload saved or choose files.' }); return true;
    },
    acceptInitialSavedSnapshots(values) {
      if (!available || disposed || state.pending || state.status !== 'idle') return false;
      const saved = nativeSaved(values); if (!saved) return false;
      publish({ ...replace(saved), status: 'ready', message: `${saved.length} saved source files loaded.` }); return true;
    },
    async refresh() {
      const token = start('refresh'); if (token === null) return;
      const result = await call('list_snapshots'); if (!current(token)) return;
      const saved = result?.outcome === 'listed' ? nativeSaved(result.snapshots) : null;
      if (!saved) { publish({ pending: null, status: state.records.length ? 'ready' : 'failed', error: publicError(result?.error), message: 'Saved files could not load. Previous results remain available.' }); return; }
      const previous = new Map(state.records.filter(r => r.origin === 'saved').map(r => [r.key, r]));
      const rows = saved.map(row => { const old = previous.get(row.key); return old && old.sha256 === row.sha256 && old.byteLength === row.byteLength ? { ...row, state: old.state, findings: old.findings, error: old.error, report: old.report, checkedAtMs: old.checkedAtMs } : row; });
      publish({ ...replace([...state.records.filter(r => r.origin !== 'saved'), ...rows]), pending: null, status: 'ready', message: `${saved.length} saved source files loaded.` });
    },
    async chooseFolder() {
      const token = start('folder'); if (token === null) return;
      const result = await call('select_and_list_sources'); if (!current(token)) return;
      if (result?.outcome === 'cancelled' && exact(result,['outcome'])) { publish({pending:null}); return; }
      const parsed = nativeFolder(result, ++serial, observedTime());
      if (!parsed) { publish({pending:null,error: result?.outcome==='failed' ? publicError(result.error) : fixedError('folder_inventory_invalid','Folder metadata could not be accepted. Previous results remain available.')}); return; }
      const records=[...parsed.rows,...state.records.filter(r=>r.origin!=='folder')];
      publish({...replace(records),folder:parsed.folder,pending:null,status:'ready',page:0,message:`${parsed.rows.length} folder files listed${parsed.folder.partialReasons.length ? ' · partial listing' : ''}. Contents not read.`});
    },
    async choose() {
      const token = start('choose'); if (token === null) return;
      const result = await call('select_and_inspect_sources'); if (!current(token)) return;
      if (result?.outcome === 'cancelled') { publish({ pending: null, message: 'Selection cancelled. Previous results remain available.' }); return; }
      const rows = nativeCollection(result, ++serial, observedTime());
      if (!rows) { publish({ pending: null, status: state.records.length ? 'ready' : 'failed', error: publicError(result?.error), message: 'Selection could not be inspected. Previous results remain available.' }); return; }
      if (rows.every(row => row.state === 'failed') && state.records.some(row => row.origin === 'selected')) {
        publish({ pending: null, error: fixedError('source_collection_unread', 'No file in the new selection could be inspected. Previous selected-file results were retained.'), message: 'New selection unread. Previous results remain available.' }); return;
      }
      const records = [...rows, ...state.records.filter(r => r.origin !== 'selected')];
      publish({ ...replace(records), selectedKey: rows[0]?.key ?? state.selectedKey, pending: null, status: 'ready', page: 0, message: `${rows.length} selected · ${rows.filter(r => r.state === 'failed').length} unread · not saved.` });
    },
    async openInAnalysis() {
      const row = state.records.find(item => item.key === state.selectedKey);
      if (!row || row.origin !== 'saved' || !sourceId(row.sourceId) || !hex(row.sha256)
        || !Number.isSafeInteger(row.byteLength) || row.byteLength < 0 || row.byteLength > MAX_BYTES
        || typeof openSavedSource !== 'function') return false;
      const token = start('handoff'); if (token === null || !current(token)) return false;
      publish({ message: 'Opening this saved file in the source workbench…' });
      if (!current(token)) return false;
      let outcome = 'failed';
      try { const result = await openSavedSource({ sourceId: row.sourceId, sha256: row.sha256, byteLength: row.byteLength }); outcome = result?.outcome; } catch { /* Preserve both workbenches after an unavailable callback. */ }
      if (!current(token)) return false;
      const selected = state.records.find(item => item.key === state.selectedKey);
      if (selected?.key !== row.key || selected.sourceId !== row.sourceId || selected.sha256 !== row.sha256 || selected.byteLength !== row.byteLength) {
        publish({ pending: null, error: fixedError('source_handoff_changed', 'The saved selection changed. Open the file again before continuing.'), message: 'Source handoff stopped.' }); return false;
      }
      const opened = outcome === 'opened';
      const errors = {
        busy: ['source_handoff_busy', 'Finish the active source-workbench action, then retry. File inspection remains available.'],
        mismatch: ['source_handoff_mismatch', 'The returned source did not match this saved file. Reload saved files and retry. Previous analysis remains available.'],
        removed: ['source_handoff_removed', 'This saved source was removed while opening. Reload saved files. Previous inspection remains available.'],
      };
      const diagnostic = Object.hasOwn(errors, outcome) ? errors[outcome] : ['source_handoff_failed', 'The saved source could not open. Retry or reload saved files. Previous file inspection remains available.'];
      publish({ pending: null, error: opened ? null : fixedError(...diagnostic),
        message: opened ? 'Saved source opened in the source workbench.' : diagnostic[1] });
      return current(token) && opened;
    },
    async open(key) {
      if (state.pending) return;
      const row = state.records.find(r => r.key === key); if (!row) return;
      if (row.origin === 'selected' || row.origin === 'folder' || row.report) { publish({ selectedKey: key }); return; }
      const token = start('inspect'); if (token === null) return;
      publish({ selectedKey: key });
      const result = await call('read_composition_source', { sourceId: row.sourceId }); if (!current(token)) return;
      const findings = result?.outcome === 'analyzed' ? inspectAgentFileReport(result.report) : null;
      const valid = findings && result.report.source.id === row.sourceId && result.report.source.sha256 === row.sha256 && result.report.source.byteLength === row.byteLength;
      publish({ records: state.records.map(r => r.key === key ? { ...r, checkedAtMs: valid ? observedTime() : null, state: valid ? 'checked' : 'failed', findings: valid ? findings : [], report: valid ? retainReport(result.report) : null, error: valid ? null : publicError(result?.error) } : r.origin === 'saved' ? { ...r, report: null } : r), pending: null, message: valid ? 'Selected saved file inspected.' : 'Saved file could not be inspected. Reload the inventory before retrying.' });
    },
    async checkSelected() {
      if (state.pending || disposed || !available) return;
      const rows = state.records.filter(r => state.markedKeys.includes(r.key) && r.origin === 'saved').map(({ key, sourceId, sha256, byteLength }) => ({ key, sourceId, sha256, byteLength }));
      if (!rows.length) { publish({ message: 'Selected local files were already checked. Mark saved files to inspect them.' }); return; }
      const token = start('check'); if (token === null) return;
      publish({ checkCompleted: 0, checkTotal: rows.length, message: `Checking ${rows.length} selected saved files…` });
      let completed = 0;
      for (const row of rows) {
        if (!current(token) || state.checkCancelled) break;
        const result = await call('read_composition_source', { sourceId: row.sourceId });
        if (!current(token)) return;
        if (state.checkCancelled) break;
        const findings = result?.outcome === 'analyzed' ? inspectAgentFileReport(result.report) : null;
        const valid = findings && result.report.source.id === row.sourceId && result.report.source.sha256 === row.sha256 && result.report.source.byteLength === row.byteLength;
        completed += 1;
        publish({ records: state.records.map(r => r.key === row.key ? { ...r, checkedAtMs: valid ? observedTime() : null, state: valid ? 'checked' : 'failed', findings: valid ? findings : [], report: valid && state.selectedKey === row.key ? retainReport(result.report) : null, error: valid ? null : publicError(result?.error) } : r), checkCompleted: completed, message: `Checked ${completed} of ${rows.length} selected saved files.` });
      }
      if (current(token)) publish({ pending: null, message: `${state.checkCancelled ? 'Stopped' : 'Finished'} after ${completed} of ${rows.length} saved files. No files were changed.` });
    },
  };
  return controller;
}
