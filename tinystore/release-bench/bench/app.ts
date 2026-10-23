// One small application's requests, served by TinyStore alone and by the
// services it runs otherwise: PostgreSQL, Redis, MinIO, VictoriaMetrics and
// Loki. Both sides do the same work a request:
//
//   every request   read its user's session, read a note, log a line, count itself
//   1 in 10         update the note and queue a job to index it
//   1 in 50         upload a 16 KiB attachment, the note's, replaced each time
//
// A worker in the same process runs the jobs as they come. What a request
// writes is on disk before it returns: the note, the job, the file. The log
// line and the counter are not, on either side, as an application's are not.
//
// Atomicity is not the same and is said: the services write the note and its
// job in one PostgreSQL transaction; TinyStore writes them as two commits, one
// a file, unless `--joined 1` keeps the queue in the application's database
// and writes both in its transaction.
//
// After the load the program waits for the queue to empty and the logs to be
// written, then counts what each side kept: a side that drops lines or falls
// behind on its jobs shows it there, where a rate would hide it.
import { arg, bytesOf, load, mark, number, random, result, sleep, text } from './lib'
import { sql as tag } from '@tinyshed/tinystore'
import { postgres, redis, s3, tinystore } from './stores'

const USERS = 10_000
const NOTES = 10_000
const ATTACHMENT = 16 << 10
const seconds = number('seconds')
const levels = [1, 8, 64, 256]

interface App {
	session(user: number): Promise<unknown>
	note(id: number): Promise<unknown>
	write(id: number, body: string): Promise<void>
	upload(id: number, bytes: Uint8Array): Promise<void>
	log(fields: Record<string, unknown>): void
	count(): void
	/** waits for the queue and the logs, at most a minute, and says what was kept */
	settle(): Promise<Record<string, number>>
	close(): Promise<void>
}

const contender = arg('contender')
const app = contender === 'tinystore' ? await tinyApp() : await servicesApp()
const attachment = bytesOf(ATTACHMENT, 1)
const draws = Array.from({ length: 256 }, (_, caller) => random(caller + 7))
let requests = 0
let writes = 0
let uploads = 0

mark('rest')
await sleep(number('rest', 20) * 1000)
for (const inFlight of levels) {
	await load('request', inFlight, seconds, async (caller, call) => {
		const draw = draws[caller] ?? Math.random
		const user = Math.floor(draw() * USERS)
		const id = Math.floor(draw() * NOTES)
		const writing = draw() < 0.1
		const uploading = draw() < 0.02
		const began = performance.now()
		if ((await app.session(user)) == null) throw new Error(`no session of user ${user}`)
		if ((await app.note(id)) == null) throw new Error(`no note ${id}`)
		if (writing) {
			await app.write(id, text(200, call))
			writes += 1
		}
		if (uploading) {
			await app.upload(id, attachment)
			uploads += 1
		}
		app.log({ user, note: id, write: writing, ms: performance.now() - began })
		app.count()
		requests += 1
	})
}
mark('settle')
const kept = await app.settle()
result({ kept: { requests, writes, uploads, ...kept } })
mark('closing')
await app.close()

async function tinyApp(): Promise<App> {
	const joined = arg('joined', '0') === '1'
	const store = await tinystore()
	const db = await store.database('app', {
		migrations: {
			'0001_notes.sql': 'create table notes (id integer primary key, user_id integer not null, body text not null, version integer not null)',
		},
	})
	const sessions = store.bucket<{ user: number }>('sessions')
	const queue = joined
		? db.queue<{ note: number }>('index', { concurrency: 32 })
		: store.queue<{ note: number }>('index', { concurrency: 32 })
	const files = store.files('attachments')
	// 8192 lines: a flush is one request, and 65,536 of them overflowed its 2 MiB in v0.1.0
	const logger = store.logger('api', { console: 'off', buffer: 8192 })
	const counter = store.metrics.counter('requests_total')
	await seed(
		async (from, to) => {
			const rows = []
			for (let id = from; id < to; id++) rows.push(sqlInsert(id))
			await db.batch(rows)
		},
		async user => sessions.set(`s${user}`, { user }),
	)
	let enqueued = 0
	let handled = 0
	const worker = queue.work(
		async () => {
			handled += 1
		},
		{ concurrency: 32 },
	)
	function sqlInsert(id: number) {
		return tag`insert into notes (id, user_id, body, version) values (${id}, ${id % USERS}, ${text(200, id)}, ${0})`
	}
	return {
		session: user => sessions.get(`s${user}`),
		note: id => db.one`select id, body, version from notes where id = ${id}`,
		write: joined
			? async (id, body) => {
					await db.tx(async tx => {
						await tx.exec`update notes set body = ${body}, version = version + 1 where id = ${id}`
						await tx.with(queue).add({ note: id })
					})
					enqueued += 1
				}
			: async (id, body) => {
					await db.exec`update notes set body = ${body}, version = version + 1 where id = ${id}`
					await queue.add({ note: id })
					enqueued += 1
				},
		upload: async (id, bytes) => {
			await files.put(`${id}.bin`, bytes)
		},
		log: fields => logger.info('request', fields),
		count: () => counter.inc(),
		settle: async () => {
			const until = Date.now() + 60_000
			while (handled < enqueued && Date.now() < until) await sleep(50)
			await logger.flush()
			const stored = await store.events('api').count({ since: '1d' })
			const versions = await db.scalar<number>`select sum(version) from notes`
			return {
				enqueued,
				handled,
				jobsLeft: enqueued - handled,
				logsStored: Number(stored),
				logsDropped: logger.dropped,
				versions: Number(versions),
			}
		},
		close: async () => {
			await worker.stop()
			await store.close()
		},
	}
}

