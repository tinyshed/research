import OpenAI from 'openai'
const client = new OpenAI({ apiKey: 'sk-test', baseURL: 'http://rssprobe-mock:8080/v1' })
const r = await client.chat.completions.create({ model: 'x', messages: [{ role: 'user', content: 'hi' }] })
console.log(JSON.stringify({ ok: true, reply: r.choices[0].message.content }))
{
  const name = 'node:fs'
  try {
    const fs = await import(name)
    const s = fs.readFileSync('/proc/self/status', 'utf8').split('\n')
    const kb = k => Number((s.find(l => l.startsWith(k)) || '0').replace(/[^0-9]/g, ''))
    console.log(JSON.stringify({ rss_mib: +(kb('VmRSS:') / 1024).toFixed(1), peak_mib: +(kb('VmHWM:') / 1024).toFixed(1) }))
  } catch (e) { console.log(JSON.stringify({ rss: 'unavailable: ' + e.message })) }
}
