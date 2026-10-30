import { escapeText } from './analysis-model.mjs';
import { icon } from './icons.mjs';
import { renderTechnicalDetails } from './technical-details.mjs';

const text = escapeText;
const short = value => String(value ?? '').length > 22 ? `${String(value).slice(0, 10)}…${String(value).slice(-8)}` : String(value ?? 'Unavailable');
const bytes = value => Number(value ?? 0).toLocaleString();
const stamp = value => Number.isSafeInteger(value) ? new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value)) : 'Date unavailable';
const plural = (count, word) => `${count} ${word}${count === 1 ? '' : 's'}`;
const storageModes = new Set(['managed_encrypted', 'legacy_plaintext']);
const storageMode = value => storageModes.has(value) ? value : 'unavailable';

function usage(state) {
  const value = state.workspace?.records;
  if (!value) return '';
  return `<section class="workspace-metrics" aria-label="Local workspace usage">
    <div><span>Sources</span><strong>${value.sources} <small>/ 128</small></strong></div>
    <div><span>Skills</span><strong>${value.capabilities} <small>/ 128</small></strong></div>
    <div><span>Capability revisions</span><strong>${value.revisions} <small>/ 1,024</small></strong></div>
    <div><span>Workflows</span><strong>${value.workflows} <small>/ 128</small></strong></div>
    <div><span>Workflow revisions</span><strong>${value.workflowRevisions} <small>/ 1,024</small></strong></div>
    <div><span>Reviews</span><strong>${value.reviews}</strong></div>
    <div><span>Recipes</span><strong>${value.recipes}</strong></div>
    <div><span>Applications</span><strong>${value.applications}</strong></div>
    <div><span>Derivations</span><strong>${value.derivations}</strong></div>
    <div><span>Database</span><strong>${bytes(state.workspace.databaseBytes)} <small>bytes</small></strong><em>${bytes(state.workspace.reusableBytes)} reusable</em></div>
  </section>`;
}

function originLabel(origin) {
  if (origin.kind === 'source') return 'Source section';
  return `Composition ${origin.operation} · output ${origin.outputIndex + 1}`;
}

function recordIdentityHint(records, record, nameKey, idKey = 'id') {
  const sameName = records.filter(item => item[nameKey] === record[nameKey]);
  if (sameName.length < 2) return '';
  const alias = short(record[idKey]);
  const collision = sameName.some(item => item[idKey] !== record[idKey] && short(item[idKey]) === alias);
  return `Record ${collision ? record[idKey] : alias}`;
}

function deletionIdentityHint(plan, workspace) {
  const definitions = { source: ['sources', 'displayName', 'sourceId'], capability: ['capabilities', 'title', 'id'], workflow: ['workflows', 'label', 'id'] };
  const [collection, nameKey, idKey] = definitions[plan.kind];
  const records = workspace?.[collection] ?? [];
  const record = records.find(item => item[idKey] === plan.id);
  return record ? recordIdentityHint(records, record, nameKey, idKey) : `Record ${short(plan.id)}`;
}

function sourceRows(state, actionsDisabled) {
  const records = state.workspace?.sources ?? [];
  if (!records.length) return '<p class="workspace-empty">No saved sources. Explicitly save analysis before it appears here.</p>';
  return records.map(source => {
    const identity = recordIdentityHint(records, source, 'displayName', 'sourceId');
    return `<article class="workspace-record"><span class="workspace-record__icon">${icon('source', { size: 20 })}</span><div><strong>${text(source.displayName)}</strong>${identity ? `<small class="workspace-record-identity">${text(identity)}</small>` : ''}<small>${bytes(source.byteLength)} bytes · Saved ${text(stamp(source.savedAtMs))}</small>${renderTechnicalDetails(`workspace-source-${source.sourceId}`, `<dl><div><dt>Source ID</dt><dd class="analysis-mono">${text(source.sourceId)}</dd></div><div><dt>SHA-256</dt><dd class="analysis-mono">${text(source.sha256 ?? 'Unavailable')}</dd></div></dl>`)}</div><div class="workspace-record__actions"><button class="analysis-button analysis-button--small" type="button" data-workspace-inspect="source" data-workspace-intent="inspect" data-workspace-id="${text(source.sourceId)}" aria-label="Inspect deletion impact for source ${text(source.displayName)}${identity ? ` (${text(identity)})` : ''}" ${actionsDisabled ? 'disabled' : ''}>Inspect</button><button class="analysis-button analysis-button--small workspace-delete" type="button" data-workspace-inspect="source" data-workspace-intent="delete" data-workspace-id="${text(source.sourceId)}" aria-label="Preview deletion for source ${text(source.displayName)}${identity ? ` (${text(identity)})` : ''}" ${actionsDisabled ? 'disabled' : ''}>${icon('clear', { size: 16 })}<span>Delete</span></button></div></article>`;
  }).join('');
}

