const invoke = globalThis.__TAURI__?.core?.invoke;
const form = document.querySelector('#credential-form');
const secret = document.querySelector('#credential-secret');
const save = document.querySelector('#credential-save');
const cancel = document.querySelector('#credential-cancel');
const status = document.querySelector('#credential-status');
const encoder = new TextEncoder();
let editId = null;
let pending = false;
const hex = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const only = (value, keys) => value && typeof value === 'object' && !Array.isArray(value) && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
const validEdit = value => only(value, ['schemaVersion', 'provider', 'authority', 'editId']) && value.schemaVersion === 'rangoon.cloud-credential-edit.v1' && value.provider === 'openai' && value.authority === 'none' && hex(value.editId);
const accepted = value => only(value, ['accepted']) && typeof value.accepted === 'boolean';
const validSecret = value => {
  const bytes = encoder.encode(value);
  if (bytes.length >= 1 && bytes.length <= 2048 && bytes.every(byte => byte >= 0x21 && byte <= 0x7e)) return bytes;
  bytes.fill(0);
  return null;
};
const theme = document.querySelector('#credential-theme');
theme.addEventListener('click', () => {
  const light = document.documentElement.dataset.theme !== 'light';
  document.documentElement.dataset.theme = light ? 'light' : 'dark';
  theme.textContent = light ? 'Dark mode' : 'Light mode';
});
const raw = value => encoder.encode(JSON.stringify(value));
const setPending = value => { pending = value; save.disabled = value || !editId; cancel.disabled = value || !editId; secret.disabled = value || !editId; };
async function decide(command, value = null) {
  if (!editId || pending || typeof invoke !== 'function') return;
  setPending(true);
  let bytes = null;
  try {
    bytes = raw(value ?? { schemaVersion: 'rangoon.cloud-credential-decision.v1', editId });
    if (bytes.length > 12544) throw new Error('too_large');
    const result = await invoke(command, bytes);
    if (!accepted(result) || !result.accepted) throw new Error('rejected');
    status.textContent = command === 'submit_cloud_credential' ? 'Waiting for the operating-system Save or Cancel decision…' : 'Cancellation sent. This entry can now close.';
  } catch {
    status.textContent = 'Credential decision unavailable. Close this entry and start again from the main workbench.';
    setPending(false);
  } finally {
    if (bytes) bytes.fill(0);
  }
}
form.addEventListener('submit', event => {
  event.preventDefault();
  if (!editId || pending) return;
  const typed = secret.value;
  secret.value = '';
  const bytes = validSecret(typed);
  if (!bytes) { status.textContent = 'Use 1–2,048 visible ASCII characters without whitespace.'; return; }
  let payload;
  try { payload = { schemaVersion: 'rangoon.cloud-credential-submit.v1', editId, secret: new TextDecoder().decode(bytes) }; }
  finally { bytes.fill(0); }
  void decide('submit_cloud_credential', payload);
});
cancel.addEventListener('click', () => { secret.value = ''; void decide('cancel_cloud_credential_edit'); });
document.addEventListener('keydown', event => { if (event.key === 'Escape') { event.preventDefault(); secret.value = ''; void decide('cancel_cloud_credential_edit'); } });
try {
  const result = typeof invoke === 'function' ? await invoke('get_cloud_credential_edit') : null;
  if (!validEdit(result)) throw new Error('unavailable');
  editId = result.editId;
  status.textContent = 'Enter a new OpenAI credential. Existing credential remains unreadable here.';
  setPending(false);
  secret.focus();
} catch {
  secret.value = '';
  secret.disabled = true;
  status.textContent = 'Credential entry unavailable. Close this entry and return to the main workbench.';
}
