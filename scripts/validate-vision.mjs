import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { renderModelAssistanceView } from '../preview/model-assistance-view.mjs';
import { renderCompositionView } from '../preview/composition-view.mjs';
import { renderSkillsView } from '../preview/skills-view.mjs';
import { renderWorkspaceView } from '../preview/workspace-view.mjs';
import { renderEngineView } from '../preview/engine-view.mjs';

// This checks narrow source contracts, not visual similarity or native qualification.
const read = path => readFile(new URL(`../${path}`, import.meta.url), 'utf8');
const contract = JSON.parse(await read('docs/vision-contract.json'));
assert.equal(contract.schemaVersion, 'rangoon.vision-contract.v1');
assert.deepEqual(contract.themes, ['dark', 'light']);
assert.deepEqual(contract.reviewWidths, [320, 768, 1440]);
await read(contract.authority);
const references = JSON.parse(await read(contract.referenceManifest));
assert.equal(references.images.length, 8, 'Retain all eight accepted reference images.');
for (const reference of references.images) {
  const bytes = await readFile(new URL(`../${reference.path}`, import.meta.url));
  assert.equal(createHash('sha256').update(bytes).digest('hex'), reference.sha256);
}
const covered = new Set();
const ids = new Set();
for (const surface of contract.surfaces) {
  assert.ok(!ids.has(surface.id), `Duplicate surface: ${surface.id}`);
  ids.add(surface.id);
  assert.ok(['implemented-source', 'sample-only', 'unavailable'].includes(surface.status));
  assert.ok(surface.next?.trim().length >= 30, `${surface.id}: record the remaining technical outcome.`);
  assert.ok(surface.tests?.length, `${surface.id}: name behavioral validation.`);
  await read(surface.viewSource);
  if (surface.status === 'implemented-source') assert.ok(surface.technicalSources?.length, `${surface.id}: native/core evidence is required separately from presentation.`);
  for (const source of surface.technicalSources ?? []) {
    assert.match(source, /^(?:apps\/desktop\/src|crates\/[a-z-]+\/src)\/[a-z_/]+\.rs$/);
    assert.ok((await read(source)).trim().length > 0, `${surface.id}: empty technical source.`);
  }
  for (const test of surface.tests) {
    assert.match(test, /^tests\/[a-z-]+\.test\.mjs$/);
    assert.match(await read(test), /\btest\(/, `${surface.id}: behavioral test file is empty.`);
  }
  for (const id of surface.references) {
    assert.ok(references.images.some(reference => reference.id === id), `Unknown reference ${id}`);
    covered.add(id);
  }
}
assert.equal(covered.size, 8, 'Each original screen needs a functional mapping.');
for (const id of ['import', 'decompose', 'merge-split', 'skills', 'workflows', 'connectors', 'command-center', 'compile', 'model-assistance', 'workspace', 'engine']) assert.ok(ids.has(id));

const html = await read('preview/analyze.html');
const sheets = [...html.matchAll(/rel="stylesheet" href="([^"]+)"/g)].map(match => match[1]);
assert.equal(`preview/${sheets.at(-1)}`, contract.sharedStylesheet, 'Shared workbench contract must own the final cascade.');
const css = await read(contract.sharedStylesheet);
for (const selector of ['.analysis-shell', '.analysis-nav', '.analysis-hero', '.skills-hero', '.composition-hero', '.model-assistance-hero']) assert.ok(css.includes(selector), `Missing shared layout surface ${selector}`);
for (const theme of contract.themes) assert.ok(css.includes(`rangoon-scenes-${theme}-v1.png`), `Missing ${theme} artwork.`);
const positions = new Set();
assert.deepEqual(Object.keys(contract.routeArtwork).sort(), ['analysis', 'decompose', 'merge', 'split', 'skills', 'compile', 'model-assistance', 'workspace', 'engine'].sort(), 'Artwork must cover exactly the nine native menu routes.');
for (const [route, [x, y]] of Object.entries(contract.routeArtwork)) {
  assert.ok([0, 50, 100].includes(x) && [0, 50, 100].includes(y));
  assert.ok(!positions.has(`${x}:${y}`), `Each menu needs distinct art: ${route}`);
  positions.add(`${x}:${y}`);
  assert.ok(css.includes(`.analysis-shell[data-area="${route}"]{--scene-x:${x}%;--scene-y:${y}%}`), `Missing route art mapping: ${route}`);
}
assert.equal(positions.size, 9, 'Keep distinct artwork for all nine native menus.');
assert.match(css, /prefers-reduced-motion/);
assert.match(css, /forced-colors/);
const cascade = (await Promise.all(sheets.map(sheet => read(`preview/${sheet}`)))).join('\n');
assert.match(cascade, /focus-visible/);
assert.match(await read('preview/analyze.mjs'), /Sample design preview/, 'Keep fixtures distinct from native workspace.');

const disabled = (markup, id) => {
  const tags = [...markup.matchAll(/<button\b[^>]*>/g)].map(match => match[0]);
  const tag = tags.find(value => value.includes(`id="${id}"`));
  assert.ok(tag, `Required control missing: ${id}`);
  assert.match(tag, /\bdisabled\b/, `${id} must not activate without native capability.`);
};
for (const endpoint of ['local', 'cloud']) {
  const markup = renderModelAssistanceView({ bridgeAvailable: false, endpoint });
  for (const panel of ['model-profile', 'model-selection', 'model-payload', 'model-results']) assert.ok(markup.includes(panel));
  for (const id of ['model-send', endpoint === 'cloud' ? 'cloud-model-prepare' : 'model-prepare']) disabled(markup, id);
  assert.match(markup, /native confirmation/);
  assert.match(markup, /Proposal application is unavailable/);
  assert.match(markup, /No endpoint call was made/);
}
for (const operation of ['decompose', 'merge', 'split']) {
  const markup = renderCompositionView({ operation, bridgeAvailable: false, status: 'unavailable' });
  disabled(markup, 'composition-start');
  assert.match(markup, /Browser preview cannot invent saved records/);
}
const skills = renderSkillsView({ bridgeAvailable: false, capabilities: [], listStatus: 'unavailable' });
for (const pane of ['skills-library', 'skills-editor', 'skills-provenance']) assert.ok(skills.includes(pane));
disabled(skills, 'skills-create');
const workspace = renderWorkspaceView({ status: 'unavailable', bridgeAvailable: false });
disabled(workspace, 'workspace-export');
disabled(workspace, 'workspace-restore');
assert.match(renderEngineView({ bridgeAvailable: false, status: 'unavailable' }), /adapter_not_implemented/);

const pkg = JSON.parse(await read('package.json'));
for (const command of ['validate:vision', 'validate:visuals', 'npm test']) assert.ok(pkg.scripts['build:check'].includes(command));
assert.match(await read('.github/workflows/source-checks.yml'), /npm run build:check/);
console.log(`PASS: 8 reference hashes, ${ids.size} feature mappings, shared-shell contracts and native-unavailable controls. Rendered review remains required.`);
