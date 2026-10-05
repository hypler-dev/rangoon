import { validateCapabilitySummary } from './skills-model.mjs';

const object = value => Boolean(value) && typeof value === 'object' && !Array.isArray(value);
const hasOnly = (value, keys) => object(value) && Object.keys(value).every(key => keys.has(key));
const hex = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const recordId = (value, prefix) => typeof value === 'string' && value.startsWith(prefix) && hex(value.slice(prefix.length));
const whole = value => Number.isSafeInteger(value) && value >= 0;
const text = value => typeof value === 'string' && value.length <= 512;
const failure = result => ({ code: typeof result?.error?.code === 'string' ? result.error.code : null, message: typeof result?.error?.message === 'string' ? result.error.message : 'The local workspace action could not finish.' });
const failed = () => ({ outcome: 'failed', error: { message: 'The local workspace action could not finish.' } });
const clone = value => value === null ? null : structuredClone(value);

function snapshot(value) {
  return hasOnly(value, new Set(['sourceId', 'displayName', 'sha256', 'byteLength', 'savedAtMs']))
    && recordId(value.sourceId, 'source:') && text(value.displayName) && hex(value.sha256)
    && whole(value.byteLength) && value.byteLength <= 256 * 1024 && whole(value.savedAtMs) && value.savedAtMs <= 8_640_000_000_000_000;
}

function usage(value) {
  return hasOnly(value, new Set(['sources', 'capabilities', 'revisions', 'reviews', 'databaseBytes', 'reusableBytes']))
    && ['sources', 'capabilities', 'revisions', 'reviews', 'databaseBytes', 'reusableBytes'].every(key => whole(value[key]))
    && value.sources <= 128 && value.capabilities <= 128 && value.revisions <= 1024 && value.reviews <= value.revisions
    && value.reusableBytes <= value.databaseBytes && value.databaseBytes <= 64 * 1024 * 1024;
}

export function validateWorkspaceData(value) {
  if (!hasOnly(value, new Set(['schemaVersion', 'stateId', 'usage', 'sources', 'capabilities']))
    || value.schemaVersion !== 'rangoon.workspace-data.v0' || !recordId(value.stateId, 'workspace:')
    || !usage(value.usage) || !Array.isArray(value.sources) || !Array.isArray(value.capabilities)
    || value.sources.length !== value.usage.sources || value.capabilities.length !== value.usage.capabilities
    || !value.sources.every(snapshot) || !value.capabilities.every(validateCapabilitySummary)) return null;
  const sourceIds = new Set(value.sources.map(item => item.sourceId));
  const capabilityIds = new Set(value.capabilities.map(item => item.id));
  const revisions = value.capabilities.reduce((sum, item) => sum + item.revisionCount, 0);
  if (sourceIds.size !== value.sources.length || capabilityIds.size !== value.capabilities.length || revisions !== value.usage.revisions) return null;
  if (!value.capabilities.every(item => sourceIds.has(item.sourceId))) return null;
  return value;
}

function restorePlan(value) {
  if (!hasOnly(value, new Set(['backupId', 'expectedStateId', 'byteLength', 'addSources', 'keptSources', 'addCapabilities', 'keptCapabilities', 'addRevisions', 'addReviews']))
    || !recordId(value.backupId, 'backup:') || !recordId(value.expectedStateId, 'workspace:')) return null;
  if (!['byteLength', 'addSources', 'keptSources', 'addCapabilities', 'keptCapabilities', 'addRevisions', 'addReviews'].every(key => whole(value[key]))) return null;
  if (value.byteLength > 68 * 1024 * 1024 || value.addSources > 128 || value.addCapabilities > 128 || value.addRevisions > 1024 || value.addReviews > value.addRevisions) return null;
  if (value.keptSources > 128 || value.keptCapabilities > 128 || value.addSources + value.keptSources > 128 || value.addCapabilities + value.keptCapabilities > 128) return null;
  if ((value.addCapabilities === 0 && value.addRevisions !== 0) || value.addRevisions < value.addCapabilities || value.addRevisions > value.addCapabilities * 32) return null;
  return value;
}

function deletionPlan(value) {
  if (!hasOnly(value, new Set(['kind', 'id', 'title', 'expectedStateId', 'revisions', 'reviews', 'dependencies']))
    || !['source', 'capability'].includes(value.kind) || !recordId(value.id, `${value.kind}:`)
    || !text(value.title) || !recordId(value.expectedStateId, 'workspace:') || !whole(value.revisions) || !whole(value.reviews)
    || !Array.isArray(value.dependencies) || value.dependencies.length > 128 || !value.dependencies.every(validateCapabilitySummary)) return null;
  const ids = new Set(value.dependencies.map(item => item.id));
  if (value.revisions > 32 || value.reviews > value.revisions || ids.size !== value.dependencies.length || (value.kind === 'capability' && (value.dependencies.length || value.revisions === 0))) return null;
  if (value.kind === 'source' && (value.revisions || value.reviews || !value.dependencies.every(item => item.sourceId === value.id))) return null;
  return value;
}

