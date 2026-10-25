// node --expose-gc load.mjs <reader>, once per process: what loading a reader's bundle costs before any frame arrives
import { readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

const reader = process.argv[2];
// a salted copy, so no compilation cache on disk can answer for it
const file = join(tmpdir(), `wire-probe-${reader}-${process.pid}.mjs`);
writeFileSync(file, `${readFileSync(new URL(`./dist/reader-${reader}.js`, import.meta.url), 'utf8')}\n//${Math.random()}\n`);
const collect = globalThis.gc ?? (() => Bun.gc(true));
const used = () => {
  collect();
  collect();
  return process.memoryUsage().heapUsed;
};
const before = used();
const t0 = performance.now();
await import(pathToFileURL(file).href);
const ms = performance.now() - t0;
console.log(JSON.stringify({ reader, ms, heapKiB: (used() - before) / 1024, ok: typeof globalThis.reader.decode }));
