// node --expose-gc node.mjs, or bun node.mjs: the rounds, then what each format keeps alive once decoded and drawn
import { readFileSync } from 'node:fs';
import { bench, files, formats, render, verify } from './run.mjs';

const bytes = {};
for (const name of files) {
  const b = readFileSync(new URL(`../out/${name}`, import.meta.url));
  bytes[name] = b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength);
}

const engine = process.versions.bun
  ? `bun ${process.versions.bun} (JavaScriptCore)`
  : `node ${process.version} (V8 ${process.versions.v8})`;
const digests = verify(bytes);
console.log(render(engine, digests, bench(bytes, () => performance.now())));

const collect = globalThis.gc ?? (globalThis.Bun && (() => Bun.gc(true)));
const used = () => {
  collect();
  collect();
  const m = process.memoryUsage();
  return m.heapUsed + m.arrayBuffers;
};

// a function of its own, so what one format left behind is garbage before the next one is measured
function retained(name) {
  const { reader } = formats.find((f) => name.endsWith(`.${f.label}`));
  const before = used();
  const frame = reader.decode(bytes[name]);
  const decoded = used();
  const drawn = reader.chart(frame);
  const after = used();
  if (drawn.time.length !== frame.rows) throw new Error(name);
  return [decoded - before, after - before];
}

if (collect) {
  console.log('\nretained beyond the received bytes, KiB, median of 5');
  console.log(`${'frame'.padEnd(20)}${'decoded'.padStart(10)}${'+ drawn'.padStart(10)}`);
  for (const name of files) {
    const runs = Array.from({ length: 5 }, () => retained(name));
    const kib = (i) => (runs.map((r) => r[i]).sort((a, b) => a - b)[2] / 1024).toFixed(0).padStart(10);
    console.log(`${name.padEnd(20)}${kib(0)}${kib(1)}`);
  }
}
