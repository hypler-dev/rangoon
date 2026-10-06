import assert from 'node:assert/strict';
import test from 'node:test';
import { bindModelAssistanceView, renderModelAssistanceView } from '../preview/model-assistance-view.mjs';

const id = 'a'.repeat(64);
test('profile drafts survive same-route renders but never cross endpoint boundaries', () => {
  let current = state({ endpoint: 'local', generation: null });
  let inputs = [];
  const root = {
    ownerDocument: { activeElement: null },
    set innerHTML(html) {
      inputs = [...html.matchAll(/<input\b[^>]*name="([^"]+)"[^>]*value="([^"]*)"[^>]*>/g)]
        .map(([, name, value]) => ({ name, value }));
    },
    querySelectorAll: selector => selector === '#model-profile-form input' ? inputs : [],
    querySelector: selector => inputs.find(input => selector === `[name="${input.name}"]`) ?? null,
    addEventListener() {}, removeEventListener() {},
  };
  const binding = bindModelAssistanceView(root, { getState: () => current });
  inputs.find(input => input.name === 'model').value = 'unsaved:local';
  binding.render();
  assert.equal(inputs.find(input => input.name === 'model').value, 'unsaved:local');
  current = state({ endpoint: 'cloud', generation: null, profile: { profileId: 'openai', model: 'synthetic-v1', maxOutputTokens: 1024 } });
  binding.render();
  assert.equal(inputs.find(input => input.name === 'profileId').value, 'openai');
  assert.equal(inputs.find(input => input.name === 'model').value, 'synthetic-v1');
  binding.dispose();
});
const state = (overrides = {}) => ({
  bridgeAvailable: true,
  tab: 'payload',
  session: {profile: {}},
  profile: { profileId: 'local', host: '127.0.0.1', port: 11434, model: 'family:model', maxOutputTokens: 1024 },
  task: 'classify_v1',
  inputs: [{ kind: 'source', sourceId: `source:${id}` }],
  snapshots: [{ sourceId: `source:${id}`, displayName: 'Saved rules', sha256: id, byteLength: 128 }],
  capabilities: [],
  selectedCapabilities: {},
  prepared: { origin: 'http://127.0.0.1:11434', model: 'family:model', pack: {packId: `pack:${id}`}, preparedId: `prepared:${id}`, requestId: id, bodyJson: '{"task":"classify_v1"}', bodyBytes: 22, inputs: [{ input: { kind: 'source', sourceId: `source:${id}`, sha256: id }, observedHead: null, byteLength: 128 }] },
  ...overrides
});

test('renders session-only profile, exact payload, and no-authority result surface', () => {
  const html = renderModelAssistanceView(state());
  assert.match(html, /SESSION-ONLY PROFILE/);
  assert.match(html, /Check endpoint without source/);
  assert.match(html, /id="model-payload-body"/);
  assert.match(html, /SHA-256/);
  assert.match(html, /Unknown/);
  assert.match(html, /Review &amp; send|Review & send/);
  assert.match(html, /authority/);
  assert.match(html, /Proposal application is unavailable/);
});

test('uses keyboard-operable tab semantics and explicit saved selectors', () => {
  const html = renderModelAssistanceView(state({ tab: 'coverage' }));
  assert.match(html, /role="tablist"/);
  assert.match(html, /id="model-tab-coverage"[^>]*aria-selected="true"[^>]*tabindex="0"/);
  assert.match(html, /data-model-record="0"/);
  assert.match(html, /Compare · exactly 2 revisions/);
  assert.doesNotMatch(html, /textarea/);
});

test('escapes all model and saved-record text as inert data', () => {
  const hostile = '<img src=x onerror=alert(1)>';
  const html = renderModelAssistanceView(state({ snapshots: [{ sourceId: `source:${id}`, displayName: hostile, sha256: id, byteLength: 1 }], completion: { freshness: 'current', completion: { packId: hostile, responseSha256: id, proposals: { proposals: [{ title: hostile, kind: hostile, authoredText: hostile, explanation: hostile, citations: [{ input: { sourceId: hostile }, startByte: 0, endByte: 1 }] }], uncertainties: [hostile] } } } }));
  assert.match(html, /&lt;img src=x onerror=alert\(1\)&gt;/);
  assert.doesNotMatch(html, /<img src=x onerror/);
  assert.doesNotMatch(html, /href=/);
  assert.match(html, /Authored output/);
  assert.match(html, /Explanation/);
  assert.match(html, /Citations/);
});

