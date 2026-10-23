// What every round's program shares: its arguments, the marks run.py samples
// memory between, a load of callers in flight, and a latency histogram.

export const args = parse(Bun.argv.slice(2))

function parse(list: string[]): Record<string, string> {
	const out: Record<string, string> = {}
	for (let at = 0; at < list.length; at += 2) {
		const name = list[at]
		if (name === undefined || !name.startsWith('--')) throw new Error(`argument ${name} is no --name`)
		out[name.slice(2)] = list[at + 1] ?? ''
	}
	return out
}

export function arg(name: string, otherwise?: string): string {
	const value = args[name] ?? otherwise
	if (value === undefined) throw new Error(`--${name} is needed`)
	return value
}

export const number = (name: string, otherwise?: number) => Number(arg(name, otherwise?.toString()))

/** A line run.py reads: a phase starts, so that memory is told apart by it. */
export function mark(phase: string) {
	process.stdout.write(`${JSON.stringify({ mark: phase, at: Date.now() })}\n`)
}

export function result(value: Record<string, unknown>) {
	process.stdout.write(`${JSON.stringify({ result: value })}\n`)
}

export const sleep = (ms: number) => new Promise(done => setTimeout(done, ms))

/**
 * Latencies in buckets 2 % apart from a microsecond up, so that a load of
 * millions of calls holds a few kilobytes rather than every one.
 */
export class Histogram {
	private readonly counts = new Float64Array(1200)
	total = 0

	record(micros: number) {
		const at = micros <= 1 ? 0 : Math.min(this.counts.length - 1, Math.ceil(Math.log(micros) / Math.log(1.02)))
		this.counts[at] = (this.counts[at] ?? 0) + 1
		this.total += 1
	}

	merge(other: Histogram) {
		for (let at = 0; at < this.counts.length; at++) this.counts[at] = (this.counts[at] ?? 0) + (other.counts[at] ?? 0)
		this.total += other.total
	}

	/** the bucket's upper bound at the rank, in microseconds */
	at(rank: number): number {
		let seen = 0
		const want = rank * this.total
		for (let at = 0; at < this.counts.length; at++) {
			seen += this.counts[at] ?? 0
			if (seen >= want) return 1.02 ** at
		}
		return Number.NaN
	}
}

export interface Stage {
	stage: string
	inFlight: number
	seconds: number
	calls: number
	perSecond: number
	p50us: number
	p99us: number
	errors: number
	firstError?: string
}

/**
 * Calls fn from inFlight callers at once for warm seconds, then for seconds
 * counted, each caller awaiting its call before the next: what a server sees
 * from that many requests in flight. A call that throws counts as an error
 * and the load goes on; the first error's text is kept.
 *
 * Each caller reads the clock itself rather than waiting for a timer: a call
 * answered within its microtask, as an embedded store answers a read, never
 * lets the event loop reach its timers, and a timer would never end the load.
 */
export async function load(
	stage: string,
	inFlight: number,
	seconds: number,
	fn: (caller: number, call: number) => Promise<unknown>,
	warm = Math.min(2, seconds / 4),
): Promise<Stage> {
	const histogram = new Histogram()
	let calls = 0
	let errors = 0
	let firstError: string | undefined
	mark(`warm:${stage}:${inFlight}`)
	const begun = performance.now()
	const counted = begun + warm * 1000
	const ended = counted + seconds * 1000
	let marked = false
	let lastEnd = counted
	const caller = async (index: number) => {
		let call = 0
		for (;;) {
			const before = performance.now()
			if (before >= ended) return
			const counting = before >= counted
			if (counting && !marked) {
				marked = true
				mark(`load:${stage}:${inFlight}`)
			}
			try {
				await fn(index, call++)
			} catch (error) {
				if (counting) {
					errors += 1
					firstError ??= error instanceof Error ? error.message : String(error)
				}
				continue
			}
			if (counting) {
				const after = performance.now()
				histogram.record((after - before) * 1000)
				calls += 1
				if (after > lastEnd) lastEnd = after
			}
		}
	}
	await Promise.all(Array.from({ length: inFlight }, (_, index) => caller(index)))
	const elapsed = (lastEnd - counted) / 1000
	mark(`after:${stage}:${inFlight}`)
	const out: Stage = {
		stage,
		inFlight,
		seconds: elapsed,
		calls,
		perSecond: calls / elapsed,
		p50us: histogram.at(0.5),
		p99us: histogram.at(0.99),
		errors,
	}
	if (firstError !== undefined) out.firstError = firstError
	result(out as unknown as Record<string, unknown>)
	return out
}

/** xorshift32, so that every contender is asked for the same keys in the same order */
export function random(seed: number) {
	let state = seed >>> 0 || 1
	return () => {
		state ^= state << 13
		state >>>= 0
		state ^= state >>> 17
		state ^= state << 5
		state >>>= 0
		return state / 4294967296
	}
}

const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_'

/**
 * A value of this many bytes, different for each n: letters and digits as a
 * token or an id spells them, which no store's compression makes much of, so
 * that a side that compresses gains no more than real values would give it.
 */
export function text(bytes: number, n: number): string {
	const next = random(n * 2654435761 + 1)
	let out = ''
	for (let at = 0; at < bytes; at++) out += ALPHABET[Math.floor(next() * 64)]
	return out
}

export function bytesOf(size: number, n: number): Uint8Array {
	const out = new Uint8Array(size)
	const next = random(n + 1)
	for (let at = 0; at < size; at += 4) {
		const word = (next() * 4294967296) >>> 0
		out[at] = word & 255
		out[at + 1] = (word >>> 8) & 255
		out[at + 2] = (word >>> 16) & 255
		out[at + 3] = word >>> 24
	}
	return out
}

/** Runs fn n times from inFlight callers and resolves once every one has. */
export async function each(n: number, inFlight: number, fn: (index: number) => Promise<unknown>) {
	let next = 0
	await Promise.all(
		Array.from({ length: inFlight }, async () => {
			while (next < n) await fn(next++)
		}),
	)
}

/** median of the times fn takes, run one after another */
export async function timed(name: string, repeat: number, fn: () => Promise<unknown>) {
	const times: number[] = []
	let size: unknown
	for (let at = 0; at < repeat; at++) {
		const before = performance.now()
		size = await fn()
		times.push(performance.now() - before)
	}
	times.sort((a, b) => a - b)
	result({ query: name, repeat, medianMs: times[Math.floor(times.length / 2)], minMs: times[0], maxMs: times.at(-1), size })
}
