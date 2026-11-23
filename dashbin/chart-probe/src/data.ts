// Frames in the shape the traffic store answers: buckets of time, a series a category, the values
// a counter's increase in the bucket. Seeded, so that every library and every run draws the same.

export interface Data {
	/** Milliseconds since 1970, a bucket each. */
	times: number[]
	series: { name: string; values: number[] }[]
}

export type Kind = 'bars' | 'line'
export interface Case {
	kind: Kind
	data: Data
	/** Drawn as a dashed horizontal line. */
	threshold: number
}

const NAMES = ['video', 'social', 'messaging', 'cdn', 'ai', 'other']

function random(seed: number) {
	let state = seed
	return () => {
		state = (state * 1664525 + 1013904223) % 4294967296
		return state / 4294967296
	}
}

function make(buckets: number, step: number, seed: number): Data {
	const next = random(seed)
	const start = Date.UTC(2026, 10, 16)
	const times = Array.from({ length: buckets }, (_, at) => start + at * step)
	const series = NAMES.map((name, which) => {
		const size = 6e9 / (which + 1)
		const values = times.map((_, at) => {
			const day = 0.6 + 0.4 * Math.sin((at * step) / 86_400_000 * 2 * Math.PI + which)
			return Math.round(size * day * (0.7 + 0.6 * next()))
		})
		return { name, values }
	})
	return { times, series }
}

export const CASES: Record<string, Case> = {
	day: { kind: 'bars', data: make(25, 3_600_000, 1), threshold: 8e9 },
	week: { kind: 'bars', data: make(1008, 600_000, 2), threshold: 8e9 },
	line: { kind: 'line', data: make(8640, 300_000, 3), threshold: 4e9 },
}

/** The page's tokens, as a drawer reads them: nothing in a library's file names a colour. */
export function tokens(element: HTMLElement) {
	const style = getComputedStyle(element)
	const token = (name: string) => style.getPropertyValue(name).trim()
	return {
		ink: token('--ink'),
		muted: token('--muted'),
		line: token('--line'),
		series: [1, 2, 3, 4, 5, 6].map(at => token(`--s${at}`)),
	}
}

export interface Drawn {
	destroy(): void
	/** Hides or shows the series `name`, as a click on the legend does. */
	toggle(name: string): void
}

export type Draw = (element: HTMLElement, drawn: Case) => Drawn
