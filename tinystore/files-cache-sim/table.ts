// Reads run.sh's JSON lines and prints the report's tables in markdown.
//
//   bun table.ts results.jsonl

const lines = (await Bun.file(process.argv[2]).text()).trim().split("\n").map((l) => JSON.parse(l));
const POLICIES = ["lru-exact", "2q-exact", "lru-coarse", "rank-6h", "rank-1d", "rank-7d"];
const pct = (x: number) => (100 * x).toFixed(1);
const signed = (x: number) => (x >= 0 ? "+" : "−") + Math.abs(100 * x).toFixed(1);

type Row = { trace: string; alpha: number; ceiling: number; seeds: any[] };
const rows = new Map<string, Row>();
for (const l of lines) {
  const id = `${l.trace} ${l.alpha} ${l.ceiling}`;
  if (!rows.has(id)) rows.set(id, { trace: l.trace, alpha: l.alpha, ceiling: l.ceiling, seeds: [] });
  rows.get(id)!.seeds.push(l);
}
const order = ["zipf", "crawler", "shift", "lecture"];
const sorted = [...rows.values()].sort((a, b) =>
  order.indexOf(a.trace) - order.indexOf(b.trace) || a.alpha - b.alpha || a.ceiling - b.ceiling);

const label = (r: Row) =>
  `${r.trace}${r.trace === "zipf" || r.trace === "crawler" ? ` α ${r.alpha.toFixed(1)}` : ""}, ${r.ceiling * 100} %`;
const rate = (s: any, p: string, part: string, bytes: boolean) => {
  const c = s.results[p][part];
  return bytes ? c.hitBytes / c.bytes : c.hits / c.reads;
};
const mean = (r: Row, p: string, part: string, bytes: boolean) =>
  r.seeds.reduce((a, s) => a + rate(s, p, part, bytes), 0) / r.seeds.length;

function table(title: string, part: string, bytes: boolean, filter: (r: Row) => boolean) {
  console.log(`\n### ${title}\n`);
  console.log(`| trace, ceiling | ${POLICIES.join(" | ")} | rank-1d − lru-coarse, each seed |`);
  console.log(`|---|${POLICIES.map(() => "---:").join("|")}|---|`);
  for (const r of sorted.filter(filter)) {
    const cells = POLICIES.map((p) => pct(mean(r, p, part, bytes)));
    const deltas = r.seeds.sort((a, b) => a.seed - b.seed)
      .map((s) => signed(rate(s, "rank-1d", part, bytes) - rate(s, "lru-coarse", part, bytes)));
    console.log(`| ${label(r)} | ${cells.join(" | ")} | ${deltas.join("; ")} |`);
  }
}

const all = () => true, crawler = (r: Row) => r.trace === "crawler";
table("Hit rate, % of reads, mean of five seeds", "users", false, all);
table("Byte hit rate, % of bytes read, mean of five seeds", "users", true, all);
table("Hit rate in the 24 hours after each crawl", "after", false, crawler);
table("Byte hit rate in the 24 hours after each crawl", "after", true, crawler);
