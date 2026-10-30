const dtoFields = ['schemaVersion', 'state', 'startupId', 'attemptId', 'epoch', 'actions', 'copyPhase', 'retainedOriginal', 'diagnostic'];
const requestFields = ['schemaVersion', 'startupId', 'attemptId', 'epoch', 'action'];
const states = new Set(['fresh', 'legacy_found', 'conversion_review', 'copying', 'restart_required', 'verify_required', 'recovery_required', 'handoff', 'unavailable']);
const phases = new Set(['creating_destination', 'copying_records', 'checking_copy', 'recording_selection']);
const originals = new Set(['unassessed', 'present_unencrypted', 'not_verified', 'not_applicable']);
const diagnostics = new Set([null, 'startup_unavailable', 'backend_unavailable', 'startup_layout_invalid', 'startup_changed', 'startup_busy', 'startup_key_missing', 'startup_copy_interrupted', 'startup_recovery_required', 'startup_control_unavailable']);
const allowed = Object.freeze({
  fresh: ['setup', 'quit'],
  legacy_found: ['review_conversion', 'quit'],
  conversion_review: ['copy', 'cancel', 'quit'],
  copying: ['quit'],
  restart_required: ['quit'],
  verify_required: ['verify', 'quit'],
  recovery_required: ['recover', 'quit'],
  handoff: [],
  unavailable: ['quit'],
});
const originalsByState = Object.freeze({
  fresh: ['not_applicable'],
  legacy_found: ['unassessed'],
  conversion_review: ['present_unencrypted'],
  copying: ['present_unencrypted'],
  restart_required: ['present_unencrypted'],
  verify_required: ['present_unencrypted', 'not_verified'],
  recovery_required: ['not_verified'],
  handoff: ['not_applicable', 'present_unencrypted', 'not_verified'],
  unavailable: ['not_applicable', 'not_verified'],
});
const maxEpoch = 18446744073709551615n;
const encoder = new TextEncoder();

const ownRecord = value => value && typeof value === 'object' && !Array.isArray(value)
  && [Object.prototype, null].includes(Object.getPrototypeOf(value));
const exact = (value, fields) => {
  if (!ownRecord(value)) return null;
  const descriptors = Object.getOwnPropertyDescriptors(value);
  if (Reflect.ownKeys(descriptors).length !== fields.length) return null;
  if (fields.some(key => !Object.hasOwn(descriptors, key) || !Object.hasOwn(descriptors[key], 'value'))) return null;
  return Object.fromEntries(fields.map(key => [key, descriptors[key].value]));
};
const id = value => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
const epoch = value => typeof value === 'string' && /^[1-9][0-9]{0,19}$/.test(value) && BigInt(value) <= maxEpoch;
const bytes = value => encoder.encode(JSON.stringify(value)).byteLength;

export function decodeDesktopStartupView(value) {
  let wire;
  try { wire = exact(value, dtoFields); } catch { return null; }
  if (!wire || wire.schemaVersion !== 'rangoon.workspace-startup.v1' || !states.has(wire.state)
    || !Array.isArray(wire.actions) || !originals.has(wire.retainedOriginal) || !diagnostics.has(wire.diagnostic)) return null;
  let responseBytes;
  try { responseBytes = bytes(wire); } catch { return null; }
  if (responseBytes > 2048 || wire.actions.length > 3 || new Set(wire.actions).size !== wire.actions.length) return null;
  const noCycle = wire.startupId === null && wire.attemptId === null && wire.epoch === null;
  const hasCycle = id(wire.startupId) && id(wire.attemptId) && epoch(wire.epoch);
  if ((!noCycle && !hasCycle) || (noCycle && !['handoff', 'unavailable'].includes(wire.state)) || (noCycle && wire.actions.length !== 0)) return null;
  if (!hasCycle && wire.copyPhase !== null) return null;
  if (wire.state === 'handoff' && (wire.diagnostic !== null || wire.retainedOriginal === 'unassessed')) return null;
  if (noCycle && wire.state === 'unavailable' && wire.retainedOriginal !== 'not_applicable') return null;
  if (!originalsByState[wire.state].includes(wire.retainedOriginal)) return null;
  if (wire.copyPhase !== null && (!phases.has(wire.copyPhase) || wire.state !== 'copying')) return null;
  if (wire.state === 'copying' && wire.copyPhase === null) return null;
  if (wire.state !== 'copying' && wire.copyPhase !== null) return null;
  const permitted = hasCycle ? allowed[wire.state] : [];
  if (wire.actions.some(action => !permitted.includes(action))
    || wire.actions.some((action, index) => index > 0 && permitted.indexOf(action) <= permitted.indexOf(wire.actions[index - 1]))) return null;
  return Object.freeze({ ...wire, actions: Object.freeze([...wire.actions]) });
}

