// What a million 128-byte values cost in memory: written from 64 calls in
// flight, then at rest, then read at random from 64. Each side as it ships:
// TinyStore 'full', Redis and Valkey without an append-only file.
import { arg, each, load, mark, number, random, sleep, text } from './lib'
import { redis, tinystore } from './stores'

const KEYS = 1_000_000
const VALUE = 128
const seconds = number('seconds')

const contender = arg('contender')
let get: (key: string) => Promise<unknown>
let set: (key: string, value: string) => Promise<unknown>
let close: () => Promise<unknown>
if (contender === 'tinystore') {
	const store = await tinystore()
	const bucket = store.bucket<string>('bench')
	get = key => bucket.get(key)
	set = (key, value) => bucket.set(key, value)
	close = () => store.close()
} else {
	const client = await redis(contender === 'valkey' ? 'valkey' : 'redis')
	get = key => client.get(key)
	set = (key, value) => client.set(key, value)
	close = async () => client.close()
}

mark('fill')
await each(KEYS, 64, async index => set(`k${index}`, text(VALUE, index)))
mark('rest')
await sleep(20_000)
const pick = Array.from({ length: 64 }, (_, caller) => random(caller + 1))
await load('get', 64, seconds, async caller => {
	if ((await get(`k${Math.floor((pick[caller] ?? Math.random)() * KEYS)}`)) == null) throw new Error('missing key')
})
mark('closing')
await close()
