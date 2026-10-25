// chapter 6's example, one change: the `api` setting points the SDK at stripe-mock, as an admin would type it
import Stripe from 'stripe'
import { settings, hosts, secret, text, url, money, read, action } from 'dashbin'

settings({ key: secret({ title: 'Secret key' }), api: url({ optional: true }) })
hosts('api.stripe.com')

const client = (config) => {
  const api = config.api && new URL(config.api)
  const at = api ? { host: api.hostname, port: Number(api.port), protocol: api.protocol.replace(':', '') } : {}
  return new Stripe(config.key.reveal(), at)
}

export const charges = read({
  params: { status: text({ optional: true }) },
  columns: { amount: money('currency') },    // minor units; the currency is that column
  precision: 'second',                       // so the kernel hands this read whole seconds
}, async ({ from, to, status }, { config, limit }) => {
  const stripe = client(config)
  const created = { gte: from.getTime() / 1000, lt: to.getTime() / 1000 }
  const rows = []
  for await (const c of stripe.charges.list({ created, limit: 100 })) {
    if (status && c.status !== status) continue    // the list takes no status filter
    rows.push({ time: new Date(c.created * 1000), amount: c.amount, currency: c.currency, status: c.status })
    if (rows.length > limit.rows) break
  }
  return rows
})

export const refund = action({
  params: { charge: text() },
}, async ({ charge }, { config, id }) => {
  const stripe = client(config)
  const r = await stripe.refunds.create({ charge }, { idempotencyKey: id })
  return { refund: r.id, status: r.status }
})