function skillRows(state, actionsDisabled) {
  const records = state.workspace?.capabilities ?? [];
  if (!records.length) return '<p class="workspace-empty">No saved skills. Skills stay local until you make them from saved source sections.</p>';
  return records.map(skill => {
    const identity = recordIdentityHint(records, skill, 'title', 'id');
    return `<article class="workspace-record"><span class="workspace-record__icon">${icon('skill', { size: 20 })}</span><div><strong>${text(skill.title)}</strong>${identity ? `<small class="workspace-record-identity">${text(identity)}</small>` : ''}<small>${plural(skill.revisionCount, 'revision')} · ${text(originLabel(skill.origin))}</small><small>${skill.reviewed ? 'Latest revision locally reviewed' : 'No local review on latest revision'}</small>${renderTechnicalDetails(`workspace-skill-${skill.id}`, `<dl><div><dt>Skill ID</dt><dd class="analysis-mono">${text(skill.id)}</dd></div><div><dt>Latest revision ID</dt><dd class="analysis-mono">${text(skill.latestRevisionId ?? 'Unavailable')}</dd></div><div><dt>Origin reference</dt><dd class="analysis-mono">${text(skill.origin.kind === 'source' ? skill.origin.sourceId : skill.origin.compositionId)}</dd></div></dl>`)}</div><div class="workspace-record__actions"><button class="analysis-button analysis-button--small" type="button" data-workspace-inspect="capability" data-workspace-intent="inspect" data-workspace-id="${text(skill.id)}" aria-label="Inspect deletion impact for skill ${text(skill.title)}${identity ? ` (${text(identity)})` : ''}" ${actionsDisabled ? 'disabled' : ''}>Inspect</button><button class="analysis-button analysis-button--small workspace-delete" type="button" data-workspace-inspect="capability" data-workspace-intent="delete" data-workspace-id="${text(skill.id)}" aria-label="Preview deletion for skill ${text(skill.title)}${identity ? ` (${text(identity)})` : ''}" ${actionsDisabled ? 'disabled' : ''}>${icon('clear', { size: 16 })}<span>Delete</span></button></div></article>`;
  }).join('');
}

function workflowRows(state, actionsDisabled) {
  const records = state.workspace?.workflows ?? [];
  if (!records.length) return '<p class="workspace-empty">No saved workflows. Workflow creation is unavailable in this view.</p>';
  return records.map(workflow => {
    const identity = recordIdentityHint(records, workflow, 'label', 'id');
    return `<article class="workspace-record"><span class="workspace-record__icon">${icon('workflow', { size: 20 })}</span><div><strong>${text(workflow.label)}</strong>${identity ? `<small class="workspace-record-identity">${text(identity)}</small>` : ''}<small>${plural(workflow.revisionCount, 'revision')}</small>${renderTechnicalDetails(`workspace-workflow-${workflow.id}`, `<dl><div><dt>Workflow ID</dt><dd class="analysis-mono">${text(workflow.id)}</dd></div><div><dt>Latest revision ID</dt><dd class="analysis-mono">${text(workflow.latestRevisionId ?? 'Unavailable')}</dd></div></dl>`)}<small>${workflow.intent === 'validated' ? 'Validated intent' : 'Draft intent'} · ${workflow.structurallyValid ? 'Structural check valid' : 'Structural check needs attention'}</small><small>${plural(workflow.unresolvedReferences, 'unresolved reference')} · Saved ${text(stamp(workflow.savedAtMs))}</small></div><div class="workspace-record__actions"><button class="analysis-button analysis-button--small" type="button" data-workspace-inspect="workflow" data-workspace-intent="inspect" data-workspace-id="${text(workflow.id)}" aria-label="Inspect deletion impact for workflow ${text(workflow.label)}${identity ? ` (${text(identity)})` : ''}" ${actionsDisabled ? 'disabled' : ''}>Inspect</button><button class="analysis-button analysis-button--small workspace-delete" type="button" data-workspace-inspect="workflow" data-workspace-intent="delete" data-workspace-id="${text(workflow.id)}" aria-label="Preview deletion for workflow ${text(workflow.label)}${identity ? ` (${text(identity)})` : ''}" ${actionsDisabled ? 'disabled' : ''}>${icon('clear', { size: 16 })}<span>Delete</span></button></div></article>`;
  }).join('');
}

