import { escapeText } from './analysis-model.mjs';
import { icon } from './icons.mjs';

const text = escapeText;
const short = value => String(value ?? '').length > 22 ? `${String(value).slice(0, 10)}…${String(value).slice(-8)}` : String(value ?? 'Unavailable');
const bytes = value => Number(value ?? 0).toLocaleString();
const stamp = value => Number.isSafeInteger(value) ? new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value)) : 'Date unavailable';
const plural = (count, word) => `${count} ${word}${count === 1 ? '' : 's'}`;

function usage(state) {
  const value = state.workspace?.records;
  if (!value) return '';
  return `<section class="workspace-metrics" aria-label="Local workspace usage">
    <div><span>Sources</span><strong>${value.sources} <small>/ 128</small></strong></div>
    <div><span>Skills</span><strong>${value.capabilities} <small>/ 128</small></strong></div>
    <div><span>Revisions</span><strong>${value.revisions} <small>/ 1,024</small></strong></div>
    <div><span>Reviews</span><strong>${value.reviews}</strong></div>
    <div><span>Recipes</span><strong>${value.recipes}</strong></div>
    <div><span>Applications</span><strong>${value.applications}</strong></div>
    <div><span>Derivations</span><strong>${value.derivations}</strong></div>
    <div><span>Database</span><strong>${bytes(state.workspace.databaseBytes)} <small>bytes</small></strong><em>${bytes(state.workspace.reusableBytes)} reusable</em></div>
  </section>`;
}

function originLabel(origin) {
  if (origin.kind === 'source') return `Source ${short(origin.sourceId)}`;
  return `Composition ${origin.operation} · output ${origin.outputIndex + 1}`;
}

function sourceRows(state) {
  const records = state.workspace?.sources ?? [];
  if (!records.length) return '<p class="workspace-empty">No saved sources. Explicitly save analysis before it appears here.</p>';
  return records.map(source => `<article class="workspace-record"><span class="workspace-record__icon">${icon('source', { size: 20 })}</span><div><strong>${text(source.displayName)}</strong><small class="analysis-mono">${text(short(source.sourceId))} · ${bytes(source.byteLength)} bytes</small><small>Saved ${text(stamp(source.savedAtMs))}</small></div><div class="workspace-record__actions"><button class="analysis-button analysis-button--small" type="button" data-workspace-inspect="source" data-workspace-intent="inspect" data-workspace-id="${text(source.sourceId)}" aria-label="Inspect deletion impact for source ${text(source.displayName)}" ${state.pending ? 'disabled' : ''}>Inspect</button><button class="analysis-button analysis-button--small workspace-delete" type="button" data-workspace-inspect="source" data-workspace-intent="delete" data-workspace-id="${text(source.sourceId)}" aria-label="Preview deletion for source ${text(source.displayName)}" ${state.pending ? 'disabled' : ''}>${icon('clear', { size: 16 })}<span>Delete</span></button></div></article>`).join('');
}

function skillRows(state) {
  const records = state.workspace?.capabilities ?? [];
  if (!records.length) return '<p class="workspace-empty">No saved skills. Skills stay local until you make them from saved source sections.</p>';
  return records.map(skill => `<article class="workspace-record"><span class="workspace-record__icon">${icon('skill', { size: 20 })}</span><div><strong>${text(skill.title)}</strong><small class="analysis-mono">${text(short(skill.id))} · ${plural(skill.revisionCount, 'revision')}</small><small>${text(originLabel(skill.origin))}</small><small>${skill.reviewed ? 'Latest revision locally reviewed' : 'No local review on latest revision'}</small></div><div class="workspace-record__actions"><button class="analysis-button analysis-button--small" type="button" data-workspace-inspect="capability" data-workspace-intent="inspect" data-workspace-id="${text(skill.id)}" aria-label="Inspect deletion impact for skill ${text(skill.title)}" ${state.pending ? 'disabled' : ''}>Inspect</button><button class="analysis-button analysis-button--small workspace-delete" type="button" data-workspace-inspect="capability" data-workspace-intent="delete" data-workspace-id="${text(skill.id)}" aria-label="Preview deletion for skill ${text(skill.title)}" ${state.pending ? 'disabled' : ''}>${icon('clear', { size: 16 })}<span>Delete</span></button></div></article>`).join('');
}

