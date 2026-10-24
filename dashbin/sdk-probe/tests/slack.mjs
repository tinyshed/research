import { WebClient } from '@slack/web-api'
const slack = new WebClient('xoxb-test', { slackApiUrl: 'http://rssprobe-mock:8080/api/' })
const r = await slack.chat.postMessage({ channel: 'C1', text: 'hi' })
console.log(JSON.stringify({ ok: r.ok, ts: r.ts }))
{
  const name = 'node:fs'
  try {
    const fs = await import(name)
    const s = fs.readFileSync('/proc/self/status', 'utf8').split('\n')
    const kb = k => Number((s.find(l => l.startsWith(k)) || '0').replace(/[^0-9]/g, ''))
    console.log(JSON.stringify({ rss_mib: +(kb('VmRSS:') / 1024).toFixed(1), peak_mib: +(kb('VmHWM:') / 1024).toFixed(1) }))
  } catch (e) { console.log(JSON.stringify({ rss: 'unavailable: ' + e.message })) }
}
