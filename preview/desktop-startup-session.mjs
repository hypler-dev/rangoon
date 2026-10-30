import { createDesktopStartupController } from './desktop-startup-model.mjs';
import { decodeWorkspaceCloseState } from './workspace-lock-model.mjs';

const encoder = new TextEncoder();
const interval = value => Math.max(10, Math.min(60000, Number(value) || 100));
const pollLimit = value => Math.max(1, Math.min(600, Number.isInteger(value) ? value : 300));
const closeKey = close => `${close.attemptId}:${close.epoch}`;
const sameCycle = (left, right) => left?.attemptId === right?.attemptId && left?.epoch === right?.epoch;
const rawAcknowledgment = close => encoder.encode(JSON.stringify({
  schemaVersion: 'rangoon.workspace-purge-ack.v1', attemptId: close.attemptId, epoch: close.epoch,
}));

export function createDesktopStartupSession({
  invoke, purge = () => {}, onChange = () => {}, onHandoff = () => {}, schedule = setTimeout,
  cancelSchedule = clearTimeout, pollInterval = 100, initialPollInterval = pollInterval, initialPollLimit = 300,
} = {}) {
  const native = typeof invoke === 'function';
  const regularDelay = interval(pollInterval);
  const initialDelay = interval(initialPollInterval);
  const maxInitialPolls = pollLimit(initialPollLimit);
  let generation = 0;
  let disposed = false;
  let failed = false;
  let initialized = false;
  let initialization = null;
  let timer = null;
  let lifecycleTail = Promise.resolve();
  let actionPending = null;
  let initialOpen = false;
  let startupStarted = false;
  let pumpFenced = false;
  let handoffStarted = false;
  let callbackStarted = false;
  let handoffPromise = null;
  let newestClose = null;
  const purged = new Set();
  const acknowledged = new Set();

  const controller = createDesktopStartupController({
    invoke,
    onChange: state => { void changed(state); },
  });
  const state = () => controller.getState();
  const active = token => !disposed && !failed && token === generation;
  const publish = () => { if (!disposed) onChange(state()); };
  const clearTimer = () => {
    if (!timer) return;
    cancelSchedule(timer.id);
    timer.resolve?.(false);
    timer = null;
  };
  const closeUnavailable = () => {
    if (disposed || failed) return;
    failed = true;
    generation += 1;
    pumpFenced = true;
    clearTimer();
    controller.dispose();
    publish();
  };
  const wait = (token, delay) => new Promise(resolve => {
    if (!active(token) || pumpFenced) { resolve(false); return; }
    const entry = { id: null, resolve };
    entry.id = schedule(() => {
      if (timer === entry) timer = null;
      resolve(active(token) && !pumpFenced);
    }, delay);
    timer = entry;
  });
  const lifecycle = work => {
    const token = generation;
    const task = lifecycleTail.then(async () => {
      if (!active(token) || pumpFenced) return null;
      return work(token);
    });
    lifecycleTail = task.catch(() => undefined);
    return task;
  };
  async function closeState(token) {
    let response;
    try { response = await invoke('get_workspace_close_state', {}); } catch { return null; }
    if (!active(token) || pumpFenced) return null;
    const close = decodeWorkspaceCloseState(response);
    return close?.storage === 'managed_encrypted' ? close : null;
  }
  async function freshPurge(token, key) {
    let result;
    try { result = await purge(); } catch { return false; }
    if (!active(token) || pumpFenced || result === false) return false;
    purged.add(key);
    return true;
  }
  async function acknowledge(token, close) {
    let response;
    try { response = await invoke('acknowledge_workspace_purge', rawAcknowledgment(close)); } catch { return null; }
    if (!active(token) || pumpFenced) return null;
    const acknowledgment = decodeWorkspaceCloseState(response);
    return acknowledgment && sameCycle(acknowledgment, close) && ['closing', 'locked'].includes(acknowledgment.state)
      ? acknowledgment : null;
  }
  async function observe(token) {
    const close = await closeState(token);
    if (!close || close.state === 'recovery_required') return null;
    if (close.state === 'open') return close;
    const key = closeKey(close);
    if (newestClose && newestClose.key !== key && BigInt(close.epoch) <= newestClose.epoch) return null;
    if (!newestClose || newestClose.key !== key) newestClose = { key, epoch: BigInt(close.epoch) };
    if (close.state === 'locked') {
      if (!purged.has(key) && !await freshPurge(token, key)) return null;
      return close;
    }
    if (close.state !== 'closing') return null;
    if (acknowledged.has(key)) return close;
    if (!await freshPurge(token, key)) return null;
    const acknowledgment = await acknowledge(token, close);
    if (!acknowledgment) return null;
    acknowledged.add(key);
    return acknowledgment;
  }
  function armPump(token) {
    clearTimer();
    if (!active(token) || pumpFenced) return;
    const entry = { id: null };
    entry.id = schedule(async () => {
      if (timer === entry) timer = null;
      if (!active(token) || pumpFenced) return;
      const observed = await lifecycle(observe);
      if (!observed && active(token) && !pumpFenced) closeUnavailable();
      if (active(token) && !pumpFenced) armPump(token);
    }, regularDelay);
    timer = entry;
  }
  async function finishHandoff() {
    if (handoffStarted || disposed || failed) return;
    handoffStarted = true;
    pumpFenced = true;
    generation += 1;
    const token = generation;
    clearTimer();
    await lifecycleTail;
    if (!active(token) || callbackStarted) return;
    callbackStarted = true;
    try { await onHandoff(); } catch { if (active(token)) closeUnavailable(); }
  }
  async function changed(next) {
    if (disposed || failed) return;
    if (startupStarted && next.status === 'unavailable') {
      closeUnavailable();
      return;
    }
    if (next.status === 'handoff') {
      publish();
      handoffPromise ??= finishHandoff();
      await handoffPromise;
      return;
    }
    if (initialOpen && startupStarted && next.status !== 'checking') {
      closeUnavailable();
      return;
    }
    publish();
  }
  async function prepareInitial(token) {
    let observed = await lifecycle(observe);
    if (!observed) return false;
    if (observed.state === 'open') {
      initialOpen = true;
      return true;
    }
    let polls = 0;
    while (observed.state !== 'locked') {
      if (++polls > maxInitialPolls || !await wait(token, initialDelay)) return false;
      observed = await lifecycle(observe);
      if (!observed) return false;
    }
    return true;
  }
  async function initialize() {
    if (initialized) return initialization;
    initialized = true;
    initialization = (async () => {
      if (!native || disposed) return controller.initialize();
      const token = generation;
      if (!await prepareInitial(token) || !active(token)) {
        if (!disposed) closeUnavailable();
        return state();
      }
      startupStarted = true;
      const starting = controller.initialize();
      armPump(token);
      await starting;
      if (handoffPromise) await handoffPromise;
      return state();
    })();
    return initialization;
  }
  async function perform(action) {
    if (disposed || failed || handoffStarted || actionPending) return false;
    const pending = controller.perform(action);
    actionPending = pending;
    try {
      const result = await pending;
      if (handoffPromise) await handoffPromise;
      return result && !failed;
    } finally { if (actionPending === pending) actionPending = null; }
  }
  function dispose() {
    if (disposed) return;
    disposed = true;
    generation += 1;
    pumpFenced = true;
    clearTimer();
    controller.dispose();
  }
  return Object.freeze({ initialize, perform, getState: state, dispose });
}
