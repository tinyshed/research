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
function pickExactly(caller: number, call: number): bigint {
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

// A 64-bit number in two halves, where a product is left: a bigint a call
// cost the program more than the SDK's own work on the call.
let high = 0
let low = 0

/** Leaves the low 64 bits of a product of two 64-bit numbers in the halves. */
function multiply(aHigh: number, aLow: number, bHigh: number, bLow: number): void {
	const a0 = aLow & 0xffff
	const a1 = aLow >>> 16
	const b0 = bLow & 0xffff
	const b1 = bLow >>> 16
	const middle = a0 * b1 + a1 * b0
	const bottom = a0 * b0 + (middle % 65536) * 65536
	low = bottom >>> 0
	const carried = a1 * b1 + Math.floor(middle / 65536) + Math.floor(bottom / 4294967296)
	high = (carried + Math.imul(aHigh, bLow) + Math.imul(aLow, bHigh)) >>> 0
}

/** Folds the halves' upper bits down by a shift under 32. */
function fold(shift: number): void {
	low = (low ^ ((low >>> shift) | (high << (32 - shift)))) >>> 0
	high = (high ^ (high >>> shift)) >>> 0
}

/** The call'th choice of a caller among `of` items: pickExactly's, with no bigint. */
function pick(caller: number, call: number, of: number): number {
	multiply(0, caller + 1, 0x9e3779b9, 0x7f4a7c15)
	const [firstHigh, firstLow] = [high, low]
	multiply(Math.floor(call / 4294967296), call >>> 0, 0xbf58476d, 0x1ce4e5b9)
	high = (high ^ firstHigh) >>> 0
	low = (low ^ firstLow) >>> 0
	fold(30)
	multiply(high, low, 0xbf58476d, 0x1ce4e5b9)
	fold(27)
	multiply(high, low, 0x94d049bb, 0x133111eb)
	fold(31)
	return ((high % of) * (4294967296 % of) + low) % of
}

for (const [caller, call] of [[0, 0], [1, 1], [63, 123_456_789], [255, 5_000_000_000], [7, 4_294_967_295]]) {
	const exactly = Number(pickExactly(caller as number, call as number) % BigInt(KEYS))
	if (pick(caller as number, call as number, KEYS) !== exactly) {
		throw new Error(`the choice of caller ${caller}, call ${call} is not the Rust and Go programs'`)
	}
}

const key = (n: number) => `k${String(n).padStart(8, '0')}`
const value = 'v'.repeat(128)

// every key spelled before the clock starts: spelling one a call is the program's work, not the store's
const keys = Array.from({ length: KEYS }, (_, n) => key(n))

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

/**
 * Calls call from callers loops for the seconds asked. A call is timed unless
 * the program makes them faster than one in 20 µs: then one in eight is, since
 * a reading of the clock costs a tenth of such a call on this round's host and
 * the calls between are no different.
 */
async function timed(
	loops: number,
	call: (caller: number, n: number) => Promise<unknown>,
): Promise<{ ops: number; elapsed: number; latencies: number[] }> {
	const latencies: number[] = []
	const began = performance.now()
	const until = began + seconds * 1000
	let ops = 0
	const loop = async (caller: number) => {
		let made = 0
		for (let unread = 0; ; ) {
			for (; unread > 0; unread--) {
				await call(caller, made++)
				ops++
			}
			const at = performance.now()
			if (at >= until) {
				return
			}
			await call(caller, made++)
			latencies.push(performance.now() - at)
			ops++
			unread = (at - began) / ops < 0.02 ? 7 : 0
		}
	}
	await Promise.all(Array.from({ length: loops }, (_, caller) => loop(caller)))
	return { ops, elapsed: performance.now() - began, latencies }
}

async function database() {
	const migrations = { '0001_notes.sql': SCHEMA }
	return flavor === 'rust' ? store.database('app', { migrations }) : store.sql('app', { migrations })
}

/** What the memory cases measured, beside their timings. */
let memory: Record<string, number> | undefined

async function run() {
	switch (name) {
		case 'nothing':
			// The harness alone: its loops, its clocks and its choice of a key,
			// with no call to the store. What it makes a second is the most any
			// case here can show.
			return timed(callers, async (caller, n) => {
				if (keys[pick(caller, n, KEYS)]?.length !== 9) {
					throw new Error('a key of another length')
				}
				await undefined
			})
		case 'kv-get':
		case 'kv-set': {
			const bucket = flavor === 'rust' ? store.bucket('sessions') : store.kv.bucket('sessions')
			if (name === 'kv-set') {
				return timed(callers, (caller, n) =>
					bucket.set(`s${String(caller).padStart(2, '0')}-${String(n).padStart(10, '0')}`, value),
				)
			}
			await fill(KEYS, n => bucket.set(keys[n], value))
			return timed(callers, async (caller, n) => {
				const found = await bucket.get(keys[pick(caller, n, KEYS)])
				if (found?.length !== 128) {
					throw new Error('a key the fill wrote is missing')
				}
			})
		}
		case 'sql-point': {
			const db = await database()
			await db.exec(FILL, NOTES)
			return timed(callers, async (caller, n) => {
				const id = pick(caller, n, NOTES) + 1
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
		case 'sql-rows-held': {
			// What a result of 25,000 rows holds and what a read leaves behind,
			// the collector run in between: the process's peak under a loop of
			// reads says how fast it reads, not what a read costs.
			const db = await database()
			await db.exec(FILL, ROWS)
			const [held, after, resident, latencies]: number[][] = [[], [], [], []]
			const began = performance.now()
			for (let n = 0; n < 20; n++) {
				const at = performance.now()
				let rows: unknown[] | undefined = await db.all(PAGE)
				latencies.push(performance.now() - at)
				Bun.gc(true)
				held.push(process.memoryUsage().heapUsed + (rows?.length === ROWS ? 0 : Number.NaN))
				rows = undefined
				Bun.gc(true)
				after.push(process.memoryUsage().heapUsed)
				resident.push(process.memoryUsage().rss)
			}
			const middle = (values: number[]) => [...values].sort((a, b) => a - b)[values.length >> 1] ?? 0
			const mib = (bytes: number) => Math.round((bytes / 2 ** 20) * 10) / 10
			memory = {
				result_mib: mib(middle(held) - middle(after)),
				heap_after_mib: mib(middle(after)),
				resident_after_mib: mib(middle(resident)),
			}
			return { ops: latencies.length, elapsed: performance.now() - began, latencies }
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
		...(memory === undefined ? {} : { memory }),
	}),
)
process.exit(0)
