// One scenario of an application's SQL, not SQL at large: a row read by its
// primary key and a row inserted, over a table of 100,000 notes, from 1, 8,
// 64 and 256 calls in flight. TinyStore's database against PostgreSQL through
// Bun's own client on its Unix socket, a pool of 64. Both commit durably, as
// they ship: TinyStore 'full', PostgreSQL synchronous_commit on, each grouping
// the commits that wait together.
import { arg, load, number, random, text } from './lib'
import { sql as tag } from '@tinyshed/tinystore'
import { postgres, tinystore } from './stores'

const ROWS = 100_000
const seconds = number('seconds')
const levels = [1, 8, 64, 256]

interface Db {
	read(id: number): Promise<unknown>
	insert(id: number, body: string): Promise<unknown>
	seed(rows: { id: number; user_id: number; body: string; created_at: number }[]): Promise<unknown>
	close(): Promise<unknown>
}

const db = await opened(arg('contender'))
for (let from = 0; from < ROWS; from += 1000) {
	const rows = []
	for (let id = from; id < from + 1000; id++) rows.push({ id, user_id: id % 1000, body: text(200, id), created_at: id })
	await db.seed(rows)
}
const pick = Array.from({ length: 256 }, (_, caller) => random(caller + 3))
for (const inFlight of levels) {
	await load('read', inFlight, seconds, async caller => {
		if ((await db.read(Math.floor((pick[caller] ?? Math.random)() * ROWS))) == null) throw new Error('a row read as none')
	})
}
let next = ROWS
for (const inFlight of levels) {
	await load('insert', inFlight, seconds, async (_, call) => db.insert(next++, text(200, call)))
}
await db.close()

async function opened(contender: string): Promise<Db> {
	if (contender === 'tinystore') {
		const store = await tinystore()
		const app = await store.database('bench', {
			migrations: {
				'0001_notes.sql':
					'create table notes (id integer primary key, user_id integer not null, body text not null, created_at integer not null)',
			},
		})
		return {
			read: id => app.one`select id, user_id, body, created_at from notes where id = ${id}`,
			insert: (id, body) => app.exec`insert into notes (id, user_id, body, created_at) values (${id}, ${id % 1000}, ${body}, ${id})`,
			seed: rows =>
				app.batch(
					rows.map(
						row =>
							tag`insert into notes (id, user_id, body, created_at) values (${row.id}, ${row.user_id}, ${row.body}, ${row.created_at})`,
					),
				),
			close: () => store.close(),
		}
	}
	const sql = postgres(64)
	await sql`create table notes (id integer primary key, user_id integer not null, body text not null, created_at bigint not null)`
	return {
		read: async id => (await sql`select id, user_id, body, created_at from notes where id = ${id}`)[0],
		insert: (id, body) => sql`insert into notes (id, user_id, body, created_at) values (${id}, ${id % 1000}, ${body}, ${id})`,
		seed: rows => sql`insert into notes ${sql(rows)}`,
		close: () => sql.close(),
	}
}
