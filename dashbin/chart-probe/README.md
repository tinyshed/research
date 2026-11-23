# chart-probe

Which chart library Dashbin's page draws with. Three candidates draw the same frames the same way and are
weighed and timed: ECharts (the plan's, chapter 9), Chart.js (what the hand-made traffic page this round is
measured against uses), uPlot (the plan's fallback).

## What is decided before it runs

Written on 23 November 2026, before any number was taken.

A library is **taken** when it passes every line, and of those that pass the lightest wins:

| Measure                                                     | To pass                                  |
|-------------------------------------------------------------|------------------------------------------|
| stacked bars over time, a series a category                 | built in, or under 60 lines of ours      |
| a click on the legend hides a series and the stack closes   | built in, or under 60 lines of ours      |
| a horizontal threshold line                                 | built in, or under 30 lines of ours      |
| every colour from the page's CSS tokens, both themes        | yes                                      |
| first draw, 25 hours × 6 categories (150 bars)              | median under 50 ms                       |
| first draw, a week by 10 minutes × 6 (6 048 bars)           | median under 150 ms                      |
| first draw, a line of 51 840 points over 6 series           | median under 300 ms                      |

Weight is the tie-breaker and is reported whatever happens: the gzipped bytes a dashboard's chunk gains, with
only the bar and line charts and what the lines above need imported. If none passes, the report says which line
each failed and by how much; writing a drawer of our own is argued from that, not chosen beforehand.

"Lines of ours" are counted in `src/<library>.ts` between the `// ours:` marks.

## How it is run

```sh
bun install
bun build.ts          # each library's bundle, minified, and its bytes raw and gzipped
bun serve.ts          # http://127.0.0.1:5199/bench.html?lib=echarts&case=day
```

`bench.html` draws a case `runs` times after `warm` unmeasured ones, each from an empty element to two animation
frames after the library returned, animation off, and leaves the times in `window.result`. The cases are drawn
in turns across libraries by whoever drives the page. Data is generated, seeded, in the shape of the traffic
store's answers: `day` 25 × 6, `week` 1 008 × 6, `line` 8 640 × 6.
