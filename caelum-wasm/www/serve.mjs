// Minimal static server that sets the COOP/COEP headers z3.js needs for its
// SharedArrayBuffer-backed threads (cross-origin isolation). Serves the
// caelum-wasm crate directory so `/www/index.html` can import `../pkg/...`.
//
//   node caelum-wasm/www/serve.mjs   # then open http://localhost:8080/www/
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const crateDir = join(fileURLToPath(new URL('.', import.meta.url)), '..');
const types = {
  '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript',
  '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css',
};

createServer(async (req, res) => {
  res.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
  res.setHeader('Cross-Origin-Embedder-Policy', 'require-corp');
  try {
    let path = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
    if (path.endsWith('/')) path += 'index.html';
    const file = join(crateDir, normalize(path));
    if (!file.startsWith(crateDir)) throw new Error('forbidden');
    const body = await readFile(file);
    res.setHeader('Content-Type', types[extname(file)] || 'application/octet-stream');
    res.end(body);
  } catch {
    res.statusCode = 404;
    res.end('not found');
  }
}).listen(8080, () => console.log('serving on http://localhost:8080/www/'));
