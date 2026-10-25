// Arrow IPC through apache-arrow's parser, values read from its buffers: its own getters drop a decimal's scale
import { tableFromIPC } from 'apache-arrow';
import { binaryCell, codes, decimalToFloat, floats, texts, wholeToFloat } from './common.mjs';

export const name = 'arrow';

export function decode(buf) {
  const table = tableFromIPC(new Uint8Array(buf));
  const rows = table.numRows;
  const columns = table.schema.fields.map((field) => {
    const vector = table.getChild(field.name);
    if (vector.data.length !== 1) throw new Error(`${field.name}: ${vector.data.length} chunks`);
    const d = vector.data[0];
    const col = { name: field.name, unit: field.metadata.get('unit') ?? null, valid: d.nullCount > 0 ? d.nullBitmap : null };
    const v = d.values;
    const t = String(field.type);
    if (t.startsWith('Timestamp')) {
      col.type = t.includes('UTC') ? 'time' : 'wall';
      col.big = v;
      col.words = new Int32Array(v.buffer, v.byteOffset, rows * 2);
    } else if (t === 'Int64') {
      col.type = 'int';
      col.big = v;
      col.words = new Int32Array(v.buffer, v.byteOffset, rows * 2);
    } else if (t.startsWith('Date32')) {
      col.type = 'date';
      col.days = v;
    } else if (t === 'Float64') {
      col.type = 'float';
      col.f64 = v;
    } else if (t.startsWith('Decimal')) {
      col.type = 'decimal';
      col.scale = field.type.scale;
      col.words = new Int32Array(v.buffer, v.byteOffset, rows * 4);
      col.lo = new BigUint64Array(v.buffer, v.byteOffset, rows * 2);
      col.hi = new BigInt64Array(v.buffer, v.byteOffset, rows * 2);
    } else if (t === 'Utf8') {
      col.type = 'text';
      col.offsets = d.valueOffsets;
      col.bytes = v;
    } else if (t === 'Bool') {
      col.type = 'bool';
      col.bits = v;
    } else throw new Error(`${field.name}: ${t}`);
    return col;
  });
  return { rows, columns };
}

const find = (frame, name) => frame.columns.find((c) => c.name === name);

function numbers(col, rows) {
  if (col.type === 'decimal') return decimalToFloat(col.words, rows, col.scale, col.valid);
  if (col.f64) return floats(col.f64, rows, col.valid);
  return wholeToFloat(col.words, rows, col.valid);
}

export function chart(frame) {
  const site = find(frame, 'site');
  const out = codes(texts(site.bytes, site.offsets, frame.rows));
  for (const n of ['time', 'count', 'ratio', 'amount']) out[n] = numbers(find(frame, n), frame.rows);
  return out;
}

export const columns = (frame) => frame.columns.map((c) => ({ name: c.name, cell: binaryCell(c, c.valid) }));
