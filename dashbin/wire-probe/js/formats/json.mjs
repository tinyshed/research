// columnar JSON: JSON.parse does the work, and a chart then copies each column into a typed array
// a text column may carry a dictionary, and then its values are codes into it
import { clock, codes, day, utf8 } from './common.mjs';

export const name = 'json';

export const decode = (buf) => JSON.parse(utf8.decode(buf));

const find = (frame, name) => frame.columns.find((c) => c.name === name);

function numbers(values) {
  const out = new Float64Array(values.length);
  for (let i = 0; i < values.length; i++) out[i] = values[i] === null ? NaN : +values[i];
  return out;
}

function labels({ values, dictionary }) {
  const out = new Int32Array(values.length);
  for (let i = 0; i < values.length; i++) out[i] = values[i] ?? 0;
  return { codes: out, names: dictionary };
}

export function chart(frame) {
  const site = find(frame, 'site');
  const out = site.dictionary ? labels(site) : codes(site.values);
  for (const n of ['time', 'count', 'ratio', 'amount']) out[n] = numbers(find(frame, n).values);
  return out;
}

const cells = {
  time: (v) => clock(v, true),
  wall: (v) => clock(v, false),
  date: day,
  int: String,
  float: String,
  decimal: String,
  text: String,
  bool: String,
};

export const columns = (frame) =>
  frame.columns.map((c) => {
    const text = c.dictionary ? (v) => c.dictionary[v] : cells[c.type];
    return { name: c.name, cell: (i) => (c.values[i] === null ? '' : text(c.values[i])) };
  });
