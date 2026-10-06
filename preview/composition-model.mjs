import {
  utf8Bytes, validId, validRange, validTitle, validateCapabilityDetail,
  validateTarget, validateWorkspace, validatePreviewEnvelope, validateReceipt,
} from './composition-contract.mjs';

const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
const clone = value => value === null || value === undefined ? value : structuredClone(value);
const object = value => Boolean(value) && typeof value === 'object' && !Array.isArray(value);
const only = (value, names) => object(value) && Object.keys(value).length === names.size && [...names].every(name => Object.hasOwn(value, name));
const whole = value => Number.isSafeInteger(value) && value >= 0;
const rangeKey = range => `${range.inputIndex}:${range.startByte}:${range.endByte}`;
const sameRange = (left, right) => left.inputIndex === right.inputIndex && left.startByte === right.startByte && left.endByte === right.endByte;
const overlap = (left, right) => left.inputIndex === right.inputIndex && left.startByte < right.endByte && right.startByte < left.endByte;
const failure = result => ({ code: typeof result?.error?.code === 'string' ? result.error.code : null, message: typeof result?.error?.message === 'string' ? result.error.message : 'The composition action did not return a confirmed result.' });
const failed = () => ({ outcome: 'failed', error: { message: 'The composition action did not return a confirmed result.' } });
const newTarget = () => ({ kind: 'new' });
const HISTORY_MAX_ENTRIES = 32;
const HISTORY_MAX_BYTES = 4 * 1024 * 1024;

export const EMPTY_COMPOSITION_STATE = Object.freeze({
  operation: null, bridgeAvailable: false, status: 'idle', pending: null, error: null,
  message: 'Open Rangoon desktop to compose saved local content.', dirty: false,
  sources: [], capabilities: [], inputs: [], draft: null, targets: [], targetDetails: [],
  preview: null, previewStale: false, acknowledged: false, results: [], requestId: 0,
  canUndo: false, canRedo: false, draftVersion: 0,
});

function pointBoundary(content, byte) {
  if (!whole(byte)) return false;
  const data = encoder.encode(content);
  if (byte > data.length) return false;
  try { decoder.decode(data.slice(0, byte)); return true; } catch { return false; }
}

function inputRange(inputs, value) {
  if (!validRange(value) || value.inputIndex >= inputs.length) return false;
  const content = inputs[value.inputIndex]?.content;
  return typeof content === 'string' && value.endByte <= utf8Bytes(content) && pointBoundary(content, value.startByte) && pointBoundary(content, value.endByte);
}

function reportInput(report) {
  const source = report?.source;
  if (!only(report, new Set(['schemaVersion', 'analyzerVersion', 'source', 'fragments', 'diagnostics', 'authority'])) || report.schemaVersion !== 'rangoon.source-analysis.v0' || report.authority !== 'none' || !object(source) || !validId(source.id, 'source:') || typeof source.displayName !== 'string' || typeof source.content !== 'string' || !validId(`x:${source.sha256}`, 'x:') || !whole(source.byteLength) || source.byteLength !== utf8Bytes(source.content) || !Array.isArray(report.fragments) || !Array.isArray(report.diagnostics)) return null;
  const parts = report.fragments.map(fragment => {
    const span = fragment?.span;
    if (!object(fragment) || !validId(fragment.id, 'fragment:') || !object(span) || !whole(span.startByte) || !whole(span.endByte) || span.endByte <= span.startByte || span.endByte > source.byteLength || !pointBoundary(source.content, span.startByte) || !pointBoundary(source.content, span.endByte) || typeof fragment.text !== 'string' || byteRangeText(source.content, { inputIndex: 0, startByte: span.startByte, endByte: span.endByte }) !== fragment.text) return null;
    return { title: typeof fragment.heading?.title === 'string' && validTitle(fragment.heading.title) ? fragment.heading.title : 'Preamble', startByte: span.startByte, endByte: span.endByte };
  });
  if (parts.some(item => item === null)) return null;
  parts.sort((left, right) => left.startByte - right.startByte);
  if (source.byteLength && (!parts.length || parts[0].startByte !== 0 || parts.at(-1).endByte !== source.byteLength || parts.some((part, index) => index && parts[index - 1].endByte !== part.startByte))) return null;
  return { reference: { kind: 'source', sourceId: source.id, sha256: source.sha256 }, title: source.displayName, content: source.content, sections: parts, history: [], latestRevisionId: null };
}

