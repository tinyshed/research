// node small.mjs, or bun small.mjs: the frames gen wrote into out-1, out-30 and out-1000
import { readFileSync } from 'node:fs';
import { formats } from './run.mjs';
import { datasets, runSmall, sizes } from './small-run.mjs';

const bytes = {};
for (const n of sizes) {
  for (const d of datasets) {
    for (const { label } of formats) {
      const b = readFileSync(new URL(`../out-${n}/${d}.${label}`, import.meta.url));
      bytes[`${n}/${d}.${label}`] = b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength);
    }
  }
}
console.log(process.versions.bun ? `bun ${process.versions.bun}` : `node ${process.version}`);
console.log(runSmall(bytes, () => performance.now()));
