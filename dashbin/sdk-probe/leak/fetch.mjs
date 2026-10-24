fetch('http://rssprobe-mock:8080/slow').catch(() => {})
console.log('left a request in flight')