async function servicesApp(): Promise<App> {
	const sql = postgres(64)
	const workerSql = postgres(2)
	const kv = await redis('redis')
	const bucket = s3()
	await sql`create table notes (id integer primary key, user_id integer not null, body text not null, version integer not null)`
	await sql`create table jobs (id bigserial primary key, note integer not null, locked_until timestamptz)`
	await seed(
		async (from, to) => {
			const rows = []
			for (let id = from; id < to; id++) rows.push({ id, user_id: id % USERS, body: text(200, id), version: 0 })
			await sql`insert into notes ${sql(rows)}`
		},
		async user => kv.set(`s${user}`, JSON.stringify({ user })),
	)

	let enqueued = 0
	let handled = 0
	let working = true
	// the queue as pg-boss and graphile-worker keep one: claimed with a lease
	// under SKIP LOCKED, a batch at a time, removed once handled
	const worker = (async () => {
		while (working) {
			const claimed = await workerSql`
				update jobs set locked_until = now() + interval '30 seconds'
				where id in (select id from jobs where locked_until is null or locked_until < now()
				             order by id limit 256 for update skip locked)
				returning id, note`
			if (claimed.length === 0) {
				await sleep(20)
				continue
			}
			handled += claimed.length
			const ids = claimed.map((row: { id: string }) => row.id)
			await workerSql`delete from jobs where id in ${workerSql(ids)}`
		}
	})()

	// a logger's lines go to Loki as its clients send them: a batch a second
	let lines: [string, string][] = []
	let sent = 0
	let stored = 0
	let dropped = 0
	let pushing: Promise<void> = Promise.resolve()
	const push = () => {
		if (lines.length === 0) return pushing
		const batch = lines
		lines = []
		sent += batch.length
		pushing = pushing.then(async () => {
			const answer = await fetch(`${arg('loki')}/loki/api/v1/push`, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({ streams: [{ stream: { app: 'api', level: 'info' }, values: batch }] }),
			})
			if (answer.ok) stored += batch.length
			else dropped += batch.length
		})
		return pushing
	}
	const pusher = setInterval(push, 1000)

	// a counter scraped from /metrics, as a Prometheus client keeps one
	let total = 0
	const exposition = Bun.serve({
		hostname: '127.0.0.1',
		port: 9464,
		fetch: () => new Response(`# TYPE requests_total counter\nrequests_total ${total}\n`),
	})

	return {
		session: async user => {
			const value = await kv.get(`s${user}`)
			return value == null ? undefined : JSON.parse(value)
		},
		note: async id => (await sql`select id, body, version from notes where id = ${id}`)[0],
		write: async (id, body) => {
			await sql.begin(async tx => {
				await tx`update notes set body = ${body}, version = version + 1 where id = ${id}`
				await tx`insert into jobs (note) values (${id})`
			})
			enqueued += 1
		},
		upload: async (id, bytes) => {
			await bucket.write(`attachments/${id}.bin`, bytes)
		},
		log: fields => {
			lines.push([`${Date.now()}000000`, JSON.stringify({ level: 'info', msg: 'request', ...fields })])
		},
		count: () => {
			total += 1
		},
		settle: async () => {
			const until = Date.now() + 60_000
			while (handled < enqueued && Date.now() < until) await sleep(50)
			clearInterval(pusher)
			await push()
			await pushing
			const [left] = await sql`select count(*)::int as n from jobs`
			const [versions] = await sql`select sum(version)::int as n from notes`
			return {
				enqueued,
				handled,
				jobsLeft: left.n,
				logsSent: sent,
				logsStored: stored,
				logsDropped: dropped,
				versions: versions.n,
			}
		},
		close: async () => {
			working = false
			await worker
			exposition.stop(true)
			kv.close()
			await sql.close()
			await workerSql.close()
		},
	}
}

/** the notes a thousand at a time and the sessions from 64 at once */
async function seed(notes: (from: number, to: number) => Promise<unknown>, session: (user: number) => Promise<unknown>) {
	mark('seed')
	for (let from = 0; from < NOTES; from += 1000) await notes(from, Math.min(NOTES, from + 1000))
	let next = 0
	await Promise.all(
		Array.from({ length: 64 }, async () => {
			while (next < USERS) await session(next++)
		}),
	)
}
