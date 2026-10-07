const object = value => Boolean(value) && typeof value === 'object' && !Array.isArray(value);
const hasOnly = (value, keys) => object(value) && Object.keys(value).length === keys.size && Object.keys(value).every(key => keys.has(key));
const hex = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const recordId = (value, prefix) => typeof value === 'string' && value.startsWith(prefix) && hex(value.slice(prefix.length));
const whole = value => Number.isSafeInteger(value) && value >= 0;
const utf8 = value => new TextEncoder().encode(value).length;
const clone = value => value === null ? null : structuredClone(value);
const failure = result => ({
  code: typeof result?.error?.code === 'string' ? result.error.code : null,
  message: typeof result?.error?.message === 'string' ? result.error.message : 'The local workspace action could not finish.',
});
const failed = () => ({ outcome: 'failed', error: { message: 'The local workspace action could not finish.' } });

const COUNT_KEYS = Object.freeze([
  'sources', 'capabilities', 'revisions', 'reviews', 'recipes',
  'applications', 'derivations', 'workflows', 'workflowRevisions',
]);
const emptyCounts = value => COUNT_KEYS.every(key => value[key] === 0);
const uniqueSorted = (items, key) => items.every((item, index) => index === 0 || items[index - 1][key] < item[key]);

function sourceLabel(value) {
  if (typeof value !== 'string' || !value || utf8(value) > 255 || value === '.' || value === '..') return false;
  if (/[\/\\:\p{Cc}]/u.test(value) || !value.toLowerCase().endsWith('.md')) return false;
  return value.slice(0, -3).length > 0;
}

function safeLabel(value) {
  return typeof value === 'string'
    && value.length > 0
    && utf8(value) <= 160
    && value.trim() === value
    && !/[\p{Cc}\u2028\u2029]/u.test(value);
}

function snapshot(value) {
  return hasOnly(value, new Set(['sourceId', 'displayName', 'sha256', 'byteLength', 'savedAtMs']))
    && recordId(value.sourceId, 'source:')
    && sourceLabel(value.displayName)
    && hex(value.sha256)
    && whole(value.byteLength)
    && value.byteLength <= 256 * 1024
    && whole(value.savedAtMs)
    && value.savedAtMs <= 8_640_000_000_000_000;
}

function validCounts(value) {
  return hasOnly(value, new Set(COUNT_KEYS))
    && COUNT_KEYS.every(key => whole(value[key]))
    && value.sources <= 128
    && value.capabilities <= 128
    && value.revisions <= 1024
    && value.workflows <= 128
    && value.workflowRevisions <= 1024
    && value.reviews <= value.revisions
    && value.recipes <= 128
    && value.applications <= 128
    && value.applications <= value.derivations
    && value.derivations <= value.revisions
    && value.recipes <= value.applications;
}

function origin(value) {
  if (!object(value) || typeof value.kind !== 'string') return false;
  if (value.kind === 'source') {
    return hasOnly(value, new Set(['kind', 'sourceId', 'fragmentId']))
      && recordId(value.sourceId, 'source:')
      && recordId(value.fragmentId, 'fragment:');
  }
  return value.kind === 'composition'
    && hasOnly(value, new Set(['kind', 'compositionId', 'operation', 'outputIndex']))
    && recordId(value.compositionId, 'composition:')
    && ['decompose', 'merge', 'split'].includes(value.operation)
    && whole(value.outputIndex)
    && value.outputIndex < 16
    && (value.operation !== 'merge' || value.outputIndex === 0);
}

function capabilitySummary(value) {
  return hasOnly(value, new Set(['id', 'origin', 'latestRevisionId', 'title', 'reviewed', 'revisionCount']))
    && recordId(value.id, 'capability:')
    && origin(value.origin)
    && recordId(value.latestRevisionId, 'revision:')
    && safeLabel(value.title)
    && typeof value.reviewed === 'boolean'
    && whole(value.revisionCount)
    && value.revisionCount >= 1
    && value.revisionCount <= 32;
}

