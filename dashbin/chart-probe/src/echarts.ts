import { BarChart, LineChart } from 'echarts/charts'
import { GridComponent, LegendComponent, MarkLineComponent, TooltipComponent } from 'echarts/components'
import { init, use } from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'

import { type Case, type Draw, tokens } from './data.ts'

use([BarChart, LineChart, GridComponent, LegendComponent, TooltipComponent, MarkLineComponent, CanvasRenderer])

export const draw: Draw = (element, { kind, data, threshold }: Case) => {
	const colours = tokens(element)
	const chart = init(element)
	chart.setOption({
		animation: false,
		color: colours.series,
		textStyle: { color: colours.muted },
		grid: { left: 56, right: 12, top: 32, bottom: 28 },
		legend: { top: 0, textStyle: { color: colours.muted } },
		tooltip: { trigger: 'axis' },
		xAxis: {
			type: 'time',
			axisLine: { lineStyle: { color: colours.line } },
			splitLine: { show: false },
		},
		yAxis: { type: 'value', splitLine: { lineStyle: { color: colours.line } } },
		series: data.series.map((series, at) => ({
			name: series.name,
			type: kind === 'bars' ? 'bar' : 'line',
			stack: kind === 'bars' ? 'all' : undefined,
			showSymbol: false,
			large: true,
			data: series.values.map((value, bucket) => [data.times[bucket], value]),
			markLine:
				at === 0
					? {
							silent: true,
							symbol: 'none',
							lineStyle: { color: colours.muted, type: 'dashed' },
							data: [{ yAxis: threshold }],
						}
					: undefined,
		})),
	})
	return {
		destroy: () => chart.dispose(),
		toggle: name => chart.dispatchAction({ type: 'legendToggleSelect', name }),
	}
}
