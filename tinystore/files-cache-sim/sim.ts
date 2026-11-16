// Replays traces of reads of derived files against a byte ceiling, a miss
// written at its size, and counts hits for each eviction order.
//
//   bun sim.ts <trace> <alpha> <seed>      one JSON line a ceiling on stdout
//
// <trace> is zipf, crawler, shift or lecture; <alpha> is the Zipf exponent
// (shift and lecture ignore it). Nothing here reads a clock or the disk: a
// trace is a function of its seed, so every number reproduces exactly.

const HOUR = 3600;
const DAY = 86400;
const DAYS = 60;
const KEYS = 100_000;
const READS_A_DAY = 30_000;
const WARMUP = 2 * DAY; // reads before it fill the cache and are not counted
const COARSE = HOUR; // a use is recorded at most once this often

// --- randomness: sfc32, seeded through splitmix32 --------------------------

function rng(seed: number): () => number {
  let s = seed >>> 0;
  const split = () => {
    s = (s + 0x9e3779b9) >>> 0;
    let z = s;
    z = Math.imul(z ^ (z >>> 16), 0x85ebca6b);
    z = Math.imul(z ^ (z >>> 13), 0xc2b2ae35);
    return (z ^ (z >>> 16)) >>> 0;
  };
  let a = split(), b = split(), c = split(), d = split();
  return () => {
    a >>>= 0; b >>>= 0; c >>>= 0; d >>>= 0;
    const t = (a + b) | 0;
    a = b ^ (b >>> 9);
    b = (c + (c << 3)) | 0;
    c = (c << 21) | (c >>> 11);
    d = (d + 1) | 0;
    const r = (t + d) | 0;
    c = (c + r) | 0;
    return (r >>> 0) / 4294967296;
  };
}

function shuffled(n: number, rand: () => number): Int32Array {
  const p = new Int32Array(n);
  for (let i = 0; i < n; i++) p[i] = i;
  for (let i = n - 1; i > 0; i--) {
    const j = Math.floor(rand() * (i + 1));
    const t = p[i]; p[i] = p[j]; p[j] = t;
  }
  return p;
}

function zipf(n: number, alpha: number): Float64Array {
  const cdf = new Float64Array(n);
  let sum = 0;
  for (let i = 0; i < n; i++) cdf[i] = sum += 1 / Math.pow(i + 1, alpha);
  for (let i = 0; i < n; i++) cdf[i] /= sum;
  return cdf;
}

function draw(cdf: Float64Array, rand: () => number): number {
  const u = rand();
  let lo = 0, hi = cdf.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (cdf[mid] < u) lo = mid + 1; else hi = mid;
  }
  return lo;
}

function poisson(mean: number, rand: () => number): number {
  const l = Math.exp(-mean);
  let k = 0, p = rand();
  while (p > l) { k++; p *= rand(); }
  return k;
}

// Log-normal around a median of 200 KiB, sigma 1, clipped to [4 KiB, 64 MiB].
function sizes(rand: () => number): Int32Array {
  const out = new Int32Array(KEYS);
  for (let i = 0; i < KEYS; i++) {
    const g = Math.sqrt(-2 * Math.log(1 - rand())) * Math.cos(2 * Math.PI * rand());
    out[i] = Math.min(64 << 20, Math.max(4 << 10, Math.round(200 * 1024 * Math.exp(g))));
  }
  return out;
}

// --- traces ----------------------------------------------------------------

type Read = { t: number; key: number; crawler: boolean };
type Trace = { reads: Read[]; passes: number[] }; // passes: when each crawl ended

function zipfReads(alpha: number, rand: () => number): Read[] {
  const cdf = zipf(KEYS, alpha);
  const byRank = shuffled(KEYS, rand);
  const reads: Read[] = [];
  for (let i = 0; i < READS_A_DAY * DAYS; i++) {
    reads.push({ t: rand() * DAYS * DAY, key: byRank[draw(cdf, rand)], crawler: false });
  }
  return reads;
}

