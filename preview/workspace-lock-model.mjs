// This cooperative renderer fence complements native custody; it never grants authority.
const lifecycle = new Set(['get_workspace_close_state', 'begin_workspace_close', 'acknowledge_workspace_purge']);
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

export function createWorkspaceLockController({ invoke, beforeInvoke = () => {}, purge = () => {}, onChange = () => {}, schedule = setTimeout, cancelSchedule = clearTimeout, pollInterval = 250 } = {}) {
  const native = typeof invoke === 'function';
  let status = native ? 'checking' : 'browser';
  let identity = Symbol('workspace');
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
        else if (view.state === 'open') { status = 'open'; publish(); }
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
    let finish;
    let result;
    let failure;
    let failed = false;
    try {
      finish = beforeInvoke(command, args);
      if (finish?.then) throw closedError();
      check();
      // Await even a synchronously reentrant transport so rejected old work is consumed.
      const pending = Promise.resolve(invoke(command, args));
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
      identity = Symbol('closed-workspace');
      clearTimer();
      status = 'recovery_required';
      purgeLocal();
      publish();
    },
  };
}
