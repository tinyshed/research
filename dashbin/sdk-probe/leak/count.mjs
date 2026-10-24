let n = 0
await new Promise(r => { const id = setInterval(() => { if (++n === 3) { clearInterval(id); r() } }, 20) })
console.log('interval fired', n, 'times')
