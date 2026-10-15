import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';

// Accepted before implementation in provider-interoperability.md, independently
// of the new manifest. Matching two newly authored values alone is insufficient.
const acceptedPin = 'bd6dd1e91e99319b8ff5e1b134ec154dfb583a529aaff6fbf0cdef04ed577c68';
const root = new URL('../', import.meta.url);

test('LNSAT identity uses the exact pre-code Hypler raster and truthful provenance', async () => {
  const manifest = JSON.parse(await readFile(new URL('preview/assets/authority/provenance.json', root), 'utf8'));
  assert.equal(manifest.schema, 'rangoon.authority-assets.v1');
  assert.equal(manifest.assets.length, 1);
  const asset = manifest.assets[0];
  assert.equal(asset.id, 'lnsat');
  assert.equal(asset.path, 'preview/assets/authority/lnsat-mark.webp');
  assert.equal(asset.sourceAsset, 'hypler.com/Hypler/client/public/assets/lnsat-mark-v1.webp');
  assert.equal(asset.sourceUse, 'client/src/components/ProjectBrand.tsx');
  assert.equal(asset.owner, 'HYPLER LLC');
  assert.equal(asset.sha256, acceptedPin);
  assert.match(asset.rights, /separate from Apache-2\.0/);
  assert.match(asset.purpose, /not adapter support, endorsement or enforcement evidence/);
  const bytes = await readFile(new URL(asset.path, root));
  assert.equal(createHash('sha256').update(bytes).digest('hex'), acceptedPin);
  assert.equal(bytes.length, asset.bytes);
  assert.ok(bytes.length < 65536);
  assert.equal(bytes.subarray(0, 4).toString('ascii'), 'RIFF');
  assert.equal(bytes.subarray(8, 12).toString('ascii'), 'WEBP');
  assert.equal(bytes.readUInt32LE(4) + 8, bytes.length);
  assert.deepEqual((await readdir(new URL('preview/assets/authority/', root))).sort(), ['lnsat-mark.webp', 'provenance.json']);
});

test('the independent logo pin is retained in the canonical catalog contract', async () => {
  const contract = await readFile(new URL('docs/provider-interoperability.md', root), 'utf8');
  assert.ok(contract.includes(acceptedPin));
  assert.match(contract, /Source and destination hash must equal this pre-code pin/);
});
