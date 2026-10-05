import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createAnalysisController, escapeText } from '../preview/analysis-model.mjs';

const report = JSON.parse(await readFile(new URL('../fixtures/contracts/source-analysis.golden.json', import.meta.url), 'utf8'));

const snapshots = [{
  sourceId: report.source.id,
  displayName: report.source.displayName,
  sha256: report.source.sha256,
  byteLength: report.source.byteLength,
  savedAtMs: 1_728_000_000_000,
}];

const listed = { outcome: 'listed', snapshots };
const native = handlers => (command, args) => handlers(command, args);

test('loads saved metadata on launch and sends only source IDs to native mutations', async () => {
  const calls = [];
  const controller = createAnalysisController({ invoke: native((command, args) => {
    calls.push([command, args]);
    if (command === 'list_snapshots') return Promise.resolve(listed);
    if (command === 'select_and_analyze') return Promise.resolve({ outcome: 'analyzed', report });
    if (command === 'save_analysis') return Promise.resolve({ outcome: 'saved', snapshot: snapshots[0], alreadySaved: false });
    throw new Error(`Unexpected command: ${command}`);
  }) });
  await Promise.resolve();
  await controller.choose();
  await controller.save();
  assert.deepEqual(calls, [
    ['list_snapshots', undefined],
    ['select_and_analyze', undefined],
    ['save_analysis', { sourceId: report.source.id }],
    ['list_snapshots', undefined],
  ]);
  assert.deepEqual(controller.getState().snapshots, snapshots);
  assert.equal(controller.getState().report, report);
});

test('golden analysis report carries the exact source and span fields shown by the workbench', () => {
  assert.equal(report.source.byteLength, 144);
  assert.equal(report.authority, 'none');
  assert.equal(report.fragments[1].span.startLine, 3);
  assert.equal(report.fragments[1].reviewState, 'unreviewed');
});

test('late response cannot replace a cleared analysis', async () => {
  let resolveChoose;
  const calls = [];
  const controller = createAnalysisController({ invoke: native(command => {
    calls.push(command);
    if (command === 'list_snapshots') return Promise.resolve({ outcome: 'listed', snapshots: [] });
    if (command === 'select_and_analyze') return new Promise(done => { resolveChoose = done; });
    if (command === 'clear_analysis') return Promise.resolve({ outcome: 'cleared' });
  }) });
  const pending = controller.choose();
  await controller.clear();
  resolveChoose({ outcome: 'analyzed', report });
  await pending;
  assert.equal(controller.getState().report, null);
  assert.equal(controller.getState().status, 'idle');
  assert.deepEqual(calls, ['list_snapshots', 'select_and_analyze', 'clear_analysis']);
});

test('cancelled and rejected picker results preserve the current report and error code', async () => {
  const outcomes = [{ outcome: 'listed', snapshots: [] }, { outcome: 'analyzed', report }, { outcome: 'cancelled' }, { outcome: 'rejected', error: { code: 'invalid_name', message: 'Markdown only' } }];
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
  assert.equal(controller.getState().snapshotsStatus, 'unavailable');
  assert.deepEqual(controller.getState().snapshots, []);
});

test('save success refreshes metadata, duplicate success does not create a duplicate row, and save failure preserves the report', async () => {
  const outcomes = [
    { outcome: 'listed', snapshots: [] },
    { outcome: 'analyzed', report },
    { outcome: 'saved', snapshot: snapshots[0], alreadySaved: false },
    { outcome: 'listed', snapshots },
    { outcome: 'saved', snapshot: snapshots[0], alreadySaved: true },
    { outcome: 'listed', snapshots: [...snapshots, snapshots[0]] },
    { outcome: 'failed', error: { code: 'disk_full', message: 'No local space' } },
  ];
  const controller = createAnalysisController({ invoke: () => Promise.resolve(outcomes.shift()) });
  await controller.choose();
  await controller.save();
  await controller.save();
  assert.equal(controller.getState().snapshots.length, 1);
  await controller.save();
  assert.equal(controller.getState().status, 'failed');
  assert.equal(controller.getState().errorCode, 'disk_full');
  assert.equal(controller.getState().report, report);
});

