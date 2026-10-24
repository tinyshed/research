import pg from 'pg'
const client = new pg.Client({ host: 'rssprobe-pg', user: 'postgres', password: 'probe' })
await client.connect()
const r = await client.query('select 1 as one')
console.log(JSON.stringify({ ok: true, one: r.rows[0].one }))
await client.end()
{
  const name = 'node:fs'
  try {
    const fs = await import(name)
    const s = fs.readFileSync('/proc/self/status', 'utf8').split('\n')
    const kb = k => Number((s.find(l => l.startsWith(k)) || '0').replace(/[^0-9]/g, ''))
    console.log(JSON.stringify({ rss_mib: +(kb('VmRSS:') / 1024).toFixed(1), peak_mib: +(kb('VmHWM:') / 1024).toFixed(1) }))
  } catch (e) { console.log(JSON.stringify({ rss: 'unavailable: ' + e.message })) }
}
