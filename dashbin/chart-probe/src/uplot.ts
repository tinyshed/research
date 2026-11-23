import uPlot from 'uplot'

import { type Case, type Draw, tokens } from './data.ts'

// ours: uPlot draws series as given and stacks nothing: a stack is each series' running sum, drawn
// from the tallest back to the first so that each bar covers the one behind it, summed again over
// the series that are shown whenever one is hidden
function stacked(values: number[][], shown: boolean[]): number[][] {
	const sums = new Array<number>(values[0]?.length ?? 0).fill(0)
	return values.map((series, at) =>
		series.map((value, bucket) => {
			if (shown[at]) sums[bucket] = (sums[bucket] ?? 0) + value
			return sums[bucket] ?? 0
		}),
	)
}
// ours: end

export const draw: Draw = (element, { kind, data, threshold }: Case) => {
	const colours = tokens(element)
	const bars = kind === 'bars'
	const seconds = data.times.map(time => time / 1000)
	const raw = data.series.map(series => series.values)
	const shown = raw.map(() => true)
	// ours: painted from the tallest sum back to the first series, so that each bar covers the one behind it
	const order = raw.map((_, at) => at)
	if (bars) order.reverse()
	const values = () => {
		const summed = bars ? stacked(raw, shown) : raw
		return order.map(at => summed[at] ?? [])
	}
	// ours: end
	const paths = bars ? uPlot.paths.bars?.({ size: [0.9, 100] }) : undefined

	// ours: a threshold line, drawn on the plot's canvas after its series
	const thresholdLine = (plot: uPlot) => {
		const { ctx, bbox } = plot
		const y = plot.valToPos(threshold, 'y', true)
		ctx.save()
		ctx.strokeStyle = colours.muted
		ctx.setLineDash([5, 4])
		ctx.beginPath()
		ctx.moveTo(bbox.left, y)
		ctx.lineTo(bbox.left + bbox.width, y)
		ctx.stroke()
		ctx.restore()
	}
	// ours: end

	const axis = { stroke: colours.muted, grid: { stroke: colours.line, width: 1 }, ticks: { show: false } }
	const plot = new uPlot(
		{
			width: element.clientWidth,
			height: element.clientHeight - 30,
			scales: { y: { range: (_plot, _min, max) => [0, max * 1.05] } },
			axes: [{ ...axis, grid: { show: false } }, { ...axis, size: 96 }],
			series: [
				{},
				...order.map(at => ({
					label: data.series[at]?.name ?? '',
					stroke: colours.series[at] ?? colours.ink,
					fill: bars ? colours.series[at] : undefined,
					width: 1,
					points: { show: false },
					...(paths ? { paths } : {}),
				})),
			],
			hooks: {
				draw: [thresholdLine],
				// ours: a legend click hides a series, and the stack is summed again without it
				setSeries: [
					(plot, at, options) => {
						if (!bars || at === null || options.show === undefined) return
						shown[order[at - 1] ?? 0] = options.show
						plot.setData([seconds, ...values()] as uPlot.AlignedData, false)
						plot.redraw()
					},
				],
				// ours: end
			},
		},
		[seconds, ...values()] as uPlot.AlignedData,
		element,
	)
	return {
		destroy: () => plot.destroy(),
		toggle: name => {
			const at = order.indexOf(data.series.findIndex(series => series.name === name)) + 1
			plot.setSeries(at, { show: !plot.series[at]?.show })
		},
	}
}
