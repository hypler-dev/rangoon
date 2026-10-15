import test from 'node:test';
import assert from 'node:assert/strict';
import { AUTHORITY_CATALOG } from '../preview/authority-catalog.mjs';
import { AUTHORITY_COMPARISONS, AUTHORITY_GLOSSARY, AUTHORITY_RESEARCH_DATE, getAuthorityComparison } from '../preview/authority-comparison.mjs';
import { renderAuthorityView } from '../preview/authority-view.mjs';

const ids = ['agentgateway', 'cerbos', 'permit', 'permit-fastmcp', 'permit-mcp-gateway', 'invariant-local', 'invariant-remote', 'lnsat'];
const fields = ['id', 'purpose', 'permissions', 'placement', 'approval', 'evidence', 'privacy', 'example', 'limit', 'references'];

test('comparison research has exactly the immutable catalog targets and fields', () => {
  assert.equal(AUTHORITY_RESEARCH_DATE, '2026-10-09');
  assert.deepEqual(AUTHORITY_COMPARISONS.map(item => item.id), ids);
  assert.deepEqual(AUTHORITY_CATALOG.map(item => item.id), ids);
  for (const item of AUTHORITY_COMPARISONS) {
    assert.deepEqual(Object.keys(item), fields);
    assert.ok(Object.isFrozen(item)); assert.ok(Object.isFrozen(item.references)); assert.ok(Object.isFrozen(item.references[0]));
    assert.equal(getAuthorityComparison(item.id), item);
  }
  assert.equal(getAuthorityComparison('unknown'), null);
  assert.ok(Object.isFrozen(AUTHORITY_COMPARISONS)); assert.ok(Object.isFrozen(AUTHORITY_GLOSSARY));
  assert.deepEqual(AUTHORITY_GLOSSARY.map(item => item.term), ['Permission', 'Approval', 'Enforcement', 'Guardrail', 'Evidence']);
});

test('comparison retains target-specific semantics without turning research into authority', () => {
  const byId = Object.fromEntries(AUTHORITY_COMPARISONS.map(item => [item.id, item]));
  assert.match(byId.agentgateway.placement, /bypassing the gateway/i);
  assert.match(byId.cerbos.placement, /must enforce/i);
  assert.match(byId['permit-fastmcp'].limit, /not Permit MCP Gateway/i);
  assert.match(byId['permit-mcp-gateway'].approval, /not proof of per-action human approval/i);
  assert.match(byId['invariant-local'].permissions, /do not assign identity-based permissions/i);
  assert.match(byId['invariant-remote'].privacy, /must never switch modes automatically/i);
  assert.match(byId.lnsat.approval, /UI clicks alone are not trusted approval evidence/i);
  for (const item of AUTHORITY_COMPARISONS) assert.equal(Object.hasOwn(item, 'authority'), false);
});

test('trusted comparison remains unfiltered with safe links after the primary workbench and planner', () => {
  const html = renderAuthorityView({ query: 'no match', category: 'invalid', selectedId: '<forged>', comparisons: [{ id: 'lnsat', purpose: '<forged>' }] });
  const full = renderAuthorityView({});
  assert.equal((html.match(/data-authority-comparison="/g) ?? []).length, 8);
  assert.equal((html.match(/<dl><div><dt>Purpose<\/dt>/g) ?? []).length, 8);
  for (const id of ids) assert.match(html, new RegExp(`data-authority-comparison="${id}"`));
  assert.match(html, /Research only/); assert.match(html, /Observed product version: Unknown/); assert.match(html, /Rangoon adapter: Unavailable for every target/);
  assert.ok(html.indexOf('authority-controls') < html.indexOf('authority-workbench'));
  assert.ok(html.indexOf('authority-workbench') < html.indexOf('authority-disclosure--planner'));
  assert.ok(html.indexOf('authority-disclosure--planner') < html.indexOf('authority-comparison'));
  assert.doesNotMatch(html, /<forged>|javascript:/);
  assert.match(html, /target="_blank" rel="noopener noreferrer"/);
  assert.equal((full.match(/data-authority-select=/g) ?? []).length, 8);
});
