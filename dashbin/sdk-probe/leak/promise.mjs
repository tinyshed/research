globalThis.pending = new Promise(() => {})
console.log('left a promise that never settles')