function revisionInput(detail) {
  const valid = validateCapabilityDetail(detail);
  if (!valid) return null;
  return {
    reference: { kind: 'revision', capabilityId: valid.id, revisionId: valid.revision.id, sha256: valid.revision.sha256 },
    title: valid.revision.title, content: valid.revision.content,
    sections: [{ title: valid.revision.title, startByte: 0, endByte: utf8Bytes(valid.revision.content) }],
    history: clone(valid.history), latestRevisionId: valid.latestRevisionId,
  };
}

function copyPiece(range) { return { kind: 'copy', range: clone(range) }; }
function authored(content, reason) { return { kind: 'authored', content, reason }; }
function draftFor(operation, inputs) {
  if (operation === 'merge') return { schemaVersion: 'rangoon.composition-draft.v0', operation, inputs: inputs.map(item => clone(item.reference)), outputs: [{ title: 'Merged skill', pieces: inputs.flatMap((item, index) => [ ...(index ? [authored('\n\n', 'Separate selected revisions.')] : []), copyPiece({ inputIndex: index, startByte: 0, endByte: utf8Bytes(item.content) }) ]) }], exclusions: [], duplications: [], conflicts: [] };
  const input = inputs[0];
  const sections = operation === 'decompose' ? input.sections : splitSections(input.content, 2, 0);
  const limited = sections.length <= 16 ? sections : [...sections.slice(0, 15), { title: 'Remaining content', startByte: sections[15].startByte, endByte: sections.at(-1).endByte }];
  return { schemaVersion: 'rangoon.composition-draft.v0', operation, inputs: inputs.map(item => clone(item.reference)), outputs: limited.map((section, index) => ({ title: uniqueTitle(section.title || `Output ${index + 1}`, index), pieces: section.endByte > section.startByte ? [copyPiece({ inputIndex: 0, startByte: section.startByte, endByte: section.endByte })] : [] })), exclusions: [], duplications: [], conflicts: [] };
}

function uniqueTitle(value, index) {
  const candidate = String(value ?? '').trim().replace(/[\r\n\t]/g, ' ').slice(0, 160);
  return validTitle(candidate) ? candidate : `Output ${index + 1}`;
}

function splitSections(content, count, inputIndex) {
  const length = utf8Bytes(content);
  if (length === 0) return [{ title: 'Part 1', startByte: 0, endByte: 0 }, { title: 'Part 2', startByte: 0, endByte: 0 }];
  const data = encoder.encode(content);
  const candidates = []; const boundaries = [];
  for (let index = 0; index <= data.length; index += 1) if (pointBoundary(content, index)) boundaries.push(index);
  for (let index = 0; index <= data.length; index += 1) if ((index === 0 || index === data.length || data[index - 1] === 10) && pointBoundary(content, index)) candidates.push(index);
  const cuts = [0];
  for (let part = 1; part < count; part += 1) {
    const desired = Math.round((data.length * part) / count);
    const interior = candidates.filter(point => point > cuts.at(-1) && point < data.length);
    const safe = boundaries.filter(point => point > cuts.at(-1) && point < data.length);
    const cut = interior.length ? interior.reduce((best, point) => Math.abs(point - desired) < Math.abs(best - desired) ? point : best, interior[0]) : (safe.length ? safe.reduce((best, point) => Math.abs(point - desired) < Math.abs(best - desired) ? point : best, safe[0]) : data.length);
    cuts.push(Math.max(cuts.at(-1), cut));
  }
  cuts.push(data.length);
  return cuts.slice(0, -1).map((startByte, index) => ({ title: `Part ${index + 1}`, startByte, endByte: cuts[index + 1], inputIndex }));
}

function visibleMutation(state, message = 'Draft changed. Prepare a new native preview before saving.') {
  return { ...state, dirty: true, previewStale: Boolean(state.preview), acknowledged: false, results: [], message, error: null };
}

function adjustmentRanges(piece) { return piece?.kind === 'copy' || piece?.kind === 'replace' ? [piece.range] : []; }

function adjustPieces(pieces, incoming, allowReplace) {
  const next = [];
  for (const piece of pieces) {
    const ranges = adjustmentRanges(piece);
    if (!ranges.length || !overlap(ranges[0], incoming)) { next.push(piece); continue; }
    if (piece.kind === 'replace' && !sameRange(piece.range, incoming) && !allowReplace) return null;
    if (piece.kind === 'replace') continue;
    const range = piece.range;
    if (range.startByte < incoming.startByte) next.push(copyPiece({ ...range, endByte: incoming.startByte }));
    if (incoming.endByte < range.endByte) next.push(copyPiece({ ...range, startByte: incoming.endByte }));
  }
  return next;
}

