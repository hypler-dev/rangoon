import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { catalogKeys, icon } from '../preview/icons.mjs';
import { COLOR_ICON_NAMES } from '../preview/color-icon-artwork.mjs';

assert.deepEqual(COLOR_ICON_NAMES, catalogKeys, 'Color and forced-color fallback catalogs must agree.');

// Validate the assets that actually ship, including closed-input SVG boundaries.
const sizes = [16, 20, 24, 40];
const shapes = new Set();
for (const name of catalogKeys) {
  assert.match(name, /^[a-z]+$/);
  const svg = icon(name);
  assert.ok(!shapes.has(svg.replace(`foldline-icon--${name}`, 'catalog-shape')), `Duplicate icon: ${name}`);
  shapes.add(svg.replace(`foldline-icon--${name}`, 'catalog-shape'));
  for (const size of sizes) {
    const output = icon(name, { size });
    assert.match(output, new RegExp(`width="${size}" height="${size}"`));
    assert.match(output, /viewBox="0 0 24 24"/);
    assert.match(output, /aria-hidden="true" focusable="false"/);
    assert.match(output, /stroke="currentColor"/);
    assert.match(output, /class="foldline-color"/);
    assert.match(output, /class="foldline-monochrome"/);
    assert.doesNotMatch(output, /<(?:script|image|foreignObject|use)\b|\bon\w+\s*=|\bhref\s*=|url\(/i);
  }
}
for (const name of ['__proto__', 'constructor', '<svg onload="alert(1)">', '', null, 12]) {
  assert.equal(icon(name), '', 'Unknown or hostile names must produce no markup.');
}
for (const size of [0, 17, -20, '20', '20" onload="alert(1)', null, Infinity]) {
  assert.equal(icon('command', { size }), '', 'Only supported numeric sizes may enter markup.');
}
// Static literal call-site audit supplements the runtime allowlist; it is not
// an SVG parser or a security boundary for arbitrary JavaScript expressions.
let fixedSizeCalls = 0;
for (const file of await readdir(new URL('../preview/', import.meta.url))) {
  if (!file.endsWith('.mjs')) continue;
  const source = await readFile(new URL(`../preview/${file}`, import.meta.url), 'utf8');
  for (const match of source.matchAll(/\bicon\([^;\n]{0,160}?\bsize:\s*(\d+)\s*}/g)) {
    assert.ok(sizes.includes(Number(match[1])), `Unsupported literal icon size ${match[1]} in ${file}`);
    fixedSizeCalls++;
  }
  for (const match of source.matchAll(/\bicon\(\s*(['"])([a-z]+)\1/g)) {
    assert.ok(catalogKeys.includes(match[2]), `Unknown literal icon ${match[2]} in ${file}`);
  }
}
assert.ok(fixedSizeCalls > 0, 'Icon call-site audit must inspect real callers.');
const manifest = JSON.parse(await readFile(new URL('../docs/visual-assets.json', import.meta.url)));
for (const asset of manifest.assets) {
  const bytes = await readFile(new URL(`../${asset.path}`, import.meta.url));
  assert.equal(createHash('sha256').update(bytes).digest('hex'), asset.sha256, `Asset hash: ${asset.path}`);
  if (asset.width) {
    assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
    assert.equal(bytes.readUInt32BE(16), asset.width);
    assert.equal(bytes.readUInt32BE(20), asset.height);
    if (asset.transparent) assert.equal(bytes[25], 6, 'Transparent artwork must retain RGBA.');
    assert.equal(bytes.length, asset.bytes);
    assert.ok(bytes.length < (asset.transparent ? 1_200_000 : 1_900_000), 'Decorative artwork exceeds its reviewed byte budget.');
  }
}
console.log(`PASS: ${catalogKeys.length} unique icons × ${sizes.length} sizes, invalid inputs, ${fixedSizeCalls} literal-size callers, and ${manifest.assets.length} asset hashes.`);
