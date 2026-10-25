// what the three readers share: words turned into numbers for a chart, values into exact text for a table
export const utf8 = new TextDecoder();
const TWO32 = 2 ** 32;

export const isValid = (valid, i) => ((valid[i >> 3] >> (i & 7)) & 1) === 1;

function maskNulls(out, valid, n) {
  if (!valid) return out;
  for (let i = 0; i < n; i++) if (!isValid(valid, i)) out[i] = NaN;
  return out;
}

// an i64 read as two i32 words, exact below 2^53, which microseconds since 1970 stay until 2255
export function wholeToFloat(words, n, valid) {
  const out = new Float64Array(n);
  for (let i = 0; i < n; i++) out[i] = words[2 * i + 1] * TWO32 + (words[2 * i] >>> 0);
  return maskNulls(out, valid, n);
}

export function floats(values, n, valid) {
  return valid ? maskNulls(values.slice(0, n), valid, n) : values;
}

// an i128 read as four i32 words, then the scale; a chart needs the shape, the table keeps the digits
export function decimalToFloat(words, n, scale, valid) {
  const out = new Float64Array(n);
  const div = 10 ** scale;
  for (let i = 0, w = 0; i < n; i++, w += 4) {
    out[i] = (words[w + 3] * 2 ** 96 + (words[w + 2] >>> 0) * 2 ** 64 + (words[w + 1] >>> 0) * TWO32 + (words[w] >>> 0)) / div;
  }
  return maskNulls(out, valid, n);
}

// one decode for the whole buffer when it is ASCII, since then a byte offset is a character offset
export function texts(bytes, offsets, n) {
  const base = offsets[0];
  const all = utf8.decode(bytes.subarray(base, offsets[n]));
  const out = new Array(n);
  if (all.length === offsets[n] - base) {
    for (let i = 0; i < n; i++) out[i] = all.substring(offsets[i] - base, offsets[i + 1] - base);
  } else {
    for (let i = 0; i < n; i++) out[i] = utf8.decode(bytes.subarray(offsets[i], offsets[i + 1]));
  }
  return out;
}

export function codes(strings) {
  const index = new Map();
  const names = [];
  const out = new Int32Array(strings.length);
  for (let i = 0; i < strings.length; i++) {
    let code = index.get(strings[i]);
    if (code === undefined) {
      code = names.length;
      index.set(strings[i], code);
      names.push(strings[i]);
    }
    out[i] = code;
  }
  return { codes: out, names };
}

const pad = (n, width) => String(n).padStart(width, '0');

export function clock(us, zoned) {
  const ms = Math.floor(us / 1000);
  return new Date(ms).toISOString().slice(0, 23) + pad(us - ms * 1000, 3) + (zoned ? 'Z' : '');
}

export const day = (days) => new Date(days * 86_400_000).toISOString().slice(0, 10);

export function decimalText(mantissa, scale) {
  if (scale <= 0) return mantissa.toString() + '0'.repeat(-scale);
  const negative = mantissa < 0n;
  const digits = (negative ? -mantissa : mantissa).toString().padStart(scale + 1, '0');
  return (negative ? '-' : '') + digits.slice(0, -scale) + '.' + digits.slice(-scale);
}

export function table(columns, rows) {
  const out = new Array(rows);
  for (let i = 0; i < rows; i++) {
    const row = new Array(columns.length);
    for (let c = 0; c < columns.length; c++) row[c] = columns[c].cell(i);
    out[i] = row;
  }
  return out;
}

// cells of a binary column, shared by Arrow and our layout since both are the same buffers in another envelope
export function binaryCell(kind, valid) {
  const at = (f) => (valid ? (i) => (isValid(valid, i) ? f(i) : '') : f);
  switch (kind.type) {
    case 'time':
    case 'wall': {
      const zoned = kind.type === 'time';
      if (kind.f64) return at((i) => clock(kind.f64[i], zoned));
      return at((i) => clock(Number(kind.big[i]), zoned));
    }
    case 'date':
      return at((i) => day(kind.days[i]));
    case 'int':
      return at((i) => kind.big[i].toString());
    case 'float':
      return at((i) => String(kind.f64[i]));
    case 'decimal': {
      const { lo, hi, scale } = kind;
      return at((i) => decimalText((hi[2 * i + 1] << 64n) + lo[2 * i], scale));
    }
    case 'text': {
      const { bytes, offsets } = kind;
      return at((i) => utf8.decode(bytes.subarray(offsets[i], offsets[i + 1])));
    }
    case 'bool':
      return at((i) => (isValid(kind.bits, i) ? 'true' : 'false'));
    case 'labels':
      return at((i) => kind.names[kind.codes[i]]);
  }
  throw new Error(`no cell for ${kind.type}`);
}
