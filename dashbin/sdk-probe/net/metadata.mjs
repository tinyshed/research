try {
  const r = await fetch('http://169.254.169.254/latest/meta-data/')
  console.log('metadata:', 'HTTP ' + r.status)
} catch (e) {
  console.log('metadata:', 'refused —', String(e.message || e).slice(0, 140))
}