function adjustAnnotations(items, incoming) {
  return items.flatMap(item => {
    if (!overlap(item.range, incoming)) return [item];
    const retained = [];
    if (item.range.startByte < incoming.startByte) retained.push({ range: { ...item.range, endByte: incoming.startByte }, reason: item.reason });
    if (incoming.endByte < item.range.endByte) retained.push({ range: { ...item.range, startByte: incoming.endByte }, reason: item.reason });
    return retained;
  });
}

export function byteRangeText(content, range) {
  if (typeof content !== 'string' || !validRange(range) || !pointBoundary(content, range.startByte) || !pointBoundary(content, range.endByte)) return null;
  try { return decoder.decode(encoder.encode(content).slice(range.startByte, range.endByte)); } catch { return null; }
}

export function textareaSelectionRange(content, start, end, inputIndex) {
  if (typeof content !== 'string' || !whole(start) || !whole(end) || start > end || !whole(inputIndex)) return null;
  let normalized = '';
  const offsets = [0];
  for (let original = 0; original < content.length;) {
    const code = content.charCodeAt(original);
    if (code >= 0xd800 && code <= 0xdbff && (original + 1 >= content.length || content.charCodeAt(original + 1) < 0xdc00 || content.charCodeAt(original + 1) > 0xdfff)) return null;
    if (code >= 0xdc00 && code <= 0xdfff) return null;
    const width = code >= 0xd800 && code <= 0xdbff ? 2 : 1;
    const cr = code === 13;
    const next = cr && content.charCodeAt(original + 1) === 10 ? original + 2 : original + width;
    const shown = cr ? '\n' : content.slice(original, original + width);
    normalized += shown;
    for (let position = 1; position <= shown.length; position += 1) offsets.push(next);
    original = next;
  }
  if (start > normalized.length || end > normalized.length || (start && /[\uDC00-\uDFFF]/u.test(normalized[start])) || (end && /[\uDC00-\uDFFF]/u.test(normalized[end]))) return null;
  const startOffset = offsets[start]; const endOffset = offsets[end];
  if (startOffset === undefined || endOffset === undefined) return null;
  const range = { inputIndex, startByte: utf8Bytes(content.slice(0, startOffset)), endByte: utf8Bytes(content.slice(0, endOffset)) };
  return validRange(range) || range.startByte === range.endByte ? range : null;
}

