// One library, one case: drawn `warm` times unmeasured and `runs` times measured, each from an
// empty element to two animation frames after the library returned, about 17 to 20 ms on a
// screen of 120 Hz when the drawing itself costs nothing: that is the method's floor. What it leaves in
// `window.result` is read by whoever drives the page.
import { CASES, type Draw, type Drawn } from './data.ts'

declare const DRAW: Draw

const asked = new URLSearchParams(location.search)
const which = asked.get('case') ?? 'day'
const runs = Number(asked.get('runs') ?? 15)
const warm = Number(asked.get('warm') ?? 3)
const keep = asked.has('keep')
const hide = asked.get('hide')

const frame = () => new Promise<void>(done => requestAnimationFrame(() => done()))
const element = document.getElementById('chart') as HTMLElement
const drawn = CASES[which]
if (!drawn) throw new Error(`no case ${which}`)

async function once(): Promise<{ ms: number; sync: number; chart: Drawn }> {
	element.replaceChildren()
	await frame()
	const started = performance.now()
	const chart = DRAW(element, drawn as never)
	// what the call itself took, before any frame: a library that draws at once shows here
	const sync = performance.now() - started
	await frame()
	await frame()
	return { ms: performance.now() - started, sync, chart }
}

const times: number[] = []
const syncs: number[] = []
let last: Drawn | undefined
let failed: string | undefined
try {
	for (let at = 0; at < warm + runs; at++) {
		last?.destroy()
		const { ms, sync, chart } = await once()
		last = chart
		if (at >= warm) {
			times.push(ms)
			syncs.push(sync)
		}
	}
	if (hide && last) {
		const started = performance.now()
		last.toggle(hide)
		await frame()
		await frame()
		times.push(-(performance.now() - started))
	}
} catch (error) {
	failed = error instanceof Error ? `${error.name}: ${error.message}` : String(error)
}
if (!keep) last?.destroy()

const measured = times.filter(ms => ms >= 0).sort((a, b) => a - b)
const toggled = times.find(ms => ms < 0)
Object.assign(window, {
	result: {
		case: which,
		runs: measured.length,
		median: measured[Math.floor(measured.length / 2)],
		least: measured[0],
		most: measured[measured.length - 1],
		sync: syncs.sort((a, b) => a - b)[Math.floor(syncs.length / 2)],
		toggle: toggled === undefined ? undefined : -toggled,
		failed,
	},
})
document.title = failed ? `failed: ${failed}` : `done ${which}`
