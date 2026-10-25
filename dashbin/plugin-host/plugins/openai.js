// one completion, in chapter 6's shape; `api` points the SDK at the mock
import OpenAI from 'openai'
import { settings, hosts, secret, url, read } from 'dashbin'

settings({ key: secret(), api: url({ optional: true }) })
hosts('api.openai.com')

export const reply = read(async (_, { config }) => {
  const client = new OpenAI({ apiKey: config.key.reveal(), baseURL: config.api ? `${config.api}/v1` : undefined })
  const r = await client.chat.completions.create({ model: 'x', messages: [{ role: 'user', content: 'hi' }] })
  return [{ reply: r.choices[0].message.content }]
})
