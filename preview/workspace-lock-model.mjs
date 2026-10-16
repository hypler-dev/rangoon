// This cooperative renderer fence complements native custody; it never grants authority.
const lifecycle = new Set(['get_workspace_close_state', 'begin_workspace_close', 'acknowledge_workspace_purge', 'get_workspace_request_state']);
const workspaceCommands = new Set([
  'select_and_analyze',
  'list_snapshots',
  'save_analysis',
  'open_snapshot',
  'read_composition_source',
  'clear_analysis',
  'list_capabilities',
  'open_capability',
  'create_capability',
  'revise_capability',
  'review_capability',
  'compile_capability',
  'export_instruction_bundle',
  'inspect_instruction_bundle',
  'get_workspace_data',
  'export_workspace_backup',
  'prepare_workspace_restore',
  'restore_workspace_backup',
  'inspect_workspace_deletion',
  'delete_workspace_record',
  'preview_composition',
  'commit_composition',
  'list_workflows',
  'open_workflow',
  'begin_workflow_draft',
  'inspect_workflow_save',
  'commit_workflow_save',
  'clear_workflow_draft',
  'get_local_model_profile',
  'configure_local_model',
  'clear_local_model',
  'prepare_local_model',
  'check_local_model',
  'send_local_model',
  'cancel_local_model',
  'get_cloud_model_profile',
  'configure_cloud_model',
  'clear_cloud_model',
  'prepare_cloud_model',
  'check_cloud_model',
  'send_cloud_model',
  'cancel_cloud_model',
  'inspect_cloud_credential',
  'edit_cloud_credential',
  'remove_cloud_credential'
]);
const fields = ['schemaVersion', 'storage', 'state', 'attemptId', 'epoch', 'diagnostic'];
const closedError = () => new Error('Workspace access is closed.');
const validEpoch = value => typeof value === 'string' && /^[1-9][0-9]{0,19}$/.test(value) && BigInt(value) <= 18446744073709551615n;
function decode(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) return null;
  const descriptors = Object.getOwnPropertyDescriptors(value);
  if (Reflect.ownKeys(descriptors).length !== fields.length || fields.some(key => !Object.hasOwn(descriptors, key) || !Object.hasOwn(descriptors[key], 'value'))) return null;
  const result = Object.fromEntries(fields.map(key => [key, descriptors[key].value]));
  if (result.schemaVersion !== 'rangoon.workspace-close-state.v1') return null;
  if (result.storage === 'legacy_plaintext') return result.state === 'unavailable' && result.attemptId === null && result.epoch === null && result.diagnostic === 'encrypted_close_unavailable' ? result : null;
  if (result.storage !== 'managed_encrypted' || !['open', 'closing', 'locked', 'recovery_required'].includes(result.state)) return null;
  if (result.state === 'open') return result.attemptId === null && result.epoch === null && result.diagnostic === null ? result : null;
  if (result.diagnostic !== null) return null;
  return typeof result.attemptId === 'string' && /^[a-f0-9]{64}$/.test(result.attemptId) && validEpoch(result.epoch) ? result : null;
}

function decodeClient(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) return null;
  const descriptors = Object.getOwnPropertyDescriptors(value);
  const keys = ['schemaVersion', 'storage', 'state', 'clientId'];
  if (Reflect.ownKeys(descriptors).length !== keys.length || keys.some(key => !Object.hasOwn(descriptors, key) || !Object.hasOwn(descriptors[key], 'value'))) return null;
  const result = Object.fromEntries(keys.map(key => [key, descriptors[key].value]));
  return result.schemaVersion === 'rangoon.workspace-request-state.v1' && result.storage === 'managed_encrypted' && result.state === 'open' && typeof result.clientId === 'string' && /^[a-f0-9]{64}$/.test(result.clientId) ? result.clientId : null;
}