// Every key once, in a random order, over three hours from 02:00 of a day.
function crawl(day: number, rand: () => number): Read[] {
  const order = shuffled(KEYS, rand);
  const start = day * DAY + 2 * HOUR;
  const reads: Read[] = [];
  for (let i = 0; i < KEYS; i++) {
    reads.push({ t: start + (i / KEYS) * 3 * HOUR, key: order[i], crawler: true });
  }
  return reads;
}

// 70 % of reads go to a hot set of 5,000 keys, Zipf 0.8 within it, chosen
// again every 7 days; 30 % to Zipf 0.8 over every key.
function shiftReads(rand: () => number): Read[] {
  const HOT = 5000;
  const all = zipf(KEYS, 0.8), hot = zipf(HOT, 0.8);
  const byRank = shuffled(KEYS, rand);
  const weeks: Int32Array[] = [];
  for (let w = 0; w * 7 < DAYS; w++) weeks.push(shuffled(KEYS, rand).subarray(0, HOT));
  const reads: Read[] = [];
  for (let i = 0; i < READS_A_DAY * DAYS; i++) {
    const t = rand() * DAYS * DAY;
    const key = rand() < 0.7
      ? weeks[Math.floor(t / (7 * DAY))][draw(hot, rand)]
      : byRank[draw(all, rand)];
    reads.push({ t, key, crawler: false });
  }
  return reads;
}

// 300 course keys, each read Poisson(4) times between 08:00 and 20:00 of every
// weekday (day 0 a Monday); every other key read once, uniformly over the 60 days.
function lectureReads(rand: () => number): Read[] {
  const COURSE = 300;
  const order = shuffled(KEYS, rand);
  const reads: Read[] = [];
  for (let d = 0; d < DAYS; d++) {
    if (d % 7 >= 5) continue;
    for (let c = 0; c < COURSE; c++) {
      const n = poisson(4, rand);
      for (let k = 0; k < n; k++) {
        reads.push({ t: d * DAY + 8 * HOUR + rand() * 12 * HOUR, key: order[c], crawler: false });
      }
    }
  }
  for (let i = COURSE; i < KEYS; i++) {
    reads.push({ t: rand() * DAYS * DAY, key: order[i], crawler: false });
  }
  return reads;
}

function trace(name: string, alpha: number, rand: () => number): Trace {
  let reads: Read[];
  const passes: number[] = [];
  switch (name) {
    case "zipf": reads = zipfReads(alpha, rand); break;
    case "crawler":
      reads = zipfReads(alpha, rand);
      for (const day of [20, 40]) {
        reads.push(...crawl(day, rand));
        passes.push(day * DAY + 5 * HOUR);
      }
      break;
    case "shift": reads = shiftReads(rand); break;
    case "lecture": reads = lectureReads(rand); break;
    default: throw new Error(`unknown trace ${name}`);
  }
  reads.sort((a, b) => a.t - b.t);
  return { reads, passes };
}

// --- policies --------------------------------------------------------------

interface Cache {
  // Answers a read at time t: true for a hit; a miss writes the file.
  read(t: number, key: number, size: number): boolean;
}

// Doubly linked lists over the key space; a key is in at most one list.
class Lists {
  prev = new Int32Array(KEYS).fill(-1);
  next = new Int32Array(KEYS).fill(-1);
  inList = new Int8Array(KEYS).fill(-1);
  head: number[]; tail: number[]; bytes: number[];
  constructor(n: number) {
    this.head = Array(n).fill(-1); this.tail = Array(n).fill(-1); this.bytes = Array(n).fill(0);
  }
  pushFront(l: number, k: number, size: number) {
    this.inList[k] = l; this.prev[k] = -1; this.next[k] = this.head[l];
    if (this.head[l] >= 0) this.prev[this.head[l]] = k; else this.tail[l] = k;
    this.head[l] = k; this.bytes[l] += size;
  }
  remove(k: number, size: number) {
    const l = this.inList[k];
    if (this.prev[k] >= 0) this.next[this.prev[k]] = this.next[k]; else this.head[l] = this.next[k];
    if (this.next[k] >= 0) this.prev[this.next[k]] = this.prev[k]; else this.tail[l] = this.prev[k];
    this.inList[k] = -1; this.bytes[l] -= size;
  }
}

