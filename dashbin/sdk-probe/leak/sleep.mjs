const t0 = Date.now()
await new Promise(r => setTimeout(r, 100))
console.log('slept', Date.now() - t0, 'ms')
