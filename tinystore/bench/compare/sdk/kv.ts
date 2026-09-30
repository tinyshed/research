// The kv round from Bun: TinyStore through its SDK's sidecar, or Redis
// through Bun's own client on its Unix socket, with the Go round's load.
//
//   bun kv.ts <tinystore|tinystore-server|redis|redis-tcp> <dir> <seconds>
//
// It prints one run as the Go harness writes it. TINYSTORE_BIN is the binary
// the SDK starts its sidecar with; TINYSTORE_URL and TINYSTORE_TOKEN the
// remote server sdk.sh started; REDIS_URL the Redis it started, and
// SERVICE_PID the server's process for its memory.

import { RedisClient } from 'bun'
import { readFileSync } from 'node:fs'
import { connect, open } from '../../../source/sdk/js/src/index.ts'

const keys = 100_000
const valueBytes = 128
const [contender, dir, secondsText] = process.argv.slice(2)
const seconds = Number(secondsText)

interface Kv {
	get(key: string): Promise<Uint8Array | undefined>
	set(key: string, value: Uint8Array): Promise<void>
	close(): Promise<void>
	servicePid(): number
}

async function openTinyStore(): Promise<Kv> {
	const store = await open(dir, { idle: 0 })
	const bucket = store.kv.bucket('sessions', 'bytes')
	return {
		get: (key) => bucket.get(key),
		set: (key, value) => bucket.set(key, value),
		close: () => store.close(),
		servicePid: () => JSON.parse(readFileSync(`${dir}/server/SERVE`, 'utf8')).pid,
	}
}

async function openTinyStoreServer(): Promise<Kv> {
	const store = await connect(process.env.TINYSTORE_URL ?? '', { token: process.env.TINYSTORE_TOKEN ?? '' })
	const bucket = store.kv.bucket('sessions', 'bytes')
	return {
		get: (key) => bucket.get(key),
		set: (key, value) => bucket.set(key, value),
		close: () => store.close(),
		servicePid: () => Number(process.env.SERVICE_PID),
	}
}

async function openRedis(): Promise<Kv> {
	const client = new RedisClient(process.env.REDIS_URL)
	await client.connect()
	return {
		get: async (key) => {
			const value = await client.getBuffer(key)
			return value === null ? undefined : value
		},
		set: async (key, value) => {
			await client.set(key, value)
		},
		close: async () => client.close(),
		servicePid: () => Number(process.env.SERVICE_PID),
	}
}

function value(i: number, version: number): Uint8Array {
	const out = new Uint8Array(valueBytes)
	let x = BigInt(i) * 0x9e3779b97f4a7c15n + BigInt(version)
	for (let j = 0; j < valueBytes; j++) {
		x = (x * 6364136223846793005n + 1442695040888963407n) & 0xffffffffffffffffn
		out[j] = Number(x >> 56n)
	}
	return out
}

// values made once, so that making one costs no set its time
const values = Array.from({ length: 1024 }, (_, i) => value(i, 0))
const key = (i: number) => `session:${i}`
// Math.random, not a small LCG: an LCG's low bit alternates, so a mixed call
// drawing twice would always decide on the same parity and write a fifth of
// the time or never
const pick = () => Math.floor(Math.random() * keys)

interface Stage {
	name: string
	goroutines: number
	ops: number
	errors: number
	seconds: number
	per_second: number
	p50_us: number
	p99_us: number
	first_error?: string
	cpu_seconds: number
}

async function timeStage(name: string, workers: number, op: () => Promise<void>): Promise<Stage> {
	const warm = Date.now() + 1000
	await Promise.all(
		Array.from({ length: workers }, async () => {
			while (Date.now() < warm) await op()
		}),
	)
	const latencies: number[] = []
	let errors = 0
	let firstError: string | undefined
	let spent = cpu('self') + cpu(service)
	const began = performance.now()
	const end = began + seconds * 1000
	await Promise.all(
		Array.from({ length: workers }, async () => {
			for (;;) {
				const start = performance.now()
				if (start >= end) return
				try {
					await op()
				} catch (err) {
					errors++
					firstError ??= String(err)
				}
				latencies.push((performance.now() - start) * 1000)
			}
		}),
	)
	const elapsed = (performance.now() - began) / 1000
	spent = cpu('self') + cpu(service) - spent
	latencies.sort((a, b) => a - b)
	const at = (q: number) => latencies[Math.min(latencies.length - 1, Math.floor(q * latencies.length))] ?? 0
	return {
		name,
		goroutines: workers,
		ops: latencies.length,
		errors,
		seconds: elapsed,
		per_second: latencies.length / elapsed,
		p50_us: at(0.5),
		p99_us: at(0.99),
		first_error: firstError,
		cpu_seconds: spent,
	}
}

/** utime and stime of a process, in seconds, at the kernel's 100 ticks a second */
function cpu(pid: number | 'self'): number {
	try {
		const fields = readFileSync(`/proc/${pid}/stat`, 'utf8').split(')').pop()?.trim().split(' ') ?? []
		return (Number(fields[11]) + Number(fields[12])) / 100
	} catch {
		return 0
	}
}

let service = 0

function peak(pid: number | 'self'): number {
	try {
		const status = readFileSync(`/proc/${pid}/status`, 'utf8')
		const kb = /VmHWM:\s+(\d+) kB/.exec(status)
		return kb ? Number(kb[1]) * 1024 : 0
	} catch {
		return 0
	}
}

const opened = performance.now()
const openers: Record<string, () => Promise<Kv>> = {
	tinystore: openTinyStore,
	'tinystore-server': openTinyStoreServer,
	redis: openRedis,
	'redis-tcp': openRedis,
}
const kv = await (openers[contender] ?? (() => Promise.reject(new Error(`no contender ${contender}`))))()
const openSeconds = (performance.now() - opened) / 1000
service = kv.servicePid()

// filled from 64 writers, as the Go round fills
await Promise.all(
	Array.from({ length: 64 }, async (_, w) => {
		for (let i = w; i < keys; i += 64) await kv.set(key(i), values[i % 1024] as Uint8Array)
	}),
)

let version = 1
const get = async () => {
	const i = pick()
	const got = await kv.get(key(i))
	if (got === undefined || got.length !== valueBytes) throw new Error(`${key(i)}: ${got?.length} bytes`)
}
const set = async () => {
	const i = pick()
	await kv.set(key(i), values[(i + version++) % 1024] as Uint8Array)
}
const mixed = () => (Math.random() < 0.1 ? set() : get())

const stages: Stage[] = []
for (const [name, op] of [
	['get', get],
	['set', set],
	['mixed', mixed],
] as const) {
	for (const workers of [1, 8, 64]) stages.push(await timeStage(name, workers, op))
}
const servicePeak = peak(service)
await kv.close()
process.stdout.write(
	`${JSON.stringify({
		contender: `bun-${contender}`,
		stages,
		open_rss_bytes: 0,
		peak_rss_bytes: peak('self'),
		service_peak_rss_bytes: servicePeak,
		disk_bytes: 0,
		open_seconds: openSeconds,
		processes: 2,
	})}\n`,
)
