try {
  const r = await fetch('http://rssprobe-mock:8080/repos/tinyshed/tinystore')
  console.log('allowed:', 'HTTP ' + r.status)
} catch (e) {
  console.log('allowed:', 'refused —', String(e.message || e).slice(0, 140))
}