function impactRows(label, value, verb) {
  const singular = { Sources: 'source', Skills: 'skill', Workflows: 'workflow', 'Capability revisions': 'capability revision', 'Workflow revisions': 'workflow revision', Reviews: 'review', Recipes: 'recipe', Applications: 'application', Derivations: 'derivation' };
  return `<div><dt>${label}</dt><dd>${verb} ${plural(value, singular[label])}</dd></div>`;
}

function restoreSummary(plan) {
  const { add } = plan;
  return `<dl class="workspace-dialog__counts"><div><dt>Sources</dt><dd>Add ${add.sources} · keep ${plan.keptSources}</dd></div><div><dt>Skills</dt><dd>Add ${add.capabilities} · keep ${plan.keptCapabilities}</dd></div><div><dt>Workflows</dt><dd>Add ${add.workflows} · keep ${plan.keptWorkflows}</dd></div>${impactRows('Capability revisions', add.revisions, 'Add')}${impactRows('Workflow revisions', add.workflowRevisions, 'Add')}${impactRows('Reviews', add.reviews, 'Add')}${impactRows('Recipes', add.recipes, 'Add')}${impactRows('Applications', add.applications, 'Add')}${impactRows('Derivations', add.derivations, 'Add')}<div><dt>Archive</dt><dd>${bytes(plan.byteLength)} bytes</dd></div></dl>`;
}

function deletionSummary(plan) {
  const { remove } = plan;
  return `<dl class="workspace-dialog__counts">${impactRows('Sources', remove.sources, 'Remove')}${impactRows('Skills', remove.capabilities, 'Remove')}${impactRows('Workflows', remove.workflows, 'Remove')}${impactRows('Capability revisions', remove.revisions, 'Remove')}${impactRows('Workflow revisions', remove.workflowRevisions, 'Remove')}${impactRows('Reviews', remove.reviews, 'Remove')}${impactRows('Recipes', remove.recipes, 'Remove')}${impactRows('Applications', remove.applications, 'Remove')}${impactRows('Derivations', remove.derivations, 'Remove')}</dl>`;
}

