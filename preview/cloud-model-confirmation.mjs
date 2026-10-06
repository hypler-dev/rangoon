const encoder = new TextEncoder();
const DIGEST = /^[0-9a-f]{64}$/;
const RUN = /^cloud-run:[0-9a-f]{64}$/;
const PACK = /^pack:[0-9a-f]{64}$/;
const EMPTY_BODY_SHA256 = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855';

const object = value => Boolean(value) && typeof value === 'object' && !Array.isArray(value);
const only = (value, keys) => object(value) && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
const decimal = value => typeof value === 'string' && /^(0|[1-9][0-9]*)$/.test(value);
const unsigned = value => Number.isSafeInteger(value) && value >= 0;
const model = value => typeof value === 'string' && /^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$/.test(value);
const source = value => typeof value === 'string' && /^source:[0-9a-f]{64}$/.test(value);
const capability = value => typeof value === 'string' && /^capability:[0-9a-f]{64}$/.test(value);
const revision = value => typeof value === 'string' && /^revision:[0-9a-f]{64}$/.test(value);
const inputReference = value => object(value)
  && ((only(value, ['kind', 'sourceId', 'sha256']) && value.kind === 'source' && source(value.sourceId) && DIGEST.test(value.sha256))
    || (only(value, ['kind', 'capabilityId', 'revisionId', 'sha256']) && value.kind === 'revision' && capability(value.capabilityId) && revision(value.revisionId) && DIGEST.test(value.sha256)));
const dependency = value => object(value) && only(value, ['input', 'observedHead', 'byteLength'])
  && inputReference(value.input) && unsigned(value.byteLength) && (value.observedHead === null || revision(value.observedHead));

export function validateCloudReview(value) {
  const keys = ['schemaVersion', 'outcome', 'authority', 'generation', 'runId', 'requestId', 'purpose', 'origin', 'model', 'method', 'path', 'credentialRevision', 'bodyBytes', 'bodySha256', 'bodyJson', 'packId', 'inputs'];
  if (!only(value, keys) || value.schemaVersion !== 'rangoon.cloud-review.v1' || value.outcome !== 'ready' || value.authority !== 'none') return null;
  if (!decimal(value.generation) || !RUN.test(value.runId) || !DIGEST.test(value.requestId) || value.origin !== 'https://api.openai.com' || !model(value.model) || !DIGEST.test(value.credentialRevision)) return null;
  if (!unsigned(value.bodyBytes) || value.bodyBytes > 262144 || !DIGEST.test(value.bodySha256) || typeof value.bodyJson !== 'string' || encoder.encode(value.bodyJson).length !== value.bodyBytes || !Array.isArray(value.inputs) || !value.inputs.every(dependency)) return null;
  if (value.purpose === 'model_check') {
    return value.method === 'GET' && value.path === `/v1/models/${value.model}` && value.bodyBytes === 0 && value.bodyJson === '' && value.bodySha256 === EMPTY_BODY_SHA256 && value.packId === null && value.inputs.length === 0 ? value : null;
  }
  if (value.purpose !== 'analysis' || value.method !== 'POST' || value.path !== '/v1/responses' || !PACK.test(value.packId) || value.inputs.length < 1 || value.inputs.length > 16 || value.bodyBytes === 0) return null;
  try {
    const request = JSON.parse(value.bodyJson);
    return object(request) && only(request, ['model', 'instructions', 'input', 'store', 'stream', 'background', 'truncation', 'tools', 'tool_choice', 'max_output_tokens', 'text'])
      && request.model === value.model && typeof request.instructions === 'string' && typeof request.input === 'string'
      && request.store === false && request.stream === false && request.background === false && request.truncation === 'disabled'
      && Array.isArray(request.tools) && request.tools.length === 0 && request.tool_choice === 'none'
      && Number.isSafeInteger(request.max_output_tokens) && request.max_output_tokens >= 1 && request.max_output_tokens <= 32768
      && object(request.text) && only(request.text, ['format']) && object(request.text.format) && only(request.text.format, ['type']) && request.text.format.type === 'json_object' ? value : null;
  } catch { return null; }
}

function row(list, label, value) {
  const item = document.createElement('div');
  const term = document.createElement('dt');
  const detail = document.createElement('dd');
  term.textContent = label;
  detail.textContent = String(value);
  item.append(term, detail);
  list.append(item);
}

function renderReview(value) {
  const facts = document.querySelector('#cloud-review-facts');
  for (const [label, fact] of [
    ['Origin', value.origin], ['Model', value.model], ['Purpose', value.purpose],
    ['Method/path', `${value.method} ${value.path}`], ['Credential revision', value.credentialRevision],
    ['Request ID', value.requestId], ['Pack ID', value.packId ?? 'None'], ['Body bytes', value.bodyBytes], ['Body SHA-256', value.bodySha256],
  ]) row(facts, label, fact);
  for (const dependency of value.inputs) {
    const item = document.createElement('li');
    item.textContent = JSON.stringify(dependency);
    document.querySelector('#cloud-review-inputs').append(item);
  }
  document.querySelector('#cloud-review-disclosure').textContent = value.purpose === 'analysis'
    ? 'Selected text and stored credential will be sent to OpenAI. Cost, retention, and region are unknown. store: false is not Zero Data Retention. Cancellation cannot recall dispatched data.'
    : 'The stored credential and selected model identifier will be sent to OpenAI. No selected source text is sent. Cost, retention, and region are unknown. store: false is not Zero Data Retention. Cancellation cannot recall dispatched data.';
  document.querySelector('#cloud-review-body').textContent = value.bodyJson || 'No source selected. GET model metadata uses zero request-body bytes.';
  document.querySelector('#cloud-review-send').textContent = 'Continue to OS confirmation';
  document.querySelector('#cloud-review-content').hidden = false;
  document.querySelector('#cloud-review-status').textContent = 'Review fixed origin, metadata, dependencies and exact body before OS confirmation.';
  document.querySelector('#cloud-review-send').disabled = false;
  document.querySelector('#cloud-review-cancel').disabled = false;
}

async function startConfirmation() {
  const invoke = globalThis.__TAURI__?.core?.invoke;
  let retained = null;
  let pending = false;
  const status = document.querySelector('#cloud-review-status');
  const theme = document.querySelector('#cloud-review-theme');
  theme.addEventListener('click', () => {
    const light = document.documentElement.dataset.theme !== 'light';
    document.documentElement.dataset.theme = light ? 'light' : 'dark';
    theme.textContent = light ? 'Dark mode' : 'Light mode';
  });
  const decide = async command => {
    if (!retained || pending || typeof invoke !== 'function') return;
    pending = true;
    document.querySelector('#cloud-review-send').disabled = true;
    document.querySelector('#cloud-review-cancel').disabled = true;
    const decision = new TextEncoder().encode(JSON.stringify({ schemaVersion: 'rangoon.cloud-review-decision.v1', runId: retained.runId, requestId: retained.requestId }));
    try { await invoke(command, decision); } catch {}
    status.textContent = 'Decision finished or unavailable. Return to the main workbench.';
  };
  document.querySelector('#cloud-review-send').onclick = () => decide('confirm_cloud_model_review');
  document.querySelector('#cloud-review-cancel').onclick = () => decide('cancel_cloud_model_review');
  try {
    retained = validateCloudReview(typeof invoke === 'function' ? await invoke('get_cloud_model_review', {}) : null);
    if (!retained) throw new Error('Cloud request unavailable');
    renderReview(retained);
  } catch {
    status.textContent = 'This cloud request is unavailable. Nothing is approved here.';
  }
}

if (typeof document !== 'undefined') void startConfirmation();
