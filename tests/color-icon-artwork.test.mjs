import test from 'node:test';
import assert from 'node:assert/strict';
import { COLOR_ICON_NAMES, colorIconArtwork } from '../preview/color-icon-artwork.mjs';

const expectedNames = ['command','import','decompose','merge','skill','workflow','check','branch','connector','evidence','roadmap','source','bundle','gate','outcome','search','density','focus','theme','chevron','unavailable','unknown','research','approval','arrow','plus','save','clear','more','zoomin','zoomout','panleft','panright','reset','undo','redo','edit','compile','export','lock','unlock','encrypted','plaintext','database','history','restore','copy','filter','folder','terminal','model','agent','api','cloud','network','warning','viewcube'];
const allowedTags = new Set(['g', 'path', 'rect', 'circle', 'ellipse', 'polygon', 'polyline', 'line']);
const allowedAttributes = new Set(['d', 'fill', 'stroke', 'stroke-width', 'stroke-linecap', 'stroke-linejoin', 'x', 'y', 'width', 'height', 'rx', 'ry', 'cx', 'cy', 'r', 'points', 'x1', 'x2', 'y1', 'y2']);

test('color artwork keeps exact closed 57-icon catalog and unique static art', () => {
  assert.deepEqual(COLOR_ICON_NAMES, expectedNames);
  assert.ok(Object.isFrozen(COLOR_ICON_NAMES));
  assert.throws(() => COLOR_ICON_NAMES.push('new-icon'), TypeError);
  const artworks = COLOR_ICON_NAMES.map(name => colorIconArtwork(name));
  assert.equal(new Set(artworks).size, expectedNames.length);
});

test('color artwork rejects unknown names and non-allowlisted or non-finite sizes', () => {
  for (const name of [null, undefined, '', 'not-in-catalog', 12]) assert.equal(colorIconArtwork(name), '');
  for (const size of [16, 20, 24, 40]) assert.notEqual(colorIconArtwork('source', { size }), '');
  for (const size of [0, 15, 16.5, 25, Infinity, -Infinity, NaN, '20', null]) assert.equal(colorIconArtwork('source', { size }), '');
});

test('color artwork is inner geometry only, uses fixed local paint, and exposes no executable or external markup', () => {
  for (const name of COLOR_ICON_NAMES) {
    const artwork = colorIconArtwork(name);
    assert.doesNotMatch(artwork, /<svg\b|<\/(?:svg|g)>|<\/?(?:script|style|defs|filter|linearGradient|radialGradient|image|text|use|a)\b/i, name);
    assert.doesNotMatch(artwork, /\b(?:href|id|class|style|on[a-z]+)\s*=|url\(|https?:|data:/i, name);
    const tags = [...artwork.matchAll(/<([a-z]+)\b([^>]*)\/?>(?!<\/\1>)/gi)];
    assert.ok(tags.length >= 2, name);
    for (const [, tag, source] of tags) {
      assert.ok(allowedTags.has(tag), `${name}: ${tag}`);
      for (const attribute of source.matchAll(/\s([a-z-]+)=/gi)) assert.ok(allowedAttributes.has(attribute[1]), `${name}: ${attribute[1]}`);
    }
    assert.ok(new Set(artwork.match(/#[0-9A-F]{6}/g) ?? []).size >= 2, name);
  }
});

test('color artwork return values cannot mutate catalog geometry', () => {
  const first = colorIconArtwork('lock');
  assert.equal(`${first} altered`, `${colorIconArtwork('lock')} altered`);
  assert.notEqual(colorIconArtwork('lock'), colorIconArtwork('unlock'));
});
