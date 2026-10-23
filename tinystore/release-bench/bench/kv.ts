// Point reads and writes of 128-byte values over 100,000 keys, from 1, 8, 64
// and 256 calls in flight: TinyStore's bucket against Redis and Valkey, each
// through Bun. Writes run twice, once a write returns synced to the disk, once
// it returns written to the operating system (see run.py); reads run once.
import { arg, each, load, number, random, text } from './lib'
import { redis, tinystore } from './stores'

const KEYS = 100_000
const VALUE = 128
const seconds = number('seconds')
const levels = [1, 8, 64, 256]
const durability = arg('durability')

interface Kv {
	get(key: string): Promise<string | null | undefined>
	set(key: string, value: string): Promise<unknown>
	close(): Promise<unknown>
}

const kv = await opened(arg('contender'))
await each(KEYS, 64, async index => kv.set(`k${index}`, text(VALUE, index)))
const pick = Array.from({ length: 256 }, (_, caller) => random(caller + 1))
const key = (caller: number) => `k${Math.floor((pick[caller] ?? Math.random)() * KEYS)}`

if (durability === 'synced') {
	for (const inFlight of levels) {
		await load('get', inFlight, seconds, async caller => {
			if ((await kv.get(key(caller))) == null) throw new Error('a seeded key read as none')
		})
	}
}
for (const inFlight of levels) {
	await load('set', inFlight, seconds, async (caller, call) => kv.set(key(caller), text(VALUE, call)))
}
await kv.close()

async function opened(contender: string): Promise<Kv> {
	if (contender === 'tinystore') {
		const store = await tinystore()
		const bucket = store.bucket<string>('bench')
		return { get: key => bucket.get(key), set: (key, value) => bucket.set(key, value), close: () => store.close() }
	}
	const client = await redis(contender === 'valkey' ? 'valkey' : 'redis')
	return {
		get: key => client.get(key),
		set: (key, value) => client.set(key, value),
		close: async () => client.close(),
	}
}
