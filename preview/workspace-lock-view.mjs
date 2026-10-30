import { icon } from './icons.mjs';

const guardStates = new Set(['checking', 'browser', 'legacy', 'unavailable', 'open', 'confirming', 'closing', 'locked', 'recovery_required', 'protection', 'reloading']);
const closeStates = Object.freeze({
  checking: ['Checking workspace protection', 'Workspace source content stays unavailable until protection status is known.'],
  unavailable: ['Workspace unavailable', 'This workspace cannot be opened safely. Workspace source content remains unavailable.'],
  closing: ['Closing workspace', 'Workspace content is being removed from this window.'],
  locked: ['Workspace locked', 'This window no longer shows workspace source content.'],
  reloading: ['Opening workspace', 'Reloading after native workspace protection opened.'],
  recovery_required: ['Workspace recovery required', 'Keep workspace files intact. This workspace remains closed.'],
});
const protectionStates = Object.freeze({
  busy: ['Workspace busy', 'Workspace protection is still completing. This window remains closed.'],
  recovery_required: ['Workspace recovery required', 'Keep workspace files intact. This workspace remains closed.'],
  unavailable: ['Workspace unavailable', 'This workspace cannot be opened safely. Workspace source content remains unavailable.'],
  legacy: ['Workspace unavailable', 'This workspace does not support encrypted workspace protection.'],
  pending: ['Preparing workspace', 'Workspace protection is preparing. This window remains closed.'],
});

function statusOf(state) {
  return guardStates.has(state?.status) ? state.status : 'unavailable';
}

function protectionCopy(state, status) {
  if (status !== 'protection') return closeStates[status] ?? closeStates.unavailable;
  if (state?.pending === true) return protectionStates.pending;
  if (state?.canProtect === true && state?.layout === 'absent') return ['Set up encrypted workspace', 'Create an encrypted workspace on this computer.'];
  if (state?.canProtect === true && state?.layout === 'locked') return ['Unlock workspace', 'Use the native system sheet to unlock this workspace.'];
  return protectionStates[state?.layout] ?? closeStates.unavailable;
}

export function renderWorkspaceGuard(state = {}) {
  const status = statusOf(state);
  const canLock = status === 'open' && state?.canLock === true;
  const message = canLock
    ? 'Managed workspace protection is open.'
    : status === 'browser'
      ? 'Browser preview has no native workspace lock.'
      : status === 'legacy'
        ? 'Legacy workspace storage is not encrypted.'
        : status === 'checking'
          ? 'Checking workspace protection…'
          : status === 'confirming'
            ? 'Confirm workspace lock in the open dialog.'
            : status === 'closing'
              ? 'Workspace is closing.'
              : status === 'locked'
                ? 'Workspace is locked.'
                : status === 'recovery_required'
                  ? 'Workspace recovery is required.'
                  : 'Workspace lock is unavailable.';
  return `<section class="workspace-guard" data-workspace-guard aria-label="Workspace protection"><p class="workspace-guard__status" role="status">${message}</p>${canLock ? '<button id="workspace-lock-open" class="workspace-guard__lock" type="button" data-workspace-lock-open>Lock workspace</button>' : ''}</section>`;
}

export function renderWorkspaceLockDialog() {
  return '<dialog id="workspace-lock-dialog" class="workspace-lock-dialog" aria-labelledby="workspace-lock-dialog-title" aria-describedby="workspace-lock-dialog-description"><section><p class="workspace-lock-dialog__eyebrow">WORKSPACE PROTECTION</p><h2 id="workspace-lock-dialog-title">Lock workspace?</h2><p id="workspace-lock-dialog-description">Unsaved drafts and window history will be discarded. Writes already begun may have completed.</p><div class="workspace-lock-dialog__actions"><button id="workspace-lock-cancel" type="button" data-workspace-lock-cancel autofocus>Cancel</button><button id="workspace-lock-confirm" type="button" data-workspace-lock-confirm>Lock and discard</button></div></section></dialog>';
}

export function workspaceCloseAnnouncement(state = {}) {
  const [title, copy] = protectionCopy(state, statusOf(state));
  return `${title}. ${copy}`;
}

