import Papa from 'papaparse'
import { z } from 'zod'
import { format, subDays } from 'date-fns'
const csv = Papa.parse('day,amount\n2026-10-01,10\n2026-10-02,20', { header: true, dynamicTyping: true })
const Row = z.object({ day: z.string(), amount: z.number() })
const rows = csv.data.filter(r => r.day).map(r => Row.parse(r))
console.log(JSON.stringify({ ok: true, rows: rows.length, label: format(subDays(new Date(2026, 9, 25), 1), 'yyyy-MM-dd') }))
{
  const name = 'node:fs'
  try {
    const fs = await import(name)
    const s = fs.readFileSync('/proc/self/status', 'utf8').split('\n')
    const kb = k => Number((s.find(l => l.startsWith(k)) || '0').replace(/[^0-9]/g, ''))
    console.log(JSON.stringify({ rss_mib: +(kb('VmRSS:') / 1024).toFixed(1), peak_mib: +(kb('VmHWM:') / 1024).toFixed(1) }))
  } catch (e) { console.log(JSON.stringify({ rss: 'unavailable: ' + e.message })) }
}
