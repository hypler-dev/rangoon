import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { RECORD_KINDS } from '../preview/operations-model.mjs';
import { renderOperationsView } from '../preview/operations-view.mjs';

const state = () => ({
  status: 'inspected', selectedOrdinal: 0, mode: 'timeline', kind: 'all', diagnostic: 'all', ordinal: '', notice: 'Selected local window — unverified', errorCode: null,
  filteredEvents: [{ inputOrdinal: 0, producerOrdinal: 0, fenceOrdinal: 0, sequence: 1, recordKind: 'evaluation', reportedValue: 'unknown', structuralDiagnostics: ['sequence_gap'], issueStages: ['structural'], references: [] }],
  view: { structuralState: 'incomplete', sourceCounts: { eventCount: 1, recordCounts: RECORD_KINDS.map(kind => ({ kind, count: kind === 'evaluation' ? 1 : 0 })) }, decisionCounts: { evaluation: { allow: 0, deny: 0, unknown: 1 }, approvalDecision: { approve: 0, deny: 0, abstain: 0 }, authorization: { reportedIssued: 0, reportedRefused: 0, reportedExpired: 0, reportedRevoked: 0, unknown: 0 }, attempt: { reportedAccepted: 0, reportedActive: 0, reportedCompleted: 0, reportedFailed: 0, reportedCancelled: 0, outcomeUnknown: 0, reportedReconciledCompleted: 0, reportedReconciledFailed: 0 } }, events: [{ inputOrdinal: 0, producerOrdinal: 0, fenceOrdinal: 0, sequence: 1, recordKind: 'evaluation', reportedValue: 'unknown', structuralDiagnostics: ['sequence_gap'], issueStages: ['structural'], references: [] }], topology: { nodes: [], edges: [] } },
});

