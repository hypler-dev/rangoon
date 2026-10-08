import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { vendorMark } from '../preview/vendor-marks.mjs';
import { renderCompilationPage } from '../preview/compilation-view.mjs';

const root = new URL('../', import.meta.url);
test('vendor marks accept only fixed local names and numeric vetted sizes', () => {
  for (const size of [20, 24, 32]) {
    const mark = vendorMark('claude', size);
    assert.match(mark, /src="assets\/vendors\/claude-spark-clay.svg"/);
    assert.match(mark, /alt="" aria-hidden="true"/);
    assert.match(mark, new RegExp(`width="${size}" height="${size}"`));
    assert.doesNotMatch(mark, /https?:|<svg|onload=|style=/);
  }
  for (const name of ['openai', '__proto__', '<script>', 'https://evil.example/icon.svg']) assert.equal(vendorMark(name), '');
  for (const size of ['24', '24" onload="bad', 0, NaN, 999]) assert.equal(vendorMark('claude', size), '');
});

test('the pinned official SVG contains only closed inert geometry and original attribution', async () => {
  const provenance = JSON.parse(await readFile(new URL('preview/assets/vendors/provenance.json', root), 'utf8'));
  assert.equal(provenance.assets.length, 1);
  const record = provenance.assets[0]; assert.equal(record.id, 'claude'); assert.equal(record.source, 'https://anthropic.com/press-kit');
  assert.equal(record.owner, 'Anthropic'); assert.match(record.license, /not covered by Rangoon Apache-2.0/);
  const bytes = await readFile(new URL(record.path, root));
  assert.equal(createHash('sha256').update(bytes).digest('hex'), record.sha256);
  const svg = bytes.toString('utf8');
  assert.ok(bytes.length < 4096);
  assert.doesNotMatch(svg, /<!|<\?|\bon\w+\s*=|(?:href|style|src)\s*=|url\s*\(|<(?:script|foreignObject|animate|set|image|text|use|style)\b/i);
  const tags = [...svg.matchAll(/<\/?([\w:.-]+)([^>]*?)\/?\s*>/g)];
  assert.deepEqual([...new Set(tags.map(tag => tag[1]))].sort(), ['path', 'svg']);
  const allowed = { svg: new Set(['width', 'height', 'viewBox', 'fill', 'xmlns']), path: new Set(['d', 'fill']) };
  for (const tag of tags) {
    let attributes = tag[2];
    for (const attribute of [...attributes.matchAll(/([\w:.-]+)\s*=\s*"([^"]*)"/g)]) {
      assert.ok(allowed[tag[1]].has(attribute[1]), `Unexpected SVG attribute ${attribute[1]}`);
      if (attribute[1] === 'xmlns') assert.equal(attribute[2], 'http://www.w3.org/2000/svg');
      attributes = attributes.replace(attribute[0], '');
    }
    assert.equal(attributes.trim(), '');
  }
});

test('format badges keep native browser refusal and never assert vendor runtime support', () => {
  const html = renderCompilationPage({ bridgeAvailable: false, capabilities: [], listStatus: 'unavailable', profile: 'claude_md_v1' });
  assert.match(html, /claude-spark-clay.svg/); assert.match(html, /Format only · runtime unqualified/); assert.match(html, /Provider-independent text profile/);
  assert.match(html.match(/<button\b[^>]*id="compile-generate"[^>]*>/)[0], /disabled/);
  assert.doesNotMatch(html, /supported integration|connected to Claude|approved by Claude|cdn\./i);
});
