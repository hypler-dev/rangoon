import { readFile, readdir, stat } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const manifest = JSON.parse(await readFile(path.join(root, 'docs/reference-manifest.json'), 'utf8'));
for (const entry of [...manifest.images, manifest.historical_brief]) {
  const bytes = await readFile(path.join(root, entry.path));
  const hash = createHash('sha256').update(bytes).digest('hex');
  if (hash !== entry.sha256) throw new Error(`Reference changed: ${entry.path}`);
}

let checked = 0;
async function inspect(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (entry.name.startsWith('.') || ['target', 'node_modules'].includes(entry.name)) continue;
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) { await inspect(file); continue; }
    if (!/\.(md|mjs|css|html|json|rs|toml|yml)$/.test(entry.name)) continue;
    if (file.includes(`${path.sep}references${path.sep}`) || file.includes(`${path.sep}fixtures${path.sep}`)) continue;
    const content = await readFile(file, 'utf8');
    if (/[\t ]+$/m.test(content)) throw new Error(`Trailing whitespace: ${file}`);
    if (entry.name.endsWith('.md')) {
      for (const match of content.matchAll(/\]\(([^)]+)\)/g)) {
        const target = match[1];
        if (/^(?:https?:|#|mailto:)/.test(target)) continue;
        await stat(path.resolve(path.dirname(file), target.split('#')[0]));
      }
    }
    checked++;
  }
}
await inspect(root);
console.log(`PASS: ${manifest.images.length} image hashes, historical brief hash, local Markdown targets, ${checked} authored text files without trailing whitespace.`);
