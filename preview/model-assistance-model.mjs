import { validateCapabilitySummary, validateCapabilityDetail } from './skills-model.mjs';

const encoder = new TextEncoder();
const SESSION_SCHEMA = 'rangoon.local-session-result.v1';
const SOURCE = /^source:[0-9a-f]{64}$/;
const CAPABILITY = /^capability:[0-9a-f]{64}$/;
const REVISION = /^revision:[0-9a-f]{64}$/;
const RUN = /^run:[0-9a-f]{64}$/;
const PREPARED = /^prepared:[0-9a-f]{64}$/;
const DIGEST = /^[0-9a-f]{64}$/;
const TASKS = new Set(['classify_v1', 'decompose_v1', 'compare_v1']);
const OUTCOMES = new Set(['unconfigured', 'configured', 'cleared', 'prepared', 'checked', 'completed', 'cancel_requested', 'cancelled', 'failed']);
const CODES = new Set(['invalid_request', 'invalid_profile', 'unconfigured', 'busy', 'session_unavailable', 'stale_prepared', 'run_not_found', 'input_unavailable', 'input_empty', 'input_stale', 'pack_invalid', 'pack_over_budget', 'workspace_busy', 'confirmation_unavailable', 'profile_mismatch', 'request_over_budget', 'timed_out', 'connection_failed', 'redirect_rejected', 'remote_rejected', 'response_too_large', 'response_invalid', 'response_incomplete', 'proposal_invalid']);
const object = value => Boolean(value) && typeof value === 'object' && !Array.isArray(value);
const only = (value, keys) => object(value) && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
const clone = value => value == null ? value : structuredClone(value);
const decimal = value => typeof value === 'string' && /^(0|[1-9][0-9]*)$/.test(value);
const profileValid = value => object(value) && only(value, ['profileId', 'host', 'port', 'model', 'maxOutputTokens'])
  && /^[A-Za-z0-9_.-]{1,64}$/.test(value.profileId) && ['127.0.0.1', '::1'].includes(value.host)
  && Number.isSafeInteger(value.port) && value.port >= 1 && value.port <= 65535
  && typeof value.model === 'string' && value.model.length <= 128 && /^[A-Za-z0-9][A-Za-z0-9._-]*(?:\/[A-Za-z0-9][A-Za-z0-9._-]*)*:[A-Za-z0-9][A-Za-z0-9._-]*$/.test(value.model)
  && Number.isSafeInteger(value.maxOutputTokens) && value.maxOutputTokens >= 1 && value.maxOutputTokens <= 32768;
const profileRecord = value => object(value) && only(value, ['config', 'profileSha256']) && object(value.config)
  && only(value.config, ['schemaVersion', 'adapter', 'profileId', 'host', 'port', 'model', 'maxOutputTokens'])
  && value.config.schemaVersion === 'rangoon.local-profile.v1' && value.config.adapter === 'ollama-loopback.v1'
  && profileValid({ profileId: value.config.profileId, host: value.config.host, port: value.config.port, model: value.config.model, maxOutputTokens: value.config.maxOutputTokens })
  && DIGEST.test(value.profileSha256);
const safeProfile = value => profileRecord({ config: value, profileSha256: '0'.repeat(64) }) || profileRecord(value);
const profileForm = value => safeProfile(value) ? { profileId: value.profileId, host: value.host, port: value.port, model: value.model, maxOutputTokens: value.maxOutputTokens } : null;
const dependency = value => object(value) && only(value, ['input', 'observedHead', 'byteLength']) && object(value.input) && Number.isSafeInteger(value.byteLength) && value.byteLength >= 0
  && (value.observedHead === null || REVISION.test(value.observedHead))
  && ((only(value.input, ['kind', 'sourceId', 'sha256']) && value.input.kind === 'source' && SOURCE.test(value.input.sourceId) && DIGEST.test(value.input.sha256))
    || (only(value.input, ['kind', 'capabilityId', 'revisionId', 'sha256']) && value.input.kind === 'revision' && CAPABILITY.test(value.input.capabilityId) && REVISION.test(value.input.revisionId) && DIGEST.test(value.input.sha256)));