function impactRows(label, value, verb) {
  return `<div><dt>${label}</dt><dd>${verb} ${plural(value, label.toLowerCase())}</dd></div>`;
}

function restoreSummary(plan) {
  const { add } = plan;
  return `<dl class="workspace-dialog__counts"><div><dt>Sources</dt><dd>Add ${add.sources} · keep ${plan.keptSources}</dd></div><div><dt>Skills</dt><dd>Add ${add.capabilities} · keep ${plan.keptCapabilities}</dd></div>${impactRows('Revisions', add.revisions, 'Add')}${impactRows('Reviews', add.reviews, 'Add')}${impactRows('Recipes', add.recipes, 'Add')}${impactRows('Applications', add.applications, 'Add')}${impactRows('Derivations', add.derivations, 'Add')}<div><dt>Archive</dt><dd>${bytes(plan.byteLength)} bytes</dd></div></dl>`;
}

function deletionSummary(plan) {
  const { remove } = plan;
  return `<dl class="workspace-dialog__counts">${impactRows('Sources', remove.sources, 'Remove')}${impactRows('Skills', remove.capabilities, 'Remove')}${impactRows('Revisions', remove.revisions, 'Remove')}${impactRows('Reviews', remove.reviews, 'Remove')}${impactRows('Recipes', remove.recipes, 'Remove')}${impactRows('Applications', remove.applications, 'Remove')}${impactRows('Derivations', remove.derivations, 'Remove')}</dl>`;
}

function dialog(state) {
  if (!state.dialog) return '';
  const stale = state.dialog.stale;
  const pending = Boolean(state.pending);
  const unavailable = stale ? '<p class="workspace-dialog__warning">This preview is stale or the local action failed. No success was recorded. Refresh inventory and prepare a new preview.</p>' : '';
  const button = (label, attributes, primary = false) => `<button class="analysis-button${primary ? ' analysis-button--primary' : ''}" type="button" ${attributes} ${pending ? 'disabled' : ''}>${label}</button>`;
  if (state.dialog.kind === 'export-warning') return `<dialog class="workspace-dialog" aria-labelledby="workspace-dialog-title" data-workspace-dialog tabindex="-1"><div class="workspace-dialog__icon">${icon('bundle', { size: 24 })}</div><p class="analysis-kicker">LOCAL BACKUP</p><h2 id="workspace-dialog-title">Export unencrypted backup?</h2><p>It includes saved source content, skills, revisions and local review records. Pick a new destination; existing files are never overwritten.</p><p class="workspace-dialog__note">Backups do not include unsaved editor drafts. No upload occurs.</p><div class="workspace-dialog__actions">${button('Cancel', 'data-workspace-cancel autofocus')}${button('Choose destination', 'data-workspace-confirm', true)}</div></dialog>`;
  if (state.dialog.kind === 'restore' && state.restorePlan) {
    const noAdditions = !state.restorePlan.add.sources && !state.restorePlan.add.capabilities;
    return `<dialog class="workspace-dialog" aria-labelledby="workspace-dialog-title" data-workspace-dialog tabindex="-1"><div class="workspace-dialog__icon">${icon('import', { size: 24 })}</div><p class="analysis-kicker">RESTORE PREVIEW</p><h2 id="workspace-dialog-title">${stale ? 'Restore preview stale' : noAdditions ? 'Nothing new to restore' : 'Restore missing records?'}</h2><p>Restore is additive. Existing source bytes and existing skills stay unchanged. Historical reviews remain unauthenticated records.</p>${restoreSummary(state.restorePlan)}${unavailable}<p class="workspace-dialog__note">If saved data changes, this preview becomes stale. Refresh and inspect a new preview; Rangoon does not retry writes.</p><div class="workspace-dialog__actions">${button(stale || noAdditions ? 'Close' : 'Cancel', 'data-workspace-cancel autofocus')}${!stale && !noAdditions ? button('Restore additions', 'data-workspace-confirm', true) : ''}</div></dialog>`;
  }
  const plan = state.deletionPlan;
  if (!plan) return '';
  const blocked = plan.dependencies.length > 0;
  const inspect = state.dialog.mode === 'inspect';
  const title = stale ? 'Deletion preview stale' : blocked ? 'Deletion blocked' : inspect ? `Impact for ${text(plan.title)}` : `Delete ${text(plan.title)}?`;
  const body = blocked ? `<p>This record is referenced by ${plural(plan.dependencies.length, 'saved skill')}. Delete the dependent skills first, or keep this record.</p><p>No records will be removed while dependent skills remain.</p><ul class="workspace-dependencies">${plan.dependencies.map(item => `<li>${text(item.title)} · ${text(originLabel(item.origin))} · ${text(short(item.id))}</li>`).join('')}</ul>` : `<p>${inspect ? 'This read-only inspection changes nothing. ' : ''}Removes ${text(plan.title)} from this local workspace. Original selected files are never changed.</p>${deletionSummary(plan)}`;
  return `<dialog class="workspace-dialog" aria-labelledby="workspace-dialog-title" data-workspace-dialog tabindex="-1"><div class="workspace-dialog__icon workspace-dialog__icon--danger">${icon('clear', { size: 24 })}</div><p class="analysis-kicker">${inspect ? 'DELETE IMPACT' : 'DELETE LOCAL'} ${text(plan.kind).toUpperCase()}</p><h2 id="workspace-dialog-title">${title}</h2>${body}${state.dialog.draftWarning && !inspect ? `<p class="workspace-dialog__warning">${text(state.dialog.draftWarning)} Deleting this skill clears that unsaved draft.</p>` : ''}${unavailable}<p class="workspace-dialog__note">Logical deletion is not secure erase. SQLite may retain reusable pages. Consider exporting a backup first.</p><div class="workspace-dialog__actions">${button(stale || blocked || inspect ? 'Close' : 'Cancel', 'data-workspace-cancel autofocus')}${!stale && !blocked && !inspect ? button('Delete local record', 'data-workspace-confirm', false) : ''}</div></dialog>`;
}

