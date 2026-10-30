// Renderer fence only. Native O1 inspection and custody remain authoritative.
export const OPERATIONS_PAGE_SIZE = 50;
export const RECORD_KINDS = Object.freeze(['actor','resource','tool','workflow','engine','policy_revision','gateway_revision','assignment','delegation','request','evaluation','approval_plan','approval_decision','authorization','attempt','suspension','metric','evidence','coverage']);
const CODES = Object.freeze(['duplicate_observation','event_identity_conflict','sequence_conflict','record_identity_conflict','sequence_gap','predecessor_missing','predecessor_conflict','prefix_fence_mismatch','prefix_watermark_mismatch','prefix_identity_conflict','scope_mismatch','correlation_mismatch','duplicate_node','node_record_missing','node_record_kind_mismatch','duplicate_edge','edge_endpoint_missing','edge_endpoint_ambiguous']);
const STAGES = Object.freeze(['structural','relationships','delegations','approvals','lifecycle']);
const NODE_KINDS = new Set(['actor','workflow','tool','resource','gateway','engine','approval_stage']);
const EDGE_KINDS = new Set(['declared_dependency','request_flow','delegation_observation','mediation_observation','evidence_lineage']);
const DECISION_GROUPS = Object.freeze([
  ['evaluation', 'evaluation', ['allow','deny','unknown'], { allow: 'allow', deny: 'deny', unknown: 'unknown' }],
  ['approvalDecision', 'approval_decision', ['approve','deny','abstain'], { approve: 'approve', deny: 'deny', abstain: 'abstain' }],
  ['authorization', 'authorization', ['reportedIssued','reportedRefused','reportedExpired','reportedRevoked','unknown'], { reported_issued: 'reportedIssued', reported_refused: 'reportedRefused', reported_expired: 'reportedExpired', reported_revoked: 'reportedRevoked', unknown: 'unknown' }],
  ['attempt', 'attempt', ['reportedAccepted','reportedActive','reportedCompleted','reportedFailed','reportedCancelled','outcomeUnknown','reportedReconciledCompleted','reportedReconciledFailed'], { reported_accepted: 'reportedAccepted', reported_active: 'reportedActive', reported_completed: 'reportedCompleted', reported_failed: 'reportedFailed', reported_cancelled: 'reportedCancelled', outcome_unknown: 'outcomeUnknown', reported_reconciled_completed: 'reportedReconciledCompleted', reported_reconciled_failed: 'reportedReconciledFailed' }],
]);
const ERROR_MESSAGES = new Map([
  ['operations_busy', ['Finish or cancel the current local file selection first.']],
  ['operations_session_unavailable', ['Reopen Rangoon to inspect a local operations window.']],
  ['operations_picker_unavailable', ['The local operations picker is unavailable.']],
  ['operations_picker_failed', ['Choose a regular file available on this computer.', 'Local file selection did not complete. Try again.']],
  ['operations_window_read_failed', ['The selected operations window could not be read. Choose a regular local file again.']],
  ['operations_window_too_large', ['Choose an operations window no larger than 2 MiB.']],
  ['operations_window_invalid', ['This file is not a valid Rangoon operations window.']],
  ['operations_projection_failed', ['This operations window could not be inspected. No partial view was loaded.']],
  ['operations_relationships_failed', ['This operations window could not be inspected. No partial view was loaded.']],
  ['operations_delegations_failed', ['This operations window could not be inspected. No partial view was loaded.']],
  ['operations_approvals_failed', ['This operations window could not be inspected. No partial view was loaded.']],
  ['operations_lifecycle_failed', ['This operations window could not be inspected. No partial view was loaded.']],
  ['operations_view_failed', ['This operations window could not be inspected. No partial view was loaded.']],
  ['operations_response_limit', ['This operations window could not be inspected. No partial view was loaded.']],
  ['operations_selection_cleared', ['This operations inspection was cleared. Choose a local window again.']],
]);
const own = value => value && typeof value === 'object' && !Array.isArray(value) && [Object.prototype, null].includes(Object.getPrototypeOf(value));
const exact = (value, keys) => {
  if (!own(value)) return null;
  const descriptors = Object.getOwnPropertyDescriptors(value);
  if (Reflect.ownKeys(descriptors).length !== keys.length || keys.some(key => !Object.hasOwn(descriptors, key) || !Object.hasOwn(descriptors[key], 'value'))) return null;
  return Object.fromEntries(keys.map(key => [key, descriptors[key].value]));
};
const integer = value => Number.isSafeInteger(value) && value >= 0;
const member = (value, choices) => choices.includes(value);
const orderedUnique = (values, upper) => Array.isArray(values) && values.every(integer) && values.every(value => value < upper) && values.every((value, index) => index === 0 || values[index - 1] < value);
const enums = (values, choices) => Array.isArray(values) && values.every(value => member(value, choices)) && values.every((value, index) => index === 0 || choices.indexOf(values[index - 1]) < choices.indexOf(value));
const reported = (kind, value) => {
  const values = { evaluation: ['allow','deny','unknown'], approval_decision: ['approve','deny','abstain'], authorization: ['reported_issued','reported_refused','reported_expired','reported_revoked','unknown'], attempt: ['reported_accepted','reported_active','reported_completed','reported_failed','reported_cancelled','outcome_unknown','reported_reconciled_completed','reported_reconciled_failed'] }[kind];
  return values ? member(value, values) : value === null;
};

