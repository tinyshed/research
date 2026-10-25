// the same rounds in every engine: each format decodes a frame, draws it and prints its first thousand rows
import * as arrow from './formats/arrow.mjs';
import { table } from './formats/common.mjs';
import * as json from './formats/json.mjs';
import * as own from './formats/own.mjs';

export const formats = ['', '-dict'].flatMap((labels) => [
  { label: `json${labels}`, reader: json },
  { label: `arrow${labels}`, reader: arrow },
  { label: `own-i64${labels}`, reader: own },
  { label: `own-f64${labels}`, reader: own },
]);
export const datasets = ['series', 'mixed'];
export const files = datasets.flatMap((d) => formats.map((f) => `${d}.${f.label}`));
const TABLE_ROWS = 1000;

function fnv(text, hash) {
  for (let i = 0; i < text.length; i++) hash = Math.imul(hash ^ text.charCodeAt(i), 16777619) >>> 0;
  return hash;
}

function digest(drawn, printed) {
  const sums = ['time', 'count', 'ratio', 'amount'].map((n) => {
    let sum = 0;
    let gaps = 0;
    for (const v of drawn[n]) Number.isFinite(v) ? (sum += v) : gaps++;
    return `${n}=${sum.toPrecision(12)}/${gaps}`;
  });
  const sites = new Map();
  for (const code of drawn.codes) sites.set(drawn.names[code], (sites.get(drawn.names[code]) ?? 0) + 1);
  const perSite = [...sites].sort().map(([s, n]) => `${s}:${n}`).join(',');
  const cells = printed.reduce((h, row) => fnv(row.join('\u001f') + '\u001e', h), 2166136261);
  return `${sums.join(' ')} sites ${perSite} table ${cells.toString(16)}`;
}

// every format must draw and print exactly what the others do, or its timings mean nothing
export function verify(bytes) {
  const out = {};
  for (const d of datasets) {
    const seen = formats.map(({ label, reader }) => {
      const frame = reader.decode(bytes[`${d}.${label}`]);
      return [label, digest(reader.chart(frame), table(reader.columns(frame), TABLE_ROWS))];
    });
    for (const [label, got] of seen) {
      if (got !== seen[0][1]) throw new Error(`${d}: ${label} disagrees with ${seen[0][0]}\n${got}\n${seen[0][1]}`);
    }
    out[d] = seen[0][1];
  }
  return out;
}

const median = (xs) => [...xs].sort((a, b) => a - b)[xs.length >> 1];

export function bench(bytes, now, { warmup = 3, rounds = 12 } = {}) {
  const samples = {};
  let sink = 0;
  for (let r = -warmup; r < rounds; r++) {
    for (const d of datasets) {
      for (let k = 0; k < formats.length; k++) {
        const { label, reader } = formats[(k + r + warmup) % formats.length];
        const buf = bytes[`${d}.${label}`];
        const t0 = now();
        const frame = reader.decode(buf);
        const t1 = now();
        const drawn = reader.chart(frame);
        const t2 = now();
        const printed = table(reader.columns(frame), TABLE_ROWS);
        const t3 = now();
        sink += drawn.time.length + printed.length;
        if (r < 0) continue;
        const s = (samples[`${d}.${label}`] ??= { decode: [], chart: [], table: [] });
        s.decode.push(t1 - t0);
        s.chart.push(t2 - t1);
        s.table.push(t3 - t2);
      }
    }
  }
  const out = {};
  for (const [key, s] of Object.entries(samples)) {
    const both = s.decode.map((x, i) => x + s.chart[i]);
    out[key] = {
      decode: median(s.decode),
      chart: median(s.chart),
      decodeAndChart: median(both),
      table: median(s.table),
      spread: [Math.min(...both), Math.max(...both)],
    };
  }
  return { results: out, sink };
}

export function render(engine, digests, { results }) {
  const f = (x) => x.toFixed(2).padStart(8);
  const lines = [`engine ${engine}`, 'median of 12 rounds after 3 warm-up, ms', ''];
  lines.push(`${'frame'.padEnd(20)}${'decode'.padStart(8)}${'chart'.padStart(8)}${'both'.padStart(8)}${'min..max both'.padStart(18)}${'table'.padStart(8)}`);
  for (const key of files) {
    const r = results[key];
    lines.push(`${key.padEnd(20)}${f(r.decode)}${f(r.chart)}${f(r.decodeAndChart)}${`${r.spread[0].toFixed(2)}..${r.spread[1].toFixed(2)}`.padStart(18)}${f(r.table)}`);
  }
  lines.push('', ...Object.entries(digests).map(([d, s]) => `agreed ${d}: ${s}`));
  return lines.join('\n');
}
