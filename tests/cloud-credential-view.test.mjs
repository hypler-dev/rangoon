import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import { renderCloudCredentialView } from '../preview/cloud-credential-view.mjs';

test('custody panel stays unknown by default and disables removal', () => {
  const html = renderCloudCredentialView({ bridgeAvailable: true, status: 'unknown', message: 'Credential custody has not been inspected.' });
  assert.match(html, /OpenAI credential/);
  assert.match(html, /Authority<\/dt><dd>None/);
  assert.match(html.match(/id="cloud-credential-remove"[^>]*>/)[0], /disabled/);
  assert.match(html, /Cloud transport remains unavailable/);
});

test('custody panel enables remove only for stored state and names no provider traffic', () => {
  const html = renderCloudCredentialView({ bridgeAvailable: false, status: 'stored', backend: 'macos_keychain', revision: 'a'.repeat(64), message: 'Native credential custody is unavailable in browser preview.' });
  assert.match(html, /macOS Keychain/);
  assert.match(html.match(/id="cloud-credential-remove"[^>]*>/)[0], /disabled/);
  assert.match(html, /No provider check or send was made/);
});

test('isolated entry source has blank password-only secret handling and bounded raw IPC', async () => {
  const [html, source, css] = await Promise.all([
    readFile(new URL('../preview/cloud-credential-entry.html', import.meta.url), 'utf8'),
    readFile(new URL('../preview/cloud-credential-entry.mjs', import.meta.url), 'utf8'),
    readFile(new URL('../preview/cloud-credential-entry.css', import.meta.url), 'utf8'),
  ]);
  assert.match(html, /type="password"/);
  assert.match(html, /autocomplete="off"/);
  assert.doesNotMatch(html, /reveal|show password/i);
  assert.match(source, /get_cloud_credential_edit/);
  assert.match(source, /submit_cloud_credential/);
  assert.match(source, /cancel_cloud_credential_edit/);
  assert.match(source, /bytes\.fill\(0\)/);
  assert.match(source, /bytes\.length > 12544/);
  assert.doesNotMatch(source, /console\.|localStorage|sessionStorage|clipboard/);
  assert.match(css, /min-height:44px/);
});
