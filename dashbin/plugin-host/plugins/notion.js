// one search, in chapter 6's shape; `api` points the SDK at the mock
import { Client } from '@notionhq/client'
import { settings, hosts, secret, url, read } from 'dashbin'

settings({ key: secret(), api: url({ optional: true }) })
hosts('api.notion.com')

export const pages = read(async (_, { config }) => {
  const notion = new Client({ auth: config.key.reveal(), baseUrl: config.api })
  const r = await notion.search({ query: 'x' })
  return r.results.map((p) => ({ id: p.id }))
})