export function renderWorkspaceView(state) {
  const unavailable = state.status === 'unavailable';
  const loading = state.status === 'loading' && !state.workspace;
  const pending = Boolean(state.pending);
  const failure = Boolean(state.error);
  const controlsDisabled = unavailable || pending || !state.workspace;
  const records = state.workspace ? `<div class="workspace-inventory"><section class="workspace-panel"><header><div><p class="analysis-kicker">SAVED SOURCES</p><h2>${icon('source', { size: 20 })} Local source records</h2></div><span>${plural(state.workspace.sources.length, 'source')}</span></header><div class="workspace-records">${sourceRows(state)}</div></section><section class="workspace-panel"><header><div><p class="analysis-kicker">SAVED SKILLS</p><h2>${icon('skill', { size: 20 })} Local capability records</h2></div><span>${plural(state.workspace.capabilities.length, 'skill')}</span></header><div class="workspace-records">${skillRows(state)}</div></section></div>` : `<section class="workspace-panel workspace-panel--empty"><span>${icon(unavailable ? 'unavailable' : failure ? 'unknown' : 'bundle', { size: 40 })}</span><h2>${unavailable ? 'Local data unavailable in browser' : loading ? 'Loading local workspace…' : failure ? 'Saved local data could not load' : 'Load saved local data'}</h2><p>${unavailable ? 'Open this page in the Rangoon desktop app. Browser preview never invents an empty workspace.' : failure ? 'Existing editor drafts and analysis stay available. Retry loading the local inventory.' : 'Inspect saved sources and skills, then export, restore or delete only after a local preview.'}</p></section>`;
  return `<section class="workspace-page" aria-labelledby="workspace-title" aria-busy="${pending || loading}"><header class="workspace-hero"><p class="analysis-kicker">WORKSPACE / LOCAL DATA CONTROLS</p><h1 id="workspace-title" tabindex="-1">Own the <em>local record.</em></h1><p>Review actual saved sources and skills. Export an unencrypted portable backup, preview additive restore, or inspect a deletion before it changes local data.</p><div class="workspace-actions"><button id="workspace-reload" class="analysis-button" type="button" ${unavailable || pending ? 'disabled' : ''}>${icon('source', { size: 16 })}${state.pending === 'load' ? 'Loading…' : 'Refresh inventory'}</button><button id="workspace-export" class="analysis-button analysis-button--primary" type="button" ${controlsDisabled ? 'disabled' : ''}>${icon('bundle', { size: 16 })}${state.pending === 'export' ? 'Exporting…' : 'Export backup'}</button><button id="workspace-restore" class="analysis-button" type="button" ${controlsDisabled ? 'disabled' : ''}>${icon('import', { size: 16 })}${state.pending === 'restore-preview' ? 'Opening backup…' : 'Restore backup'}</button></div><p class="workspace-status${failure ? ' workspace-status--error' : ''}" role="status">${text(state.message)}${state.error?.code ? ` Error code: ${text(state.error.code)}.` : ''}</p></header>${usage(state)}<section class="workspace-boundaries" aria-label="Workspace data limits"><span>64 MiB local allocation</span><span>Unencrypted backup content</span><span>Native picker only</span><span>No automatic removal</span></section>${records}${dialog(state)}</section>`;
}