const session = value => object(value) && only(value, ['generation', 'profile', 'active', 'persistence', 'processingLocation', 'retention', 'authority'])
  && decimal(value.generation) && (value.profile === null || profileRecord(value.profile)) && (value.active === null || (only(value.active, ['runId', 'kind']) && RUN.test(value.active.runId) && ['prepare', 'check', 'send'].includes(value.active.kind)))
  && value.persistence === 'session_only' && value.processingLocation === 'unknown' && value.retention === 'unknown' && value.authority === 'none';
const prepared = value => object(value) && only(value, ['generation', 'preparedId', 'requestId', 'origin', 'model', 'bodyJson', 'bodyBytes', 'inputs', 'pack', 'processingLocation', 'retention', 'authority'])
  && decimal(value.generation) && PREPARED.test(value.preparedId) && DIGEST.test(value.requestId) && typeof value.origin === 'string' && typeof value.model === 'string'
  && typeof value.bodyJson === 'string' && Number.isSafeInteger(value.bodyBytes) && value.bodyBytes === encoder.encode(value.bodyJson).length
  && Array.isArray(value.inputs) && value.inputs.length >= 1 && value.inputs.length <= 16 && value.inputs.every(dependency) && object(value.pack)
  && only(value.pack, ['schemaVersion', 'packId', 'bodyJson', 'bodySha256', 'bodyBytes', 'selectedBytes', 'uniqueTextBytes', 'omittedBytes', 'tokenAccounting', 'authority'])
  && value.pack.schemaVersion === 'rangoon.context-pack.v1' && /^pack:[0-9a-f]{64}$/.test(value.pack.packId) && typeof value.pack.bodyJson === 'string' && DIGEST.test(value.pack.bodySha256)
  && ['bodyBytes', 'selectedBytes', 'uniqueTextBytes', 'omittedBytes'].every(key => Number.isSafeInteger(value.pack[key]) && value.pack[key] >= 0) && value.pack.bodyBytes === encoder.encode(value.pack.bodyJson).length && value.pack.bodyBytes <= 262144 && value.bodyBytes <= 262144
  && value.pack.tokenAccounting === 'unknown' && value.pack.authority === 'none'
  && value.processingLocation === 'unknown' && value.retention === 'unknown' && value.authority === 'none';
const check = value => object(value) && only(value, ['schemaVersion', 'profileSha256', 'serverVersion', 'processingLocation', 'authority'])
  && value.schemaVersion === 'rangoon.local-check.v1' && DIGEST.test(value.profileSha256) && typeof value.serverVersion === 'string'
  && value.processingLocation === 'unknown' && value.authority === 'none';
const citation = value => object(value) && only(value, ['input', 'startByte', 'endByte']) && dependency({ input: value.input, observedHead: null, byteLength: 0 })
  && Number.isSafeInteger(value.startByte) && Number.isSafeInteger(value.endByte) && value.startByte >= 0 && value.endByte > value.startByte;
const proposal = value => object(value) && only(value, ['kind', 'title', 'authoredText', 'explanation', 'citations'])
  && ['classification', 'capability', 'difference'].includes(value.kind) && ['title', 'authoredText', 'explanation'].every(key => typeof value[key] === 'string')
  && Array.isArray(value.citations) && value.citations.length <= 256 && value.citations.every(citation);
