import { Octokit } from '@octokit/rest'
const octokit = new Octokit({ baseUrl: 'http://rssprobe-mock:8080' })
const { data } = await octokit.rest.repos.get({ owner: 'tinyshed', repo: 'tinystore' })
console.log(JSON.stringify({ ok: true, repo: data.full_name, stars: data.stargazers_count }))
{
  const name = 'node:fs'
  try {
    const fs = await import(name)
    const s = fs.readFileSync('/proc/self/status', 'utf8').split('\n')
    const kb = k => Number((s.find(l => l.startsWith(k)) || '0').replace(/[^0-9]/g, ''))
    console.log(JSON.stringify({ rss_mib: +(kb('VmRSS:') / 1024).toFixed(1), peak_mib: +(kb('VmHWM:') / 1024).toFixed(1) }))
  } catch (e) { console.log(JSON.stringify({ rss: 'unavailable: ' + e.message })) }
}
