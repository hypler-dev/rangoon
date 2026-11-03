import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { vendorMark, VENDOR_MARK_IDS } from '../preview/vendor-marks.mjs';
import { renderCompilationPage } from '../preview/compilation-view.mjs';

const root = new URL('../', import.meta.url);
// Independent reviewed identity pins: changing branding requires updating these explicitly.
const expectedMarks = Object.freeze({
  "claude": {
    "path": "preview/assets/vendors/claude-spark-clay.svg",
    "sha256": "6d53db4be375e899c937c26cf16684a80d6e869b1928d72b37748bef2560e219",
    "source": "https://anthropic.com/press-kit"
  },
  "agentgateway": {
    "path": "preview/assets/vendors/agentgateway-mark.svg",
    "sha256": "b7ba567137787b0c41afe5d43a942680dc7fb2db475cd5ec2db2cdaaec9eee9e",
    "source": "https://agentgateway.dev/agw-mark-color.svg"
  },
  "cerbos": {
    "path": "preview/assets/vendors/cerbos-mark.png",
    "sha256": "06669c72f70245cc347d5a657bb270c7cbaf038044d78d2e0de11a679a2caed0",
    "source": "https://www.cerbos.dev/apple-touch-icon.png"
  },
  "permit": {
    "path": "preview/assets/vendors/permit-mark.svg",
    "sha256": "a39c45a1e144f2409cda7c1eb723ec83696387385663ef4dab6c417399ce1d0d",
    "source": "https://www.permit.io/images/svgs/permit-logo.svg"
  },
  "invariant": {
    "path": "preview/assets/vendors/invariant-mark.png",
    "sha256": "9852d767913fa7f9936230effb025602424a469a467cf4929e38b72cd6fb5143",
    "source": "https://invariantlabs.ai/theme/icon/apple-touch-icon.png"
  }
});
test('vendor marks accept only fixed local names and numeric vetted sizes', () => {
  assert.deepEqual([...VENDOR_MARK_IDS].sort(),Object.keys(expectedMarks).sort());
  for (const name of VENDOR_MARK_IDS) for (const size of [20, 24, 32]) {
    const mark = vendorMark(name, size);
    assert.ok(mark.includes('src="'+expectedMarks[name].path.replace(/^preview\//,'')+'"'));
    assert.match(mark, /alt="" aria-hidden="true"/);
    assert.match(mark, new RegExp(`width="${size}" height="${size}"`));
    assert.doesNotMatch(mark, /https?:|<svg|onload=|style=/);
  }
  for (const name of ['openai', '__proto__', '<script>', 'https://evil.example/icon.svg']) assert.equal(vendorMark(name), '');
  for (const size of ['24', '24" onload="bad', 0, NaN, 999]) assert.equal(vendorMark('claude', size), '');
});

test('pinned official software assets retain inert geometry and exact provenance', async () => {
  const provenance = JSON.parse(await readFile(new URL('preview/assets/vendors/provenance.json', root), 'utf8'));
  assert.deepEqual(provenance.assets.map(record=>record.id).sort(),Object.keys(expectedMarks).sort());
  const allowed = {
    svg: new Set(['width','height','viewBox','fill','xmlns']),
    path: new Set(['d','fill','fill-rule','clip-rule']),
    g: new Set(['clip-path']), defs: new Set(),
    linearGradient: new Set(['id','x1','x2','y1','y2','gradientUnits']),
    stop: new Set(['offset','stop-color']), clipPath: new Set(['id']),
    rect: new Set(['width','height','fill']),
  };
  for (const record of provenance.assets) {
    assert.equal(record.path,expectedMarks[record.id].path);
    assert.equal(record.sha256,expectedMarks[record.id].sha256);
    assert.equal(record.source,expectedMarks[record.id].source);
    assert.ok(record.source.startsWith('https://'));
    assert.ok(record.owner); assert.match(record.license,/Apache-2.0/);
    const bytes = await readFile(new URL(record.path,root));
    assert.equal(createHash('sha256').update(bytes).digest('hex'),record.sha256);
    assert.ok(bytes.length<100000);
    if (record.path.endsWith('.png')) {
      assert.equal(bytes.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
      assert.equal(bytes.readUInt32BE(16),record.width);assert.equal(bytes.readUInt32BE(20),record.height);
      assert.ok(record.width>0&&record.width<=512&&record.height>0&&record.height<=512);
      assert.equal(bytes.length,record.bytes);
      continue;
    }
    const svg = bytes.toString('utf8');
    assert.ok(bytes.length<12000);
    assert.doesNotMatch(svg,/<!|<\?|\bon\w+\s*=|(?:href|style|src)\s*=|<(?:script|foreignObject|animate|set|image|text|use|style)\b/i);
    const tags=[...svg.matchAll(/<\/?([\w:.-]+)([^>]*?)\/?\s*>/g)];
    const ids=new Map(); const references=[];
    for (const tag of tags) {
      assert.ok(Object.hasOwn(allowed,tag[1]),`Unexpected tag ${tag[1]}`);
      let attributes=tag[2];
      for (const attr of [...attributes.matchAll(/([\w:.-]+)\s*=\s*"([^"]*)"/g)]) {
        assert.ok(allowed[tag[1]].has(attr[1]),`Unexpected ${tag[1]}.${attr[1]}`);
        if(attr[1]==='xmlns') assert.equal(attr[2],'http://www.w3.org/2000/svg');
        else assert.doesNotMatch(attr[2],/https?:|data:|javascript:|[<>]/i);
        if(attr[1]==='id') {assert.match(attr[2],/^[A-Za-z_][A-Za-z0-9_.:-]*$/);assert.ok(!ids.has(attr[2]));ids.set(attr[2],tag[1]);}
        if(attr[2].includes('url(')) {const m=attr[2].match(/^url\(#([A-Za-z_][A-Za-z0-9_.:-]*)\)$/);assert.ok(m);references.push({id:m[1],kind:attr[1]});}
        attributes=attributes.replace(attr[0],'');
      }
      assert.equal(attributes.trim(),'');
    }
    for(const ref of references) assert.equal(ids.get(ref.id),ref.kind==='clip-path'?'clipPath':'linearGradient');
  }
  const claude=provenance.assets.find(record=>record.id==='claude');
  assert.equal(claude.source,'https://anthropic.com/press-kit');assert.equal(claude.owner,'Anthropic');
});

test('format badges keep native browser refusal and never assert vendor runtime support', () => {
  const html = renderCompilationPage({ bridgeAvailable: false, capabilities: [], listStatus: 'unavailable', profile: 'claude_md_v1' });
  assert.match(html, /claude-spark-clay.svg/); assert.match(html, /Text format only · target loading not verified/); assert.match(html, /Standard instruction format/);
  assert.match(html.match(/<button\b[^>]*id="compile-generate"[^>]*>/)[0], /disabled/);
  assert.doesNotMatch(html, /supported integration|connected to Claude|approved by Claude|cdn\./i);
});