function dialog(state, mode) {
  if (!state.dialog) return '';
  const stale = state.dialog.stale;
  const pending = Boolean(state.pending);
  const unavailable = stale ? '<p class="workspace-dialog__warning">This preview is stale or the local action failed. No success was recorded. Refresh workspace and prepare a new preview.</p>' : '';
  const button = (label, attributes, primary = false) => `<button class="analysis-button${primary ? ' analysis-button--primary' : ''}" type="button" ${attributes} ${pending ? 'disabled' : ''}>${label}</button>`;
  if (state.dialog.kind === 'pending') return `<dialog class="workspace-dialog" aria-labelledby="workspace-dialog-title" data-workspace-dialog tabindex="-1" aria-busy="${pending}"><div class="workspace-dialog__icon">${icon(state.dialog.action === 'delete' ? 'clear' : 'import', { size: 24 })}</div><p class="analysis-kicker">LOCAL DATA ${text(state.dialog.action).toUpperCase()}</p><h2 id="workspace-dialog-title">${stale ? 'Confirmation needs a fresh preview' : state.dialog.action === 'delete' ? 'Removing local record…' : 'Restoring local records…'}</h2><p>${stale ? 'The host did not return a confirmed receipt. No success is recorded here. Refresh workspace, then inspect a new preview before trying again.' : 'Waiting for local storage confirmation. Keep this dialog open; this attempt cannot be retried automatically.'}</p>${stale ? `<div class="workspace-dialog__actions">${button('Close', 'data-workspace-cancel autofocus')}</div>` : ''}</dialog>`;
  if (state.dialog.kind === 'export-warning') {
    if (mode !== 'legacy_plaintext') return '';
    return `<dialog class="workspace-dialog" aria-labelledby="workspace-dialog-title" data-workspace-dialog tabindex="-1" aria-busy="${pending}"><div class="workspace-dialog__icon">${icon('bundle', { size: 24 })}</div><p class="analysis-kicker">LOCAL BACKUP</p><h2 id="workspace-dialog-title">Export unencrypted backup?</h2><p>It includes saved source content, skills, workflow history, recipes and local review records. Pick a new destination; existing files are never overwritten.</p><p class="workspace-dialog__note">Backups do not include unsaved editor drafts. No upload occurs.</p><div class="workspace-dialog__actions">${button('Cancel', 'data-workspace-cancel autofocus')}${button('Choose destination', 'data-workspace-confirm', true)}</div></dialog>`;
  }
  if (state.dialog.kind === 'encrypted-export-warning') {
    if (mode !== 'managed_encrypted' || state.encryptedRecoverySupported !== true) return '';
    const recoveryAction = (label, attributes, primary = false) => `<button class="analysis-button${primary ? ' analysis-button--primary' : ''}" type="button" ${attributes} ${pending || stale ? 'disabled' : ''}>${label}</button>`;
    const recoveryFailure = stale ? `<p class="workspace-dialog__warning">${text(state.error?.message ?? 'The local recovery action could not finish.')} No key or backup success is recorded. Close this dialog, then open Back up workspace to start a new native action.</p>` : '';
    return `<dialog class="workspace-dialog" aria-labelledby="workspace-dialog-title" data-workspace-dialog tabindex="-1" aria-busy="${pending}"><div class="workspace-dialog__icon">${icon('bundle', { size: 24 })}</div><p class="analysis-kicker">ENCRYPTED RECOVERY BACKUP</p><h2 id="workspace-dialog-title">${stale ? 'Recovery action needs a fresh start' : 'Back up encrypted workspace?'}</h2><p>Recovery requires both the encrypted backup and its separate secret recovery key file. If the recovery key is lost, that backup cannot be restored.</p><p class="workspace-dialog__note">The recovery key file is itself unencrypted. Protect it like a password and store it separately. Anyone with both files can read this backup.</p><p class="workspace-dialog__note">Rangoon never shows the recovery key here. Pick files only in native dialogs. Existing files are never overwritten.</p>${recoveryFailure}<div class="workspace-dialog__actions">${button(stale ? 'Close' : 'Cancel', 'data-workspace-cancel autofocus')}${recoveryAction('Use recovery key', 'data-workspace-confirm-encrypted-export', true)}${recoveryAction('Create recovery key', 'data-workspace-create-recovery-key')}</div></dialog>`;
  }
  if (state.dialog.kind === 'restore' && state.restorePlan) {
    const encrypted = state.restoreOrigin === 'encrypted';
    const noAdditions = Object.values(state.restorePlan.add).every(value => value === 0);
    return `<dialog class="workspace-dialog" aria-labelledby="workspace-dialog-title" data-workspace-dialog tabindex="-1" aria-busy="${pending}"><div class="workspace-dialog__icon">${icon('import', { size: 24 })}</div><p class="analysis-kicker">${encrypted ? 'ENCRYPTED RECOVERY RESTORE' : 'OLDER PLAINTEXT BACKUP RESTORE'} PREVIEW</p><h2 id="workspace-dialog-title">${stale ? 'Restore preview stale' : noAdditions ? 'Nothing new to restore' : encrypted ? 'Restore encrypted backup?' : 'Restore older backup?'}</h2><p>Restore is additive. Existing heads and complete histories stay retained. Incompatible incoming history is refused; historical reviews remain unauthenticated records.</p>${restoreSummary(state.restorePlan)}${unavailable}<p class="workspace-dialog__note">If saved data changes, this preview becomes stale. Refresh and inspect a new preview; Rangoon does not retry writes.</p><div class="workspace-dialog__actions">${button(stale || noAdditions ? 'Close' : 'Cancel', 'data-workspace-cancel autofocus')}${!stale && !noAdditions ? button('Restore additions', 'data-workspace-confirm', true) : ''}</div></dialog>`;
  }
  const plan = state.deletionPlan;
  if (!plan) return '';
  const blocked = plan.capabilityDependencies.length + plan.workflowDependencies.length > 0;
  const inspect = state.dialog.mode === 'inspect';
  const identity = deletionIdentityHint(plan, state.workspace);
  const title = stale ? 'Deletion preview stale' : blocked ? 'Deletion blocked' : inspect ? `Impact for ${text(plan.label)}` : `Delete ${text(plan.label)}?`;
  const capabilityBlockers = plan.capabilityDependencies.length ? `<h3>Blocking skills</h3><ul class="workspace-dependencies">${plan.capabilityDependencies.map(item => `<li>${text(item.title)} · ${text(originLabel(item.origin))} · ${text(short(item.id))}</li>`).join('')}</ul>` : '';
  const workflowBlockers = plan.workflowDependencies.length ? `<h3>Blocking workflows</h3><ul class="workspace-dependencies">${plan.workflowDependencies.map(item => `<li>${text(item.label)} · ${text(short(item.id))} · ${text(item.intent === 'validated' ? 'Validated intent' : 'Draft intent')}</li>`).join('')}</ul>` : '';
  const body = blocked ? `<p>This record has saved dependents. Delete dependents first, or keep this record.</p><p>No records will be removed while blockers remain.</p>${capabilityBlockers}${workflowBlockers}` : `<p>${inspect ? 'This read-only inspection changes nothing. ' : ''}Removes ${text(plan.label)} from this local workspace. ${plan.kind === 'workflow' ? 'Workflow deletion removes its history only; original input files and dependent inputs stay preserved.' : 'Original selected files are never changed.'}</p>${deletionSummary(plan)}`;
  const deletionNote = mode === 'legacy_plaintext'
    ? 'Logical deletion is not secure erase. SQLite may retain reusable pages. Consider exporting a backup first.'
    : 'Logical deletion is not secure erase. SQLite may retain reusable pages.';
  return `<dialog class="workspace-dialog" aria-labelledby="workspace-dialog-title" data-workspace-dialog tabindex="-1" aria-busy="${pending}"><div class="workspace-dialog__icon workspace-dialog__icon--danger">${icon('clear', { size: 24 })}</div><p class="analysis-kicker">${inspect ? 'DELETE IMPACT' : 'DELETE LOCAL'} ${text(plan.kind).toUpperCase()}</p><h2 id="workspace-dialog-title">${title}</h2>${identity ? `<p class="workspace-dialog__note">${text(identity)}</p>` : ''}${renderTechnicalDetails('workspace-delete-target-details', `<dl><div><dt>Record type</dt><dd>${text(plan.kind)}</dd></div><div><dt>Record ID</dt><dd class="analysis-mono">${text(plan.id)}</dd></div></dl>`)}${body}${state.dialog.draftWarning && !inspect ? `<p class="workspace-dialog__warning">${text(state.dialog.draftWarning)} Deleting this skill clears that unsaved draft.</p>` : ''}${unavailable}<p class="workspace-dialog__note">${deletionNote}</p><div class="workspace-dialog__actions">${button(stale || blocked || inspect ? 'Close' : 'Cancel', 'data-workspace-cancel autofocus')}${!stale && !blocked && !inspect ? button('Delete local record', 'data-workspace-confirm', false) : ''}</div></dialog>`;
}

