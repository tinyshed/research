// Files by path: 16 KiB and 1 MiB, read whole and replaced whole, from 1, 16
// and 64 calls in flight (1 and 16 for 1 MiB). TinyStore's files against MinIO
// through Bun's own S3 client. Both return a write once it is on disk as they
// ship.
import { arg, bytesOf, each, load, number, random } from './lib'
import { s3, tinystore } from './stores'

const seconds = number('seconds')
const SMALL = { size: 16 << 10, count: 1000, levels: [1, 16, 64], name: '16k' }
const LARGE = { size: 1 << 20, count: 100, levels: [1, 16], name: '1m' }

interface Store {
	put(path: string, bytes: Uint8Array): Promise<unknown>
	get(path: string): Promise<Uint8Array | undefined>
	close(): Promise<unknown>
}

const files = await opened(arg('contender'))
for (const kind of [SMALL, LARGE]) {
	const bodies = Array.from({ length: 8 }, (_, n) => bytesOf(kind.size, n))
	await each(kind.count, 16, async index => files.put(`${kind.name}/${index}.bin`, bodies[index % 8] as Uint8Array))
	const pick = Array.from({ length: 64 }, (_, caller) => random(caller + 5))
	const path = (caller: number) => `${kind.name}/${Math.floor((pick[caller] ?? Math.random)() * kind.count)}.bin`
	for (const inFlight of kind.levels) {
		await load(`get-${kind.name}`, inFlight, seconds, async caller => {
			const bytes = await files.get(path(caller))
			if (bytes?.length !== kind.size) throw new Error(`read ${bytes?.length} bytes of ${kind.size}`)
		})
	}
	for (const inFlight of kind.levels) {
		await load(`put-${kind.name}`, inFlight, seconds, async (caller, call) =>
			files.put(path(caller), bodies[call % 8] as Uint8Array),
		)
	}
}
await files.close()

async function opened(contender: string): Promise<Store> {
	if (contender === 'tinystore') {
		const store = await tinystore()
		const folder = store.files('bench')
		return {
			put: (path, bytes) => folder.put(path, bytes),
			get: async path => {
				const file = await folder.get(path)
				return file === undefined ? undefined : file.bytes()
			},
			close: () => store.close(),
		}
	}
	const bucket = s3()
	return {
		put: (path, bytes) => bucket.write(path, bytes),
		get: async path => bucket.file(path).bytes(),
		close: async () => {},
	}
}