function workflowSummary(value) {
  return hasOnly(value, new Set([
    'id', 'latestRevisionId', 'label', 'labelAdjusted', 'intent',
    'structurallyValid', 'revisionCount', 'savedAtMs', 'unresolvedReferences',
  ]))
    && recordId(value.id, 'workflow:')
    && recordId(value.latestRevisionId, 'workflow-revision:')
    && safeLabel(value.label)
    && typeof value.labelAdjusted === 'boolean'
    && ['draft', 'validated'].includes(value.intent)
    && typeof value.structurallyValid === 'boolean'
    && (value.intent !== 'validated' || (value.structurallyValid && value.unresolvedReferences === 0))
    && whole(value.revisionCount)
    && value.revisionCount >= 1
    && value.revisionCount <= 32
    && whole(value.savedAtMs)
    && value.savedAtMs <= 8_640_000_000_000_000
    && whole(value.unresolvedReferences)
    && value.unresolvedReferences <= 128;
}

export function validateWorkspaceData(value) {
  if (!hasOnly(value, new Set([
    'schemaVersion', 'stateId', 'records', 'databaseBytes', 'reusableBytes',
    'sources', 'capabilities', 'workflows',
  ]))
    || value.schemaVersion !== 'rangoon.workspace-data.v2'
    || !recordId(value.stateId, 'workspace:')
    || !validCounts(value.records)
    || !whole(value.databaseBytes)
    || !whole(value.reusableBytes)
    || value.reusableBytes > value.databaseBytes
    || value.databaseBytes > 64 * 1024 * 1024
    || !Array.isArray(value.sources)
    || !Array.isArray(value.capabilities)
    || !Array.isArray(value.workflows)
    || value.sources.length !== value.records.sources
    || value.capabilities.length !== value.records.capabilities
    || value.workflows.length !== value.records.workflows
    || !value.sources.every(snapshot)
    || !value.capabilities.every(capabilitySummary)
    || !value.workflows.every(workflowSummary)) return null;

  const sourceIds = new Set(value.sources.map(item => item.sourceId));
  const capabilityIds = new Set(value.capabilities.map(item => item.id));
  const workflowIds = new Set(value.workflows.map(item => item.id));
  const capabilityRevisions = value.capabilities.reduce((sum, item) => sum + item.revisionCount, 0);
  const workflowRevisions = value.workflows.reduce((sum, item) => sum + item.revisionCount, 0);
  if (sourceIds.size !== value.sources.length
    || capabilityIds.size !== value.capabilities.length
    || workflowIds.size !== value.workflows.length
    || capabilityRevisions !== value.records.revisions
    || workflowRevisions !== value.records.workflowRevisions
    || !uniqueSorted(value.workflows, 'id')
    || !value.capabilities.every(item => item.origin.kind !== 'source' || sourceIds.has(item.origin.sourceId))) return null;
  return value;
}

function restorePlan(value) {
  if (!hasOnly(value, new Set([
    'schemaVersion', 'backupId', 'expectedStateId', 'byteLength', 'add',
    'keptSources', 'keptCapabilities', 'keptWorkflows',
  ]))
    || value.schemaVersion !== 'rangoon.restore-plan.v2'
    || !recordId(value.backupId, 'backup:')
    || !recordId(value.expectedStateId, 'workspace:')
    || !whole(value.byteLength)
    || value.byteLength > 68 * 1024 * 1024
    || !validCounts(value.add)
    || !whole(value.keptSources)
    || !whole(value.keptCapabilities)
    || !whole(value.keptWorkflows)) return null;
  if (value.keptSources > 128
    || value.keptCapabilities > 128
    || value.keptWorkflows > 128
    || value.add.sources + value.keptSources > 128
    || value.add.capabilities + value.keptCapabilities > 128
    || value.add.workflows + value.keptWorkflows > 128
    || (value.add.capabilities === 0 && value.add.revisions !== 0)
    || value.add.revisions < value.add.capabilities
    || value.add.revisions > value.add.capabilities * 32
    || (value.add.workflows === 0 && value.add.workflowRevisions !== 0)
    || value.add.workflowRevisions < value.add.workflows
    || value.add.workflowRevisions > value.add.workflows * 32) return null;
  return value;
}

