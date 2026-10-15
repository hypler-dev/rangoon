import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { AUTHORITY_CATALOG, AUTHORITY_CATALOG_SCHEMA, AUTHORITY_OPERATIONS, createAuthorityController, parseAuthorityHash } from '../preview/authority-catalog.mjs';

const targets = ['agentgateway standalone', 'Cerbos PDP', 'Permit authorization', 'Permit FastMCP middleware', 'Permit MCP Gateway', 'Invariant local', 'Invariant remote', 'LNSAT'];
const ids = ['agentgateway', 'cerbos', 'permit', 'permit-fastmcp', 'permit-mcp-gateway', 'invariant-local', 'invariant-remote', 'lnsat'];
const fields = ['id','productTarget','family','category','badge','logoPath','summary','abilities','limitations','references','areas','state','reason','observedVersion','capabilities','compatibility','connection','runtime','authority','networkAttempted'];

test('catalog exactly reflects the I1 literal inventory and fixed closed metadata', async () => {
  const descriptor = await readFile(new URL('../crates/rangoon-integrations/src/descriptor.rs', import.meta.url), 'utf8');
  const source = await readFile(new URL('../crates/rangoon-integrations/src/lib.rs', import.meta.url), 'utf8');
  assert.equal(AUTHORITY_CATALOG_SCHEMA, 'rangoon.authority-catalog.v1');
  assert.deepEqual(AUTHORITY_OPERATIONS, ['configuration_preview','connection_check','evidence_read','authorization_evaluation','guardrail_evaluation']);
  for (const operation of AUTHORITY_OPERATIONS) assert.match(source, new RegExp(`"${operation}"`));
  assert.deepEqual(AUTHORITY_CATALOG.map(item => item.id), ids);
  assert.deepEqual(AUTHORITY_CATALOG.map(item => item.productTarget), targets);
  for (const target of targets) assert.match(descriptor, new RegExp(`"${target}"`));
  for (const item of AUTHORITY_CATALOG) {
    assert.deepEqual(Object.keys(item), fields);
    assert.equal(item.state, 'unavailable'); assert.equal(item.reason, 'adapter_not_implemented'); assert.equal(item.observedVersion, null);
    assert.equal(item.compatibility, 'unknown'); assert.equal(item.connection, 'unknown'); assert.equal(item.runtime, 'unknown'); assert.equal(item.authority, 'none'); assert.equal(item.networkAttempted, false);
    assert.deepEqual(item.capabilities, AUTHORITY_OPERATIONS.map(operation => ({ operation, state: 'unavailable' })));
    assert.ok(Object.isFrozen(item)); assert.ok(Object.isFrozen(item.capabilities)); assert.ok(Object.isFrozen(item.capabilities[0]));
  }
});

test('provider distinctions keep products and privacy modes separate', () => {
  const byId = Object.fromEntries(AUTHORITY_CATALOG.map(item => [item.id, item]));
  assert.match(byId.cerbos.limitations, /not an approval/i);
  assert.match(byId['permit-fastmcp'].limitations, /not the commercial Permit MCP Gateway/i);
  assert.match(byId['permit-mcp-gateway'].limitations, /separate commercial product/i);
  assert.match(byId['invariant-local'].limitations, /egress/i);
  assert.match(byId['invariant-remote'].limitations, /no automatic local-to-remote fallback/i);
  assert.match(byId.lnsat.limitations, /separate A1 diagnostic seams/i);
  assert.equal(byId.lnsat.logoPath, 'assets/authority/lnsat-mark.webp');
  assert.equal(byId.permit.logoPath, null);
  assert.ok(byId.lnsat.areas.some(area => area.id === 'engine'));
});

test('controller filters immutable catalog and never substitutes a hidden selection', () => {
  const controller = createAuthorityController();
  controller.setRoute('#authority-engines/permit');
  assert.equal(controller.getState().selected.id, 'permit');
  controller.setCategory('guardrail');
  const hidden = controller.getState();
  assert.equal(hidden.inspector, 'filtered-empty'); assert.equal(hidden.selected, null); assert.equal(hidden.selectedId, 'permit');
  controller.clearFilters();
  controller.setCategory('not-a-category');
  assert.equal(controller.getState().category, 'all');
  controller.setQuery('FastMCP');
  assert.deepEqual(controller.getState().entries.map(item => item.id), ['permit-fastmcp']);
  controller.setQuery('authorization_evaluation');
  assert.ok(controller.getState().entries.some(item => item.id === 'cerbos'));
  controller.setQuery('x'.repeat(200));
  assert.equal(controller.getState().query.length, 160);
  controller.clearFilters();
  const snapshot = controller.getState();
  assert.ok(snapshot.entries.length > 0);
  assert.ok(Object.isFrozen(snapshot)); assert.ok(Object.isFrozen(snapshot.entries)); assert.ok(Object.isFrozen(snapshot.entries[0]));
  assert.throws(() => { snapshot.entries.push(snapshot.entries[0]); }, TypeError);
  assert.throws(() => { snapshot.entries[0].capabilities[0].state = 'available'; }, TypeError);
  assert.equal(AUTHORITY_CATALOG[0].capabilities[0].state, 'unavailable');
});

test('deep links are allowlisted and hostile route text is never retained', () => {
  assert.deepEqual(parseAuthorityHash('#authority-engines/lnsat/from/engine'), { matched: true, valid: true, id: 'lnsat', area: 'engine' });
  for (const hash of ['#authority-engines/%6cnSAT', '#authority-engines/lnsat/from/engine/extra', '#authority-engines/not-real', '#authority-engines/lnsat?x=1', `#authority-engines/${'a'.repeat(300)}`]) {
    const parsed = parseAuthorityHash(hash); assert.equal(parsed.matched, true); assert.equal(parsed.valid, false); assert.equal(parsed.id, null);
  }
  assert.equal(parseAuthorityHash('#skills').matched, false);
  const controller = createAuthorityController(); controller.setRoute('#authority-engines/<img src=x>');
  assert.equal(controller.getState().inspector, 'unavailable'); assert.equal(controller.getState().selected, null);
});

test('area references and local context identity use the same immutable catalog entries', () => {
  const areas = new Set();
  for (const entry of AUTHORITY_CATALOG) for (const reference of entry.areas) areas.add(reference.id);
  assert.deepEqual([...areas].sort(), ['analysis','compile','decompose','engine','merge','model-assistance','skills','split','workflows','workspace']);
});

test('dated LNSAT source abilities remain research through filtering and selection', () => {
  const controller = createAuthorityController();
  controller.setRoute('#authority-engines/lnsat/from/workflows');
  controller.setQuery('unknown-outcome recovery');
  const selected = controller.getState().selected;
  assert.equal(selected?.id, 'lnsat');
  assert.equal(selected.observedVersion, null);
  assert.equal(selected.authority, 'none');
  assert.equal(selected.networkAttempted, false);
  assert.ok(selected.capabilities.every(operation => operation.state === 'unavailable'));
  assert.ok(selected.abilities.every(ability => AUTHORITY_OPERATIONS.includes(ability.operation)));
  assert.match(selected.limitations, /snapshot\/watch are proposed/);
  assert.ok(selected.references.every(reference => reference.url.includes('/5cad4719e89395d4a0c90b7b06aacfe1f83555f5/')));
  assert.equal(AUTHORITY_CATALOG.filter(item => item.id !== 'lnsat').every(item => item.observedVersion === null), true);
});
