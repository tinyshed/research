// How each contender is opened, so that every round opens it the same way.
import { open, type Store } from '@tinyshed/tinystore'
import { SQL } from 'bun'
import { arg } from './lib'

/**
 * TinyStore as a Bun program opens it: `open(dir)` finds or starts the
 * directory's sidecar, the SDK's default; `{ embedded: true }` loads the core
 * into this process. The round's `synced` is the store's default, 'full';
 * `written` is 'os'.
 */
export async function tinystore(): Promise<Store> {
	// 'full' is the default and is left unsaid: a server started by itself
	// says none, and v0.1.0 refuses a program that asks one of it
	const durability = arg('durability', 'synced') === 'written' ? { durability: 'os' as const } : {}
	const dir = arg('dir')
	return arg('mode') === 'embedded' ? open(dir, { embedded: true, ...durability }) : open(dir, durability)
}

/** Bun's own Redis client, which speaks to Valkey the same, on the service's Unix socket */
export async function redis(name: 'redis' | 'valkey') {
	const client = new Bun.RedisClient(`redis+unix://${arg(name)}`, { enableAutoPipelining: true })
	await client.connect()
	return client
}

/** Bun's own PostgreSQL client on the server's Unix socket, a pool as wide as the round's callers */
export function postgres(max = 64) {
	return new SQL({ path: `${arg('postgres')}/.s.PGSQL.5432`, user: 'bench', database: 'bench', max })
}

/** Bun's own S3 client against MinIO */
export function s3() {
	return new Bun.S3Client({
		endpoint: arg('minio'),
		accessKeyId: 'bench',
		secretAccessKey: 'bench-secret-key',
		bucket: 'bench',
		region: 'us-east-1',
	})
}