export function bindWorkspaceView(root, controller) {
  let lastFocus = null;
  let dialogInitiator = null;
  let activeDialog = false;
  const selectors = 'button:not([disabled]), [href], input:not([disabled]), [tabindex]:not([tabindex="-1"])';
  const focusSelector = element => {
    if (!element || !root.contains(element)) return null;
    if (element.id) return `#${CSS.escape(element.id)}`;
    const { workspaceInspect, workspaceIntent, workspaceId } = element.dataset ?? {};
    if (workspaceInspect && workspaceIntent && workspaceId) {
      return `[data-workspace-inspect="${CSS.escape(workspaceInspect)}"][data-workspace-intent="${CSS.escape(workspaceIntent)}"][data-workspace-id="${CSS.escape(workspaceId)}"]`;
    }
    return null;
  };
  const render = () => {
    const prior = root.ownerDocument?.activeElement;
    const priorSelector = focusSelector(prior);
    if (priorSelector) lastFocus = priorSelector;
    const hadWorkspaceFocus = Boolean(priorSelector);
    root.innerHTML = renderWorkspaceView(controller.getState());
    const dialogNode = root.querySelector('[data-workspace-dialog]');
    if (dialogNode) {
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
      (target?.matches('button:not([disabled]), [href], input:not([disabled])') ? target : root.querySelector('#workspace-title'))?.focus({ preventScroll: true });
    }
  };
  const invoke = action => { void action(); };
  const click = event => {
    const initiator = event.target.closest('button');
    const initiatorSelector = focusSelector(initiator);
    if (initiatorSelector) {
      lastFocus = initiatorSelector;
      if (initiator.matches('#workspace-export, #workspace-restore, [data-workspace-inspect]')) dialogInitiator = initiatorSelector;
    }
    if (event.target.closest('#workspace-reload')) return invoke(() => controller.load());
    if (event.target.closest('#workspace-export')) return invoke(() => controller.requestExport());
    if (event.target.closest('#workspace-restore')) return invoke(() => controller.prepareRestore());
    if (event.target.closest('[data-workspace-cancel]')) return invoke(() => controller.cancel());
    if (event.target.closest('[data-workspace-confirm]')) {
      const kind = controller.getState().dialog?.kind;
      return invoke(() => kind === 'export-warning' ? controller.confirmExport() : kind === 'restore' ? controller.confirmRestore() : controller.confirmDeletion());
    }
    const item = event.target.closest('[data-workspace-inspect]');
    if (item) return invoke(() => controller.inspectDeletion(item.dataset.workspaceInspect, item.dataset.workspaceId, item.dataset.workspaceIntent === 'delete'));
  };
  const keydown = event => {
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
    if (event.target.matches?.('[data-workspace-dialog]')) event.preventDefault();
  };
  root.addEventListener('click', click);
  root.addEventListener('keydown', keydown);
  root.addEventListener('cancel', cancel, true);
  render();
  return { render, dispose: () => { root.removeEventListener('click', click); root.removeEventListener('keydown', keydown); root.removeEventListener('cancel', cancel, true); } };
}
