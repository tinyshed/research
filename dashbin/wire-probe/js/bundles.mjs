// bun bundles.mjs: what each reader adds to a browser bundle, minified, raw and gzip; load.mjs then loads them
import { gzipSync } from 'node:zlib';

console.log(`${'reader'.padEnd(8)}${'raw'.padStart(10)}${'gzip'.padStart(10)}`);
for (const reader of ['json', 'arrow', 'own']) {
  const entry = `./dist/entry-${reader}.mjs`;
  await Bun.write(entry, `import * as r from '../formats/${reader}.mjs';\nimport { table } from '../formats/common.mjs';\nglobalThis.reader = { ...r, table };\n`);
  const built = await Bun.build({ entrypoints: [entry], minify: true, target: 'browser' });
  if (!built.success) throw new AggregateError(built.logs, reader);
  const code = new Uint8Array(await built.outputs[0].arrayBuffer());
  await Bun.write(`./dist/reader-${reader}.js`, code);
  console.log(`${reader.padEnd(8)}${String(code.length).padStart(10)}${String(gzipSync(code, { level: 6 }).length).padStart(10)}`);
}
