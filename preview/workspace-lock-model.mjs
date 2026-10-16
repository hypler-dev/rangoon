// Renderer fence only. Native control and custody remain authoritative.
const lifecycle = new Set([
  'get_workspace_close_state', 'begin_workspace_close', 'acknowledge_workspace_purge', 'get_workspace_request_state',
  'get_workspace_control_state', 'get_workspace_protection_state', 'prepare_workspace_control',
  'confirm_workspace_control', 'cancel_workspace_control'
]);
const workspaceCommands = new Set([
  'select_and_analyze', 'list_snapshots', 'save_analysis', 'open_snapshot', 'read_composition_source', 'clear_analysis',
  'list_capabilities', 'open_capability', 'create_capability', 'revise_capability', 'review_capability',
  'compile_capability', 'export_instruction_bundle', 'inspect_instruction_bundle', 'get_workspace_data',
  'export_workspace_backup', 'prepare_workspace_restore', 'restore_workspace_backup', 'inspect_workspace_deletion',
  'delete_workspace_record', 'preview_composition', 'commit_composition', 'list_workflows', 'open_workflow',
  'begin_workflow_draft', 'inspect_workflow_save', 'commit_workflow_save', 'clear_workflow_draft',
  'get_local_model_profile', 'configure_local_model', 'clear_local_model', 'prepare_local_model', 'check_local_model',
  'send_local_model', 'cancel_local_model', 'get_cloud_model_profile', 'configure_cloud_model', 'clear_cloud_model',
  'prepare_cloud_model', 'check_cloud_model', 'send_cloud_model', 'cancel_cloud_model', 'inspect_cloud_credential',
  'edit_cloud_credential', 'remove_cloud_credential'
]);
const closeFields = ['schemaVersion', 'storage', 'state', 'attemptId', 'epoch', 'diagnostic'];
const controlFields = ['schemaVersion', 'close', 'phase', 'action', 'outcome', 'diagnostic', 'reload'];
const protectionFields = ['schemaVersion', 'close', 'layout', 'diagnostic'];
const ownRecord = value => value && typeof value === 'object' && !Array.isArray(value)
  && [Object.prototype, null].includes(Object.getPrototypeOf(value));
const exact = (value, fields) => {
  if (!ownRecord(value)) return null;
  const descriptors = Object.getOwnPropertyDescriptors(value);
  if (Reflect.ownKeys(descriptors).length !== fields.length) return null;
  if (fields.some(key => !Object.hasOwn(descriptors, key) || !Object.hasOwn(descriptors[key], 'value'))) return null;
  return Object.fromEntries(fields.map(key => [key, descriptors[key].value]));
};
const closedError = () => new Error('Workspace access is closed.');
const validEpoch = value => typeof value === 'string' && /^[1-9][0-9]{0,19}$/.test(value)
  && BigInt(value) <= 18446744073709551615n;