function deletionPlan(value) {
  if (!hasOnly(value, new Set([
    'schemaVersion', 'kind', 'id', 'label', 'expectedStateId', 'remove',
    'capabilityDependencies', 'workflowDependencies',
  ]))
    || !['source', 'capability', 'workflow'].includes(value.kind)
    || !recordId(value.id, value.kind + ':')
    || value.schemaVersion !== 'rangoon.deletion-plan.v2'
    || !(value.kind === 'source' ? sourceLabel(value.label) : safeLabel(value.label))
    || !recordId(value.expectedStateId, 'workspace:')
    || !validCounts(value.remove)
    || !Array.isArray(value.capabilityDependencies)
    || !Array.isArray(value.workflowDependencies)
    || value.capabilityDependencies.length > 128
    || value.workflowDependencies.length > 128
    || !value.capabilityDependencies.every(capabilitySummary)
    || !value.workflowDependencies.every(workflowSummary)
    || !uniqueSorted(value.capabilityDependencies, 'id')
    || !uniqueSorted(value.workflowDependencies, 'id')) return null;

  const remove = value.remove;
  if (value.kind === 'source' && (remove.sources !== 1 || !COUNT_KEYS.slice(1).every(key => remove[key] === 0))) return null;
  if (value.kind === 'capability' && (remove.sources !== 0 || remove.capabilities !== 1 || remove.revisions < 1 || remove.revisions > 32 || remove.workflows !== 0 || remove.workflowRevisions !== 0)) return null;
  if (value.kind === 'workflow' && (remove.workflows !== 1 || remove.workflowRevisions < 1 || remove.workflowRevisions > 32 || !COUNT_KEYS.slice(0, 7).every(key => remove[key] === 0))) return null;
  return value;
}

export const EMPTY_WORKSPACE_STATE = Object.freeze({
  bridgeAvailable: false,
  status: 'idle',
  workspace: null,
  pending: null,
  error: null,
  dialog: null,
  restorePlan: null,
  deletionPlan: null,
  message: 'Open Rangoon desktop to inspect saved local data.',
  requestId: 0,
});