function actionRequest(view, action) {
  const request = { schemaVersion: 'rangoon.workspace-startup-action.v1', startupId: view.startupId, attemptId: view.attemptId, epoch: view.epoch, action };
  const exactRequest = exact(request, requestFields);
  if (!exactRequest || !id(exactRequest.startupId) || !id(exactRequest.attemptId) || !epoch(exactRequest.epoch)
    || !view.actions.includes(action)) return null;
  const raw = JSON.stringify(exactRequest);
  return encoder.encode(raw).byteLength <= 512 ? raw : null;
}

export function createDesktopStartupController({ invoke, onChange = () => {} } = {}) {
  const native = typeof invoke === 'function';
  let status = native ? 'checking' : 'browser';
  let view = null;
  let generation = 0;
  let disposed = false;
  let initialized = false;
  let initialization = null;
  let mutation = null;
  const consumed = new Set();
  const snapshot = () => Object.freeze({ status, view, generation });
  const publish = () => { if (!disposed) onChange(snapshot()); };
  const current = token => !disposed && token === generation;
  const unavailable = token => {
    if (!current(token)) return snapshot();
    status = 'unavailable'; view = null; publish();
    return snapshot();
  };
  const accept = (token, next) => {
    if (!current(token)) return snapshot();
    view = next;
    status = next.state === 'handoff' ? 'handoff' : 'ready';
    publish();
    return snapshot();
  };
  async function read() {
    if (disposed) return snapshot();
    if (!native) { status = 'browser'; publish(); return snapshot(); }
    const token = ++generation;
    status = 'checking'; view = null; publish();
    try {
      const next = decodeDesktopStartupView(await invoke('workspace_startup_status'));
      return next ? accept(token, next) : unavailable(token);
    } catch { return unavailable(token); }
  }
  function initialize() {
    if (initialized) return initialization;
    initialized = true;
    initialization = read();
    return initialization;
  }
  async function refresh() {
    if (disposed) return snapshot();
    if (mutation) return mutation.then(() => snapshot());
    return read();
  }
  async function perform(action) {
    if (disposed || !native || status !== 'ready' || !view || typeof action !== 'string') return false;
    const raw = actionRequest(view, action);
    const actionKey = raw ? `${view.startupId}:${view.attemptId}:${view.epoch}:${action}` : null;
    if (!raw || consumed.has(actionKey)) return false;
    consumed.add(actionKey);
    const token = ++generation;
    status = 'working'; publish();
    if (!current(token)) return false;
    const pending = (async () => {
      try {
        const next = decodeDesktopStartupView(await invoke('workspace_startup_action', encoder.encode(raw)));
        if (!next) { unavailable(token); return false; }
        if (!current(token)) return false;
        accept(token, next);
        return true;
      } catch {
        unavailable(token);
        return false;
      }
    })();
    mutation = pending;
    try { return await pending; } finally { if (mutation === pending) mutation = null; }
  }
  function dispose() {
    if (disposed) return;
    disposed = true;
    generation += 1;
    view = null;
    status = 'unavailable';
  }
  return Object.freeze({ initialize, refresh, perform, getState: snapshot, dispose });
}
