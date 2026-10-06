// Deliberately isolated from the main workbench, storage, URL state and events.
// Dynamic source and request data are inserted only as text.
const invoke = globalThis.__TAURI__?.core?.invoke;
const status = document.querySelector('#review-status');
const send = document.querySelector('#review-send');
const cancel = document.querySelector('#review-cancel');
let retained = null;
let pending = false;
const hex = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const handle = (value, prefix) => typeof value === 'string' && value.startsWith(prefix) && hex(value.slice(prefix.length));
const only = (value, keys) => value && typeof value === 'object' && !Array.isArray(value) && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
function validInput(entry) {
  if (!only(entry, ['input', 'observedHead', 'byteLength']) || !Number.isSafeInteger(entry.byteLength) || entry.byteLength < 1 || entry.byteLength > 262144) return false;
  const ref = entry.input;
  if (ref?.kind === 'source') return only(ref, ['kind','sourceId','sha256']) && handle(ref.sourceId, 'source:') && hex(ref.sha256) && entry.observedHead === null;
  return ref?.kind === 'revision' && only(ref, ['kind','capabilityId','revisionId','sha256']) && handle(ref.capabilityId, 'capability:') && handle(ref.revisionId, 'revision:') && hex(ref.sha256) && handle(entry.observedHead, 'revision:');
}
function validReview(value) {
  return only(value, ['schemaVersion','outcome','authority','generation','runId','requestId','origin','model','bodyBytes','bodyJson','packId','inputs'])
    && value.schemaVersion === 'rangoon.local-review.v1' && value.outcome === 'ready' && value.authority === 'none'
    && typeof value.generation === 'string' && /^(0|[1-9][0-9]*)$/.test(value.generation)
    && handle(value.runId, 'run:') && hex(value.requestId) && handle(value.packId, 'pack:')
    && typeof value.origin === 'string' && /^http:\/\/(127\.0\.0\.1|\[::1\]):[1-9][0-9]{0,4}$/.test(value.origin)
    && typeof value.model === 'string' && value.model.length >= 1 && value.model.length <= 128
    && typeof value.bodyJson === 'string' && Number.isSafeInteger(value.bodyBytes) && value.bodyBytes > 0 && value.bodyBytes <= 262144 && new TextEncoder().encode(value.bodyJson).length === value.bodyBytes
    && Array.isArray(value.inputs) && value.inputs.length >= 1 && value.inputs.length <= 16 && value.inputs.every(validInput);
}
function field(list, label, value) {
  const row = document.createElement('div');
  const dt = document.createElement('dt'); dt.textContent = label;
  const dd = document.createElement('dd'); dd.textContent = String(value);
  row.append(dt, dd); list.append(row);
}
function show(value) {
  const facts = document.querySelector('#review-facts');
  for (const [label, text] of [['Endpoint',value.origin],['Model',value.model],['Outgoing bytes',value.bodyBytes],['Selected records',value.inputs.length],['Pack ID',value.packId],['Request ID',value.requestId]]) field(facts,label,text);
  const list = document.querySelector('#review-inputs');
  for (const entry of value.inputs) {
    const item = document.createElement('li'); const fields = document.createElement('dl');
    field(fields,'Kind',entry.input.kind === 'source' ? 'Saved source' : 'Saved skill revision');
    if (entry.input.kind === 'source') field(fields,'Source ID',entry.input.sourceId);
    else { field(fields,'Skill ID',entry.input.capabilityId); field(fields,'Revision ID',entry.input.revisionId); field(fields,'Observed current head',entry.observedHead); }
    field(fields,'Content SHA-256',entry.input.sha256); field(fields,'Exact input bytes',entry.byteLength);
    item.append(fields); list.append(item);
  }
  document.querySelector('#review-payload').textContent = value.bodyJson;
  document.querySelector('#review-content').hidden = false;
  status.textContent = 'Review the destination, selected records and exact payload before continuing.';
  send.disabled = false; cancel.disabled = false;
  cancel.focus();
}
async function decide(command) {
  if (!retained || pending) return;
  pending = true; send.disabled = true; cancel.disabled = true;
  status.textContent = command === 'confirm_local_model_review' ? 'Waiting for the operating-system decision. No text is sent without confirmation.' : 'Cancelling this request…';
  const payload = new TextEncoder().encode(JSON.stringify({schemaVersion:'rangoon.local-review-decision.v1',runId:retained.runId,requestId:retained.requestId}));
  try { await invoke(command,payload); } catch { /* The native parent may close before IPC returns. */ }
  status.textContent = 'Decision finished or unavailable. Return to the main workbench for the request outcome. Close this window if it remains open.';
}
send.addEventListener('click', () => { void decide('confirm_local_model_review'); });
cancel.addEventListener('click', () => { void decide('cancel_local_model_review'); });
try {
  const result = typeof invoke === 'function' ? await invoke('get_local_model_review') : null;
  if (!validReview(result)) throw new Error('unavailable');
  retained = result; show(result);
} catch {
  status.textContent = 'This request is unavailable. Close this window and prepare a new request in Rangoon. Nothing is approved here.';
}