function decodeClose(value) {
  const result = exact(value, closeFields);
  if (!result || result.schemaVersion !== 'rangoon.workspace-close-state.v1') return null;
  if (result.storage === 'legacy_plaintext') {
    return result.state === 'unavailable' && result.attemptId === null && result.epoch === null
      && result.diagnostic === 'encrypted_close_unavailable' ? result : null;
  }
  if (result.storage !== 'managed_encrypted' || !['open', 'closing', 'locked', 'recovery_required'].includes(result.state)) return null;
  if (result.state === 'open') return result.attemptId === null && result.epoch === null && result.diagnostic === null ? result : null;
  return result.diagnostic === null && typeof result.attemptId === 'string'
    && /^[a-f0-9]{64}$/.test(result.attemptId) && validEpoch(result.epoch) ? result : null;
}
function decodeClient(value) {
  const result = exact(value, ['schemaVersion', 'storage', 'state', 'clientId']);
  return result?.schemaVersion === 'rangoon.workspace-request-state.v1' && result.storage === 'managed_encrypted'
    && result.state === 'open' && typeof result.clientId === 'string' && /^[a-f0-9]{64}$/.test(result.clientId)
    ? result.clientId : null;
}
function decodeControl(value) {
  const result = exact(value, controlFields);
  const close = decodeClose(result?.close);
  if (!result || !close || result.schemaVersion !== 'rangoon.workspace-control-state.v1') return null;
  if (!['unavailable', 'recovery_required', 'open', 'locked', 'closing', 'awaiting_confirmation', 'active'].includes(result.phase)) return null;
  if (![null, 'create_empty', 'unlock'].includes(result.action)) return null;
  if (!['observed', 'prepared', 'opened', 'closed', 'cancelled', 'refused'].includes(result.outcome)) return null;
  if (![null, 'workspace_control_unavailable', 'workspace_control_not_completed'].includes(result.diagnostic)) return null;
  if (typeof result.reload !== 'boolean') return null;
  return { ...result, close };
}
function decodeProtection(value) {
  const result = exact(value, protectionFields);
  const close = decodeClose(result?.close);
  if (!result || !close || result.schemaVersion !== 'rangoon.workspace-protection-state.v1') return null;
  if (!['absent', 'locked', 'busy', 'recovery_required', 'unavailable', 'legacy'].includes(result.layout)) return null;
  const expected = {
    absent: null, locked: null, busy: 'workspace_protection_busy',
    recovery_required: 'workspace_protection_recovery_required', unavailable: 'workspace_protection_unavailable',
    legacy: 'encrypted_protection_unavailable'
  };
  return result.diagnostic === expected[result.layout] ? { ...result, close } : null;
}
const sameClose = (left, right) => left?.storage === right?.storage && left?.state === right?.state
  && left?.attemptId === right?.attemptId && left?.epoch === right?.epoch && left?.diagnostic === right?.diagnostic;
const raw = value => new TextEncoder().encode(JSON.stringify(value));