test('open failure preserves current analysis and allows retry to replace it', async () => {
  const opened = { ...report, source: { ...report.source, id: 'saved-source' } };
  const outcomes = [
    { outcome: 'listed', snapshots },
    { outcome: 'analyzed', report },
    { outcome: 'failed', error: { code: 'missing', message: 'Saved source is unavailable' } },
    { outcome: 'analyzed', report: opened },
  ];
  const controller = createAnalysisController({ invoke: () => Promise.resolve(outcomes.shift()) });
  await controller.choose();
  await controller.openSnapshot(report.source.id);
  assert.equal(controller.getState().report, report);
  assert.equal(controller.getState().errorCode, 'missing');
  await controller.openSnapshot(report.source.id);
  assert.equal(controller.getState().report, opened);
  assert.equal(controller.getState().selectedFragmentId, opened.fragments[0].id);
});

test('a pending save is not cancelled by choose, open, or clear requests', async () => {
  let resolveSave;
  const calls = [];
  const controller = createAnalysisController({ invoke: native((command, args) => {
    calls.push([command, args]);
    if (command === 'list_snapshots') return Promise.resolve({ outcome: 'listed', snapshots });
    if (command === 'select_and_analyze') return Promise.resolve({ outcome: 'analyzed', report });
    if (command === 'save_analysis') return new Promise(done => { resolveSave = done; });
    throw new Error(`Unexpected command: ${command}`);
  }) });
  await controller.choose();
  const saving = controller.save();
  await controller.choose();
  await controller.openSnapshot(report.source.id);
  await controller.clear();
  assert.equal(controller.getState().busyAction, 'save');
  resolveSave({ outcome: 'saved', snapshot: snapshots[0], alreadySaved: false });
  await saving;
  assert.deepEqual(calls.map(([command]) => command), ['list_snapshots', 'select_and_analyze', 'save_analysis', 'list_snapshots']);
  assert.equal(controller.getState().report, report);
});

test('list failures preserve the current report and later retry can recover saved metadata', async () => {
  const outcomes = [
    { outcome: 'failed', error: { code: 'db_locked', message: 'Saved sources are busy' } },
    { outcome: 'analyzed', report },
    listed,
  ];
  const controller = createAnalysisController({ invoke: () => Promise.resolve(outcomes.shift()) });
  await Promise.resolve();
  assert.equal(controller.getState().snapshotsStatus, 'failed');
  await controller.choose();
  await controller.listSnapshots();
  assert.equal(controller.getState().report, report);
  assert.equal(controller.getState().snapshotsStatus, 'ready');
  assert.deepEqual(controller.getState().snapshots, snapshots);
});

test('untrusted report text is escaped before HTML rendering', () => {
  assert.equal(escapeText('<img src=x onerror=alert(1)>'), '&lt;img src=x onerror=alert(1)&gt;');
  assert.equal(escapeText('"quoted" &'), '&quot;quoted&quot; &amp;');
});

test('confirmed snapshot deletion preserves active source and invalidates stale list results', async () => {
  let resolveList;
  let delayed = false;
  const controller = createAnalysisController({ invoke: async command => {
    if (command === 'list_snapshots') return delayed ? new Promise(resolve => { resolveList = resolve; }) : listed;
    return { outcome: 'analyzed', report };
  } });
  await controller.choose();
  delayed = true;
  const pending = controller.listSnapshots();
  controller.noteSnapshotDeleted(report.source.id);
  resolveList(listed);
  await pending;
  assert.equal(controller.getState().report, report);
  assert.deepEqual(controller.getState().snapshots, []);
  assert.match(controller.getState().message, /now unsaved/);
});
