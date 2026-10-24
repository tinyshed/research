// a plugin as its own process: waits on stdin, does the same work as the quickjs probe
const fs = require('fs')

function rss() {
  const status = fs.readFileSync('/proc/self/status', 'utf8').split('\n')
  const pick = key => Number((status.find(l => l.startsWith(key)) || '0').replace(/[^0-9]/g, ''))
  const mib = key => (pick(key) / 1024).toFixed(1)
  return `VmRSS ${mib('VmRSS:')} MiB  RssAnon ${mib('RssAnon:')}  RssFile ${mib('RssFile:')}  VmHWM ${mib('VmHWM:')}  threads ${pick('Threads:')}`
}

const checkpoint = what => console.log(what.padEnd(44) + ' ' + rss())

function query(n) {
  const time = new Float64Array(n), value = new Float64Array(n)
  const t0 = Date.now()
  for (let i = 0; i < n; i++) { time[i] = t0 - (n - i) * 15000; value[i] = Math.sin(i / 10) * 100 + 200 }
  const body = JSON.stringify(Array.from({ length: n }, (_, i) => ({ created: time[i] / 1000, amount: value[i] })))
  const data = JSON.parse(body)
  let sum = 0
  for (const r of data) sum += r.amount
  return sum
}

process.stdin.on('data', line => process.stdout.write(JSON.stringify({ result: query(10000) }) + '\n'))

setTimeout(() => {
  checkpoint('idle, 3 s after start')
  query(10000)
  checkpoint('after one call')
  const started = Date.now()
  for (let i = 0; i < 100; i++) query(10000)
  console.log(`  100 calls took ${Date.now() - started} ms`)
  checkpoint('after 100 calls')
  setTimeout(() => {
    checkpoint('10 s later')
    if (globalThis.Bun) Bun.gc(true)
    else if (global.gc) global.gc()
    checkpoint('after a full gc')
    process.exit(0)
  }, 10000)
}, 3000)
