import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import { validateCloudReview } from '../preview/cloud-model-confirmation.mjs';

const source = readFileSync(new URL('../preview/cloud-model-confirmation.mjs', import.meta.url), 'utf8');
const html = readFileSync(new URL('../preview/cloud-model-confirmation.html', import.meta.url), 'utf8');

test('cloud confirmation has only fixed cloud review commands and text-only rendering', () => {
  assert.match(source,/get_cloud_model_review/);
  assert.match(source,/confirm_cloud_model_review/);
  assert.match(source,/cancel_cloud_model_review/);
  assert.match(source,/textContent/);
  assert.doesNotMatch(source,/innerHTML|fetch\(|localStorage|window\.open/);
  assert.match(source,/rangoon\.cloud-review-decision\.v1/);
  assert.match(source,/cloud-run/);
});

test('cloud confirmation keeps purpose-specific disclosure and no secret entry', () => {
  assert.match(source,/No selected source text is sent/);
  assert.match(html,/Continue to OS confirmation/);
  assert.match(html,/store: false/);
  assert.doesNotMatch(html,/<input[^>]+(?:password|api[_ -]?key|credential)/i);
});

const cloudRunId = `cloud-run:${'1'.repeat(64)}`;
const requestId = '2'.repeat(64);
const analysisBody = JSON.stringify({ model: 'gpt-5.3', instructions: 'Synthetic instructions', input: '{"schemaVersion":"rangoon.analysis-body.v1"}', store: false, stream: false, background: false, truncation: 'disabled', tools: [], tool_choice: 'none', max_output_tokens: 1024, text: { format: { type: 'json_object' } } });
const analysisReview = () => ({ schemaVersion: 'rangoon.cloud-review.v1', outcome: 'ready', authority: 'none', generation: '1', runId: cloudRunId, requestId, purpose: 'analysis', origin: 'https://api.openai.com', model: 'gpt-5.3', method: 'POST', path: '/v1/responses', credentialRevision: '3'.repeat(64), bodyBytes: new TextEncoder().encode(analysisBody).length, bodySha256: '4'.repeat(64), bodyJson: analysisBody, packId: `pack:${'5'.repeat(64)}`, inputs: [{ input: { kind: 'source', sourceId: `source:${'6'.repeat(64)}`, sha256: '7'.repeat(64) }, observedHead: null, byteLength: 8 }] });

test('cloud confirmation accepts exact native analysis review and rejects hostile variants', () => {
  const valid = analysisReview();
  assert.deepEqual(validateCloudReview(valid), valid);
  assert.equal(validateCloudReview({ ...valid, inputs: { length: 1 } }), null);
  assert.equal(validateCloudReview({ ...valid, path: '/v1/models/gpt-5.3' }), null);
  assert.equal(validateCloudReview({ ...valid, ignored: true }), null);
  const modelCheck = { ...valid, purpose: 'model_check', method: 'GET', path: '/v1/models/gpt-5.3', packId: null, inputs: [], bodyBytes: 0, bodyJson: '', bodySha256: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855' };
  assert.deepEqual(validateCloudReview(modelCheck), modelCheck);
  assert.equal(validateCloudReview({ ...modelCheck, path: '/v1/models/other' }), null);
});
