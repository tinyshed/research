// serves the probe isolated across origins, which is what gives performance.now() its fine resolution
import { createReadStream, readFileSync, statSync } from 'node:fs';
import { createServer } from 'node:http';
import { extname, join, normalize } from 'node:path';

const root = new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1');
const types = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript' };
const port = Number(process.argv[2] ?? 8765);

createServer((req, res) => {
  const path = normalize(join(root, decodeURIComponent(new URL(req.url, 'http://x').pathname)));
  if (!path.startsWith(normalize(root))) return res.writeHead(403).end();
  try {
    statSync(path);
  } catch {
    return res.writeHead(404).end();
  }
  res.writeHead(200, {
    'content-type': types[extname(path)] ?? 'application/octet-stream',
    'cross-origin-opener-policy': 'same-origin',
    'cross-origin-embedder-policy': 'require-corp',
    'cache-control': 'no-store',
  });
  // a salt makes a script new source text, so no compilation cache can answer for it
  const salt = new URL(req.url, 'http://x').searchParams.get('salt');
  if (salt) return res.end(readFileSync(path, 'utf8') + `
//${salt}
`);
  createReadStream(path).pipe(res);
}).listen(port, '127.0.0.1', () => console.log(`http://127.0.0.1:${port}/js/bench.html`));
