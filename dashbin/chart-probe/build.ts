// Each library's bundle as a dashboard's chunk would carry it: the adapter of src/ and what it
// imports, minified, and its bytes raw and gzipped. The harness around a library is the same
// few hundred bytes for each, built in with it so that one file is one page's script.
import { mkdir, rm } from 'node:fs/promises'
import { join } from 'node:path'

const out = join(import.meta.dir, 'dist')
await rm(out, { recursive: true, force: true })
await mkdir(out, { recursive: true })

const LIBRARIES: Record<string, [string, string]> = {
	echarts: ['./src/echarts.ts', 'draw'],
	chartjs: ['./src/chartjs.ts', 'draw'],
	uplot: ['./src/uplot.ts', 'draw'],
	tanstack: ['./src/tanstack.ts', 'draw'],
	'tanstack-canvas': ['./src/tanstack.ts', 'drawCanvas'],
}

// the harness alone, to take its bytes off each library's
const bare = join(out, 'entry-bare.ts')
await Bun.write(bare, `globalThis.DRAW = () => ({ destroy() {}, toggle() {} })\nawait import('../src/bench.ts')\n`)

const sizes: Record<string, { raw: number; gzip: number }> = {}
for (const [name, [file, exported]] of [['bare', ['', '']] as const, ...Object.entries(LIBRARIES)]) {
	const entry = name === 'bare' ? bare : join(out, `entry-${name}.ts`)
	if (name !== 'bare') {
		await Bun.write(
			entry,
			`import { ${exported} as draw } from '.${file}'\nglobalThis.DRAW = draw\nawait import('../src/bench.ts')\n`,
		)
	}
	const built = await Bun.build({
		entrypoints: [entry],
		outdir: out,
		naming: `${name}.js`,
		minify: true,
		target: 'browser',
		format: 'esm',
	})
	if (!built.success) {
		process.stderr.write(`${name}: ${built.logs.map(log => log.message).join('; ')}\n`)
		continue
	}
	const bytes = new Uint8Array(await Bun.file(join(out, `${name}.js`)).arrayBuffer())
	sizes[name] = { raw: bytes.length, gzip: Bun.gzipSync(bytes, { level: 9 }).length }
}

const harness = sizes.bare ?? { raw: 0, gzip: 0 }
const kib = (bytes: number) => (bytes / 1024).toFixed(1).padStart(7)
process.stdout.write('library            raw KiB  gzip KiB   (the harness taken off)\n')
for (const [name, size] of Object.entries(sizes)) {
	if (name === 'bare') continue
	process.stdout.write(
		`${name.padEnd(16)} ${kib(size.raw - harness.raw)}  ${kib(size.gzip - harness.gzip)}\n`,
	)
}
await Bun.write(join(out, 'sizes.json'), JSON.stringify({ harness, sizes }, null, 2))
