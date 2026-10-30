import { escapeText } from './analysis-model.mjs';
import { icon } from './icons.mjs';

const text = escapeText;
const copy = Object.freeze({
  fresh: ['Set up encrypted workspace', 'Create an encrypted workspace on this computer.'],
  legacy_found: ['Existing workspace found', 'Review conversion before creating an encrypted copy.'],
  conversion_review: ['Copy to encrypted workspace', 'Review the copy in the native confirmation before any change begins.'],
  copying: ['Copying workspace', 'Rangoon is completing the current local copy step.'],
  restart_required: ['Copy saved', 'Close and reopen Rangoon to verify the encrypted copy.'],
  verify_required: ['Verify encrypted copy', 'Verification is separate from normal workspace unlock.'],
  recovery_required: ['Review interrupted conversion', 'Review the retained conversion state before any further action.'],
  unavailable: ['Workspace unavailable', 'This startup route cannot continue safely.'],
  checking: ['Checking workspace', 'Checking the local startup route.'],
  working: ['Preparing workspace', 'Waiting for the native desktop response.'],
  browser: ['Workspace unavailable', 'Open this page in the Rangoon desktop app.'],
});
const details = Object.freeze({
  fresh: ['Encrypted workspace', 'Fresh setup', 'No original copy is part of this setup.'],
  legacy_found: ['Existing local workspace', 'Conversion review not started', 'Original copy status has not been assessed.'],
  conversion_review: ['Existing local workspace', 'Conversion review', 'Original copy status has not been assessed.'],
  copying: ['Encrypted destination and retained original', 'Copy in progress', 'Original copy is retained while this conversion is reviewed.'],
  restart_required: ['Encrypted destination', 'Restart required', 'Original copy status is retained by native review.'],
  verify_required: ['Encrypted destination', 'Verification required', 'Original copy status is retained by native review.'],
  recovery_required: ['Local startup state', 'Recovery review', 'Original copy is preserved for review.'],
  unavailable: ['Local startup route', 'Unavailable', 'Original copy status is unavailable.'],
});
const actions = Object.freeze({
  setup: 'Set up encrypted workspace',
  quit: 'Quit Rangoon',
  review_conversion: 'Review conversion',
  copy: 'Copy to encrypted workspace',
  cancel: 'Cancel review',
  verify: 'Verify encrypted copy',
  recover: 'Review recovery',
});
const phaseNames = Object.freeze({
  creating_destination: 'Creating encrypted destination',
  copying_records: 'Copying records',
  checking_copy: 'Checking copy',
  recording_selection: 'Recording selection',
});
const validStatus = new Set(['checking', 'ready', 'working', 'handoff', 'browser', 'unavailable']);

function displayState(state) {
  if (!validStatus.has(state?.status)) return 'unavailable';
  if (state.status === 'ready' && copy[state.view?.state]) return state.view.state;
  if (state.status === 'working') return 'working';
  if (state.status === 'checking') return 'checking';
  if (state.status === 'browser') return 'browser';
  return 'unavailable';
}

function originalNotice(value) {
  if (value === 'present_unencrypted') return '<p class="desktop-startup__notice">The original copy is kept. It remains unencrypted.</p>';
  if (value === 'not_verified') return '<p class="desktop-startup__notice desktop-startup__notice--warning">The original copy could not be verified.</p>';
  if (value === 'unassessed') return '<p class="desktop-startup__neutral">Original copy status has not been assessed.</p>';
  return '';
}

function controls(state, key, { unavailableQuit = false, quitPending = false } = {}) {
  if (state?.status === 'ready' && Array.isArray(state.view?.actions)) {
    return state.view.actions.filter(action => Object.hasOwn(actions, action)).map((action, index) =>
      `<button class="desktop-startup__action${index === 0 && action !== 'quit' ? ' desktop-startup__action--primary' : ''}" type="button" data-desktop-startup-action="${action}">${actions[action]}</button>`
    ).join('');
  }
  if (key === 'unavailable' && state?.status === 'unavailable' && unavailableQuit) {
    return `<button class="desktop-startup__action" type="button" data-desktop-startup-unavailable-quit${quitPending ? ' disabled' : ''}>${quitPending ? 'Quitting Rangoon…' : 'Quit Rangoon'}</button>`;
  }
  return '';
}

export function renderDesktopStartup(state = {}, { theme = 'dark', unavailableQuit = false, quitPending = false } = {}) {
  if (state?.status === 'handoff') return '';
  const key = displayState(state);
  const [heading, defaultDescription] = copy[key];
  const [storage, defaultPhase, fallbackOriginal] = details[key] ?? details.unavailable;
  const nativePhase = key === 'copying' ? phaseNames[state?.view?.copyPhase] : null;
  const description = nativePhase ? `Rangoon is ${nativePhase.toLowerCase()}.` : defaultDescription;
  const phase = nativePhase ?? defaultPhase;
  const original = state?.status === 'ready' ? state.view?.retainedOriginal : null;
  const originalDetail = original === 'present_unencrypted' ? 'Present and unencrypted'
    : original === 'not_verified' ? 'Could not be verified'
      : original === 'unassessed' ? 'Not assessed'
        : original === 'not_applicable' ? 'Not applicable' : fallbackOriginal;
  const themeName = theme === 'light' ? 'light' : 'dark';
  const themeLabel = themeName === 'light' ? 'Dark mode' : 'Light mode';
  return `<div class="analysis-shell desktop-startup-shell" data-desktop-startup-state="${key}" data-desktop-startup-theme="${themeName}"><aside class="analysis-rail desktop-startup-rail"><div class="analysis-brand"><img src="assets/brand-symbol.png" alt=""><span>Rangoon.si</span></div><button class="analysis-theme" type="button" data-desktop-startup-theme aria-label="${themeLabel}">${icon('theme', { size: 16 })}<span>${themeLabel}</span></button></aside><main id="analysis-main" class="desktop-startup-main"><section class="desktop-startup-card" aria-labelledby="desktop-startup-title" aria-busy="${['checking', 'working'].includes(state?.status) ? 'true' : 'false'}"><p class="desktop-startup__eyebrow">ENCRYPTED WORKSPACE</p><h1 id="desktop-startup-title" tabindex="-1">${heading}</h1><p class="desktop-startup__copy" role="status" aria-live="polite" aria-atomic="true">${description}</p>${originalNotice(original)}<div class="desktop-startup__actions">${controls(state, key, { unavailableQuit, quitPending })}</div><details class="desktop-startup__details"><summary>Details</summary><dl><div><dt>Storage</dt><dd>${text(storage)}</dd></div><div><dt>Phase</dt><dd>${text(phase)}</dd></div><div><dt>Original copy</dt><dd>${text(originalDetail)}</dd></div><div><dt>Portable backup</dt><dd>Encrypted portable backup unavailable.</dd></div></dl></details></section></main></div>`;
}
