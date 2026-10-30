import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { workspaceSaveDisclosure } from '../preview/workspace-lock-model.mjs';

const encrypted = disclosure => {
  assert.match(disclosure, /encrypted workspace/i);
  assert.doesNotMatch(disclosure, /unencrypted/i);
  assert.match(disclosure, /Original files and exports remain readable/i);
  assert.match(disclosure, /No upload/i);
};

const unencrypted = disclosure => {
  assert.match(disclosure, /unencrypted/i);
  assert.doesNotMatch(disclosure, /encrypted workspace/i);
  assert.match(disclosure, /No upload/i);
};

test('save disclosure claims encrypted workspace storage only for usable validated managed states', () => {
  encrypted(workspaceSaveDisclosure({ status: 'open', blocked: false }));
  encrypted(workspaceSaveDisclosure({ status: 'confirming', blocked: false }));

  unencrypted(workspaceSaveDisclosure({ status: 'legacy', blocked: false }));

  for (const state of [
    { status: 'open', blocked: true },
    { status: 'confirming', blocked: true },
    { status: 'browser', blocked: false },
    { status: 'unavailable', blocked: false },
    { status: 'checking', blocked: false },
    { status: 'unknown', blocked: false },
    null,
    {},
  ]) {
    const disclosure = workspaceSaveDisclosure(state);
    assert.doesNotMatch(disclosure, /encrypted workspace/i);
  }
});

test('analysis delegates its save disclosure to the workspace-state helper', async () => {
  const source = await readFile(new URL('../preview/analyze.mjs', import.meta.url), 'utf8');
  assert.match(source, /import\s*\{[^}]*workspaceSaveDisclosure[^}]*\}\s*from\s*['"]\.\/workspace-lock-model\.mjs['"]/);
  assert.match(source, /workspaceSaveDisclosure\(workspaceLock\.getState\(\)\)/);
  assert.doesNotMatch(source, /Stores original source text unencrypted on this computer\. No upload\./);
});
