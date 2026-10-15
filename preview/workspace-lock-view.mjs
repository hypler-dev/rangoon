import { icon } from './icons.mjs';

const guardStates = new Set(['checking', 'browser', 'legacy', 'unavailable', 'open', 'confirming', 'closing', 'locked', 'recovery_required']);
const closeStates = Object.freeze({
  checking: ['Checking workspace protection', 'Workspace source content stays unavailable until protection status is known.'],
  unavailable: ['Workspace unavailable', 'This workspace cannot be opened safely. Workspace source content remains unavailable.'],
  closing: ['Closing workspace', 'Workspace content is being removed from this window.'],
  locked: ['Workspace locked', 'This window no longer shows workspace source content.'],
  recovery_required: ['Workspace recovery required', 'Workspace source content remains unavailable until a supported recovery path is available.'],
});

function statusOf(state) {
  return guardStates.has(state?.status) ? state.status : 'unavailable';
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
  const [title, copy] = closeStates[statusOf(state)] ?? closeStates.unavailable;
  return `${title}. ${copy}`;
}

export function renderWorkspaceCloseShell(state = {}, { theme = 'dark' } = {}) {
  const status = statusOf(state);
  const [title, copy] = closeStates[status] ?? closeStates.unavailable;
  const themeLabel = theme === 'light' ? 'Dark mode' : 'Light mode';
  return `<div class="analysis-shell workspace-close-shell" data-workspace-close="${status}"><aside class="analysis-rail workspace-close-rail"><div class="analysis-brand workspace-close-brand"><img src="assets/brand-symbol.png" alt=""><span>Rangoon.si</span></div><button id="analysis-theme" class="analysis-theme" type="button">${icon('theme', { size: 16 })}<span>${themeLabel}</span></button></aside><main id="analysis-main" class="workspace-close-main"><section class="workspace-close-card" aria-labelledby="workspace-close-title"><p class="workspace-close-card__eyebrow">WORKSPACE PROTECTION</p><h1 id="workspace-close-title" tabindex="-1">${title}</h1><p>${copy}</p><details class="workspace-close-help"><summary>Workspace help</summary><p>Close this window and reopen the desktop app to inspect workspace status. Work already started may have completed. Recovery controls are not available in this window.</p></details></section></main></div>`;
}
