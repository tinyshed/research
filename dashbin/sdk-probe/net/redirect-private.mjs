try {
  const r = await fetch('http://rssprobe-mock:8080/redirect?to=http://rssprobe-stripe:12111/v1/charges')
  console.log('redirect-private:', 'HTTP ' + r.status)
} catch (e) {
  console.log('redirect-private:', 'refused —', String(e.message || e).slice(0, 140))
}
