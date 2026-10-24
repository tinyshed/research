try {
  const r = await fetch('https://example.com/')
  console.log('public:', 'HTTP ' + r.status)
} catch (e) {
  console.log('public:', 'refused —', String(e.message || e).slice(0, 140))
}
