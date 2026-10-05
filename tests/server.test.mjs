import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, symlink, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createPreviewServer } from '../scripts/serve.mjs';

test('preview server serves assets and confines reads to its public directory', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'rangoon-preview-test-'));
  const root = join(directory, 'root');
  const { mkdir } = await import('node:fs/promises');
  await mkdir(root);
  await writeFile(join(root, 'index.html'), '<h1>Sample</h1>');
  await writeFile(join(root, 'app.mjs'), 'export const sample = true;');
  await writeFile(join(directory, 'private.html'), 'outside');
  await symlink(join(directory, 'private.html'), join(root, 'escape.html'));
  const server = createPreviewServer(root);
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  try {
    const page = await fetch(base);
    assert.equal(page.status, 200);
    assert.match(page.headers.get('content-security-policy'), /connect-src 'none'/);
    assert.equal(await page.text(), '<h1>Sample</h1>');
    const module = await fetch(`${base}/app.mjs`);
    assert.match(module.headers.get('content-type'), /javascript/);
    for (const path of ['/escape.html', '/%2e%2e%2fprivate.html', '/.git/config', '/assets/', '/missing.html']) {
      assert.equal((await fetch(base + path)).status, 404, path);
    }
    assert.equal((await fetch(base + '/%00')).status, 400);
    assert.equal((await fetch(base, { method: 'POST' })).status, 405);
    assert.equal(await (await fetch(base, { method: 'HEAD' })).text(), '');
  } finally {
    await new Promise(resolve => server.close(resolve));
    await rm(directory, { recursive: true, force: true });
  }
});
