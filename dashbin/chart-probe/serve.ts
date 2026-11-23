// The probe's page and its bundles on http://127.0.0.1:5199, nothing cached.
import { join } from 'node:path'

const root = import.meta.dir
Bun.serve({
	hostname: '127.0.0.1',
	port: 5199,
	async fetch(request) {
		const path = new URL(request.url).pathname
		const file = Bun.file(join(root, path === '/' ? 'bench.html' : path))
		if (!(await file.exists())) return new Response('not found', { status: 404 })
		return new Response(file, { headers: { 'cache-control': 'no-store' } })
	},
})
process.stdout.write('http://127.0.0.1:5199/bench.html?lib=echarts&case=day\n')