const completion = value => object(value) && only(value, ['schemaVersion', 'requestId', 'profileSha256', 'packId', 'responseSha256', 'observedModel', 'serverCreatedAt', 'usageSource', 'usage', 'proposals', 'processingLocation', 'authority'])
  && value.schemaVersion === 'rangoon.local-completion.v1' && DIGEST.test(value.requestId) && DIGEST.test(value.profileSha256) && typeof value.packId === 'string' && DIGEST.test(value.responseSha256)
  && typeof value.observedModel === 'string' && typeof value.serverCreatedAt === 'string' && value.usageSource === 'server_reported' && object(value.usage)
  && object(value.proposals) && only(value.proposals, ['schemaVersion', 'packId', 'responseSha256', 'task', 'proposals', 'uncertainties', 'contentKind', 'authority'])
  && value.proposals.schemaVersion === 'rangoon.validated-analysis.v1' && value.proposals.packId === value.packId && DIGEST.test(value.proposals.responseSha256)
  && TASKS.has(value.proposals.task) && Array.isArray(value.proposals.proposals) && value.proposals.proposals.length <= 32 && value.proposals.proposals.every(proposal)
  && Array.isArray(value.proposals.uncertainties) && value.proposals.uncertainties.length <= 32 && value.proposals.uncertainties.every(item => typeof item === 'string')
  && value.proposals.contentKind === 'model_authored' && value.proposals.authority === 'none' && value.processingLocation === 'unknown' && value.authority === 'none';

function envelope(value) {
  if (!object(value) || value.schemaVersion !== SESSION_SCHEMA || value.authority !== 'none' || !OUTCOMES.has(value.outcome) || !(value.generation === null || decimal(value.generation)) || !(value.runId === null || RUN.test(value.runId))) return null;
  const common = ['schemaVersion', 'generation', 'runId', 'authority', 'outcome'];
  const extra = { unconfigured: ['session'], configured: ['session'], cleared: ['session'], prepared: ['prepared'], checked: ['check'], completed: ['completion', 'freshness'], cancel_requested: ['session'], cancelled: [], failed: ['code'] }[value.outcome];
  if (!only(value, [...common, ...extra])) return null;
  if (['unconfigured', 'configured', 'cleared', 'cancel_requested'].includes(value.outcome) && (!session(value.session) || value.session.generation !== value.generation)) return null;
  if (value.outcome === 'configured' && value.session.profile === null) return null;
  if (['unconfigured', 'cleared'].includes(value.outcome) && value.session.profile !== null) return null;
  if (value.outcome === 'prepared' && (!prepared(value.prepared) || value.generation !== value.prepared.generation)) return null;
  if (value.outcome === 'checked' && !check(value.check)) return null;
  if (value.outcome === 'completed' && (!completion(value.completion) || !['current', 'stale', 'unavailable'].includes(value.freshness))) return null;
  if (value.outcome === 'failed' && !CODES.has(value.code)) return null;
  if (['prepared', 'checked', 'completed', 'cancelled'].includes(value.outcome) && value.runId === null) return null;
  return value;
}

function snapshot(value) {
  return object(value) && SOURCE.test(value.sourceId) && typeof value.displayName === 'string' && DIGEST.test(value.sha256) && Number.isSafeInteger(value.byteLength) && value.byteLength >= 0;
}

function failedEnvelope() { return { schemaVersion: SESSION_SCHEMA, generation: null, runId: null, authority: 'none', outcome: 'failed', code: 'session_unavailable' }; }
function inputKey(input) { return input.kind === 'source' ? `source:${input.sourceId}` : `capability:${input.capabilityId}:${input.revisionId}`; }
function selectedInput(input) { return input.kind === 'source' ? { kind: 'source', sourceId: input.sourceId } : { kind: 'capability', capabilityId: input.capabilityId, revisionId: input.revisionId }; }
function preparedMatches(value, profile, task, inputs) {
  if (value.model !== profile.model || value.inputs.length !== inputs.length) return false;
  if (!value.inputs.every((item, index) => inputKey(item.input.kind === 'source' ? { kind: 'source', sourceId: item.input.sourceId } : { kind: 'capability', capabilityId: item.input.capabilityId, revisionId: item.input.revisionId }) === inputKey(inputs[index]))) return false;
  try { const body = JSON.parse(value.pack.bodyJson); const wire = JSON.parse(value.bodyJson); return object(body) && body.task === task && body.target?.model === profile.model && body.target?.maxOutputTokens === profile.maxOutputTokens && wire.model === profile.model && wire.options?.num_predict === profile.maxOutputTokens && wire.stream === false && wire.think === false && wire.format === 'json' && Array.isArray(wire.messages) && wire.messages.length === 2 && wire.messages[0].role === 'system' && wire.messages[1].role === 'user' && wire.messages[1].content === value.pack.bodyJson; } catch { return false; }
}

