import Stripe from 'stripe'
const stripe = new Stripe('sk_test_123', { host: 'rssprobe-stripe', port: 12111, protocol: 'http', httpClient: Stripe.createFetchHttpClient() })
const rows = []
for await (const c of stripe.charges.list({ limit: 3 })) {
  rows.push({ time: c.created * 1000, amount: c.amount, currency: c.currency })
  if (rows.length >= 5) break
}
const refund = await stripe.refunds.create({ charge: 'ch_123' }, { idempotencyKey: 'refund-ch_123' })
console.log(JSON.stringify({ ok: true, rows: rows.length, first: rows[0], refund: refund.id }))
{
  const name = 'node:fs'
  try {
    const fs = await import(name)
    const s = fs.readFileSync('/proc/self/status', 'utf8').split('\n')
    const kb = k => Number((s.find(l => l.startsWith(k)) || '0').replace(/[^0-9]/g, ''))
    console.log(JSON.stringify({ rss_mib: +(kb('VmRSS:') / 1024).toFixed(1), peak_mib: +(kb('VmHWM:') / 1024).toFixed(1) }))
  } catch (e) { console.log(JSON.stringify({ rss: 'unavailable: ' + e.message })) }
}