test('operator view keeps truth fixed, names incomplete and unknown states, and has no provider action', () => {
  const html = renderOperationsView(state(), { recordKinds: RECORD_KINDS, codes: ['sequence_gap'] });
  assert.match(html, /Authority: none/);
  assert.match(html, /Structural references incomplete/);
  assert.doesNotMatch(html, /reported unknown outcome/);
  assert.match(html, /Reported decisions in selected window/);
  assert.match(html, /Model advice: unavailable/);
  assert.match(html, /1 observed evaluation record/);
  assert.doesNotMatch(html, /data-(?:provider|execute|approve|authorize)/);
  assert.match(html, /Event #0/);
  assert.match(html, /<details class="operations-decisions">/);
  assert.ok(html.indexOf('operations-workbench') < html.indexOf('operations-decisions'));
});

test('decision denominators use singular and plural record grammar', () => {
  const singular = renderOperationsView(state(), { recordKinds: RECORD_KINDS, codes: [] });
  assert.match(singular, /1 observed evaluation record/);
  const pluralState = state();
  pluralState.view.sourceCounts.recordCounts = RECORD_KINDS.map(kind => ({ kind, count: kind === 'evaluation' ? 2 : 0 }));
  pluralState.view.decisionCounts.evaluation = { allow: 1, deny: 0, unknown: 1 };
  const plural = renderOperationsView(pluralState, { recordKinds: RECORD_KINDS, codes: [] });
  assert.match(plural, /2 observed evaluation records/);
});

test('operations rerender preserves an open decision disclosure and summary focus', () => {
  const source = readFileSync(new URL('../preview/analyze.mjs', import.meta.url), 'utf8');
  assert.match(source, /const decisionsOpen = document\.querySelector\('\.operations-decisions'\)\?\.open === true;/);
  assert.match(source, /active\?\.matches\?\.\('\.operations-decisions > summary'\)/);
  assert.match(source, /if \(decisionsOpen\) document\.querySelector\('\.operations-decisions'\)\?\.setAttribute\('open', ''\);/);
});

test('operations announcer keeps recovery copy fixed and does not expose native error codes', () => {
  const source = readFileSync(new URL('../preview/analyze.mjs', import.meta.url), 'utf8');
  const renderOperations = source.slice(source.indexOf('function renderOperations()'), source.indexOf('function renderSkills()'));
  assert.match(renderOperations, /This local window was rejected\. Choose a local window again\./);
  assert.match(renderOperations, /This local inspection could not finish\. Choose a local window again\./);
  assert.match(renderOperations, /announcer\.textContent = operationsAnnouncement;/);
  assert.doesNotMatch(renderOperations, /announcer\.textContent[^;]*errorCode/);
});

test('timeline uses the filtered records, exposes pressed selection, and uses pressed view controls', () => {
  const input = state(); input.filteredEvents = [];
  const html = renderOperationsView(input, { recordKinds: RECORD_KINDS, codes: ['sequence_gap'] });
  assert.match(html, /No observations match these filters/);
  assert.doesNotMatch(html, /role="tab/);
  assert.match(html, /data-operations-mode="timeline" aria-pressed="true"/);
  input.filteredEvents = input.view.events;
  const selected = renderOperationsView(input, { recordKinds: RECORD_KINDS, codes: ['sequence_gap'] });
  assert.match(selected, /data-operations-select="0" aria-pressed="true"/);
});

test('no-view states never claim a selected window', () => {
  for (const status of ['empty', 'cleared', 'stale', 'cancelled']) {
    const input = state(); input.status = status; input.view = null; input.filteredEvents = []; input.notice = null;
    const html = renderOperationsView(input, { recordKinds: RECORD_KINDS, codes: [] });
    assert.match(html, /No local window selected/);
    assert.doesNotMatch(html, /No observations in this selected window/);
  }
});

test('rejected and failed no-view states use fixed recovery copy without reflecting native errors', () => {
  for (const status of ['rejected', 'failed']) {
    const input = state(); input.status = status; input.view = null; input.filteredEvents = []; input.errorCode = '<native-private-error>';
    const html = renderOperationsView(input, { recordKinds: RECORD_KINDS, codes: [] });
    assert.match(html, /Choose a local window again/);
    assert.match(html, /operations-empty--error/);
    assert.equal((html.match(/Choose a local window again/g) ?? []).length, 1);
    assert.doesNotMatch(html, /native-private-error|errorCode/);
  }
});

test('unavailable browser guidance appears once beside the disabled action', () => {
  const input = state(); input.status = 'unavailable'; input.view = null;
  const html = renderOperationsView(input, { recordKinds: RECORD_KINDS, codes: [] });
  assert.match(html, /id="operations-choose"[^>]*disabled/);
  assert.equal((html.match(/Open this route in Rangoon desktop to inspect a local window/g) ?? []).length, 1);
});

test('zero-observation view is distinct from a nonempty filtered no-match', () => {
  const zero = state();
  zero.view.events = []; zero.filteredEvents = []; zero.view.sourceCounts.eventCount = 0;
  zero.view.sourceCounts.recordCounts = RECORD_KINDS.map(kind => ({ kind, count: 0 }));
  const zeroHtml = renderOperationsView(zero, { recordKinds: RECORD_KINDS, codes: [] });
  assert.match(zeroHtml, /No observations in this selected window/);
  assert.doesNotMatch(zeroHtml, /No observations match these filters/);
  const filtered = state(); filtered.filteredEvents = [];
  const filteredHtml = renderOperationsView(filtered, { recordKinds: RECORD_KINDS, codes: [] });
  assert.match(filteredHtml, /No observations match these filters/);
});

test('filter targets and pressed selections keep explicit responsive visual hooks', () => {
  const css = readFileSync(new URL('../preview/operations.css', import.meta.url), 'utf8');
  assert.match(css, /\.operations-filters :is\(select,input\)\{[^}]*min-height:44px/);
  assert.match(css, /\.operations-record\[aria-pressed=true\],\.operations-event\[aria-pressed=true\]/);
});

test('only an unknown attempt outcome is labeled as an unknown outcome', () => {
  const input = state();
  input.view.events = [{ ...input.view.events[0], recordKind: 'attempt', reportedValue: 'outcome_unknown' }];
  input.filteredEvents = input.view.events;
  const html = renderOperationsView(input, { recordKinds: RECORD_KINDS, codes: ['sequence_gap'] });
  assert.match(html, /1 reported unknown outcome/);
});

test('topology mode names an absent declared graph and escapes closed renderer text', () => {
  const input = state(); input.mode = 'topology'; input.ordinal = '0<script>';
  const html = renderOperationsView(input, { recordKinds: RECORD_KINDS, codes: ['sequence_gap'] });
  assert.match(html, /No declared topology is present/);
  assert.doesNotMatch(html, /<script>/);
});