export const DEFAULT_LOCAL_MODEL_PROFILE = Object.freeze({ profileId: 'local', host: '127.0.0.1', port: 11434, model: '', maxOutputTokens: 1024 });
export const EMPTY_MODEL_ASSISTANCE_STATE = Object.freeze({
  bridgeAvailable: false, status: 'unavailable', pending: null, draining: false, activeRunId: null, generation: null,
  profile: DEFAULT_LOCAL_MODEL_PROFILE, session: null, snapshots: [], capabilities: [], selectedCapabilities: {},
  inputs: [], task: 'classify_v1', prepared: null, check: null, completion: null, tab: 'payload',
  error: null, message: 'Native model workbench unavailable in browser preview.', requestId: 0,
});

export function createModelAssistanceController({ invoke, onChange = () => {} } = {}) {
  const available = typeof invoke === 'function';
  let state = { ...EMPTY_MODEL_ASSISTANCE_STATE, bridgeAvailable: available, status: available ? 'idle' : 'unavailable', message: available ? 'Configure a session-only local profile, then choose saved inputs.' : EMPTY_MODEL_ASSISTANCE_STATE.message };
  let epoch = 0;
  let selectionEpoch = 0;
  let pollTimer = null;
  const publish = () => onChange(clone(state));
  const set = changes => { state = { ...state, ...changes }; publish(); };
  const stopPoll = () => { if (pollTimer !== null) { clearTimeout(pollTimer); pollTimer = null; } };
  const raw = value => encoder.encode(JSON.stringify(value));
  const call = async (command, payload, bytes = false) => { try { return await invoke(command, bytes ? raw(payload) : payload); } catch { return failedEnvelope(); } };
  const unavailable = () => !available;
  const invalidated = (message = 'Selection changed. Prepare a new request.') => {
    ++selectionEpoch;
    set({ prepared: null, completion: null, check: null, error: null, message, requestId: state.requestId + 1 });
  };
  const terminal = result => ['unconfigured', 'configured', 'cleared', 'prepared', 'checked', 'completed', 'cancelled', 'failed'].includes(result.outcome);
  const poll = async () => {
    if ((!state.pending && !state.draining) || unavailable()) return stopPoll();
    const local = epoch;
    const result = envelope(await call('get_local_model_profile'));
    if (local !== epoch || (!state.pending && !state.draining)) return;
    if (result && ['configured', 'unconfigured', 'cleared'].includes(result.outcome)) {
      const active = result.session.active;
      const drained = state.draining && !active;
      const activeRunId = active?.runId ?? (drained ? null : state.activeRunId);
      const draining = drained ? false : state.draining;
      // Preserve live controls and focus when a poll only repeats known state.
      if (JSON.stringify(result.session) !== JSON.stringify(state.session)
        || result.generation !== state.generation || activeRunId !== state.activeRunId || draining !== state.draining) {
        set({ session: clone(result.session), generation: result.generation, activeRunId, draining });
      }
      if (drained && !state.pending) return stopPoll();
    }
    pollTimer = setTimeout(poll, 250); pollTimer.unref?.();
  };
  const startPoll = () => { stopPoll(); void poll(); };
  const accept = (result, localEpoch, pending) => {
    const value = envelope(result);
    if (localEpoch !== epoch || !value) {
      if (localEpoch === epoch && !value) set({ status: 'failed', pending: null, error: { code: 'session_unavailable' }, message: 'Native result unavailable. Prepare a new request.' });
      return null;
    }
    if (state.generation !== null && value.generation !== null && value.generation !== state.generation && value.outcome !== 'configured' && value.outcome !== 'cleared') return null;
    if (pending && state.pending !== pending) return null;
    if (terminal(value)) stopPoll();
    return value;
  };
  const controller = {
    getState: () => clone(state),
    async load() {
      if (unavailable() || state.pending || state.draining) return;
      const local = ++epoch;
      set({ status: 'loading', pending: 'load', error: null, message: 'Loading saved inputs and session profile…', requestId: state.requestId + 1 });
      const [profileResult, snapshotsResult, capabilitiesResult] = await Promise.all([call('get_local_model_profile'), call('list_snapshots'), call('list_capabilities')]);
      if (local !== epoch) return;
      const profile = envelope(profileResult);
      const snapshotsValid = snapshotsResult?.outcome === 'listed' && Array.isArray(snapshotsResult.snapshots) && snapshotsResult.snapshots.length <= 128 && snapshotsResult.snapshots.every(snapshot) && new Set(snapshotsResult.snapshots.map(item => item.sourceId)).size === snapshotsResult.snapshots.length;
      const capabilitiesValid = capabilitiesResult?.outcome === 'listed' && Array.isArray(capabilitiesResult.capabilities) && capabilitiesResult.capabilities.length <= 128 && capabilitiesResult.capabilities.every(validateCapabilitySummary) && new Set(capabilitiesResult.capabilities.map(item => item.id)).size === capabilitiesResult.capabilities.length;
      if (!profile || !['unconfigured', 'configured', 'cleared'].includes(profile.outcome)) return set({ status: 'failed', pending: null, error: { code: 'session_unavailable' }, message: 'Native model session unavailable.' });
      if (!snapshotsValid || !capabilitiesValid) return set({ status: 'failed', pending: null, error: { code: 'invalid_inventory' }, message: 'Saved-input inventory unavailable. Reload before selecting inputs.' });
      const remote = profileForm(profile.session?.profile?.config ?? profile.session?.profile ?? null);
      set({ status: 'ready', pending: null, generation: profile.generation, session: clone(profile.session), profile: remote ?? state.profile, snapshots: snapshotsResult.snapshots, capabilities: capabilitiesResult.capabilities, error: null, message: 'Saved inputs ready. Local profile is session-only.' });
    },
    async configure(profile) {
      if (unavailable() || state.pending || !profileValid(profile)) return set({ error: { code: 'invalid_profile' }, message: 'Profile is invalid. No model request was made.' });
      const local = ++epoch;
      ++selectionEpoch;
      stopPoll();
      set({ pending: 'configure', prepared: null, completion: null, check: null, error: null, profile: { ...profile }, message: 'Configuring session-only local profile…' });
      const result = accept(await call('configure_local_model', { schemaVersion: 'rangoon.local-profile-request.v1', ...profile }, true), local, 'configure');
      if (!result) return;
      if (result.outcome !== 'configured') return set({ status: 'failed', pending: null, error: { code: result.code ?? 'invalid_profile' }, message: 'Profile was not configured. Prepare a new request.' });
      set({ status: 'ready', pending: null, session: clone(result.session), generation: result.generation, error: null, message: 'Local profile configured for this session only.' });
    },
    async clear() {
      if (unavailable()) return;
      const local = ++epoch;
      ++selectionEpoch;
      stopPoll();
      set({ pending: 'clear', prepared: null, completion: null, check: null, error: null, activeRunId: null, message: 'Clearing local model session…' });
      const result = accept(await call('clear_local_model'), local, 'clear');
      if (!result) return;
      if (result.outcome !== 'cleared') return set({ status: 'failed', pending: null, error: { code: result.code ?? 'session_unavailable' }, message: 'Session clear could not finish.' });
      const draining = Boolean(result.session.active);
      set({ status: 'ready', pending: null, draining, session: clone(result.session), generation: result.generation, profile: { ...DEFAULT_LOCAL_MODEL_PROFILE }, activeRunId: result.session.active?.runId ?? null, error: null, message: 'Local model session cleared. Any active native operation remains cancellable until it exits.' });
      if (draining) startPoll();
    },
    setTask(task) {
      if (!TASKS.has(task)) return;
      state = { ...state, task, inputs: task === 'compare_v1' && state.inputs.some(input => input.kind !== 'capability') ? [] : state.inputs };
      invalidated(task === 'compare_v1' ? 'Compare requires exactly two capability revisions. Prepare a new request.' : 'Task changed. Prepare a new request.');
    },
    toggleSource(sourceId) {
      if (!SOURCE.test(sourceId) || !state.snapshots.some(item => item.sourceId === sourceId) || state.task === 'compare_v1') return;
      const candidate = { kind: 'source', sourceId }; const key = inputKey(candidate); const exists = state.inputs.some(input => inputKey(input) === key);
      const inputs = exists ? state.inputs.filter(input => inputKey(input) !== key) : state.inputs.length < 16 ? [...state.inputs, candidate] : state.inputs;
      if (inputs !== state.inputs) { state = { ...state, inputs }; invalidated('Saved input selection changed. Prepare a new request.'); }
    },
    async selectCapability(capabilityId, revisionId = null) {
      if (unavailable() || state.pending || !CAPABILITY.test(capabilityId) || (revisionId !== null && !REVISION.test(revisionId))) return;
      const local = epoch;
      const result = await call('open_capability', { capabilityId, revisionId });
      if (local !== epoch || result?.outcome !== 'opened') return;
      const detail = validateCapabilityDetail(result.capability);
      if (!detail || detail.id !== capabilityId || (revisionId !== null && detail.revision.id !== revisionId)) return;
      const selectedCapabilities = { ...state.selectedCapabilities, [capabilityId]: detail };
      set({ selectedCapabilities });
      const candidate = { kind: 'capability', capabilityId, revisionId: detail.revision.id };
      if (state.task === 'compare_v1' && state.inputs.some(input => input.kind === 'source')) return;
      const key = inputKey(candidate); const inputs = state.inputs.some(input => inputKey(input) === key) ? state.inputs : state.inputs.length < 16 ? [...state.inputs, candidate] : state.inputs;
      if (inputs !== state.inputs) { state = { ...state, inputs }; invalidated('Capability revision selected. Prepare a new request.'); }
    },
    async toggleRevision(capabilityId, revisionId) {
      if (!CAPABILITY.test(capabilityId) || !REVISION.test(revisionId)) return;
      const candidate = { kind: 'capability', capabilityId, revisionId }; const key = inputKey(candidate);
      if (state.inputs.some(input => inputKey(input) === key)) { state = { ...state, inputs: state.inputs.filter(input => inputKey(input) !== key) }; return invalidated('Capability revision removed. Prepare a new request.'); }
      const detail = state.selectedCapabilities[capabilityId];
      if (!detail || !detail.history.some(item => item.id === revisionId)) return controller.selectCapability(capabilityId, revisionId);
      if (state.inputs.length >= 16 || (state.task === 'compare_v1' && state.inputs.some(input => input.kind !== 'capability'))) return;
      state = { ...state, inputs: [...state.inputs, candidate] }; invalidated('Capability revision selected. Prepare a new request.');
    },
    async prepare() {
      if (unavailable() || state.pending || !profileValid(state.profile) || state.inputs.length < 1 || state.inputs.length > 16 || (state.task === 'compare_v1' && (state.inputs.length !== 2 || state.inputs.some(input => input.kind !== 'capability')))) return;
      const local = ++epoch; const visible = selectionEpoch; const task = state.task; const inputs = clone(state.inputs); const profile = { ...state.profile };
      set({ pending: 'prepare', prepared: null, completion: null, check: null, error: null, message: 'Preparing exact local request…' }); startPoll();
      const result = accept(await call('prepare_local_model', { schemaVersion: 'rangoon.local-selection.v1', task, inputs: inputs.map(selectedInput) }, true), local, 'prepare');
      if (!result) return;
      if (result.outcome !== 'prepared' || !preparedMatches(result.prepared, profile, task, inputs)) return set({ status: 'failed', pending: null, error: { code: result.code ?? 'pack_invalid' }, message: 'Request preparation failed. Prepare a new request.' });
      if (visible !== selectionEpoch) return set({ pending: null, activeRunId: null, message: 'Selection changed. Prepare a new request.' });
      set({ status: 'ready', pending: null, prepared: clone(result.prepared), generation: result.generation, activeRunId: null, error: null, message: 'Exact payload prepared locally. Review it before opening native confirmation.' });
    },
    async check() {
      if (unavailable() || state.pending) return;
      const local = ++epoch; const profileSha256 = state.session?.profile?.profileSha256 ?? null; set({ pending: 'check', check: null, error: null, message: 'Checking source-free local protocol…' }); startPoll();
      const result = accept(await call('check_local_model'), local, 'check');
      if (!result) return;
      if (result.outcome !== 'checked' || !profileSha256 || result.check.profileSha256 !== profileSha256) return set({ status: 'failed', pending: null, error: { code: result.code ?? 'connection_failed' }, message: 'Protocol check failed. No source text was sent.' });
      set({ status: 'ready', pending: null, check: clone(result.check), generation: result.generation, activeRunId: null, error: null, message: 'Source-free protocol observation recorded. It does not establish model trust or upload permission.' });
    },
    async send() {
      const preparedView = state.prepared;
      if (unavailable() || state.pending || !preparedView) return;
      const local = ++epoch; const visible = selectionEpoch; const profileSha256 = state.session?.profile?.profileSha256 ?? null; const task = state.task; set({ pending: 'send', completion: null, error: null, message: 'Native review or request in progress. Cancel remains available until terminal result.' }); startPoll();
      const result = accept(await call('send_local_model', { schemaVersion: 'rangoon.local-send.v1', preparedId: preparedView.preparedId, requestId: preparedView.requestId }, true), local, 'send');
      if (!result) return;
      if (result.outcome === 'completed') {
        if (!profileSha256 || result.completion.requestId !== preparedView.requestId || result.completion.packId !== preparedView.pack.packId || result.completion.profileSha256 !== profileSha256 || result.completion.observedModel !== preparedView.model || result.completion.proposals.task !== task) return set({ status: 'failed', pending: null, prepared: null, activeRunId: null, error: { code: 'proposal_invalid' }, message: 'Native review or request failed. Prepare a new request.' });
        if (visible !== selectionEpoch) return set({ pending: null, prepared: null, activeRunId: null, message: 'Selection changed. Prepare a new request.' });
        return set({ status: 'ready', pending: null, prepared: null, completion: { completion: clone(result.completion), freshness: result.freshness }, generation: result.generation, activeRunId: null, error: null, message: `Advisory result received. Freshness: ${result.freshness}.` });
      }
      if (result.outcome === 'cancelled') return set({ status: 'ready', pending: null, prepared: null, activeRunId: null, error: null, message: 'Native review or request cancelled. Prepare a new request.' });
      set({ status: 'failed', pending: null, prepared: null, activeRunId: null, error: { code: result.code ?? 'confirmation_unavailable' }, message: 'Native review or request failed. Prepare a new request.' });
    },
    async cancel() {
      if (unavailable() || (!state.pending && !state.draining) || !RUN.test(state.activeRunId ?? '')) return;
      const local = epoch;
      const result = accept(await call('cancel_local_model', { schemaVersion: 'rangoon.local-cancel.v1', runId: state.activeRunId }, true), local, null);
      if (!result || result.outcome !== 'cancel_requested') return;
      set({ session: clone(result.session), generation: result.generation, message: 'Cancel requested; remote work may already continue if bytes left device.' });
    },
    setTab(tab) { if (['payload', 'coverage', 'facts'].includes(tab)) set({ tab }); },
    invalidate() { invalidated(); },
  };
  return controller;
}
