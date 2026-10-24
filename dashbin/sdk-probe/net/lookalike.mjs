try {
  const r = await fetch('http://evil-stripe.com/')
  console.log('lookalike:', 'HTTP ' + r.status)
} catch (e) {
  console.log('lookalike:', 'refused —', String(e.message || e).slice(0, 140))
}