export function createWorkspaceLockController({
  invoke, beforeInvoke = () => {}, purge = () => {}, onChange = () => {}, schedule = setTimeout,
  cancelSchedule = clearTimeout, pollInterval = 250, openPollInterval = 1250, reload = () => location.reload()
} = {}) {
  const native = typeof invoke === 'function';
  let status = native ? 'checking' : 'browser';
  let layout = null;
  let pending = false;
  let identity = Symbol('workspace');
  let clientId = null;
  let c0 = null;
  let disposed = false;
  let initialized = false;
  let initialization;
  let timer = null;
  let attempt = null;
  let cleanupHealthy = false;
  const blocked = () => disposed || !['browser', 'legacy', 'open', 'confirming'].includes(status);
  const canProtect = () => !disposed && status === 'protection' && !pending && ['absent', 'locked'].includes(layout);
  const getState = () => Object.freeze({
    status, blocked: blocked(), canLock: !disposed && status === 'open', layout, canProtect: canProtect(), pending
  });
  const publish = () => onChange(getState());
  const clearTimer = () => { if (timer !== null) cancelSchedule(timer); timer = null; };
  function purgeLocal({ fresh = false } = {}) {
    if (!fresh && cleanupHealthy) return true;
    try { cleanupHealthy = purge() !== false; } catch { cleanupHealthy = false; }
    return cleanupHealthy;
  }
  function closeLocal(next = 'recovery_required') {
    clientId = null; c0 = null; attempt = null; layout = null; pending = false;
    clearTimer(); identity = Symbol('closed-workspace'); status = next;
    purgeLocal({ fresh: true }); publish();
  }
  const current = token => !disposed && token === identity;
  async function fixed(command, payload, token, decode) {
    const result = await invoke(command, payload);
    if (!current(token)) throw closedError();
    const decoded = decode(result);
    if (!decoded) throw closedError();
    return decoded;
  }
  async function adoptClosing(view) {
    clientId = null;
    c0 = null;
    layout = null;
    pending = false;
    clearTimer();
    identity = Symbol('closing-workspace');
    const token = identity;
    status = 'closing';
    attempt = { attemptId: view.attemptId, epoch: view.epoch };
    if (!purgeLocal({ fresh: true })) throw closedError();
    publish();
    const acknowledged = await fixed(
      'acknowledge_workspace_purge', raw({ schemaVersion: 'rangoon.workspace-purge-ack.v1', ...attempt }), token, decodeClose
    );
    if (acknowledged.attemptId !== attempt.attemptId || acknowledged.epoch !== attempt.epoch
      || !['closing', 'locked'].includes(acknowledged.state)) throw closedError();
    status = acknowledged.state;
    publish();
    if (status === 'closing') armClosePoll();
    else await refreshProtection(token);
  }
  function armOpenObserver() {
    clearTimer();
    if (disposed || status !== 'open') return;
    const token = identity;
    timer = schedule(async () => {
      timer = null;
      if (!current(token) || status !== 'open') return;
      try {
        const observed = await fixed('get_workspace_control_state', {}, token, decodeControl);
        if (observed.outcome === 'observed' && observed.phase === 'closing' && observed.action === null
          && observed.diagnostic === null && !observed.reload && observed.close.storage === 'managed_encrypted'
          && observed.close.state === 'closing') {
          await adoptClosing(observed.close);
          return;
        }
        if (observed.outcome !== 'observed' || observed.phase !== 'open' || observed.action !== null
          || observed.diagnostic !== null || observed.reload || observed.close.storage !== 'managed_encrypted'
          || observed.close.state !== 'open') throw closedError();
        armOpenObserver();
      } catch {
        if (current(token)) closeLocal();
      }
    }, Math.max(250, Math.min(60000, Number(openPollInterval) || 1250)));
    if (typeof timer?.unref === 'function') timer.unref();
  }
  async function refreshProtection(token) {
    const observed = await fixed('get_workspace_control_state', {}, token, decodeControl);
    await enterProtection(token, observed);
  }
  function armClosePoll() {
    clearTimer();
    if (disposed || status !== 'closing') return;
    const token = identity;
    timer = schedule(async () => {
      timer = null;
      if (!current(token) || status !== 'closing') return;
      try {
        const view = await fixed('get_workspace_close_state', {}, token, decodeClose);
        if (view.storage !== 'managed_encrypted' || !attempt || view.attemptId !== attempt.attemptId
          || view.epoch !== attempt.epoch || !['closing', 'locked'].includes(view.state)) throw closedError();
        status = view.state;
        publish();
        if (status === 'closing') armClosePoll();
        else await refreshProtection(token);
      } catch {
        if (current(token)) closeLocal();
      }
    }, Math.max(50, Math.min(60000, Number(pollInterval) || 250)));
    if (typeof timer?.unref === 'function') timer.unref();
  }
  async function enterProtection(token, observed) {
    if (observed.outcome !== 'observed' || observed.phase !== 'locked' || observed.action !== null
      || observed.diagnostic !== null || observed.reload || observed.close.storage !== 'managed_encrypted'
      || observed.close.state !== 'locked') throw closedError();
    c0 = observed.close;
    // C0 and C1 require distinct fresh renderer purges. Never cache C0 proof.
    if (!purgeLocal({ fresh: true })) throw closedError();
    const protection = await fixed('get_workspace_protection_state', {}, token, decodeProtection);
    if (!sameClose(protection.close, c0)) throw closedError();
    status = 'protection';
    layout = protection.layout;
    pending = false;
    publish();
  }
  async function initialize() {
    if (initialized) return initialization;
    initialized = true;
    initialization = (async () => {
      if (!native || disposed) return getState();
      const token = identity;
      try {
        // Existing close observation keeps Browser/Legacy workflows free of E1c control IPC.
        const view = await fixed('get_workspace_close_state', {}, token, decodeClose);
        if (view.storage === 'legacy_plaintext') {
          status = 'legacy';
          publish();
          return getState();
        }
        if (view.state === 'open') {
          const client = await invoke('get_workspace_request_state', {});
          if (!current(token)) throw closedError();
          clientId = decodeClient(client);
          if (!clientId) throw closedError();
          status = 'open';
          publish();
          armOpenObserver();
          return getState();
        }
        if (view.state === 'closing') {
          await adoptClosing(view);
          return getState();
        }
        if (view.state !== 'locked') throw closedError();
        await enterProtection(token, await fixed('get_workspace_control_state', {}, token, decodeControl));
      } catch {
        if (!disposed) closeLocal();
      }
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
      const options = requestClient && workspaceCommands.has(command)
        ? { headers: { 'x-rangoon-workspace-client': requestClient } } : undefined;
      const work = Promise.resolve(invoke(command, args, options));
      void work.catch(() => {});
      check();
      result = await work;
      check();
    } catch (error) {
      failed = true;
      failure = error;
    }
    try {
      if (typeof finish === 'function') finish();
    } catch (error) {
      failed = true;
      failure = error;
    }
    check();
    if (failed) throw failure;
    return result;
  } : undefined;
  return {
    getState, bridge, initialize, isBlocked: blocked,
    openConfirmation() {
      if (disposed || status !== 'open') return false;
      status = 'confirming';
      publish();
      return true;
    },
    cancelConfirmation() {
      if (disposed || status !== 'confirming') return false;
      status = 'open';
      publish();
      armOpenObserver();
      return true;
    },
    async beginProtection() {
      if (!canProtect()) return false;
      const token = identity;
      const action = layout === 'absent' ? 'create_empty' : 'unlock';
      pending = true;
      publish();
      try {
        const observed = await fixed('get_workspace_control_state', {}, token, decodeControl);
        if (observed.outcome !== 'observed' || observed.phase !== 'locked' || observed.action !== null
          || observed.diagnostic !== null || observed.reload || !sameClose(observed.close, c0)) throw closedError();
        const prepared = await fixed(
          'prepare_workspace_control',
          raw({ schemaVersion: 'rangoon.workspace-control-prepare.v1', attemptId: c0.attemptId, epoch: c0.epoch, action }),
          token,
          decodeControl
        );
        if (prepared.outcome !== 'prepared' || prepared.phase !== 'awaiting_confirmation' || prepared.action !== action
          || prepared.diagnostic !== null || prepared.reload || prepared.close.storage !== 'managed_encrypted'
          || prepared.close.state !== 'closing' || prepared.close.attemptId === c0.attemptId
          || prepared.close.epoch === c0.epoch) throw closedError();
        attempt = { attemptId: prepared.close.attemptId, epoch: prepared.close.epoch };
        // Required fresh C1 purge; failure blocks both acknowledgment and native sheet.
        if (!purgeLocal({ fresh: true })) throw closedError();
        const acknowledged = await fixed(
          'acknowledge_workspace_purge', raw({ schemaVersion: 'rangoon.workspace-purge-ack.v1', ...attempt }), token, decodeClose
        );
        if (!sameClose(acknowledged, prepared.close)) throw closedError();
        const opened = await fixed(
          'confirm_workspace_control', raw({ schemaVersion: 'rangoon.workspace-control-cycle.v1', ...attempt }), token, decodeControl
        );
        if (opened.outcome !== 'opened' || opened.phase !== 'open' || opened.action !== null || opened.diagnostic !== null
          || opened.reload !== true || opened.close.storage !== 'managed_encrypted' || opened.close.state !== 'open') throw closedError();
        identity = Symbol('reloading-workspace');
        clientId = null;
        status = 'reloading';
        layout = null;
        pending = false;
        publish();
        reload();
        return true;
      } catch {
        if (current(token)) closeLocal();
        return false;
      }
    },
    async confirmClose() {
      if (disposed || status !== 'confirming') return false;
      clientId = null;
      identity = Symbol('closed-workspace');
      status = 'closing';
      const token = identity;
      let begin;
      try {
        begin = Promise.resolve(invoke('begin_workspace_close', {}));
      } catch {
        begin = Promise.reject(closedError());
      }
      const response = begin.then(value => ({ value }), () => ({ failed: true }));
      purgeLocal({ fresh: true });
      publish();
      const outcome = await response;
      if (!current(token)) return false;
      try {
        if (outcome.failed || !cleanupHealthy) throw closedError();
        const view = decodeClose(outcome.value);
        if (!view || view.state !== 'closing') throw closedError();
        attempt = { attemptId: view.attemptId, epoch: view.epoch };
        const acknowledged = await fixed(
          'acknowledge_workspace_purge', raw({ schemaVersion: 'rangoon.workspace-purge-ack.v1', ...attempt }), token, decodeClose
        );
        if (acknowledged.attemptId !== view.attemptId || acknowledged.epoch !== view.epoch
          || !['closing', 'locked'].includes(acknowledged.state)) throw closedError();
        status = acknowledged.state;
        publish();
        if (status === 'closing') armClosePoll();
        else await refreshProtection(token);
        return true;
      } catch {
        if (current(token)) closeLocal();
        return false;
      }
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      clientId = null;
      identity = Symbol('closed-workspace');
      clearTimer();
      status = 'recovery_required';
      layout = null;
      pending = false;
      purgeLocal({ fresh: true });
      publish();
    },
  };
}
