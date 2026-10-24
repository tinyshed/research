try {
  const r = await fetch('http://rssprobe-mock:8080/redirect?to=http://169.254.169.254/latest/meta-data/')
  console.log('redirect-metadata:', 'HTTP ' + r.status)
} catch (e) {
  console.log('redirect-metadata:', 'refused —', String(e.message || e).slice(0, 140))
}
