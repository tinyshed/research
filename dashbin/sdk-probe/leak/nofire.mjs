let fired = false
const id = setTimeout(() => { fired = true }, 10)
clearTimeout(id)
await new Promise(r => setTimeout(r, 50))
console.log('cleared timer fired:', fired)
