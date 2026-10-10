// The most a Bun program can make of the core in its process: kv gets over
// the C ABI with no SDK between, frames written by hand into one buffer a
// turn and answers read where they arrive. What the SDK makes of the same
// load is measured against this, not against a Rust program's threads.
//
// bun raw.ts --library <libtinystore_ffi> --dir <dir> --case kv-get --callers <n> --seconds <s>
//   [--decode none|text|json]

import { dlopen, FFIType, JSCallback, type Pointer, ptr, toArrayBuffer } from 'bun:ffi'

const args = process.argv.slice(2)
const arg = (name: string, otherwise?: string): string => {
	const at = args.indexOf(name)
	const value = at < 0 ? otherwise : args[at + 1]
	if (value === undefined) {
		throw new Error(`missing ${name}`)
	}
	return value
}
const callers = Number(arg('--callers'))
const seconds = Number(arg('--seconds'))
const decoding = arg('--decode', 'json')
if (arg('--case', 'kv-get') !== 'kv-get') {
	throw new Error('the bare loop reads keys and nothing else')
}
const KEYS = 100_000

const lib = dlopen(arg('--library'), {
	tinystore_open: {
		args: [FFIType.ptr, FFIType.u64, FFIType.function, FFIType.ptr, FFIType.ptr, FFIType.ptr],
		returns: FFIType.u64_fast,
	},
	tinystore_send: { args: [FFIType.ptr, FFIType.ptr, FFIType.u64, FFIType.ptr], returns: FFIType.u64_fast },
	tinystore_recv: { args: [FFIType.ptr, FFIType.u32, FFIType.ptr], returns: FFIType.u64_fast },
	tinystore_close: { args: [FFIType.ptr], returns: FFIType.void },
	tinystore_free: { args: [FFIType.ptr, FFIType.u64], returns: FFIType.void },
})

const WELCOME = 2
const REQUEST = 3
const RESPONSE = 4
const GOAWAY = 10
const END = 1
const ERROR = 2
const KV_BUCKET_OPEN = 0x0101
const KV_GET = 0x0102
const KV_SET = 0x0104

// what leaves in this turn's one write
let out = new Uint8Array(1 << 16)
let outLength = 0
let flushing = false
const outAt = ptr(out)
const handed = new BigUint64Array(1)
const handedAt = ptr(handed)
let connection: Pointer | null = null

// a call's answer by its stream: the stream numbers go round a table
const SLOTS = 4096
const waiting: (((body: Uint8Array, at: number, stop: number) => void) | undefined)[] = new Array(SLOTS)
let nextStream = 0

function header(kind: number, flags: number, method: number, stream: number, length: number): void {
	const at = outLength
	out[at] = length
	out[at + 1] = length >>> 8
	out[at + 2] = length >>> 16
	out[at + 3] = length >>> 24
	out[at + 4] = kind
	out[at + 5] = flags
	out[at + 6] = method
	out[at + 7] = method >>> 8
	out[at + 8] = stream
	out[at + 9] = stream >>> 8
	out[at + 10] = stream >>> 16
	out[at + 11] = stream >>> 24
}

function flush(): void {
	flushing = false
	const length = outLength
	outLength = 0
	if (connection !== null && length > 0) {
		take(lib.symbols.tinystore_send(connection, outAt, length, handedAt))
	}
}

function leave(): void {
	if (!flushing) {
		flushing = true
		queueMicrotask(flush)
	}
}

/** Reads the frames the core handed over, each answer given to its call. */
function take(length: number | bigint): void {
	const n = Number(length)
	if (n === 0) {
		return
	}
	const address = Number(handed[0]) as Pointer
	const bytes = new Uint8Array(toArrayBuffer(address, 0, n))
	let at = 0
	while (at < n) {
		const size = bytes[at]! | (bytes[at + 1]! << 8) | (bytes[at + 2]! << 16) | (bytes[at + 3]! << 24)
		const stream = (bytes[at + 8]! | (bytes[at + 9]! << 8) | (bytes[at + 10]! << 16) | (bytes[at + 11]! << 24)) >>> 0
		const kind = bytes[at + 4]!
		if (kind === GOAWAY || (bytes[at + 5]! & ERROR) !== 0) {
			console.error(`the core refused: ${new TextDecoder().decode(bytes.subarray(at + 12, at + 12 + size))}`)
			process.exit(1)
		}
		// a CREDIT comes on no stream, and is nobody's answer
		if (kind === WELCOME || kind === RESPONSE) {
			const answer = waiting[stream % SLOTS]
			waiting[stream % SLOTS] = undefined
			answer?.(bytes, at + 12, at + 12 + size)
		}
		at += 12 + size
	}
	lib.symbols.tinystore_free(address, n)
}

const wake = new JSCallback(
	() => {
		if (connection !== null) {
			take(lib.symbols.tinystore_recv(connection, 0, handedAt))
		}
	},
	{ args: [FFIType.ptr], returns: FFIType.void, threadsafe: true },
)

