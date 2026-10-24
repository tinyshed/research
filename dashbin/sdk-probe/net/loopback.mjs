try {
  const r = await fetch('http://127.0.0.1:8080/')
  console.log('loopback:', 'HTTP ' + r.status)
} catch (e) {
  console.log('loopback:', 'refused —', String(e.message || e).slice(0, 140))
}
