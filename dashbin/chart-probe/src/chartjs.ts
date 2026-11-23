import {
	BarController,
	BarElement,
	CategoryScale,
	Chart,
	Legend,
	LinearScale,
	LineController,
	LineElement,
	type Plugin,
	PointElement,
	Tooltip,
} from 'chart.js'

import { type Case, type Draw, tokens } from './data.ts'

Chart.register(
	BarController,
	BarElement,
	LineController,
	LineElement,
	PointElement,
	CategoryScale,
	LinearScale,
	Legend,
	Tooltip,
)

// ours: a threshold line, which Chart.js has only in a separate annotation plugin
const thresholdLine = (value: number, colour: string): Plugin => ({
	id: 'threshold',
	afterDatasetsDraw(chart) {
		const { ctx, chartArea, scales } = chart
		const y = scales.y?.getPixelForValue(value)
		if (y === undefined || y < chartArea.top || y > chartArea.bottom) return
		ctx.save()
		ctx.strokeStyle = colour
		ctx.setLineDash([5, 4])
		ctx.beginPath()
		ctx.moveTo(chartArea.left, y)
		ctx.lineTo(chartArea.right, y)
		ctx.stroke()
		ctx.restore()
	},
})
// ours: end

export const draw: Draw = (element, { kind, data, threshold }: Case) => {
	const colours = tokens(element)
	const canvas = element.appendChild(document.createElement('canvas'))
	const bars = kind === 'bars'
	const labels = data.times.map(time => new Date(time).toISOString().slice(5, 16).replace('T', ' '))
	const chart = new Chart(canvas, {
		type: bars ? 'bar' : 'line',
		data: {
			labels,
			datasets: data.series.map((series, at) => ({
				label: series.name,
				data: series.values,
				backgroundColor: colours.series[at],
				borderColor: colours.series[at],
				borderWidth: bars ? 0 : 1,
				pointRadius: 0,
				stack: bars ? 'all' : undefined,
			})),
		},
		options: {
			animation: false,
			maintainAspectRatio: false,
			normalized: true,
			interaction: { mode: 'index', intersect: false },
			color: colours.muted,
			scales: {
				x: { stacked: bars, grid: { display: false }, ticks: { color: colours.muted, maxTicksLimit: 12 } },
				y: { stacked: bars, grid: { color: colours.line }, ticks: { color: colours.muted } },
			},
		},
		plugins: [thresholdLine(threshold, colours.muted)],
	})
	return {
		destroy: () => {
			chart.destroy()
			canvas.remove()
		},
		toggle: name => {
			const at = data.series.findIndex(series => series.name === name)
			chart.setDatasetVisibility(at, !chart.isDatasetVisible(at))
			chart.update()
		},
	}
}
