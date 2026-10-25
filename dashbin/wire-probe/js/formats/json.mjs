// columnar JSON: JSON.parse does the work, and a chart then copies each column into a typed array
import { clock, codes, day, utf8 } from './common.mjs';

export const name = 'json';

export const decode = (buf) => JSON.parse(utf8.decode(buf));

const find = (frame, name) => frame.columns.find((c) => c.name === name);

function numbers(values) {
  const out = new Float64Array(values.length);
  for (let i = 0; i < values.length; i++) out[i] = values[i] === null ? NaN : +values[i];
  return out;
}

export function chart(frame) {
  const out = codes(find(frame, 'site').values);
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
    const text = cells[c.type];
    return { name: c.name, cell: (i) => (c.values[i] === null ? '' : text(c.values[i])) };
  });
