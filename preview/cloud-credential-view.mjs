import { escapeText } from './analysis-model.mjs';

const text = escapeText;
const ready = state => state.bridgeAvailable === true && !state.pending;
const status = state => state.status === 'stored' ? 'Stored in OS custody' : state.status === 'missing' ? 'No credential stored' : 'Unknown';
const backend = state => ({ macos_keychain: 'macOS Keychain', windows_credential_manager: 'Windows Credential Manager', linux_secret_service: 'Linux Secret Service' })[state.backend] ?? 'Unknown';

export function renderCloudCredentialView(state = {}) {
  const canRemove = ready(state) && !state.needsInspection && state.status === 'stored';
  const action = state.pending === 'inspect' ? 'Inspecting…' : state.pending === 'edit' ? 'Waiting for isolated entry…' : state.pending === 'remove' ? 'Waiting for OS decision…' : 'Inspect custody';
  const alert = state.error ? `<p class="cloud-credential-alert" role="alert">${text(state.message)}</p>` : '';
  return `<section class="cloud-credential-panel" aria-labelledby="cloud-credential-title" aria-busy="${Boolean(state.pending)}"><header><div><p class="analysis-kicker">CLOUD CREDENTIAL / OS CUSTODY</p><h2 id="cloud-credential-title" tabindex="-1">OpenAI credential</h2></div><span class="cloud-credential-state">${text(status(state))}</span></header><div class="cloud-credential-body"><p>Optional credential slot only. Storing a credential does not check a provider, select a payload, or send source text.</p><dl><div><dt>Provider</dt><dd>OpenAI</dd></div><div><dt>Authority</dt><dd>None</dd></div><div><dt>Custody backend</dt><dd>${text(backend(state))}</dd></div><div><dt>Revision</dt><dd class="analysis-mono">${text(state.revision ?? 'Unknown')}</dd></div></dl><p class="cloud-credential-boundary">OS custody does not protect a compromised user session or application process. Cloud transport remains unavailable.</p>${!state.bridgeAvailable ? '<p class="cloud-credential-alert" role="status">Native custody unavailable in browser preview. No provider check or send was made.</p>' : ''}${state.message ? `<p class="cloud-credential-status" role="status">${text(state.message)}</p>` : ''}${alert}${state.needsInspection ? '<p class="cloud-credential-status">Inspect custody before another credential change.</p>' : ''}<div class="cloud-credential-actions"><button id="cloud-credential-inspect" class="analysis-button" type="button" ${ready(state) ? '' : 'disabled'}>${action}</button><button id="cloud-credential-edit" class="analysis-button analysis-button--primary" type="button" ${ready(state) && !state.needsInspection ? '' : 'disabled'}>${state.status === 'stored' ? 'Replace credential' : 'Add credential'}</button><button id="cloud-credential-remove" class="analysis-button" type="button" ${canRemove ? '' : 'disabled'}>Remove credential</button></div></div></section>`;
}

export function cloudCredentialAction(target, controller) {
  if (!target || target.disabled) return false;
  if (target.matches('#cloud-credential-inspect')) { void controller.inspect(); return true; }
  if (target.matches('#cloud-credential-edit')) { void controller.edit(); return true; }
  if (target.matches('#cloud-credential-remove')) { void controller.remove(); return true; }
  return false;
}