function decodeView(value) {
  const view = exact(value, ['schemaVersion','authority','authenticity','eligibility','execution','outcome','coverage','structuralState','sourceCounts','diagnosticCounts','decisionCounts','events','topology']);
  if (!view || view.schemaVersion !== 'rangoon.ops.operator-view.v1' || view.authority !== 'none' || view.authenticity !== 'unverified' || view.eligibility !== 'unknown' || view.execution !== 'unavailable' || view.outcome !== 'unknown' || view.coverage !== 'unknown' || !member(view.structuralState, ['conflict','incomplete','structural_unverified'])) return null;
  const source = exact(view.sourceCounts, ['eventCount','producerCount','topologyNodeCount','topologyEdgeCount','recordCounts']);
  const diagnostics = exact(view.diagnosticCounts, STAGES);
  const decisions = exact(view.decisionCounts, ['evaluation','approvalDecision','authorization','attempt']);
  const topology = exact(view.topology, ['nodes','edges']);
  if (!source || !diagnostics || !decisions || !topology || !integer(source.eventCount) || source.eventCount > 1024 || !integer(source.producerCount) || source.producerCount > 32 || !integer(source.topologyNodeCount) || source.topologyNodeCount > 128 || !integer(source.topologyEdgeCount) || source.topologyEdgeCount > 512 || !Array.isArray(source.recordCounts) || source.recordCounts.length !== RECORD_KINDS.length || !STAGES.every(stage => integer(diagnostics[stage])) || !Array.isArray(view.events) || view.events.length !== source.eventCount || !Array.isArray(topology.nodes) || topology.nodes.length !== source.topologyNodeCount || !Array.isArray(topology.edges) || topology.edges.length !== source.topologyEdgeCount) return null;
  if (!source.recordCounts.every((entry, index) => { const row = exact(entry, ['kind','count']); return row && row.kind === RECORD_KINDS[index] && integer(row.count); })) return null;
  if (!DECISION_GROUPS.every(([field, kind, buckets]) => { const group = exact(decisions[field], buckets); const denominator = source.recordCounts.find(row => row.kind === kind)?.count; return group && integer(denominator) && buckets.every(bucket => integer(group[bucket])) && buckets.reduce((total, bucket) => total + group[bucket], 0) === denominator; })) return null;
  const producers = new Map(); const fences = new Map(); let referenceTotal = 0;
  const observedRecords = new Map(RECORD_KINDS.map(kind => [kind, 0]));
  const observedDecisions = Object.fromEntries(DECISION_GROUPS.map(([field,, buckets]) => [field, Object.fromEntries(buckets.map(bucket => [bucket, 0]))]));
  // Native inventory also counts producers present only in snapshot prefixes.
  if (!view.events.every((entry, index) => {
    const event = exact(entry, ['inputOrdinal','producerOrdinal','fenceOrdinal','sequence','recordKind','reportedValue','structuralDiagnostics','issueStages','references']);
    if (!event || event.inputOrdinal !== index || !integer(event.producerOrdinal) || !integer(event.fenceOrdinal) || !integer(event.sequence) || !member(event.recordKind, RECORD_KINDS) || !reported(event.recordKind, event.reportedValue) || !enums(event.structuralDiagnostics, CODES) || !enums(event.issueStages, STAGES) || !Array.isArray(event.references)) return false;
    if (!producers.has(event.producerOrdinal)) producers.set(event.producerOrdinal, producers.size);
    if (producers.get(event.producerOrdinal) !== event.producerOrdinal) return false;
    const fenceKey = `${event.producerOrdinal}:${event.fenceOrdinal}`;
    if (!fences.has(fenceKey)) fences.set(fenceKey, [...fences.keys()].filter(key => key.startsWith(`${event.producerOrdinal}:`)).length);
    if (fences.get(fenceKey) !== event.fenceOrdinal) return false;
    observedRecords.set(event.recordKind, observedRecords.get(event.recordKind) + 1);
    const decision = DECISION_GROUPS.find(([, kind]) => kind === event.recordKind);
    if (decision) observedDecisions[decision[0]][decision[3][event.reportedValue]] += 1;
    referenceTotal += event.references.length;
    return event.references.every((entry, refIndex) => { const ref = exact(entry, ['referenceIndex','status','matchingEventOrdinals']); return ref && ref.referenceIndex === refIndex && member(ref.status, ['missing','ambiguous','wrong_kind','unique_unverified']) && orderedUnique(ref.matchingEventOrdinals, source.eventCount); });
  }) || producers.size > source.producerCount || referenceTotal > 8192) return null;
  if (!source.recordCounts.every(row => row.count === observedRecords.get(row.kind))) return null;
  if (!DECISION_GROUPS.every(([field,, buckets]) => buckets.every(bucket => decisions[field][bucket] === observedDecisions[field][bucket]))) return null;
  if (!topology.nodes.every((entry, index) => { const node = exact(entry, ['inputNodeIndex','kind','matchingEventOrdinals','hasIssue']); return node && node.inputNodeIndex === index && NODE_KINDS.has(node.kind) && orderedUnique(node.matchingEventOrdinals, source.eventCount) && typeof node.hasIssue === 'boolean'; })) return null;
  if (!topology.edges.every((entry, index) => { const edge = exact(entry, ['inputEdgeIndex','kind','fromNodeIndices','toNodeIndices','hasIssue']); return edge && edge.inputEdgeIndex === index && EDGE_KINDS.has(edge.kind) && orderedUnique(edge.fromNodeIndices, source.topologyNodeCount) && orderedUnique(edge.toNodeIndices, source.topologyNodeCount) && typeof edge.hasIssue === 'boolean'; })) return null;
  return view;
}