export function createCompositionController({ operation, invoke, onChange = () => {}, onCommitted = () => {} } = {}) {
  if (!['decompose', 'merge', 'split'].includes(operation)) throw new TypeError('operation must be decompose, merge, or split');
  const bridgeAvailable = typeof invoke === 'function';
  let state = { ...EMPTY_COMPOSITION_STATE, operation, bridgeAvailable, status: bridgeAvailable ? 'idle' : 'unavailable', message: bridgeAvailable ? 'Load saved local content to begin composition.' : EMPTY_COMPOSITION_STATE.message };
  let epoch = 0;
  let undoHistory = [];
  let redoHistory = [];
  const publish = () => onChange(clone(state));
  const set = changes => {
    const next = { ...state, ...changes };
    state = {
      ...next,
      canUndo: bridgeAvailable && !next.pending && undoHistory.length > 0,
      canRedo: bridgeAvailable && !next.pending && redoHistory.length > 0,
    };
    publish();
  };
  const mutationAllowed = () => bridgeAvailable && !state.pending && state.pending !== 'save';
  const call = async (command, args) => { try { return await invoke(command, args); } catch { return failed(); } };
  const fail = result => set({ status: state.sources.length || state.capabilities.length ? 'ready' : 'failed', pending: null, error: failure(result), message: failure(result).message });
  const freeze = value => {
    if (value && typeof value === 'object' && !Object.isFrozen(value)) {
      Object.freeze(value);
      for (const item of Object.values(value)) freeze(item);
    }
    return value;
  };
  const snapshotFor = value => ({ draft: clone(value.draft), targets: clone(value.targets), targetDetails: clone(value.targetDetails) });
  const draftSnapshot = () => freeze(snapshotFor(state));
  const snapshotBytes = snapshot => {
    try { return encoder.encode(JSON.stringify(snapshot)).length; } catch { return HISTORY_MAX_BYTES + 1; }
  };
  const historyBytes = () => [...undoHistory, ...redoHistory].reduce((total, entry) => total + entry.bytes, 0);
  const trimHistory = () => {
    while (undoHistory.length + redoHistory.length > HISTORY_MAX_ENTRIES || historyBytes() > HISTORY_MAX_BYTES) {
      if (undoHistory.length) undoHistory.shift();
      else redoHistory.shift();
    }
  };
  const remember = (history, snapshot) => {
    history.push({ snapshot, bytes: snapshotBytes(snapshot) });
    trimHistory();
  };
  const clearHistory = () => { undoHistory = []; redoHistory = []; };
  const applyDraftEdit = (changes, message = 'Draft changed. Prepare a new native preview before saving.') => {
    const before = draftSnapshot();
    const after = snapshotFor({ ...state, ...changes });
    if (JSON.stringify(before) === JSON.stringify(after)) return false;
    redoHistory = [];
    remember(undoHistory, before);
    set({ ...visibleMutation({ ...state, ...changes }, message), draftVersion: state.draftVersion + 1 });
    return true;
  };
  const replaceDraft = (draft, message) => applyDraftEdit({ draft }, message);
  const resetPreview = message => ({ preview: state.preview, previewStale: Boolean(state.preview), acknowledged: false, results: state.results, message });

  const controller = {
    getState: () => clone(state),
    async load() {
      if (!bridgeAvailable || state.pending) return false;
      const request = ++epoch;
      set({ status: state.sources.length || state.capabilities.length ? 'ready' : 'loading', pending: 'load', error: null, message: 'Loading saved local content…' });
      const result = await call('get_workspace_data');
      if (request !== epoch) return false;
      const workspace = result?.outcome === 'loaded' ? validateWorkspace(result.workspace) : null;
      if (!workspace) { fail(result); return false; }
      set({ status: 'ready', pending: null, error: null, sources: clone(workspace.sources), capabilities: clone(workspace.capabilities), message: workspace.sources.length || workspace.capabilities.length ? 'Saved local content is ready.' : 'No saved local content is available yet.' });
      return true;
    },
    async addSource(sourceId) {
      if (!mutationAllowed() || operation !== 'decompose' || state.draft || !validId(sourceId, 'source:') || !state.sources.some(source => source.sourceId === sourceId)) return false;
      const request = ++epoch; set({ pending: 'source', error: null, message: 'Opening saved source…' });
      const result = await call('read_composition_source', { sourceId });
      if (request !== epoch) return false;
      const input = result?.outcome === 'analyzed' ? reportInput(result.report) : null;
      if (!input || input.reference.sourceId !== sourceId) { fail(result); return false; }
      set({ pending: null, inputs: [input], message: 'Saved source selected. Start a draft to edit its proposed partition.' });
      return true;
    },
    async addRevision(capabilityId, revisionId = null) {
      if (!mutationAllowed() || operation === 'decompose' || state.draft || !validId(capabilityId, 'capability:') || (revisionId !== null && !validId(revisionId, 'revision:'))) return false;
      const max = operation === 'merge' ? 16 : 1;
      if (state.inputs.length >= max) return false;
      const request = ++epoch; set({ pending: 'revision', error: null, message: 'Opening saved skill revision…' });
      const result = await call('open_capability', { capabilityId, revisionId });
      if (request !== epoch) return false;
      const input = result?.outcome === 'opened' ? revisionInput(result.capability) : null;
      if (!input || input.reference.capabilityId !== capabilityId || (revisionId ? input.reference.revisionId !== revisionId : input.reference.revisionId !== input.latestRevisionId) || state.inputs.some(item => item.reference.kind === 'revision' && item.reference.capabilityId === input.reference.capabilityId && item.reference.revisionId === input.reference.revisionId)) { fail(result); return false; }
      set({ pending: null, inputs: [...state.inputs, input], message: 'Saved skill revision selected.' });
      return true;
    },
    async replaceRevision(inputIndex, revisionId) {
      if (!mutationAllowed() || state.draft || !whole(inputIndex) || !validId(revisionId, 'revision:') || state.inputs[inputIndex]?.reference.kind !== 'revision') return false;
      const prior = state.inputs[inputIndex];
      const request = ++epoch; set({ pending: 'revision', error: null, message: 'Opening saved skill revision…' });
      const result = await call('open_capability', { capabilityId: prior.reference.capabilityId, revisionId });
      if (request !== epoch) return false;
      const replacement = result?.outcome === 'opened' ? revisionInput(result.capability) : null;
      if (!replacement || replacement.reference.capabilityId !== prior.reference.capabilityId || replacement.reference.revisionId !== revisionId || state.inputs.some((item, index) => index !== inputIndex && item.reference.kind === 'revision' && item.reference.capabilityId === replacement.reference.capabilityId && item.reference.revisionId === replacement.reference.revisionId)) { fail(result); return false; }
      set({ pending: null, inputs: state.inputs.map((item, index) => index === inputIndex ? replacement : item), message: 'Saved skill revision replaced.' });
      return true;
    },
    removeInput(index) {
      if (!mutationAllowed() || state.draft || !whole(index) || index >= state.inputs.length) return false;
      const next = state.inputs.filter((_, item) => item !== index);
      if ((operation === 'merge' && next.length < 2 && state.inputs.length === 2) || (operation !== 'merge' && next.length < 1)) { set({ inputs: next, message: 'Input removed. Choose required inputs before starting a draft.' }); return true; }
      set({ inputs: next, message: 'Input removed.' }); return true;
    },
    moveInput(index, delta) {
      if (!mutationAllowed() || state.draft || !whole(index) || !Number.isInteger(delta) || !delta || index + delta < 0 || index + delta >= state.inputs.length) return false;
      const inputs = [...state.inputs]; [inputs[index], inputs[index + delta]] = [inputs[index + delta], inputs[index]]; set({ inputs, message: 'Input order changed.' }); return true;
    },
    startDraft() {
      if (!mutationAllowed() || state.draft || (operation === 'decompose' && state.inputs.length !== 1) || (operation === 'merge' && state.inputs.length < 2) || (operation === 'split' && state.inputs.length !== 1)) return false;
      const draft = draftFor(operation, state.inputs);
      clearHistory();
      set({ ...visibleMutation({ ...state, draft, targets: draft.outputs.map(newTarget), targetDetails: draft.outputs.map(() => null), preview: null, previewStale: false, acknowledged: false }, 'Draft started. Edit then prepare a native preview.'), dirty: true, draftVersion: state.draftVersion + 1 });
      return true;
    },
    resetDraft() {
      if (!mutationAllowed() || !state.draft) return false;
      clearHistory();
      set({ ...state, draft: null, targets: [], targetDetails: [], preview: null, previewStale: false, acknowledged: false, results: [], dirty: false, error: null, message: 'Draft discarded. Selected inputs remain available.', draftVersion: state.draftVersion + 1 }); return true;
    },
    setOutputTitle(index, value) {
      if (!mutationAllowed() || !state.draft || !whole(index) || !validTitle(value) || !state.draft.outputs[index]) return false;
      const draft = clone(state.draft); draft.outputs[index].title = value; return replaceDraft(draft);
    },
    addOutput(value) {
      if (!mutationAllowed() || !state.draft || state.draft.outputs.length >= 16 || (operation === 'merge') || !validTitle(value)) return false;
      const draft = clone(state.draft); draft.outputs.push({ title: value, pieces: [] }); return applyDraftEdit({ draft, targets: [...state.targets, newTarget()], targetDetails: [...state.targetDetails, null] });
    },
    removeOutput(index) {
      const min = operation === 'split' ? 2 : 1;
      if (!mutationAllowed() || !state.draft || !whole(index) || state.draft.outputs.length <= min || !state.draft.outputs[index]) return false;
      const draft = clone(state.draft); draft.outputs.splice(index, 1); const targets = state.targets.filter((_, item) => item !== index); const targetDetails = state.targetDetails.filter((_, item) => item !== index); return applyDraftEdit({ draft, targets, targetDetails });
    },
    moveOutput(index, delta) {
      if (!mutationAllowed() || !state.draft || !whole(index) || !Number.isInteger(delta) || !delta || index + delta < 0 || index + delta >= state.draft.outputs.length) return false;
      const draft = clone(state.draft); const targets = clone(state.targets); const targetDetails = clone(state.targetDetails); for (const values of [draft.outputs, targets, targetDetails]) [values[index], values[index + delta]] = [values[index + delta], values[index]]; return applyDraftEdit({ draft, targets, targetDetails });
    },
    async setTarget(outputIndex, capabilityId = null) {
      if (!mutationAllowed() || !state.draft || !whole(outputIndex) || outputIndex >= state.draft.outputs.length || (capabilityId !== null && !validId(capabilityId, 'capability:'))) return false;
      if (capabilityId === null) {
        if (state.targets[outputIndex]?.kind === 'new' && state.targetDetails[outputIndex] === null) return false;
        const targets = clone(state.targets); const details = clone(state.targetDetails); targets[outputIndex] = newTarget(); details[outputIndex] = null; return applyDraftEdit({ targets, targetDetails: details });
      }
      const request = ++epoch; set({ pending: 'target', error: null, message: 'Opening current target head…' });
      const result = await call('open_capability', { capabilityId, revisionId: null });
      if (request !== epoch) return false;
      const detail = result?.outcome === 'opened' ? validateCapabilityDetail(result.capability) : null;
      if (!detail || detail.id !== capabilityId || detail.revision.id !== detail.latestRevisionId || state.targets.some((target, index) => index !== outputIndex && target.kind === 'append' && target.capabilityId === capabilityId)) { fail(result); return false; }
      const targets = clone(state.targets); const details = clone(state.targetDetails); targets[outputIndex] = { kind: 'append', capabilityId, expectedRevisionId: detail.latestRevisionId }; details[outputIndex] = detail;
      if (JSON.stringify(targets[outputIndex]) === JSON.stringify(state.targets[outputIndex]) && JSON.stringify(details[outputIndex]) === JSON.stringify(state.targetDetails[outputIndex])) { set({ pending: null, error: null, message: 'Target already uses the current head.' }); return true; }
      return applyDraftEdit({ pending: null, targets, targetDetails: details });
    },
    refreshTarget(outputIndex) {
      const target = state.targets[outputIndex]; return target?.kind === 'append' ? controller.setTarget(outputIndex, target.capabilityId) : Promise.resolve(false);
    },
    applyRange(range, { kind, outputIndex, content = '', reason = '' } = {}) {
      if (!mutationAllowed() || !state.draft || !inputRange(state.inputs, range) || !['assign', 'duplicate', 'replace', 'exclude'].includes(kind) || !whole(outputIndex) || outputIndex >= state.draft.outputs.length || ((kind === 'duplicate' || kind === 'replace' || kind === 'exclude') && (!reason.trim() || reason.includes('\0'))) || (kind === 'replace' && (typeof content !== 'string' || content.includes('\0')))) return false;
      const draft = clone(state.draft);
      if (kind === 'duplicate') {
        draft.outputs[outputIndex].pieces.push(copyPiece(range));
        if (!draft.duplications.some(item => sameRange(item.range, range))) draft.duplications.push({ range: clone(range), reason: reason.trim() });
      } else {
        for (let index = 0; index < draft.outputs.length; index += 1) {
          const adjusted = adjustPieces(draft.outputs[index].pieces, range, false);
          if (!adjusted) { set({ error: { code: 'replace_overlap', message: 'Replace an authored replacement only with its exact original range.' }, message: 'Replace range rejected.' }); return false; }
          draft.outputs[index].pieces = adjusted;
        }
        draft.exclusions = adjustAnnotations(draft.exclusions, range);
        draft.duplications = adjustAnnotations(draft.duplications, range);
        if (kind === 'assign') draft.outputs[outputIndex].pieces.push(copyPiece(range));
        if (kind === 'replace') draft.outputs[outputIndex].pieces.push({ kind: 'replace', range: clone(range), content, reason: reason.trim() });
        if (kind === 'exclude') draft.exclusions.push({ range: clone(range), reason: reason.trim() });
      }
      return replaceDraft(draft);
    },
    addAuthored(outputIndex, content, reason) {
      if (!mutationAllowed() || !state.draft || !whole(outputIndex) || !state.draft.outputs[outputIndex] || typeof content !== 'string' || typeof reason !== 'string' || !reason.trim() || content.includes('\0')) return false;
      const draft = clone(state.draft); draft.outputs[outputIndex].pieces.push(authored(content, reason.trim())); return replaceDraft(draft);
    },
    updatePiece(outputIndex, pieceIndex, content, reason) {
      if (!mutationAllowed() || !state.draft || !whole(outputIndex) || !whole(pieceIndex) || typeof content !== 'string' || typeof reason !== 'string' || !reason.trim() || content.includes('\0')) return false;
      const piece = state.draft.outputs[outputIndex]?.pieces[pieceIndex]; if (!piece || piece.kind === 'copy') return false;
      const draft = clone(state.draft); draft.outputs[outputIndex].pieces[pieceIndex] = { ...piece, content, reason: reason.trim() }; return replaceDraft(draft);
    },
    removePiece(outputIndex, pieceIndex) {
      if (!mutationAllowed() || !state.draft || !whole(outputIndex) || !whole(pieceIndex) || !state.draft.outputs[outputIndex]?.pieces[pieceIndex]) return false;
      const draft = clone(state.draft); draft.outputs[outputIndex].pieces.splice(pieceIndex, 1); return replaceDraft(draft);
    },
    movePiece(outputIndex, pieceIndex, delta) {
      const pieces = state.draft?.outputs[outputIndex]?.pieces;
      if (!mutationAllowed() || !pieces || !whole(pieceIndex) || !Number.isInteger(delta) || !delta || pieceIndex + delta < 0 || pieceIndex + delta >= pieces.length) return false;
      const draft = clone(state.draft); [draft.outputs[outputIndex].pieces[pieceIndex], draft.outputs[outputIndex].pieces[pieceIndex + delta]] = [draft.outputs[outputIndex].pieces[pieceIndex + delta], draft.outputs[outputIndex].pieces[pieceIndex]]; return replaceDraft(draft);
    },
    movePieceTo(fromOutputIndex, pieceIndex, toOutputIndex, beforeIndex = null, expectedVersion = state.draftVersion) {
      const source = state.draft?.outputs[fromOutputIndex]?.pieces;
      const target = state.draft?.outputs[toOutputIndex]?.pieces;
      if (!mutationAllowed() || !source || !target || !whole(fromOutputIndex) || !whole(pieceIndex) || !whole(toOutputIndex) || !whole(expectedVersion) || expectedVersion !== state.draftVersion || pieceIndex >= source.length || (beforeIndex !== null && (!whole(beforeIndex) || beforeIndex > target.length))) return false;
      let insertionIndex = beforeIndex === null ? target.length : beforeIndex;
      if (fromOutputIndex === toOutputIndex && beforeIndex !== null && beforeIndex > pieceIndex) insertionIndex -= 1;
      if (fromOutputIndex === toOutputIndex && (insertionIndex === pieceIndex || (beforeIndex === null && pieceIndex === source.length - 1))) return false;
      const draft = clone(state.draft);
      const [piece] = draft.outputs[fromOutputIndex].pieces.splice(pieceIndex, 1);
      if (beforeIndex === null) insertionIndex = draft.outputs[toOutputIndex].pieces.length;
      draft.outputs[toOutputIndex].pieces.splice(insertionIndex, 0, piece);
      return applyDraftEdit({ draft }, 'Piece moved. Prepare a new native preview before saving.');
    },
    removeAnnotation(kind, index) {
      if (!mutationAllowed() || !state.draft || !['exclusions', 'duplications'].includes(kind) || !whole(index) || !state.draft[kind][index]) return false;
      const draft = clone(state.draft); draft[kind].splice(index, 1); return replaceDraft(draft);
    },
    setAnnotationReason(kind, index, reason) {
      if (!mutationAllowed() || !state.draft || !['exclusions', 'duplications'].includes(kind) || !whole(index) || typeof reason !== 'string' || !reason.trim() || !state.draft[kind][index]) return false;
      const draft = clone(state.draft); draft[kind][index].reason = reason.trim(); return replaceDraft(draft);
    },
    addConflict({ title, context, ranges, resolution = null } = {}) {
      if (!mutationAllowed() || !state.draft || !validTitle(title) || typeof context !== 'string' || !context.trim() || !Array.isArray(ranges) || ranges.length < 2 || ranges.length > 16 || !ranges.every(range => inputRange(state.inputs, range)) || (resolution !== null && (typeof resolution !== 'string' || !resolution.trim()))) return false;
      const keys = new Set(ranges.map(rangeKey)); if (keys.size !== ranges.length) return false;
      const draft = clone(state.draft); const id = Math.max(0, ...draft.conflicts.map(conflict => conflict.id)) + 1; draft.conflicts.push({ id, title, ranges: clone(ranges), context: context.trim(), resolution: resolution === null ? null : resolution.trim() }); return replaceDraft(draft);
    },
    resolveConflict(index, resolution) {
      if (!mutationAllowed() || !state.draft || !whole(index) || !state.draft.conflicts[index] || typeof resolution !== 'string' || !resolution.trim()) return false;
      const draft = clone(state.draft); draft.conflicts[index].resolution = resolution.trim(); return replaceDraft(draft);
    },
    removeConflict(index) {
      if (!mutationAllowed() || !state.draft || !whole(index) || !state.draft.conflicts[index]) return false;
      const draft = clone(state.draft); draft.conflicts.splice(index, 1); return replaceDraft(draft);
    },
    undo() {
      if (!mutationAllowed() || !state.draft || !undoHistory.length) return false;
      const prior = undoHistory.pop();
      remember(redoHistory, draftSnapshot());
      const snapshot = clone(prior.snapshot);
      set({ ...visibleMutation({ ...state, ...snapshot }, 'Draft edit undone. Prepare a new native preview before saving.'), draftVersion: state.draftVersion + 1 });
      return true;
    },
    redo() {
      if (!mutationAllowed() || !state.draft || !redoHistory.length) return false;
      const next = redoHistory.pop();
      remember(undoHistory, draftSnapshot());
      const snapshot = clone(next.snapshot);
      set({ ...visibleMutation({ ...state, ...snapshot }, 'Draft edit redone. Prepare a new native preview before saving.'), draftVersion: state.draftVersion + 1 });
      return true;
    },
    async requestPreview() {
      if (!mutationAllowed() || !state.draft || state.targets.length !== state.draft.outputs.length || !state.targets.every(validateTarget)) return false;
      const request = ++epoch; const payload = { schemaVersion: 'rangoon.composition-application-request.v0', draft: state.draft, targets: state.targets };
      set({ pending: 'preview', error: null, previewStale: Boolean(state.preview), acknowledged: false, message: 'Preparing native composition preview…' });
      const result = await call('preview_composition', encoder.encode(JSON.stringify(payload)));
      if (request !== epoch) return false;
      let preview = null;
      try { preview = validatePreviewEnvelope(result, state.draft, state.targets, state.inputs); } catch { preview = null; }
      if (!preview) { set({ pending: null, error: failure(result), previewStale: Boolean(state.preview), acknowledged: false, message: failure(result).message }); return false; }
      set({ pending: null, preview, previewStale: false, acknowledged: false, error: null, message: preview.preview.saveable ? 'Native preview ready. Review exact output content before acknowledging.' : 'Native preview is blocked. Edit the draft and prepare another preview.' }); return true;
    },
    acknowledge(value) {
      if (!mutationAllowed() || typeof value !== 'boolean' || !state.preview || state.previewStale || !state.preview.preview.saveable) return false;
      set({ acknowledged: value, message: value ? 'Exact native preview acknowledged. Save remains local.' : 'Preview acknowledgment removed.' }); return true;
    },
    async save() {
      if (!bridgeAvailable || state.pending || !state.preview || state.previewStale || !state.preview.preview.saveable || !state.acknowledged) return false;
      const preview = state.preview; const request = ++epoch;
      const payload = { schemaVersion: 'rangoon.composition-confirmation.v0', previewId: preview.previewId, expectedStateId: preview.expectedStateId, acknowledged: true };
      set({ pending: 'save', error: null, message: 'Saving exact acknowledged composition locally…' });
      const result = await call('commit_composition', encoder.encode(JSON.stringify(payload)));
      let receipt = null;
      try { receipt = result?.outcome === 'committed' ? validateReceipt(result.receipt, preview.preview) : null; } catch { receipt = null; }
      if (!receipt) { set({ pending: null, error: failure(result), previewStale: true, acknowledged: false, message: 'Save outcome needs a new preview and readback. Draft retained. ' + failure(result).message }); return false; }
      clearHistory();
      set({ pending: null, results: receipt.capabilities, dirty: false, acknowledged: false, previewStale: false, error: null, message: 'Composition saved locally. Returned skills remain unreviewed.' });
      try { await onCommitted(clone(receipt)); } catch { set({ message: 'Composition saved locally. Related view refresh failed.' }); }
      return true;
    },
    invalidatePreview(message = 'Saved workspace changed. Prepare a new native preview before saving.') {
      if (!state.preview && state.pending !== 'preview') return false;
      if (state.pending === 'preview') {
        ++epoch;
        if (!state.preview) { set({ pending: null }); return true; }
        set({ pending: null, previewStale: true, acknowledged: false, message, error: null }); return true;
      }
      if (!state.preview) return false;
      set({ previewStale: true, acknowledged: false, message, error: null }); return true;
    },
  };
  return controller;
}