class ExactLru implements Cache {
  lists = new Lists(1);
  constructor(private ceiling: number, private sizeOf: Int32Array) {}
  read(_t: number, key: number, size: number): boolean {
    const hit = this.lists.inList[key] === 0;
    if (hit) this.lists.remove(key, size);
    this.lists.pushFront(0, key, size);
    while (this.lists.bytes[0] > this.ceiling) {
      const victim = this.lists.tail[0];
      this.lists.remove(victim, this.sizeOf[victim]);
    }
    return hit;
  }
}

// 2Q, the full version (Johnson and Shasha, 1994) in bytes: A1in a FIFO of a
// quarter of the ceiling, A1out the ghosts of half of it, Am an LRU.
class TwoQ implements Cache {
  static A1IN = 0; static AM = 1; static A1OUT = 2;
  lists = new Lists(3);
  constructor(private ceiling: number, private sizeOf: Int32Array) {}
  read(_t: number, key: number, size: number): boolean {
    const L = this.lists, where = L.inList[key];
    if (where === TwoQ.AM) { L.remove(key, size); L.pushFront(TwoQ.AM, key, size); return true; }
    if (where === TwoQ.A1IN) return true;
    if (where === TwoQ.A1OUT) { L.remove(key, size); L.pushFront(TwoQ.AM, key, size); }
    else L.pushFront(TwoQ.A1IN, key, size);
    while (L.bytes[TwoQ.A1IN] + L.bytes[TwoQ.AM] > this.ceiling) {
      if (L.bytes[TwoQ.A1IN] > this.ceiling / 4 || L.head[TwoQ.AM] < 0) {
        const v = L.tail[TwoQ.A1IN];
        L.remove(v, this.sizeOf[v]);
        L.pushFront(TwoQ.A1OUT, v, this.sizeOf[v]);
        while (L.bytes[TwoQ.A1OUT] > this.ceiling / 2) {
          const g = L.tail[TwoQ.A1OUT];
          L.remove(g, this.sizeOf[g]);
        }
      } else {
        const v = L.tail[TwoQ.AM];
        L.remove(v, this.sizeOf[v]);
      }
    }
    return false;
  }
}

// The rank of the book: a file's rank is its last recorded use, and a day (or
// `head`) later once it was used again after the write that made it. A use is
// recorded at most once an hour. head = 0 is plain LRU on the coarse clock.
// Eviction pops the lowest rank from a heap whose stale entries are skipped.
class Rank implements Cache {
  cached = new Uint8Array(KEYS);
  recorded = new Float64Array(KEYS);
  rank = new Float64Array(KEYS);
  heapRank: Float64Array; heapKey: Int32Array; heapSeq: Float64Array;
  seqOf = new Float64Array(KEYS);
  n = 0; seq = 0; bytes = 0;
  constructor(private ceiling: number, private sizeOf: Int32Array, private head: number) {
    const cap = 1 << 22;
    this.heapRank = new Float64Array(cap); this.heapKey = new Int32Array(cap); this.heapSeq = new Float64Array(cap);
  }
  read(t: number, key: number, size: number): boolean {
    if (this.cached[key]) {
      if (t - this.recorded[key] >= COARSE) {
        this.recorded[key] = t;
        this.setRank(key, t + this.head);
      }
      return true;
    }
    this.cached[key] = 1; this.bytes += size;
    this.recorded[key] = t;
    this.setRank(key, t);
    while (this.bytes > this.ceiling) this.evictLowest();
    return false;
  }
  private setRank(key: number, r: number) {
    this.rank[key] = r;
    this.seqOf[key] = ++this.seq;
    if (this.n === this.heapRank.length) this.compact();
    this.push(r, key, this.seq);
  }
  private evictLowest() {
    for (;;) {
      const key = this.heapKey[0], s = this.heapSeq[0];
      this.pop();
      if (this.cached[key] && this.seqOf[key] === s) {
        this.cached[key] = 0; this.bytes -= this.sizeOf[key];
        return;
      }
    }
  }
  private compact() {
    const live: [number, number, number][] = [];
    for (let i = 0; i < this.n; i++) {
      const k = this.heapKey[i];
      if (this.cached[k] && this.seqOf[k] === this.heapSeq[i]) live.push([this.heapRank[i], k, this.heapSeq[i]]);
    }
    this.n = 0;
    for (const [r, k, s] of live) this.push(r, k, s);
  }
  private less(i: number, j: number) {
    return this.heapRank[i] < this.heapRank[j] ||
      (this.heapRank[i] === this.heapRank[j] && this.heapSeq[i] < this.heapSeq[j]);
  }
  private swap(i: number, j: number) {
    let x = this.heapRank[i]; this.heapRank[i] = this.heapRank[j]; this.heapRank[j] = x;
    x = this.heapSeq[i]; this.heapSeq[i] = this.heapSeq[j]; this.heapSeq[j] = x;
    const k = this.heapKey[i]; this.heapKey[i] = this.heapKey[j]; this.heapKey[j] = k;
  }
  private push(r: number, key: number, s: number) {
    let i = this.n++;
    this.heapRank[i] = r; this.heapKey[i] = key; this.heapSeq[i] = s;
    while (i > 0) {
      const p = (i - 1) >> 1;
      if (!this.less(i, p)) break;
      this.swap(i, p); i = p;
    }
  }
  private pop() {
    this.n--;
    if (this.n === 0) return;
    this.heapRank[0] = this.heapRank[this.n]; this.heapKey[0] = this.heapKey[this.n]; this.heapSeq[0] = this.heapSeq[this.n];
    let i = 0;
    for (;;) {
      const l = 2 * i + 1, r = l + 1;
      let m = i;
      if (l < this.n && this.less(l, m)) m = l;
      if (r < this.n && this.less(r, m)) m = r;
      if (m === i) break;
      this.swap(i, m); i = m;
    }
  }
}