function decodeResult(value) {
  if (!own(value) || !member(value.outcome, ['inspected','cancelled','rejected','failed'])) return null;
  if (value.outcome === 'inspected') { const result = exact(value, ['outcome','schemaVersion','view']); const view = result && result.schemaVersion === 'rangoon.ops.local-inspection.v1' ? decodeView(result.view) : null; return view ? { outcome: 'inspected', view } : null; }
  if (value.outcome === 'cancelled') return exact(value, ['outcome','schemaVersion'])?.schemaVersion === 'rangoon.ops.local-inspection.v1' ? { outcome: 'cancelled' } : null;
  const result = exact(value, ['outcome','schemaVersion','error']); const error = exact(result?.error, ['code','message']);
  return result?.schemaVersion === 'rangoon.ops.local-inspection.v1' && error && ERROR_MESSAGES.get(error.code)?.includes(error.message) ? { outcome: result.outcome, code: error.code } : null;
}

export function createOperationsController({ invoke, onChange = () => {} } = {}) {
  let identity = Symbol('operations'); let generation = 0;
  let state = { status: typeof invoke === 'function' ? 'empty' : 'unavailable', view: null, selectedOrdinal: null, mode: 'timeline', kind: 'all', diagnostic: 'all', ordinal: '', page: 0, notice: null, errorCode: null, generation };
  const publish = () => onChange({ ...state });
  const set = changes => { state = { ...state, ...changes }; publish(); };
  const filtered = () => state.view?.events.filter(event => (state.kind === 'all' || event.recordKind === state.kind) && (state.diagnostic === 'all' || event.structuralDiagnostics.includes(state.diagnostic)) && (!state.ordinal || String(event.inputOrdinal) === state.ordinal)) ?? [];
  return {
    getState: () => {
      const filteredEvents = filtered();
      const pageCount = Math.max(1, Math.ceil(filteredEvents.length / OPERATIONS_PAGE_SIZE));
      return { ...state, filteredEvents, pageCount, pageEvents: filteredEvents.slice(state.page * OPERATIONS_PAGE_SIZE, (state.page + 1) * OPERATIONS_PAGE_SIZE) };
    },
    purgeForWorkspaceLock() { identity = Symbol('operations'); generation += 1; set({ status: typeof invoke === 'function' ? 'empty' : 'unavailable', view: null, selectedOrdinal: null, kind: 'all', diagnostic: 'all', ordinal: '', page: 0, notice: null, errorCode: null, generation }); },
    clear() { identity = Symbol('operations'); generation += 1; set({ status: typeof invoke === 'function' ? 'cleared' : 'unavailable', view: null, selectedOrdinal: null, kind: 'all', diagnostic: 'all', ordinal: '', page: 0, notice: 'Inspection cleared. No local window selected.', errorCode: null, generation }); },
    setMode(mode) { if (member(mode, ['timeline','topology']) && mode !== state.mode) set({ mode }); },
    setFilters(filters = {}) { const kind = member(filters.kind, ['all', ...RECORD_KINDS]) ? filters.kind : state.kind; const diagnostic = member(filters.diagnostic, ['all', ...CODES]) ? filters.diagnostic : state.diagnostic; const ordinal = typeof filters.ordinal === 'string' && (/^$|^(0|[1-9][0-9]{0,3})$/).test(filters.ordinal) ? filters.ordinal : state.ordinal; if (kind !== state.kind || diagnostic !== state.diagnostic || ordinal !== state.ordinal) set({ kind, diagnostic, ordinal, page: 0 }); },
    resetFilters() { if (state.kind !== 'all' || state.diagnostic !== 'all' || state.ordinal !== '' || state.page !== 0) set({ kind: 'all', diagnostic: 'all', ordinal: '', page: 0 }); },
    setPage(page) { if (Number.isSafeInteger(page) && page >= 0 && page < Math.max(1, Math.ceil(filtered().length / OPERATIONS_PAGE_SIZE)) && page !== state.page) set({ page }); },
    revealSelected() {
      const index = state.view?.events.findIndex(event => event.inputOrdinal === state.selectedOrdinal) ?? -1;
      if (index >= 0) {
        const page = Math.floor(index / OPERATIONS_PAGE_SIZE);
        if (state.kind !== 'all' || state.diagnostic !== 'all' || state.ordinal !== '' || page !== state.page) set({ kind: 'all', diagnostic: 'all', ordinal: '', page });
      }
    },
    select(ordinal) { if (ordinal !== state.selectedOrdinal && state.view?.events.some(event => event.inputOrdinal === ordinal)) set({ selectedOrdinal: ordinal }); },
    async choose() {
      if (typeof invoke !== 'function' || state.status === 'choosing') return false;
      const request = ++generation; const captured = identity; const prior = state.view;
      set({ status: 'choosing', errorCode: null, notice: prior ? 'Previous inspected window — opening replacement' : null, generation });
      let decoded;
      try { decoded = decodeResult(await invoke('select_and_inspect_operations')); } catch { decoded = null; }
      if (captured !== identity || request !== generation) {
        if (state.status === 'cleared' || state.status === 'empty') set({ status: 'stale', view: null, selectedOrdinal: null, notice: 'Stale inspection result discarded. No local window selected.', errorCode: null });
        return false;
      }
      if (decoded?.outcome === 'inspected') { set({ status: 'inspected', view: decoded.view, selectedOrdinal: decoded.view.events[0]?.inputOrdinal ?? null, kind: 'all', diagnostic: 'all', ordinal: '', page: 0, notice: 'Selected local window — unverified', errorCode: null }); return true; }
      if (decoded?.outcome === 'cancelled') { set({ status: 'cancelled', view: prior, selectedOrdinal: prior ? state.selectedOrdinal : null, notice: prior ? 'Previous inspected window — selection cancelled' : null, errorCode: null }); return false; }
      const failure = decoded?.outcome === 'rejected' ? 'rejected' : 'failed';
      set({ status: failure, view: prior, selectedOrdinal: prior ? state.selectedOrdinal : null, notice: prior ? 'Previous inspected window — replacement failed' : null, errorCode: decoded?.code ?? 'operations_response_invalid' });
      return false;
    },
  };
}