const dir = new TextEncoder().encode(arg('--dir'))
const opened = new BigUint64Array(1)
if (Number(lib.symbols.tinystore_open(ptr(dir), dir.length, wake, null, ptr(opened), handedAt)) > 0) {
	throw new Error('the store did not open')
}
connection = Number(opened[0]) as Pointer

/** Sends one frame now and returns the first frame that answers it, copied. */
function exchange(kind: number, flags: number, method: number, stream: number, body: number[]): Promise<Uint8Array> {
	return new Promise(resolve => {
		header(kind, flags, method, stream, body.length)
		out.set(body, outLength + 12)
		outLength += 12 + body.length
		waiting[stream % SLOTS] = (bytes, at, stop) => resolve(bytes.slice(at, stop))
		leave()
	})
}

const ascii = (text: string) => [...text].map(c => c.charCodeAt(0))
// HELLO { 1 protocol: 2, 2 client: "raw/0" }, which WELCOME answers on no stream
await exchange(1, 0, 0, 0, [0x82, 0x01, 0x02, 0x02, 0xa5, ...ascii('raw/0')])
// kv.bucket.open { 1 name: "sessions" } -> Handle { 1 handle }
const handle = (await exchange(REQUEST, END, KV_BUCKET_OPEN, ++nextStream, [0x81, 0x01, 0xa8, ...ascii('sessions')]))[2]!

const value = JSON.stringify('v'.repeat(128))
const text = new TextDecoder()

/** Writes kv.Call { 1 handle, 2 under: [], 3 key: "k%08d" } and what follows it, and returns its stream. */
function call(method: number, key: number, valued: boolean): number {
	// never a number whose place a call still waits in: an answer may come
	// after thousands of later ones
	do {
		nextStream = nextStream === 0x7fff_ffff ? 1 : nextStream + 1
	} while (waiting[nextStream % SLOTS] !== undefined)
	const stream = nextStream
	const length = 7 + 9 + (valued ? 3 + value.length : 0)
	if (outLength + 12 + length > out.length) {
		flush()
	}
	header(REQUEST, END, method, stream, length)
	let at = outLength + 12
	out[at++] = valued ? 0x84 : 0x83
	out[at++] = 0x01
	out[at++] = handle
	out[at++] = 0x02
	out[at++] = 0x90
	out[at++] = 0x03
	out[at++] = 0xa9
	out[at++] = 0x6b
	for (let place = 10_000_000; place >= 1; place = Math.floor(place / 10)) {
		out[at++] = 0x30 + (Math.floor(key / place) % 10)
	}
	if (valued) {
		out[at++] = 0x04
		out[at++] = 0xc4
		out[at++] = value.length
		for (let i = 0; i < value.length; i++) {
			out[at++] = value.charCodeAt(i)
		}
	}
	outLength = at
	leave()
	return stream
}

function set(key: number): Promise<void> {
	return new Promise(resolve => {
		waiting[call(KV_SET, key, true) % SLOTS] = () => resolve()
	})
}

/** kv.Entry { 1 found: true, 2 value: bin, … }: the value as the bucket's type has it. */
function get(key: number): Promise<unknown> {
	return new Promise(resolve => {
		waiting[call(KV_GET, key, false) % SLOTS] = (bytes, at) => {
			if (bytes[at + 2] !== 0xc3 || bytes[at + 4] !== 0xc4) {
				throw new Error('a key the fill wrote is missing')
			}
			const length = bytes[at + 5]!
			if (decoding === 'none') {
				return resolve(length)
			}
			const spelled = text.decode(bytes.subarray(at + 6, at + 6 + length))
			resolve(decoding === 'text' ? spelled : JSON.parse(spelled))
		}
	})
}

let filled = 0
await Promise.all(
	Array.from({ length: 64 }, async () => {
		for (let n = filled++; n < KEYS; n = filled++) {
			await set(n)
		}
	}),
)

// the keys a caller asks for: a cheap generator, since the choice is not what is measured
let seed = 0x9e3779b9
const pick = () => {
	seed ^= seed << 13
	seed ^= seed >>> 17
	seed ^= seed << 5
	return (seed >>> 0) % KEYS
}

let made = 0
const began = performance.now()
const until = began + seconds * 1000
await Promise.all(
	Array.from({ length: callers }, async () => {
		while (performance.now() < until) {
			await get(pick())
			made++
		}
	}),
)
const elapsed = performance.now() - began
console.log(
	JSON.stringify({
		engine: 'bun-raw-embedded',
		case: 'kv-get',
		decode: decoding,
		callers,
		ops: made,
		elapsed_ns: Math.round(elapsed * 1e6),
		per_second: made / (elapsed / 1000),
	}),
)
lib.symbols.tinystore_close(connection)
process.exit(0)