export const EMPTY_WORKSPACE_STATE = Object.freeze({
  bridgeAvailable: false, status: 'idle', workspace: null, pending: null, error: null,
  dialog: null, restorePlan: null, deletionPlan: null, message: 'Open Rangoon desktop to inspect saved local data.', requestId: 0,
});

export function createWorkspaceController({ invoke, onChange = () => {}, onMutation = () => {}, getDraftWarning = () => null } = {}) {
  const bridgeAvailable = typeof invoke === 'function';
  let state = { ...EMPTY_WORKSPACE_STATE, bridgeAvailable, status: bridgeAvailable ? 'idle' : 'unavailable', message: bridgeAvailable ? 'Load local data to review saved records.' : EMPTY_WORKSPACE_STATE.message };
  const publish = () => onChange({ ...state, workspace: clone(state.workspace), restorePlan: clone(state.restorePlan), deletionPlan: clone(state.deletionPlan), dialog: clone(state.dialog), error: clone(state.error) });
  const set = changes => { state = { ...state, ...changes }; publish(); };
  const unavailable = () => !bridgeAvailable;
  const call = async (command, args) => { try { return await invoke(command, args); } catch { return failed(); } };
  const begin = action => !unavailable() && !state.pending && action;
  const setError = result => {
    const error = failure(result);
    const dialog = state.dialog ? { ...state.dialog, stale: true } : null;
    set({ status: state.workspace ? 'ready' : 'failed', pending: null, error, dialog, message: error.message });
  };
  const freshWorkspace = result => result?.outcome === 'loaded' ? validateWorkspaceData(result.workspace) : null;
  const currentPlan = plan => state.restorePlan && plan?.backupId === state.restorePlan.backupId && plan?.expectedStateId === state.restorePlan.expectedStateId;
  const sameRestorePlan = (left, right) => left && right && ['backupId', 'expectedStateId', 'byteLength', 'addSources', 'keptSources', 'addCapabilities', 'keptCapabilities', 'addRevisions', 'addReviews'].every(key => left[key] === right[key]);

  const controller = {
    getState: () => ({ ...state, workspace: clone(state.workspace), restorePlan: clone(state.restorePlan), deletionPlan: clone(state.deletionPlan), dialog: clone(state.dialog), error: clone(state.error) }),
    async load({ preserveMessage = false, allowPending = false } = {}) {
      if (unavailable() || (state.pending && !allowPending)) return;
      const requestId = state.requestId + 1;
      set({ status: state.workspace ? 'ready' : 'loading', pending: allowPending ? 'refresh' : 'load', error: preserveMessage ? state.error : null, requestId, message: preserveMessage ? state.message : 'Loading saved local data…' });
      const result = await call('get_workspace_data');
      if (state.requestId !== requestId) return;
      const workspace = freshWorkspace(result);
      if (!workspace) {
        if (!preserveMessage) return setError(result);
        const error = failure(result);
        return set({ status: 'ready', pending: null, error, message: `${state.message} Inventory refresh failed. ${error.message}` });
      }
      set({ status: 'ready', pending: null, workspace, error: null, message: preserveMessage ? state.message : (workspace.usage.sources || workspace.usage.capabilities ? 'Saved local data is ready to inspect.' : 'No local records have been saved yet.') });
    },
    requestExport() {
      if (!begin('export') || !state.workspace) return;
      set({ dialog: { kind: 'export-warning' }, error: null, message: 'Backups include unencrypted saved source content.' });
    },
    cancel() {
      if (state.pending) return;
      if (state.dialog?.kind === 'restore' || state.dialog?.kind === 'delete') set({ dialog: null, message: 'Confirmation cancelled. Saved records are unchanged.' });
      else set({ dialog: null, message: 'Backup export cancelled. Saved records are unchanged.' });
    },
    async confirmExport() {
      if (!begin('export') || state.dialog?.kind !== 'export-warning') return;
      const requestId = state.requestId + 1;
      set({ pending: 'export', dialog: null, error: null, requestId, message: 'Choose a new backup destination…' });
      const result = await call('export_workspace_backup');
      if (state.requestId !== requestId) return;
      if (result?.outcome === 'cancelled') return set({ status: 'ready', pending: null, message: 'Backup export cancelled. Saved records are unchanged.' });
      if (!hasOnly(result, new Set(['outcome', 'backupId', 'byteLength'])) || result?.outcome !== 'exported' || !recordId(result.backupId, 'backup:') || !whole(result.byteLength) || result.byteLength > 68 * 1024 * 1024) return setError(result);
      set({ status: 'ready', pending: null, error: null, message: `Backup exported (${result.byteLength.toLocaleString()} bytes).` });
    },
    async prepareRestore() {
      if (!begin('restore-preview')) return;
      const requestId = state.requestId + 1;
      set({ pending: 'restore-preview', error: null, requestId, message: 'Choose one Rangoon backup to inspect…' });
      const result = await call('prepare_workspace_restore');
      if (state.requestId !== requestId) return;
      if (result?.outcome === 'cancelled') return set({ status: state.workspace ? 'ready' : 'idle', pending: null, dialog: state.restorePlan ? { kind: 'restore' } : null, message: state.restorePlan ? 'Backup selection cancelled. Existing restore preview remains available.' : 'Backup selection cancelled.' });
      const plan = hasOnly(result, new Set(['outcome', 'plan'])) && result?.outcome === 'restore_ready' ? restorePlan(result.plan) : null;
      if (!plan) return setError(result);
      set({ status: 'ready', pending: null, restorePlan: plan, dialog: { kind: 'restore' }, error: null, message: 'Restore preview ready. Review additions before restoring.' });
    },
    async confirmRestore() {
      const plan = state.restorePlan;
      if (!begin('restore') || state.dialog?.kind !== 'restore' || !plan || (!plan.addSources && !plan.addCapabilities)) return;
      const requestId = state.requestId + 1;
      set({ pending: 'restore', error: null, requestId, message: 'Restoring missing local records…' });
      const result = await call('restore_workspace_backup', { backupId: plan.backupId, expectedStateId: plan.expectedStateId });
      if (state.requestId !== requestId) return;
      const receipt = hasOnly(result, new Set(['outcome', 'plan'])) && result?.outcome === 'restored' ? restorePlan(result.plan) : null;
      if (!receipt || !currentPlan(receipt) || !sameRestorePlan(receipt, plan)) return setError(result);
      set({ status: 'ready', pending: 'refresh', dialog: null, restorePlan: null, error: null, message: 'Restore completed. Refreshing saved local data…' });
      let callbackFailed = false;
      try { await onMutation({ kind: 'restore' }); } catch { callbackFailed = true; }
      await controller.load({ preserveMessage: true, allowPending: true });
      if (callbackFailed && !state.error) set({ error: { code: 'workspace_refresh_failed', message: 'Local data changed, but related local views could not refresh.' }, message: 'Restore completed. Related local views could not refresh.' });
      else if (!state.error) set({ message: 'Restore completed. Local data refreshed.' });
    },
    async inspectDeletion(kind, id, deletionIntent = false) {
      if (!begin('delete-preview') || !['source', 'capability'].includes(kind) || !recordId(id, `${kind}:`)) return;
      const requestId = state.requestId + 1;
      set({ pending: 'delete-preview', error: null, requestId, message: 'Inspecting deletion impact…' });
      const result = await call('inspect_workspace_deletion', { kind, id });
      if (state.requestId !== requestId) return;
      const plan = hasOnly(result, new Set(['outcome', 'plan'])) && result?.outcome === 'deletion_ready' ? deletionPlan(result.plan) : null;
      if (!plan || plan.kind !== kind || plan.id !== id) return setError(result);
      const warning = kind === 'capability' ? getDraftWarning(kind, id) : null;
      set({ status: 'ready', pending: null, deletionPlan: plan, dialog: { kind: 'delete', mode: deletionIntent ? 'confirm' : 'inspect', draftWarning: typeof warning === 'string' ? warning : warning ? 'This skill has unsaved editor changes.' : null }, error: null, message: plan.dependencies.length ? 'This source is still referenced by saved skills.' : deletionIntent ? 'Deletion preview ready. Review impact before deleting.' : 'Deletion impact inspected. Saved records are unchanged.' });
    },
    async confirmDeletion() {
      const plan = state.deletionPlan;
      if (!begin('delete') || state.dialog?.kind !== 'delete' || state.dialog?.mode !== 'confirm' || !plan || plan.dependencies.length) return;
      const requestId = state.requestId + 1;
      set({ pending: 'delete', error: null, requestId, message: 'Removing the selected local record…' });
      const result = await call('delete_workspace_record', { kind: plan.kind, id: plan.id, expectedStateId: plan.expectedStateId });
      if (state.requestId !== requestId) return;
      if (!hasOnly(result, new Set(['outcome', 'kind', 'id'])) || result?.outcome !== 'deleted' || result.kind !== plan.kind || result.id !== plan.id) return setError(result);
      set({ status: 'ready', pending: 'refresh', dialog: null, deletionPlan: null, error: null, message: 'Local record removed. Refreshing saved data…' });
      let callbackFailed = false;
      try { await onMutation({ kind: 'delete', recordKind: plan.kind, id: plan.id }); } catch { callbackFailed = true; }
      await controller.load({ preserveMessage: true, allowPending: true });
      if (callbackFailed && !state.error) set({ error: { code: 'workspace_refresh_failed', message: 'Local data changed, but related local views could not refresh.' }, message: 'Local record removed. Related local views could not refresh.' });
      else if (!state.error) set({ message: 'Local record removed. Local data refreshed.' });
    },
  };
  return controller;
}
