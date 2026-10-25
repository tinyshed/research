// our layout: the header is read, every buffer becomes a view over the bytes as they came
import { binaryCell, codes, decimalToFloat, floats, texts, utf8, wholeToFloat } from './common.mjs';

export const name = 'own';
const TYPES = [null, 'time', 'wall', 'date', 'int', 'float', 'decimal', 'text', 'bool'];
const align = (n) => (n + 7) & ~7;

export function decode(buf) {
  const view = new DataView(buf);
  if (view.getUint32(0, true) !== 0x31464244) throw new Error('not a frame');
  const rows = view.getUint32(4, true);
  const count = view.getUint32(8, true);
  let at = 16;
  const string = () => {
    const n = view.getUint16(at, true);
    const s = utf8.decode(new Uint8Array(buf, at + 2, n));
    at += 2 + n;
    return s;
  };
  const heads = [];
  for (let c = 0; c < count; c++) {
    const head = { type: TYPES[view.getUint8(at)], flags: view.getUint8(at + 1), scale: view.getInt8(at + 2), textBytes: view.getUint32(at + 4, true) };
    at += 8;
    head.name = string();
    head.unit = string() || null;
    heads.push(head);
  }
  at = view.getUint32(12, true);
  const bitmap = rows + 7 >> 3;
  const columns = heads.map((head) => {
    const col = { name: head.name, unit: head.unit, type: head.type, valid: null };
    if (head.flags & 1) {
      col.valid = new Uint8Array(buf, at, bitmap);
      at = align(at + bitmap);
    }
    switch (head.type) {
      case 'time':
      case 'wall':
      case 'int':
        if (head.flags & 2) col.f64 = new Float64Array(buf, at, rows);
        else {
          col.words = new Int32Array(buf, at, rows * 2);
          col.big = new BigInt64Array(buf, at, rows);
        }
        at += rows * 8;
        break;
      case 'date':
        col.days = new Int32Array(buf, at, rows);
        at = align(at + rows * 4);
        break;
      case 'float':
        col.f64 = new Float64Array(buf, at, rows);
        at += rows * 8;
        break;
      case 'decimal':
        col.scale = head.scale;
        col.words = new Int32Array(buf, at, rows * 4);
        col.lo = new BigUint64Array(buf, at, rows * 2);
        col.hi = new BigInt64Array(buf, at, rows * 2);
        at += rows * 16;
        break;
      case 'text':
        col.offsets = new Uint32Array(buf, at, rows + 1);
        at = align(at + (rows + 1) * 4);
        col.bytes = new Uint8Array(buf, at, head.textBytes);
        at = align(at + head.textBytes);
        break;
      case 'bool':
        col.bits = new Uint8Array(buf, at, bitmap);
        at = align(at + bitmap);
        break;
    }
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
