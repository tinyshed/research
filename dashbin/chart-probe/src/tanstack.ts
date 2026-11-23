import { barY, colorLegend, defineChart, lineY, mountChart, ruleY } from '@tanstack/charts'
import { mountCanvasChart } from '@tanstack/charts/canvas'
import { controlledSignal } from '@tanstack/charts/interaction/signal'
import { interactiveColorLegend } from '@tanstack/charts/legend'
import { scaleBand } from '@tanstack/charts/scales/band'
import { scaleLinear } from '@tanstack/charts/scales/linear'
import { scaleUtc } from '@tanstack/charts/scales/time'

import { type Case, type Draw, tokens } from './data.ts'

// A grammar of graphics takes rows, a row an observation, where the frame holds columns.
interface Row {
	time: Date
	bucket: string
	category: string
	value: number
}

const drawWith =
	(mount: typeof mountChart): Draw =>
	(element, { kind, data, threshold }: Case) => {
		const colours = tokens(element)
		const bars = kind === 'bars'
		const rows: Row[] = data.series.flatMap(series =>
			series.values.map((value, at) => {
				const time = new Date(data.times[at] ?? 0)
				return { time, bucket: time.toISOString(), category: series.name, value }
			}),
		)
		const names = data.series.map(series => series.name)
		let visible = names
		const definition = () =>
			defineChart({
				marks: [
					bars
						? barY(rows, { x: 'bucket', y: 'value', color: 'category' })
						: lineY(rows, { x: 'time', y: 'value', color: 'category', z: 'category' }),
					ruleY([threshold], { stroke: colours.muted, strokeDasharray: '5 4' }),
				],
				scales: {
					x: bars ? { scale: () => scaleBand<string>().padding(0.1), axis: false } : { scale: scaleUtc },
					y: { scale: scaleLinear, nice: true, grid: true },
				},
				color: {
					domain: names,
					range: colours.series,
					legend: interactiveColorLegend({
						visible: controlledSignal(visible, next => {
							visible = [...next]
							host.update(options())
						}),
						placement: 'top',
						ariaLabel: 'Series',
					}),
				},
				theme: {
					foreground: colours.ink,
					muted: colours.muted,
					grid: colours.line,
					palette: colours.series,
				},
			})
		const options = () => ({
			definition: definition(),
			height: element.clientHeight,
			initialWidth: element.clientWidth,
			ariaLabel: 'Traffic',
		})
		const host = mount(element, options() as never)
		return {
			destroy: () => host.destroy(),
			toggle: name => {
				visible = visible.includes(name) ? visible.filter(shown => shown !== name) : [...visible, name]
				host.update(options() as never)
			},
		}
	}

void colorLegend
export const draw = drawWith(mountChart)
export const drawCanvas = drawWith(mountCanvasChart as unknown as typeof mountChart)
