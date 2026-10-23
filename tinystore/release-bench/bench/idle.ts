// What an application holds at rest: Bun alone; TinyStore with every engine a
// small application uses opened and written once; the services it runs
// otherwise, each client connected and asked once; and the alternatives.
import { arg, mark, number, result, sleep } from './lib'
import { postgres, redis, s3, tinystore } from './stores'

const rest = number('rest', 60) * 1000

mark('before')
await sleep(5000)
mark('open')
const began = performance.now()
const close = await opened(arg('contender'))
result({ openMs: performance.now() - began })
mark('idle')
await sleep(rest)
mark('closing')
await close()

async function opened(contender: string): Promise<() => Promise<unknown>> {
	switch (contender) {
		case 'bun':
			return async () => {}
		case 'tinystore': {
			const store = await tinystore()
			const db = await store.database('app', {
				migrations: { '0001_notes.sql': 'create table notes (id integer primary key, body text not null)' },
			})
			await db.exec`insert into notes (id, body) values (${1}, ${'first'})`
			await store.bucket<{ user: number }>('sessions').set('a', { user: 1 })
			const queue = store.queue<{ note: number }>('index')
			const worker = queue.work(async () => {}, { concurrency: 32 })
			await queue.add({ note: 1 })
			await store.files('attachments').put('1/a.bin', new Uint8Array(16384))
			store.logger('api', { console: 'off' }).info('started', { port: 3000 })
			store.metrics.counter('requests_total').inc()
			return async () => {
				await worker.stop()
				await store.close()
			}
		}
		case 'services': {
			const sql = postgres()
			await sql`create table notes (id integer primary key, body text not null)`
			await sql`insert into notes (id, body) values (${1}, ${'first'})`
			const kv = await redis('redis')
			await kv.set('session:a', JSON.stringify({ user: 1 }))
			await s3().write('attachments/1/a.bin', new Uint8Array(16384))
			const loki = await fetch(`${arg('loki')}/loki/api/v1/push`, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({
					streams: [{ stream: { app: 'api' }, values: [[`${Date.now()}000000`, '{"msg":"started","port":3000}']] }],
				}),
			})
			if (!loki.ok) throw new Error(`loki: ${loki.status} ${await loki.text()}`)
			const victoria = await fetch(`${arg('victoria')}/api/v1/import/prometheus`, {
				method: 'POST',
				body: `requests_total 1 ${Date.now()}\n`,
			})
			if (!victoria.ok) throw new Error(`victoria-metrics: ${victoria.status}`)
			return async () => {
				kv.close()
				await sql.close()
			}
		}
		case 'valkey': {
			const kv = await redis('valkey')
			await kv.set('session:a', JSON.stringify({ user: 1 }))
			return async () => kv.close()
		}
		case 'prometheus': {
			const answer = await fetch(`${arg('prometheus')}/api/v1/query?query=up`)
			if (!answer.ok) throw new Error(`prometheus: ${answer.status}`)
			return async () => {}
		}
		default:
			throw new Error(`no contender ${contender}`)
	}
}
