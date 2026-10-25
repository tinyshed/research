// small frames, where a fixed cost per frame is all there is: decode, chart and table, many times over
import { table } from './formats/common.mjs';
import { formats } from './run.mjs';

export const sizes = [1, 30, 1000];
export const datasets = ['series', 'mixed'];
const repeat = { 1: 20000, 30: 5000, 1000: 300 };

export function runSmall(bytes, now) {
  const lines = [`${'frame'.padEnd(24)}${'µs per frame'.padStart(14)}`];
  for (const n of sizes) {
    for (const d of datasets) {
      for (const { label, reader } of formats) {
        const buf = bytes[`${n}/${d}.${label}`];
        let sink = 0;
        const batches = [];
        for (let b = 0; b < 8; b++) {
          const t0 = now();
          for (let i = 0; i < repeat[n]; i++) {
            const frame = reader.decode(buf);
            sink += reader.chart(frame).time.length + table(reader.columns(frame), Math.min(n, 1000)).length;
          }
          if (b > 0) batches.push(((now() - t0) * 1000) / repeat[n]);
        }
        if (sink === 0) throw new Error('nothing ran');
        batches.sort((a, b) => a - b);
        lines.push(`${`${n}/${d}.${label}`.padEnd(24)}${batches[batches.length >> 1].toFixed(2).padStart(14)}`);
      }
    }
  }
  return lines.join('\n');
}
