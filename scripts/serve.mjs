import { createServer } from 'node:http';
import { readFile, realpath, stat } from 'node:fs/promises';
import { extname, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const previewDirectory = fileURLToPath(new URL('../preview/', import.meta.url));
const types = {
  '.html': 'text/html; charset=utf-8', '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8', '.png': 'image/png', '.webp': 'image/webp',
  '.svg': 'image/svg+xml', '.json': 'application/json; charset=utf-8',
};

export function createPreviewServer(directory = previewDirectory) {
  const rootPromise = realpath(directory);
  return createServer(async (request, response) => {
    const headers = {
      'X-Content-Type-Options': 'nosniff',
      'Referrer-Policy': 'no-referrer',
      'Cache-Control': 'no-store',
      'Content-Security-Policy': "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'; font-src 'self'; connect-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
    };
    const send = (status, body, type = 'text/plain; charset=utf-8') => {
      response.writeHead(status, { ...headers, 'Content-Type': type });
      response.end(request.method === 'HEAD' ? undefined : body);
    };
    if (!['GET', 'HEAD'].includes(request.method)) {
      response.setHeader('Allow', 'GET, HEAD');
      return send(405, 'Method not allowed');
    }
    let pathname;
    try { pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname); }
    catch { return send(400, 'Bad request'); }
    if (pathname.includes('\0') || pathname.includes('\\')) return send(400, 'Bad request');
    if (pathname.split('/').some(part => part.startsWith('.'))) return send(404, 'Not found');
    try {
      const root = await rootPromise;
      const file = resolve(root, `.${pathname === '/' ? '/index.html' : pathname}`);
      if (!file.startsWith(root + sep)) return send(404, 'Not found');
      const target = await realpath(file);
      if (!target.startsWith(root + sep) || !(await stat(target)).isFile()) return send(404, 'Not found');
      const type = types[extname(target)];
      if (!type) return send(404, 'Not found');
      return send(200, await readFile(target), type);
    } catch (error) {
      return send(['ENOENT', 'ENOTDIR', 'EACCES'].includes(error.code) ? 404 : 500, 'Preview file unavailable');
    }
  });
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const port = Number(process.env.PORT || 4377);
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('Invalid PORT');
  const server = createPreviewServer();
  server.listen(port, '127.0.0.1', () => console.log(`Rangoon app preview: http://127.0.0.1:${port}`));
}