export function createWorkspaceLockController({ invoke, beforeInvoke = () => {}, purge = () => {}, onChange = () => {}, schedule = setTimeout, cancelSchedule = clearTimeout, pollInterval = 250 } = {}) {
  const native = typeof invoke === 'function';
  let status = native ? 'checking' : 'browser';
  let identity = Symbol('workspace');
  let clientId = null;
  let disposed = false;
  let initialized = false;
  let initialization;
  let timer = null;
  let attempt = null;
  let purged = false;
  let cleanupHealthy = false;
  let acknowledged = false;
  const blocked = () => disposed || !['browser', 'legacy', 'open', 'confirming'].includes(status);
  const getState = () => Object.freeze({ status, blocked: blocked(), canLock: !disposed && status === 'open' });
  const publish = () => onChange(getState());
  const clearTimer = () => { if (timer !== null) cancelSchedule(timer); timer = null; };
  function purgeLocal() {
    if (purged) return cleanupHealthy;
    purged = true;
    try { cleanupHealthy = purge() !== false; } catch { cleanupHealthy = false; }
    return cleanupHealthy;
  }
  function recovery() {
    clientId = null;
    clearTimer();
    identity = Symbol('closed-workspace');
    status = 'recovery_required';
    purgeLocal();
    publish();
  }
  const same = view => view?.storage === 'managed_encrypted' && view.attemptId === attempt?.attemptId && view.epoch === attempt?.epoch;
  const current = token => !disposed && token === identity;
  async function fixed(command, payload, token) {
    const result = await invoke(command, payload);
    if (!current(token)) throw closedError();
    const view = decode(result);
    if (!view) throw closedError();
    return view;
  }
  function acceptProgress(view) {
    if (!same(view) || !cleanupHealthy || !acknowledged || !['closing', 'locked'].includes(view.state)) throw closedError();
    status = view.state;
    publish();
    if (status === 'closing') armPoll();
  }
  function armPoll() {
    clearTimer();
    if (disposed || status !== 'closing') return;
    const token = identity;
    timer = schedule(async () => {
      timer = null;
      if (!current(token) || status !== 'closing') return;
      try { acceptProgress(await fixed('get_workspace_close_state', {}, token)); }
      catch { if (current(token)) recovery(); }
    }, Math.max(10, Math.min(60000, Number(pollInterval) || 250)));
  }
  async function acknowledge(view, token) {
    if (!cleanupHealthy || view.storage !== 'managed_encrypted' || view.state !== 'closing') throw closedError();
    attempt = { attemptId: view.attemptId, epoch: view.epoch };
    // Tauri's Request<Raw> requires bytes. A JSON object is deliberately not used.
    const payload = new TextEncoder().encode(JSON.stringify({ schemaVersion: 'rangoon.workspace-purge-ack.v1', ...attempt }));
    const result = await fixed('acknowledge_workspace_purge', payload, token);
    if (!same(result) || !['closing', 'locked'].includes(result.state)) throw closedError();
    acknowledged = true;
    acceptProgress(result);
  }
  async function initialize() {
    if (initialized) return initialization;
    initialized = true;
    initialization = (async () => {
      if (!native || disposed) return getState();
      const token = identity;
      try {
        const view = await fixed('get_workspace_close_state', {}, token);
        if (view.storage === 'legacy_plaintext') { status = 'legacy'; publish(); }
        else if (view.state === 'open') {
          const client = await invoke('get_workspace_request_state', {});
          if (!current(token)) throw closedError();
          clientId = decodeClient(client);
          if (!clientId) throw closedError();
          status = 'open'; publish();
        }
        else {
          identity = Symbol('closed-workspace');
          status = 'closing';
          if (!purgeLocal()) throw closedError();
          publish();
          if (view.state === 'locked') {
            // A fresh window has loaded no source data. Purge before trusting native custody.
            attempt = { attemptId: view.attemptId, epoch: view.epoch };
            status = 'locked';
            publish();
          } else await acknowledge(view, identity);
        }
      } catch { if (!disposed) recovery(); }
      return getState();
    })();
    return initialization;
  }
  const bridge = native ? async (command, args) => {
    const token = identity;
    const check = () => { if (!current(token) || blocked()) throw closedError(); };
    check();
    if (typeof command !== 'string' || lifecycle.has(command)) throw closedError();
    const requestClient = clientId;
    if (requestClient && command !== 'get_engine_status' && !workspaceCommands.has(command)) throw closedError();
    let finish;
    let result;
    let failure;
    let failed = false;
    try {
      finish = beforeInvoke(command, args);
      if (finish?.then) throw closedError();
      check();
      // Await even a synchronously reentrant transport so rejected old work is consumed.
      const options = requestClient && workspaceCommands.has(command) ? { headers: { 'x-rangoon-workspace-client': requestClient } } : undefined;
      const pending = Promise.resolve(invoke(command, args, options));
      void pending.catch(() => {});
      check();
      result = await pending;
      check();
    } catch (error) { failed = true; failure = error; }
    try { if (typeof finish === 'function') finish(); }
    catch (error) { failed = true; failure = error; }
    check();
    if (failed) throw failure;
    return result;
  } : undefined;
  return {
    getState, bridge, initialize, isBlocked: blocked,
    openConfirmation() {
      if (disposed || status !== 'open') return false;
      status = 'confirming'; publish(); return true;
    },
    cancelConfirmation() {
      if (disposed || status !== 'confirming') return false;
      status = 'open'; publish(); return true;
    },
    async confirmClose() {
      if (disposed || status !== 'confirming') return false;
      // Set the barrier before observers, native IPC, disposal or DOM replacement.
      clientId = null;
      identity = Symbol('closed-workspace');
      status = 'closing';
      const token = identity;
      let begin;
      try { begin = Promise.resolve(invoke('begin_workspace_close', {})); }
      catch { begin = Promise.reject(closedError()); }
      // Consume a possible transport rejection immediately, even if cleanup fails.
      const response = begin.then(value => ({ value }), () => ({ failed: true }));
      purgeLocal();
      publish();
      const outcome = await response;
      if (!current(token)) return false;
      try {
        if (outcome.failed || !cleanupHealthy) throw closedError();
        const view = decode(outcome.value);
        if (!view) throw closedError();
        await acknowledge(view, token);
        return true;
      } catch { if (current(token)) recovery(); return false; }
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      clientId = null;
      identity = Symbol('closed-workspace');
      clearTimer();
      status = 'recovery_required';
      purgeLocal();
      publish();
    },
  };
}
