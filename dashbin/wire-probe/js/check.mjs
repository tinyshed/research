// proves the readers agree, without timing anything
import { readFileSync } from 'node:fs';
import { files, verify } from './run.mjs';

const bytes = {};
for (const name of files) {
  const b = readFileSync(new URL(`../out/${name}`, import.meta.url));
  bytes[name] = b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength);
}
for (const [d, s] of Object.entries(verify(bytes))) console.log(`agreed ${d}: ${s}`);
