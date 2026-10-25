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
if (collect) {
  const used = () => {
    collect();
    collect();
    const m = process.memoryUsage();
    return m.heapUsed + m.arrayBuffers;
  };
  console.log('\nretained beyond the received bytes, KiB');
  console.log(`${'frame'.padEnd(16)}${'decoded'.padStart(10)}${'+ drawn'.padStart(10)}`);
  for (const name of files) {
    const { reader } = formats.find((f) => name.endsWith(`.${f.label}`));
    const before = used();
    const frame = reader.decode(bytes[name]);
    const decoded = used();
    const drawn = reader.chart(frame);
    const after = used();
    const kib = (x) => (x / 1024).toFixed(0).padStart(10);
    console.log(`${name.padEnd(16)}${kib(decoded - before)}${kib(after - before)}`, frame.rows === drawn.time.length ? '' : '!');
  }
}
