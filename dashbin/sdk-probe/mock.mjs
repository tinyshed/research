// answers whatever the SDK tests ask, so a passing test means the SDK really sent and parsed a request
Bun.serve({
  port: 8080,
  async fetch(req) {
    const u = new URL(req.url)
    if (u.pathname === '/redirect') return Response.redirect(u.searchParams.get('to'), 302)
    if (u.pathname === '/slow') { await Bun.sleep(20000); return Response.json({ slow: true }) }
    if (u.pathname.startsWith('/repos/')) return Response.json({ full_name: 'tinyshed/tinystore', stargazers_count: 42 })
    if (u.pathname === '/v1/chat/completions')
      return Response.json({ id: 'c1', object: 'chat.completion', created: 0, model: 'x', choices: [{ index: 0, finish_reason: 'stop', message: { role: 'assistant', content: 'hello' } }] })
    if (u.pathname === '/v1/search') return Response.json({ object: 'list', results: [], has_more: false, next_cursor: null })
    if (u.pathname.startsWith('/api/chat.postMessage')) return Response.json({ ok: true, ts: '1.2', channel: 'C1' })
    return Response.json({ error: 'not mocked', path: u.pathname }, { status: 404 })
  },
})