export function renderWorkspaceView(state, { storageMode: requestedStorageMode } = {}) {
  const mode = storageMode(requestedStorageMode);
  const encryptedRecoverySupported = mode === 'managed_encrypted' && state.encryptedRecoverySupported === true;
  const unavailable = state.status === 'unavailable' || mode === 'unavailable';
  const loading = state.status === 'loading' && !state.workspace;
  const pending = Boolean(state.pending);
  const failure = Boolean(state.error);
  const controlsDisabled = unavailable || pending || !state.workspace;
  const actionDisabled = unavailable || pending;
  const copy = mode === 'managed_encrypted'
    ? encryptedRecoverySupported
      ? 'Manage saved sources, skills and workflows. Back up encrypted workspace data with a separate recovery key, restore an encrypted backup, or import an older plaintext backup explicitly.'
      : 'Manage saved sources, skills and workflows. Encrypted recovery is unavailable in this desktop build. Older plaintext backup import remains separate.'
    : mode === 'legacy_plaintext'
      ? 'Manage saved sources, skills and workflows. Export an unencrypted backup, preview a restore, or review what will be deleted before confirming.'
      : 'Open Rangoon desktop to manage workspace data.';
  const boundaries = mode === 'managed_encrypted'
    ? encryptedRecoverySupported
      ? '<span>64 MiB encrypted recovery image</span><span>Separate recovery key file</span><span>Native picker only</span><span>No automatic removal</span><span>Authority: none</span>'
      : '<span>Encrypted recovery unavailable</span><span>Older backup input may be unencrypted</span><span>Native picker only</span><span>No automatic removal</span><span>Authority: none</span>'
    : mode === 'legacy_plaintext'
      ? '<span>64 MiB local allocation</span><span>Unencrypted backup content</span><span>Native picker only</span><span>No automatic removal</span><span>Authority: none</span>'
      : '<span>Native data actions unavailable</span><span>Authority: none</span>';
  const records = state.workspace ? `<div class="workspace-inventory"><section class="workspace-panel"><header><div><p class="analysis-kicker">SAVED SOURCES</p><h2>${icon('source', { size: 20 })} Source files</h2></div><span>${plural(state.workspace.sources.length, 'source')}</span></header><div class="workspace-records">${sourceRows(state, actionDisabled)}</div></section><section class="workspace-panel"><header><div><p class="analysis-kicker">SAVED SKILLS</p><h2>${icon('skill', { size: 20 })} Skills</h2></div><span>${plural(state.workspace.capabilities.length, 'skill')}</span></header><div class="workspace-records">${skillRows(state, actionDisabled)}</div></section><section class="workspace-panel"><header><div><p class="analysis-kicker">SAVED WORKFLOWS</p><h2>${icon('workflow', { size: 20 })} Workflows</h2></div><span>${plural(state.workspace.workflows.length, 'workflow')}</span></header><p class="workspace-panel__note">Save intent and structural checks are observations, not review, approval or execution authority.</p><div class="workspace-records">${workflowRows(state, actionDisabled)}</div></section></div>` : `<section class="workspace-panel workspace-panel--empty"><span>${icon(unavailable ? 'unavailable' : failure ? 'unknown' : 'bundle', { size: 40 })}</span><h2>${unavailable ? 'Local data unavailable in browser' : loading ? 'Loading local workspace…' : failure ? 'Saved local data could not load' : 'Load saved local data'}</h2><p>${unavailable ? 'Open this page in the Rangoon desktop app. Browser preview never invents an empty workspace.' : failure ? 'Existing editor drafts and analysis stay available. Retry loading the local inventory.' : 'Inspect saved sources, skills and workflows, then export, restore or delete only after a local preview.'}</p></section>`;
  const recoveryActions = mode === 'managed_encrypted'
    ? `<button id="workspace-encrypted-export" class="analysis-button analysis-button--primary" type="button" ${controlsDisabled || !encryptedRecoverySupported ? 'disabled' : ''}>${icon('bundle', { size: 16 })}${state.pending === 'encrypted-export' ? 'Backing up…' : 'Back up workspace'}</button><button id="workspace-encrypted-restore" class="analysis-button" type="button" ${controlsDisabled || !encryptedRecoverySupported ? 'disabled' : ''}>${icon('import', { size: 16 })}${state.pending === 'encrypted-restore-preview' ? 'Opening backup…' : 'Restore backup'}</button><details id="workspace-recovery-options" class="workbench-technical"><summary>More recovery options</summary><p>Older backups can be imported only as unencrypted backup input. This does not make the input encrypted.</p><button id="workspace-restore" class="analysis-button" type="button" ${controlsDisabled ? 'disabled' : ''}>${icon('import', { size: 16 })}${state.pending === 'restore-preview' ? 'Opening older backup…' : 'Import older backup'}</button>${renderTechnicalDetails('workspace-encryption-limits', '<p>Encryption covers saved workspace data and an explicitly created encrypted backup at rest. It does not protect an unlocked or compromised host, original files, or readable instruction exports. Recovery integrity does not establish an author, trust, or approval. Windows and Linux native recovery qualification remains pending.</p>', 'Encryption limits')}</details>`
    : `${mode === 'legacy_plaintext' ? `<button id="workspace-export" class="analysis-button analysis-button--primary" type="button" ${controlsDisabled ? 'disabled' : ''}>${icon('bundle', { size: 16 })}${state.pending === 'export' ? 'Exporting…' : 'Export backup'}</button>` : ''}<button id="workspace-restore" class="analysis-button" type="button" ${controlsDisabled ? 'disabled' : ''}>${icon('import', { size: 16 })}${state.pending === 'restore-preview' ? 'Opening backup…' : 'Restore backup'}</button>`;
  return `<section class="workspace-page" aria-labelledby="workspace-title" aria-busy="${pending || loading}"><header class="workspace-hero"><p class="analysis-kicker">WORKSPACE / LOCAL DATA CONTROLS</p><h1 id="workspace-title" tabindex="-1">Workspace <em>data</em></h1><p>${copy}</p><div class="workspace-actions"><button id="workspace-reload" class="analysis-button" type="button" ${actionDisabled ? 'disabled' : ''}>${icon('source', { size: 16 })}${state.pending === 'load' ? 'Loading…' : 'Refresh workspace'}</button>${recoveryActions}</div><p class="workspace-status${failure ? ' workspace-status--error' : ''}" role="status">${text(state.message)}${state.error?.code ? ` Error code: ${text(state.error.code)}.` : ''}</p></header>${usage(state)}<section class="workspace-boundaries" aria-label="Workspace data limits">${boundaries}</section>${records}${dialog(state, mode)}</section>`;
}