export function createWorkspaceController({ invoke, onChange = () => {}, onMutation = () => {}, getDraftWarning = () => null } = {}) {
  const bridgeAvailable = typeof invoke === 'function';
  let state = {
    ...EMPTY_WORKSPACE_STATE,
    bridgeAvailable,
    status: bridgeAvailable ? 'idle' : 'unavailable',
    message: bridgeAvailable ? 'Load local data to review saved records.' : EMPTY_WORKSPACE_STATE.message,
  };
  const publish = () => onChange({
    ...state,
    workspace: clone(state.workspace),
    restorePlan: clone(state.restorePlan),
    deletionPlan: clone(state.deletionPlan),
    dialog: clone(state.dialog),
    error: clone(state.error),
  });
  const set = changes => {
    state = { ...state, ...changes };
    publish();
  };
  const call = async (command, args) => {
    try {
      return await invoke(command, args);
    } catch {
      return failed();
    }
  };
  const setError = result => {
    const error = failure(result);
    set({
      status: state.workspace ? 'ready' : 'failed',
      pending: null,
      error,
      dialog: state.dialog ? { ...state.dialog, stale: true } : null,
      message: error.message,
    });
  };
  const readyFor = action => Boolean(bridgeAvailable && !state.pending && action);
  const sameRestorePlan = (left, right) => Boolean(left && right
    && ['schemaVersion', 'backupId', 'expectedStateId', 'byteLength', 'keptSources', 'keptCapabilities', 'keptWorkflows']
      .every(key => left[key] === right[key])
    && COUNT_KEYS.every(key => left.add[key] === right.add[key]));

  const controller = {
    getState: () => ({
      ...state,
      workspace: clone(state.workspace),
      restorePlan: clone(state.restorePlan),
      deletionPlan: clone(state.deletionPlan),
      dialog: clone(state.dialog),
      error: clone(state.error),
    }),
    async load({ preserveMessage = false, allowPending = false } = {}) {
      if (!bridgeAvailable || (state.pending && !allowPending)) return;
      const requestId = state.requestId + 1;
      set({
        status: state.workspace ? 'ready' : 'loading',
        pending: allowPending ? 'refresh' : 'load',
        error: preserveMessage ? state.error : null,
        requestId,
        message: preserveMessage ? state.message : 'Loading saved local data…',
      });
      const result = await call('get_workspace_data');
      if (state.requestId !== requestId) return;
      const workspace = result?.outcome === 'loaded' ? validateWorkspaceData(result.workspace) : null;
      if (!workspace) {
        if (!preserveMessage) return setError(result);
        const error = failure(result);
        return set({ status: 'ready', pending: null, error, message: state.message + ' Inventory refresh failed. ' + error.message });
      }
      const hasRecords = workspace.records.sources || workspace.records.capabilities || workspace.records.workflows;
      set({
        status: 'ready',
        pending: null,
        workspace,
        error: null,
        message: preserveMessage ? state.message : (hasRecords ? 'Saved local data is ready to inspect.' : 'No local records have been saved yet.'),
      });
    },
    requestExport() {
      if (!readyFor('export') || !state.workspace) return;
      set({ dialog: { kind: 'export-warning' }, error: null, message: 'Backups include unencrypted saved source content.' });
    },
    cancel() {
      if (state.pending) return;
      set({
        dialog: null,
        message: state.dialog?.kind === 'export-warning'
          ? 'Backup export cancelled. Saved records are unchanged.'
          : 'Confirmation cancelled. Saved records are unchanged.',
      });
    },
    async confirmExport() {
      if (!readyFor('export') || state.dialog?.kind !== 'export-warning') return;
      const requestId = state.requestId + 1;
      set({ pending: 'export', dialog: null, error: null, requestId, message: 'Choose a new backup destination…' });
      const result = await call('export_workspace_backup');
      if (state.requestId !== requestId) return;
      if (result?.outcome === 'cancelled') return set({ status: 'ready', pending: null, message: 'Backup export cancelled. Saved records are unchanged.' });
      if (!hasOnly(result, new Set(['outcome', 'backupId', 'byteLength']))
        || result?.outcome !== 'exported'
        || !recordId(result.backupId, 'backup:')
        || !whole(result.byteLength)
        || result.byteLength > 68 * 1024 * 1024) return setError(result);
      set({ status: 'ready', pending: null, error: null, message: 'Backup exported (' + result.byteLength.toLocaleString() + ' bytes).' });
    },
    async prepareRestore() {
      if (!readyFor('restore-preview')) return;
      const requestId = state.requestId + 1;
      set({ pending: 'restore-preview', error: null, requestId, message: 'Choose one Rangoon backup to inspect…' });
      const result = await call('prepare_workspace_restore');
      if (state.requestId !== requestId) return;
      if (result?.outcome === 'cancelled') {
        return set({
          status: state.workspace ? 'ready' : 'idle',
          pending: null,
          dialog: state.restorePlan ? { kind: 'restore' } : null,
          message: state.restorePlan ? 'Backup selection cancelled. Existing restore preview remains available.' : 'Backup selection cancelled.',
        });
      }
      const plan = hasOnly(result, new Set(['outcome', 'plan'])) && result?.outcome === 'restore_ready' ? restorePlan(result.plan) : null;
      if (!plan) return setError(result);
      set({ status: 'ready', pending: null, restorePlan: plan, dialog: { kind: 'restore' }, error: null, message: 'Restore preview ready. Review additions before restoring.' });
    },
    async confirmRestore() {
      const plan = state.restorePlan;
      if (!readyFor('restore') || state.dialog?.kind !== 'restore' || !plan || emptyCounts(plan.add)) return;
      const requestId = state.requestId + 1;
      set({ pending: 'restore', dialog: { kind: 'pending', action: 'restore' }, restorePlan: null, error: null, requestId, message: 'Restoring missing local records…' });
      const result = await call('restore_workspace_backup', { backupId: plan.backupId, expectedStateId: plan.expectedStateId });
      if (state.requestId !== requestId) return;
      const receipt = hasOnly(result, new Set(['outcome', 'plan'])) && result?.outcome === 'restored' ? restorePlan(result.plan) : null;
      if (!receipt || !sameRestorePlan(receipt, plan)) return setError(result);
      set({ status: 'ready', pending: 'refresh', dialog: null, error: null, message: 'Restore completed. Refreshing saved local data…' });
      let callbackFailed = false;
      try {
        await onMutation({ kind: 'restore' });
      } catch {
        callbackFailed = true;
      }
      await controller.load({ preserveMessage: true, allowPending: true });
      if (callbackFailed && !state.error) {
        set({ error: { code: 'workspace_refresh_failed', message: 'Local data changed, but related local views could not refresh.' }, message: 'Restore completed. Related local views could not refresh.' });
      } else if (!state.error) {
        set({ message: 'Restore completed. Local data refreshed.' });
      }
    },
    async inspectDeletion(kind, id, deletionIntent = false) {
      if (!readyFor('delete-preview') || !['source', 'capability', 'workflow'].includes(kind) || !recordId(id, kind + ':')) return;
      const requestId = state.requestId + 1;
      set({ pending: 'delete-preview', error: null, requestId, message: 'Inspecting deletion impact…' });
      const result = await call('inspect_workspace_deletion', { kind, id });
      if (state.requestId !== requestId) return;
      const plan = hasOnly(result, new Set(['outcome', 'plan'])) && result?.outcome === 'deletion_ready' ? deletionPlan(result.plan) : null;
      if (!plan || plan.kind !== kind || plan.id !== id) return setError(result);
      const warning = kind === 'capability' ? getDraftWarning(kind, id) : null;
      const blockers = plan.capabilityDependencies.length + plan.workflowDependencies.length;
      set({
        status: 'ready',
        pending: null,
        deletionPlan: plan,
        dialog: {
          kind: 'delete',
          mode: deletionIntent ? 'confirm' : 'inspect',
          draftWarning: typeof warning === 'string' ? warning : warning ? 'This skill has unsaved editor changes.' : null,
        },
        error: null,
        message: blockers ? 'This record has saved dependencies.' : deletionIntent ? 'Deletion preview ready. Review impact before deleting.' : 'Deletion impact inspected. Saved records are unchanged.',
      });
    },
    async confirmDeletion() {
      const plan = state.deletionPlan;
      const blocked = plan && (plan.capabilityDependencies.length || plan.workflowDependencies.length);
      if (!readyFor('delete') || state.dialog?.kind !== 'delete' || state.dialog?.mode !== 'confirm' || !plan || blocked) return;
      const requestId = state.requestId + 1;
      set({ pending: 'delete', dialog: { kind: 'pending', action: 'delete' }, deletionPlan: null, error: null, requestId, message: 'Removing the selected local record…' });
      const result = await call('delete_workspace_record', { kind: plan.kind, id: plan.id, expectedStateId: plan.expectedStateId });
      if (state.requestId !== requestId) return;
      if (!hasOnly(result, new Set(['outcome', 'kind', 'id']))
        || result?.outcome !== 'deleted'
        || result.kind !== plan.kind
        || result.id !== plan.id) return setError(result);
      set({ status: 'ready', pending: 'refresh', dialog: null, error: null, message: 'Local record removed. Refreshing saved data…' });
      let callbackFailed = false;
      try {
        await onMutation({ kind: 'delete', recordKind: plan.kind, id: plan.id });
      } catch {
        callbackFailed = true;
      }
      await controller.load({ preserveMessage: true, allowPending: true });
      if (callbackFailed && !state.error) {
        set({ error: { code: 'workspace_refresh_failed', message: 'Local data changed, but related local views could not refresh.' }, message: 'Local record removed. Related local views could not refresh.' });
      } else if (!state.error) {
        set({ message: 'Local record removed. Local data refreshed.' });
      }
    },
  };
  return controller;
}
