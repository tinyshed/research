await new Promise(r => setTimeout((a, b) => { console.log('callback got', a, b); r() }, 1, 'x', 2))