const POLICIES: [string, (c: number, s: Int32Array) => Cache][] = [
  ["lru-exact", (c, s) => new ExactLru(c, s)],
  ["2q-exact", (c, s) => new TwoQ(c, s)],
  ["lru-coarse", (c, s) => new Rank(c, s, 0)],
  ["rank-6h", (c, s) => new Rank(c, s, 6 * HOUR)],
  ["rank-1d", (c, s) => new Rank(c, s, DAY)],
  ["rank-7d", (c, s) => new Rank(c, s, 7 * DAY)],
];

const CEILINGS: Record<string, number[]> = {
  zipf: [0.1, 0.3], crawler: [0.1, 0.3], shift: [0.1, 0.3], lecture: [0.01, 0.02, 0.1, 0.3],
};

// --- the run ---------------------------------------------------------------

type Count = { reads: number; hits: number; bytes: number; hitBytes: number };
const zero = (): Count => ({ reads: 0, hits: 0, bytes: 0, hitBytes: 0 });
function add(c: Count, hit: boolean, size: number) {
  c.reads++; c.bytes += size;
  if (hit) { c.hits++; c.hitBytes += size; }
}

function main() {
  const [name, alphaArg, seedArg] = process.argv.slice(2);
  const alpha = Number(alphaArg), seed = Number(seedArg);
  const rand = rng(seed * 7919 + Math.round(alpha * 100) + name.length * 104729);
  const sizeOf = sizes(rand);
  const { reads, passes } = trace(name, alpha, rand);
  let total = 0;
  for (let i = 0; i < KEYS; i++) total += sizeOf[i];
  for (const fraction of CEILINGS[name]) {
    const results: Record<string, unknown> = {};
    for (const [policy, make] of POLICIES) {
      const cache = make(fraction * total, sizeOf);
      const users = zero(), crawler = zero(), after = zero();
      for (const r of reads) {
        const size = sizeOf[r.key];
        const hit = cache.read(r.t, r.key, size);
        if (r.t < WARMUP) continue;
        if (r.crawler) { add(crawler, hit, size); continue; }
        add(users, hit, size);
        if (passes.some((p) => r.t >= p && r.t < p + DAY)) add(after, hit, size);
      }
      results[policy] = { users, crawler, after };
    }
    console.log(JSON.stringify({ trace: name, alpha, seed, ceiling: fraction, reads: reads.length, totalBytes: total, results }));
  }
}

main();
