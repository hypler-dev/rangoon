import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createAnalysisController, escapeText } from '../preview/analysis-model.mjs';

const report = JSON.parse(await readFile(new URL('../fixtures/contracts/source-analysis.golden.json', import.meta.url), 'utf8'));

test('request sends only the native command name', async () => {
  const calls = [];
  const controller = createAnalysisController({ invoke: (...args) => { calls.push(args); return Promise.resolve({ outcome: 'analyzed', report }); } });
  await controller.choose();
  assert.deepEqual(calls, [['select_and_analyze']]);
  assert.equal(controller.getState().report, report);
});

test('golden analysis report carries the exact source and span fields shown by the workbench', () => {
  assert.equal(report.source.byteLength, 144);
  assert.equal(report.authority, 'none');
  assert.equal(report.fragments[1].span.startLine, 3);
  assert.equal(report.fragments[1].reviewState, 'unreviewed');
});

test('late response cannot replace a cleared analysis', async () => {
  let resolve;
  const controller = createAnalysisController({ invoke: () => new Promise(done => { resolve = done; }) });
  const pending = controller.choose();
  controller.clear();
  resolve({ outcome: 'analyzed', report });
  await pending;
  assert.equal(controller.getState().report, null);
  assert.equal(controller.getState().status, 'idle');
});

test('cancelled and rejected picker results preserve the current report and error code', async () => {
  const outcomes = [{ outcome: 'analyzed', report }, { outcome: 'cancelled' }, { outcome: 'rejected', error: { code: 'invalid_name', message: 'Markdown only' } }];
  const controller = createAnalysisController({ invoke: () => Promise.resolve(outcomes.shift()) });
  await controller.choose();
  await controller.choose();
  assert.equal(controller.getState().status, 'cancelled');
  assert.equal(controller.getState().report, report);
  await controller.choose();
  assert.equal(controller.getState().status, 'rejected');
  assert.equal(controller.getState().report, report);
  assert.equal(controller.getState().errorCode, 'invalid_name');
});

test('missing native bridge stays unavailable and does not fabricate an analysis', async () => {
  const controller = createAnalysisController();
  await controller.choose();
  assert.equal(controller.getState().status, 'unavailable');
  assert.equal(controller.getState().report, null);
});

test('untrusted report text is escaped before HTML rendering', () => {
  assert.equal(escapeText('<img src=x onerror=alert(1)>'), '&lt;img src=x onerror=alert(1)&gt;');
  assert.equal(escapeText('"quoted" &'), '&quot;quoted&quot; &amp;');
});