export function bindWorkspaceView(root, controller, { getStorageMode = () => 'unavailable' } = {}) {
  let lastFocus = null;
  let dialogInitiator = null;
  let activeDialog = false;
  let disposed = false;
  let dialogStorageMode = null;
  let renderedDialogKind = null;
  const currentStorageMode = () => {
    try { return storageMode(getStorageMode()); } catch { return 'unavailable'; }
  };
  const selectors = 'button:not([disabled]), [href], input:not([disabled]), summary, [tabindex]:not([tabindex="-1"])';
  const focusSelector = element => {
    if (!element || !root.contains(element)) return null;
    if (element.id) return `#${CSS.escape(element.id)}`;
    if (element.matches?.('summary')) {
      const disclosure = element.closest?.('details.workbench-technical[id]');
      if (disclosure) return `#${CSS.escape(disclosure.id)} > summary`;
    }
    const { workspaceInspect, workspaceIntent, workspaceId } = element.dataset ?? {};
    if (workspaceInspect && workspaceIntent && workspaceId) {
      return `[data-workspace-inspect="${CSS.escape(workspaceInspect)}"][data-workspace-intent="${CSS.escape(workspaceIntent)}"][data-workspace-id="${CSS.escape(workspaceId)}"]`;
    }
    return null;
  };
  const render = () => {
    if (disposed) return;
    const prior = root.ownerDocument?.activeElement;
    const priorSelector = focusSelector(prior);
    if (priorSelector) lastFocus = priorSelector;
    const hadWorkspaceFocus = Boolean(priorSelector);
    const mode = currentStorageMode();
    let state = controller.getState();
    const nextDialogKind = state.dialog?.kind ?? null;
    if (!nextDialogKind) {
      renderedDialogKind = null;
      dialogStorageMode = null;
    } else if (nextDialogKind !== renderedDialogKind) {
      renderedDialogKind = nextDialogKind;
      dialogStorageMode = mode;
    }
    const unsupportedRecoveryDialog = nextDialogKind === 'encrypted-export-warning' && state.encryptedRecoverySupported !== true;
    if ((nextDialogKind && dialogStorageMode !== mode || unsupportedRecoveryDialog) && !state.pending) {
      controller.cancel?.();
      state = controller.getState();
      renderedDialogKind = null;
      dialogStorageMode = null;
    }
    const openDetails = [...root.querySelectorAll('details.workbench-technical[open][id]')].map(element => element.id);
    root.innerHTML = renderWorkspaceView(state, { storageMode: mode });
    for (const id of openDetails) root.querySelector(`#${CSS.escape(id)}`)?.setAttribute('open', '');
    const dialogNode = root.querySelector('[data-workspace-dialog]');
    if (dialogNode) {
      const content = root.ownerDocument.createElement('div');
      content.className = 'workspace-dialog__content';
      for (const child of [...dialogNode.children]) {
        if (!child.matches('.workspace-dialog__actions')) content.append(child);
      }
      dialogNode.prepend(content);
      activeDialog = true;
      if (!dialogNode.open) dialogNode.showModal();
      const cancel = dialogNode.querySelector('[data-workspace-cancel]');
      if (controller.getState().pending || cancel?.disabled) dialogNode.focus({ preventScroll: true });
      else cancel?.focus({ preventScroll: true });
    } else if (activeDialog) {
      activeDialog = false;
      const fallback = (dialogInitiator || lastFocus) && root.querySelector(dialogInitiator || lastFocus);
      (fallback?.matches('button:not([disabled])') ? fallback : root.querySelector('#workspace-title'))?.focus({ preventScroll: true });
      dialogInitiator = null;
    } else if (hadWorkspaceFocus) {
      const target = lastFocus && root.querySelector(lastFocus);
      (target?.matches('button:not([disabled]), [href], input:not([disabled]), summary') ? target : root.querySelector('#workspace-title'))?.focus({ preventScroll: true });
    }
  };
  const invoke = action => { if (!disposed) void action(); };
  const click = event => {
    if (disposed) return;
    const initiator = event.target.closest('button');
    const initiatorSelector = focusSelector(initiator);
    if (initiatorSelector) {
      lastFocus = initiatorSelector;
      if (initiator.matches('#workspace-export, #workspace-restore, #workspace-encrypted-export, #workspace-encrypted-restore, [data-workspace-inspect]')) dialogInitiator = initiatorSelector;
    }
    const state = controller.getState();
    const mode = currentStorageMode();
    const available = mode !== 'unavailable' && state.status !== 'unavailable' && !state.pending;
    const staleDialog = Boolean(state.dialog) && dialogStorageMode !== null && dialogStorageMode !== mode;
    if (event.target.closest('#workspace-reload')) return available ? invoke(() => controller.load()) : undefined;
    if (event.target.closest('#workspace-export')) {
      if (available && mode === 'legacy_plaintext' && state.workspace) {
        return invoke(() => controller.requestExport());
      }
      return undefined;
    }
    if (event.target.closest('#workspace-encrypted-export')) {
      if (available && mode === 'managed_encrypted' && state.workspace && state.encryptedRecoverySupported === true) {
        return invoke(() => controller.requestEncryptedExport());
      }
      return undefined;
    }
    if (event.target.closest('#workspace-encrypted-restore')) {
      if (available && mode === 'managed_encrypted' && state.workspace && state.encryptedRecoverySupported === true) {
        return invoke(() => controller.prepareEncryptedRestore());
      }
      return undefined;
    }
    if (event.target.closest('#workspace-restore')) return available && state.workspace ? invoke(() => controller.prepareRestore()) : undefined;
    if (event.target.closest('[data-workspace-cancel]')) return invoke(() => controller.cancel());
    if (event.target.closest('[data-workspace-create-recovery-key]')) {
      if (!available || mode !== 'managed_encrypted' || state.encryptedRecoverySupported !== true || staleDialog) return undefined;
      return invoke(() => controller.createRecoveryKey());
    }
    if (event.target.closest('[data-workspace-confirm-encrypted-export]')) {
      if (!available || mode !== 'managed_encrypted' || state.encryptedRecoverySupported !== true || staleDialog) return undefined;
      return invoke(() => controller.confirmEncryptedExport());
    }
    if (event.target.closest('[data-workspace-confirm]')) {
      const kind = state.dialog?.kind;
      if (staleDialog) return invoke(() => controller.cancel());
      if (!available) return undefined;
      if (kind === 'export-warning') return mode === 'legacy_plaintext' ? invoke(() => controller.confirmExport()) : undefined;
      return invoke(() => kind === 'restore' ? controller.confirmRestore() : controller.confirmDeletion());
    }
    const item = event.target.closest('[data-workspace-inspect]');
    if (item && available && state.workspace) return invoke(() => controller.inspectDeletion(item.dataset.workspaceInspect, item.dataset.workspaceId, item.dataset.workspaceIntent === 'delete'));
  };
  const keydown = event => {
    if (disposed) return;
    const dialogNode = root.querySelector('[data-workspace-dialog]');
    if (!dialogNode) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      if (!controller.getState().pending) invoke(() => controller.cancel());
      return;
    }
    if (event.key !== 'Tab') return;
    const items = [...dialogNode.querySelectorAll(selectors)];
    if (!items.length) { event.preventDefault(); return; }
    const first = items[0]; const last = items.at(-1);
    if (event.shiftKey && root.ownerDocument.activeElement === first) { event.preventDefault(); last.focus(); }
    if (!event.shiftKey && root.ownerDocument.activeElement === last) { event.preventDefault(); first.focus(); }
  };
  const cancel = event => {
    if (disposed) return;
    if (event.target.matches?.('[data-workspace-dialog]')) event.preventDefault();
  };
  root.addEventListener('click', click);
  root.addEventListener('keydown', keydown);
  root.addEventListener('cancel', cancel, true);
  render();
  const dispose = () => {
    if (disposed) return;
    disposed = true;
    lastFocus = null; dialogInitiator = null; activeDialog = false; dialogStorageMode = null; renderedDialogKind = null;
    root.removeEventListener('click', click); root.removeEventListener('keydown', keydown); root.removeEventListener('cancel', cancel, true);
  };
  return { render, dispose, purgeForWorkspaceLock: dispose };
}