function canRequestQuit(state) {
  if (state?.pending === true) return false;
  return state?.status === 'recovery_required'
    || (state?.status === 'protection' && (state.layout === 'recovery_required'
      || (state.layout === 'absent' && state.canProtect === true)));
}

// Presentation admission only. The native main-window check and exit coordinator
// remain responsible for deciding whether and how the application can exit.
export function createWorkspaceCloseQuit({ invoke, getState, onChange = () => {}, isFenced = () => false }) {
  let pending = false;
  let failed = false;
  let disposed = false;
  const available = () => !disposed && !isFenced() && typeof invoke === 'function' && canRequestQuit(getState());
  return Object.freeze({
    get pending() { return pending; },
    get failed() { return failed; },
    get available() { return available(); },
    async request() {
      if (pending || !available()) return false;
      pending = true;
      failed = false;
      onChange();
      if (!available()) {
        pending = false;
        return false;
      }
      try {
        await invoke('workspace_startup_quit', {});
        return true;
      } catch {
        pending = false;
        failed = true;
        if (available()) onChange();
        return false;
      }
    },
    dispose() { disposed = true; },
  });
}

export function renderWorkspaceCloseShell(state = {}, { theme = 'dark', quitAvailable = false, quitPending = false, quitFailed = false } = {}) {
  const status = statusOf(state);
  const protect = status === 'protection' && state?.canProtect === true && !state?.pending && ['absent', 'locked'].includes(state?.layout);
  const [title, copy] = protectionCopy(state, status);
  const themeLabel = theme === 'light' ? 'Dark mode' : 'Light mode';
  const recovery = status === 'recovery_required' || (status === 'protection' && state.layout === 'recovery_required');
  const fresh = status === 'protection' && state.canProtect === true && state.layout === 'absent';
  const button = protect ? `<button class="workspace-protection-action" type="button" data-workspace-protection-action${quitPending ? ' disabled' : ''}>${title}</button>` : '';
  const quit = quitAvailable && canRequestQuit(state) ? `<button class="workspace-close-quit" type="button" data-workspace-close-quit${quitPending ? ' disabled' : ''}>${quitPending ? 'Quitting Rangoon…' : 'Quit Rangoon'}</button>` : '';
  const quitNotice = quitFailed && quit ? '<p role="status">Rangoon could not quit. You can try Quit again.</p>' : '';
  const guidance = recovery ? '<p class="workspace-close-recovery">If operating-system account or credential-store access changed, restore access to the original account or store, then reopen Rangoon. If the original key is unavailable, Rangoon cannot replace it. This guidance does not repair damaged workspace files.</p>' : '';
  const details = fresh ? '<dl><div><dt>Storage</dt><dd>Encrypted workspace</dd></div><div><dt>Phase</dt><dd>Fresh setup</dd></div><div><dt>Original copy</dt><dd>No original copy is part of this setup.</dd></div><div><dt>Portable backup</dt><dd>Encrypted portable backup unavailable.</dd></div></dl>' : '<p>Protection uses native desktop storage. Exports can remain readable outside this window. Legacy storage is not encrypted. Recovery controls are not available in this window.</p>';
  return `<div class="analysis-shell workspace-close-shell" data-workspace-close="${status}" data-workspace-layout="${protect ? state.layout : 'none'}" data-workspace-pending="${state?.pending === true || quitPending ? 'true' : 'false'}"><aside class="analysis-rail workspace-close-rail"><div class="analysis-brand workspace-close-brand"><img src="assets/brand-symbol.png" alt=""><span>Rangoon.si</span></div><button id="analysis-theme" class="analysis-theme" type="button">${icon('theme', { size: 16 })}<span>${themeLabel}</span></button></aside><main id="analysis-main" class="workspace-close-main"><section class="workspace-close-card" aria-labelledby="workspace-close-title" aria-busy="${quitPending}"><p class="workspace-close-card__eyebrow">WORKSPACE PROTECTION</p><h1 id="workspace-close-title" tabindex="-1">${title}</h1><p>${copy}</p>${guidance}<div class="workspace-close-actions">${button}${quit}</div>${quitNotice}<details class="workspace-close-help"><summary>${fresh ? 'Details' : 'Technical details'}</summary>${details}</details></section></main></div>`;
}
