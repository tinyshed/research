try {
  const r = await fetch('http://localhost:8080/')
  console.log('localhost:', 'HTTP ' + r.status)
} catch (e) {
  console.log('localhost:', 'refused —', String(e.message || e).slice(0, 140))
}
