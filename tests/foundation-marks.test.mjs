import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { AUTHORITY_CATALOG } from '../preview/authority-catalog.mjs';
import { FOUNDATION_MARKS, renderFoundationAuthorityLink, renderFoundationContext } from '../preview/foundation-marks.mjs';

const root = new URL('../', import.meta.url);
const vuslHash = 'd6f2a28ee947790a7324ae1c86fe70b4c4bad327f8dca53d4d0ee4097f54c71a';
const lnsatHash = 'bd6dd1e91e99319b8ff5e1b134ec154dfb583a529aaff6fbf0cdef04ed577c68';

test('foundation registry keeps VUSL outside the authority catalog and LNSAT optional', () => {
  assert.deepEqual(Object.keys(FOUNDATION_MARKS), ['vusl', 'lnsat']);
  assert.equal(FOUNDATION_MARKS.vusl.role, 'Shared language');
  assert.equal(FOUNDATION_MARKS.vusl.state, 'Integration planned');
  assert.equal(FOUNDATION_MARKS.lnsat.role, 'Authority engine');
  assert.equal(FOUNDATION_MARKS.lnsat.state, 'Adapter unavailable');
  assert.equal(AUTHORITY_CATALOG.some(item => item.id === 'vusl'), false);
  const html = renderFoundationContext();
  assert.match(html, /data-foundation="vusl"/);
  assert.match(html, /data-foundation="lnsat"/);
  assert.equal((html.match(/id="foundation-context-title">Foundations/g) ?? []).length, 1);
  assert.doesNotMatch(html, /<p class="analysis-kicker">FOUNDATION<\/p>/);
  assert.match(html, /<details><summary>VUSL details<\/summary>/);
  assert.match(html, /<details><summary>LNSAT details<\/summary>/);
  assert.match(html, /Maintained independently; Rangoon integration is planned/);
  assert.match(html, /Live connection unavailable/);
  assert.doesNotMatch(html, /<details[^>]*\bopen\b/);
  assert.doesNotMatch(html, /data-authority-card="vusl"/);
});

test('foundation identities have visible names and decorative local marks', () => {
  const context = renderFoundationContext();
  assert.match(context, /assets\/foundations\/vusl-logo\.png/);
  assert.match(context, /assets\/authority\/lnsat-mark\.webp/);
  assert.match(context, /foundation-card__mark--vusl/);
  assert.match(context, /foundation-card__mark--lnsat/);
  assert.equal((context.match(/alt="" aria-hidden="true"/g) ?? []).length, 2);
  const link = renderFoundationAuthorityLink('#authority-engines/lnsat/from/engine');
  assert.match(link, /href="#authority-engines\/lnsat\/from\/engine"/);
  assert.match(link, /<span>LNSAT<\/span>/);
  assert.match(link, /assets\/authority\/lnsat-mark\.webp/);
});

test('foundation assets retain exact hash, dimensions, and rights constraints', async () => {
  const vuslManifest = JSON.parse(await readFile(new URL('preview/assets/foundations/provenance.json', root), 'utf8'));
  const vusl = vuslManifest.assets[0];
  assert.equal(vuslManifest.schema, 'rangoon.foundation-assets.v1');
  assert.equal(vusl.sha256, vuslHash);
  assert.match(vusl.rights, /unchanged without crop, recolor, dark variant/i);
  assert.match(vusl.purpose, /not a catalog, provider, connection, readiness, adapter, or authorization claim/i);
  const vuslBytes = await readFile(new URL(vusl.path, root));
  assert.equal(createHash('sha256').update(vuslBytes).digest('hex'), vuslHash);
  assert.equal(vuslBytes.length, vusl.bytes);
  assert.equal(vuslBytes.readUInt32BE(16), 2048);
  assert.equal(vuslBytes.readUInt32BE(20), 768);
  assert.equal(vuslBytes[25], 2);

  const lnsatManifest = JSON.parse(await readFile(new URL('preview/assets/authority/provenance.json', root), 'utf8'));
  const lnsat = lnsatManifest.assets.find(asset => asset.id === 'lnsat');
  assert.equal(lnsat.sha256, lnsatHash);
  assert.match(lnsat.rights, /separate from Apache-2\.0/i);
  const lnsatBytes = await readFile(new URL(lnsat.path, root));
  assert.equal(createHash('sha256').update(lnsatBytes).digest('hex'), lnsatHash);
});

test('analysis page loads the foundation stylesheet exactly once', async () => {
  const page = await readFile(new URL('../preview/analyze.html', import.meta.url), 'utf8');
  assert.equal((page.match(/href="foundations\.css"/g) ?? []).length, 1);
});
