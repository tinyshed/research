// A Bun program under the slice round's load, through one SDK and one way of
// reaching the store: the Rust core in this process or beside it, or the Go
// server beside it. One case a process, a store of its own.
//
// bun bench.ts --sdk <index.ts> --flavor rust|go --mode embedded|sidecar
//   --binary <tinystore> [--library <libtinystore_ffi>] --dir <dir>
//   --case <case> --callers <n> --seconds <s>

const args = process.argv.slice(2)
const arg = (name: string): string => {
	const at = args.indexOf(name)
	if (at < 0 || args[at + 1] === undefined) {
		throw new Error(`missing ${name}`)
	}
	return args[at + 1] as string
}
const optional = (name: string): string | undefined => (args.includes(name) ? arg(name) : undefined)

const KEYS = 100_000
const NOTES = 100_000
const ROWS = 25_000
const SCHEMA =
	'create table note (id integer primary key, title text not null, body text not null) strict'
// the notes the Rust and Go programs write, by one statement
const FILL = `with recursive n(id) as (select 1 union all select id + 1 from n where id < ?)
	insert into note (id, title, body)
	select id, printf('note %012d', id), substr(replace(hex(zeroblob(10)), '00', printf('%010d ', id)), 1, 100) from n`
const POINT = 'select id, title, body from note where id = ?'
const PAGE = 'select id, title, body from note order by id limit 25000'

const flavor = arg('--flavor')
const mode = arg('--mode')
const name = arg('--case')
const callers = Number(arg('--callers'))
const seconds = Number(arg('--seconds'))
const dir = arg('--dir')

const sdk = await import(arg('--sdk'))
const options: Record<string, unknown> =
	mode === 'embedded'
		? { embedded: true, library: arg('--library') }
		: { idle: '1s', binary: arg('--binary') }
const store = await sdk.open(dir, options)

/** The call'th choice of a caller: the xorshift of the Rust and Go programs. */
function pick(caller: number, call: number): bigint {
	const mask = (1n << 64n) - 1n
	let x =
		(((BigInt(caller) + 1n) * 0x9e3779b97f4a7c15n) & mask) ^
		((BigInt(call) * 0xbf58476d1ce4e5b9n) & mask)
	x ^= x >> 30n
	x = (x * 0xbf58476d1ce4e5b9n) & mask
	x ^= x >> 27n
	x = (x * 0x94d049bb133111ebn) & mask
	return x ^ (x >> 31n)
}

const key = (n: number | bigint) => `k${String(n).padStart(8, '0')}`
const value = 'v'.repeat(128)

/** Makes count calls, 64 in flight, untimed. */
async function fill(count: number, call: (n: number) => Promise<unknown>): Promise<void> {
	let next = 0
	const worker = async () => {
		for (let n = next++; n < count; n = next++) {
			await call(n)
		}
	}
	await Promise.all(Array.from({ length: 64 }, worker))
}

/** Calls call from callers loops for the seconds asked, each call timed. */
async function timed(
	loops: number,
	call: (caller: number, n: number) => Promise<unknown>,
): Promise<{ ops: number; elapsed: number; latencies: number[] }> {
	const latencies: number[] = []
	const began = performance.now()
	const until = began + seconds * 1000
	const loop = async (caller: number) => {
		for (let made = 0; performance.now() < until; made++) {
			const at = performance.now()
			await call(caller, made)
			latencies.push(performance.now() - at)
		}
	}
	await Promise.all(Array.from({ length: loops }, (_, caller) => loop(caller)))
	return { ops: latencies.length, elapsed: performance.now() - began, latencies }
}

async function database() {
	const migrations = { '0001_notes.sql': SCHEMA }
	return flavor === 'rust' ? store.database('app', { migrations }) : store.sql('app', { migrations })
}

async function run() {
	switch (name) {
		case 'kv-get':
		case 'kv-set': {
			const bucket = flavor === 'rust' ? store.bucket('sessions') : store.kv.bucket('sessions')
			if (name === 'kv-set') {
				return timed(callers, (caller, n) =>
					bucket.set(`s${String(caller).padStart(2, '0')}-${String(n).padStart(10, '0')}`, value),
				)
			}
			await fill(KEYS, n => bucket.set(key(n), value))
			return timed(callers, async (caller, n) => {
				const found = await bucket.get(key(pick(caller, n) % BigInt(KEYS)))
				if (found?.length !== 128) {
					throw new Error('a key the fill wrote is missing')
				}
			})
		}
		case 'sql-point': {
			const db = await database()
			await db.exec(FILL, NOTES)
			return timed(callers, async (caller, n) => {
				const id = Number(pick(caller, n) % BigInt(NOTES)) + 1
				const note = await db.one(POINT, id)
				if (Number(note?.id) !== id) {
					throw new Error(`note ${id} is missing`)
				}
			})
		}
		case 'sql-rows': {
			const db = await database()
			await db.exec(FILL, ROWS)
			return timed(1, async () => {
				const rows = await db.all(PAGE)
				if (rows.length !== ROWS) {
					throw new Error(`${rows.length} rows of ${ROWS}`)
				}
			})
		}
		case 'jobs-add': {
			const queue = flavor === 'rust' ? store.queue('mail') : store.jobs.queue('mail')
			const add = (n: number) => {
				const job = { n, text: 'send the weekly digest' }
				return flavor === 'rust' ? queue.add(job) : queue.enqueue(job)
			}
			return timed(callers, (_, n) => add(n))
		}
	}
	throw new Error(`no case ${name}`)
}

const measured = await run()
await store.close()
const sorted = measured.latencies.sort((a, b) => a - b)
const at = (quantile: number) =>
	Math.round((sorted[Math.round((sorted.length - 1) * quantile)] ?? 0) * 1e6)
console.log(
	JSON.stringify({
		engine: `bun-${flavor}-${mode}`,
		case: name,
		callers,
		ops: measured.ops,
		elapsed_ns: Math.round(measured.elapsed * 1e6),
		per_second: measured.ops / (measured.elapsed / 1000),
		p50_ns: at(0.5),
		p99_ns: at(0.99),
	}),
)
process.exit(0)
