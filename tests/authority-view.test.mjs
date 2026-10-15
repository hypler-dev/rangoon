import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createAuthorityController } from '../preview/authority-catalog.mjs';
import { renderAuthorityContext, renderAuthorityView } from '../preview/authority-view.mjs';

const operations = ['configuration_preview', 'connection_check', 'evidence_read', 'authorization_evaluation', 'guardrail_evaluation'];

test('compact workspace puts neutral adapter state, controls, and LNSAT-first library before closed secondary surfaces', () => {
  const controller = createAuthorityController();
  const html = renderAuthorityView(controller.getState());
  assert.match(html, /id="authority-title"/); assert.match(html, /Adapters<\/span> Unavailable/);
  assert.match(html, /Search catalog <input[^>]*maxlength="160"/); assert.match(html, /Category <select/);
  assert.ok(html.indexOf('authority-controls') < html.indexOf('authority-workbench'));
  assert.ok(html.indexOf('authority-workbench') < html.indexOf('Plan module roles'));
  assert.ok(html.indexOf('Plan module roles') < html.indexOf('Compare module references'));
  assert.ok(html.indexOf('data-authority-card="lnsat"') < html.indexOf('data-authority-card="agentgateway"'));
  assert.match(html, /data-authority-card="lnsat"[\s\S]*?authority-recommended">Recommended/);
  assert.match(html, /Unavailable · 0 \/ 5 available/);
  assert.doesNotMatch(html, /authority-card__state|Catalog ID<\/dt>/);
  assert.match(html, /<details class="authority-disclosure authority-disclosure--planner"><summary>Plan module roles<\/summary>/);
  assert.match(html, /<details class="authority-disclosure authority-disclosure--comparison"><summary>Compare module references<\/summary>/);
  assert.doesNotMatch(html, /authority-disclosure[^>]* open/);
  assert.equal(controller.getState().selectedId, null);
});

test('presentation sorting is LNSAT-first only when visible and preserves remaining catalog order', () => {
  const initial = renderAuthorityView({});
  const allIds = ['lnsat', 'agentgateway', 'cerbos', 'permit', 'permit-fastmcp', 'permit-mcp-gateway', 'invariant-local', 'invariant-remote'];
  let cursor = -1;
  for (const id of allIds) { const next = initial.indexOf(`data-authority-card="${id}"`); assert.ok(next > cursor); cursor = next; }
  const gateways = renderAuthorityView({ category: 'gateway' });
  assert.equal(gateways.includes('data-authority-card="lnsat"'), false);
  assert.ok(gateways.indexOf('data-authority-card="agentgateway"') < gateways.indexOf('data-authority-card="permit-mcp-gateway"'));
});

test('selected inspector keeps neutral meter and all unavailable functions inside closed disclosures', () => {
  const controller = createAuthorityController(); controller.setRoute('#authority-engines/lnsat/from/engine');
  const html = renderAuthorityView(controller.getState());
  assert.match(html, /No adapter available\.<\/strong> 0 of 5 functions available/);
  assert.equal((html.match(/<li aria-label="[^"\n]+: unavailable">/g) ?? []).length, 5);
  assert.match(html, /<small>Config<\/small>/); assert.match(html, /<small>Connect<\/small>/); assert.match(html, /<small>Evidence<\/small>/); assert.match(html, /<small>Policy<\/small>/); assert.match(html, /<small>Guardrail<\/small>/);
  assert.match(html, /<details class="authority-fold authority-function-details" data-authority-function-details><summary>Function details \(5\)<\/summary>/);
  assert.match(html, /<details class="authority-fold authority-evidence" data-authority-evidence><summary>Evidence meanings<\/summary>/);
  assert.match(html, /Coverage unknown/); assert.match(html, /Control assessment not assessed/); assert.match(html, /Status key<\/summary>/);
  assert.match(html, /<details class="authority-fold authority-module-details" data-authority-module-details><summary>Module details<\/summary>/);
  assert.match(html, /Catalog ID/); assert.match(html, /Researched abilities/); assert.match(html, /Official research references/); assert.match(html, /Adapter details/);
  assert.match(html, /href="#engine">LNSAT diagnostic/); assert.match(html, /<\/details><a class="analysis-button analysis-button--small authority-origin" href="#engine">Return to LNSAT diagnostic<\/a>/);
  assert.doesNotMatch(html, /data-authority-function-details open|data-authority-evidence open|data-authority-module-details open|data-authority-status-key open/);
});

test('function details keep five disabled controls, exact reason IDs, nested details, and no enabled provider action', () => {
  const controller = createAuthorityController(); controller.setRoute('#authority-engines/lnsat');
  const html = renderAuthorityView(controller.getState());
  assert.equal((html.match(/<button[^>]*\bdisabled\b[^>]*aria-describedby="authority-readiness-/g) ?? []).length, 5);
  assert.equal((html.match(/data-authority-readiness-details/g) ?? []).length, 5);
  for (const operation of operations) {
    assert.match(html, new RegExp(`id="authority-readiness-lnsat-${operation}-reason"`));
    assert.match(html, new RegExp(`id="authority-readiness-lnsat-${operation}-details"`));
  }
  assert.match(html, /adapter_not_implemented/);
  assert.doesNotMatch(html, /<button\b(?![^>]*\bdisabled\b)[^>]*>[^<]*\b(?:Check|Install|Connect|Assign|Save|Enforce)\b[^<]*<\/button>/i);
});

test('base, hidden, no-match, and invalid selections retain short recovery without substituting readiness', () => {
  const controller = createAuthorityController();
  const base = renderAuthorityView(controller.getState());
  assert.match(base, /Choose a module/); assert.match(base, /Adapter details/); assert.doesNotMatch(base, /Function details \(5\)/);
  controller.setRoute('#authority-engines/lnsat'); controller.setCategory('gateway');
  const hidden = renderAuthorityView(controller.getState());
  assert.match(hidden, /Selected module is hidden/); assert.match(hidden, /data-authority-clear/); assert.doesNotMatch(hidden, /Function details \(5\)/);
  controller.clearFilters(); controller.setQuery('no-match');
  assert.match(renderAuthorityView(controller.getState()), /No modules match/);
  controller.setRoute('#authority-engines/%3Cimg%3E');
  assert.match(renderAuthorityView(controller.getState()), /Module unavailable/);
});

test('renderer projects trusted immutable fields and ignores forged selection/readiness/provider data', () => {
  const html = renderAuthorityView({ selectedId: 'lnsat', readiness: { availableCount: 5, coverage: { state: 'qualified' } }, allow: true, assessment: 'met', entries: [{ productTarget: '<raw provider>', references: [{ url: 'javascript:alert(1)' }], authority: 'granted' }], selected: { authority: 'granted' } });
  assert.match(html, /0 of 5 functions available/); assert.match(html, /Coverage unknown/); assert.match(html, /Rangoon authority<\/dt><dd>None/);
  assert.doesNotMatch(html, /raw provider|javascript:|granted|>Available<\/dd>/);
});

test('context links remain closed-catalog routes and source has no network or native dispatch', async () => {
  const html = renderAuthorityContext('workflows');
  assert.match(html, /#authority-engines\/lnsat\/from\/workflows/); assert.match(html, /no active binding/);
  assert.equal(renderAuthorityContext('unknown-area'), '');
  const source = await readFile(new URL('../preview/authority-view.mjs', import.meta.url), 'utf8');
  assert.doesNotMatch(source, /fetch\(|XMLHttpRequest|WebSocket|navigator\.sendBeacon|invoke\(|tauri/i);
  assert.match(source, /restoreFocusKey = \{ type: 'inspector', id \}/); assert.match(source, /state\.selectedId === restoreFocusKey\.id/); assert.match(source, /cancelInspectorFocus\(\)/);
  assert.doesNotMatch(source, /setTimeout|setInterval/);
});

test('comparison keeps research date distinct from observed version inside its closed module reference surface', () => {
  const html = renderAuthorityView({});
  assert.match(html, /Public research dated <time datetime="2026-10-09">2026-10-09<\/time>; not an observed or pinned version/);
  assert.match(html, /Observed product version: Unknown/); assert.match(html, /Rangoon adapter: Unavailable for every target/);
  assert.equal((html.match(/data-authority-comparison="/g) ?? []).length, 8);
});