test('browser mode disables calls and says no endpoint call was made', () => {
  const html = renderModelAssistanceView(state({ bridgeAvailable: false, prepared: null }));
  assert.match(html, /Native model assistance is unavailable in browser preview. No endpoint call was made/);
  assert.match(html.match(/id="model-sourcefree-check"[^>]*>/)[0], /disabled/);
  assert.match(html.match(/id="model-prepare"[^>]*>/)[0], /disabled/);
  assert.match(html.match(/id="model-send"[^>]*>/)[0], /disabled/);
});

test('cloud route fixes origin and keeps credential custody separate', () => {
  const html = renderModelAssistanceView(state({ endpoint: 'cloud', profile: { profileId: 'openai', model: 'gpt-test', maxOutputTokens: 1024 } }));
  assert.match(html,/name="model-endpoint" value="cloud" checked/);
  assert.match(html,/https:\/\/api\.openai\.com/);
  assert.match(html,/id="cloud-model-profile"/);
  assert.match(html,/id="cloud-model-model"/);
  assert.match(html,/id="cloud-model-sourcefree-check"/);
  assert.match(html,/Credential custody is separate/);
  assert.doesNotMatch(html,/type="password"/);
});

test('css has three, two, and one-column responsive workbench plus reduced motion', async () => {
  const { readFile } = await import('node:fs/promises');
  const css = await readFile(new URL('../preview/model-assistance.css', import.meta.url), 'utf8');
  assert.match(css, /model-workbench\{display:grid;grid-template-columns:minmax\(220px,.75fr\) minmax\(0,1.32fr\) minmax\(240px,.85fr\)/);
  assert.match(css, /@media \(max-width:1120px\)[\s\S]*model-workbench\{grid-template-columns:minmax\(230px,.7fr\) minmax\(0,1.3fr\)/);
  assert.match(css, /@media \(max-width:760px\)[\s\S]*model-workbench\{grid-template-columns:minmax\(0,1fr\)/);
  assert.match(css, /prefers-reduced-motion:reduce/);
});


test('renders nested native proposals and pack identity, with honest pending controls', () => {
  const html = renderModelAssistanceView(state({pending:'send',activeRunId:null,completion:{freshness:'stale',completion:{packId:`pack:${id}`,responseSha256:id,proposals:{proposals:[{kind:'classification',title:'Named synthetic proposal',authoredText:'Exact authored bytes',explanation:'Explicit explanation',citations:[]}],uncertainties:['Synthetic uncertainty']}}}}));
  for (const content of [`pack:${id}`,'Named synthetic proposal','Exact authored bytes','Explicit explanation','Synthetic uncertainty','Freshness is not current']) assert.ok(html.includes(content));
  assert.match(html.match(/id="model-cancel"[^>]*>/)[0],/disabled/);
  assert.match(html.match(/id="model-prepare"[^>]*>/)[0],/disabled/);
  assert.match(html,/Review or request active/);
});


test('cloud check receipt exposes metadata and uncertainty while escaping provider text', () => {
  const hostile = '<img src=x onerror=alert(1)>';
  const html = renderModelAssistanceView(state({ endpoint: 'cloud', check: {
    requestId: id, profileSha256: id, credentialRevision: id, responseSha256: id,
    observedModel: hostile, ownedBy: hostile, created: 0, shutdownDate: null,
    observation: 'model_visibility_only', inferenceCompatibility: 'unknown',
    processingLocation: 'unknown', retention: 'unknown', cost: 'unknown', authority: 'none'
  }, prepared: { ...state().prepared, credentialRevision: id, bodySha256: id } }));
  for (const label of ['Model visibility only', 'Provider-declared owner', 'Created (Unix seconds)', 'Shutdown date', 'Not reported', 'Inference compatibility', 'Profile SHA-256', 'Credential revision', 'POST /v1/responses', 'Body SHA-256']) assert.ok(html.includes(label));
  assert.match(html, /&lt;img src=x onerror=alert\(1\)&gt;/);
  assert.doesNotMatch(html, /<img src=x/);
  assert.match(html, /Created \(Unix seconds\)<\/dt><dd>0<\/dd>/);
  assert.match(html, /Authority<\/dt><dd>none<\/dd>/);
});

test('local check receipt reports version without promoting trust', () => {
  const html = renderModelAssistanceView(state({ check: { serverVersion: 'synthetic-version', profileSha256: id, processingLocation: 'unknown', authority: 'none' } }));
  assert.match(html, /Local protocol observation/);
  assert.match(html, /synthetic-version/);
  assert.match(html, /does not establish model trust/);
});
