# 9. Dashboards, the browser and the terminal

**Status: draft, second round, 27 October 2026.** The first round went
through two reviews; what they changed is listed under *Forks*. Nothing is
agreed until Dmitry says so.

Everything a person looks at: a dashboard, its panels and their grid, the
range and the variables above them, how a frame becomes a line or a number,
which way of drawing is suggested, what a plugin's own visualization may do
in someone's browser, the same dashboard in a terminal, and, after the first
version, in a chat.

**The kernel computes, a drawer draws.** A total, a sum per site, a last
value are the query's calculation (chapter 5), computed once for a panel and
an alert alike. The browser and the terminal are two drawers of the same
frames: they lay out, format and colour, and compute nothing that means
something. A number that a drawer would have to compute for itself is a sign
that something leaked out of the query.

The first user's case sets the bar: revenue of ten sites on one dashboard,
one switch that points every panel at one site, and a line per site that
still draws when one site is away.

## The application

**One single-page application, built once and embedded in the binary**, as
in Go Dashbin: SvelteKit with Svelte 5, built static, no server rendering and
no Node.js in production. Bun builds it at build time; the binary carries
each asset gzipped, once.

- **A route loads what it draws.** The chart library sits in the dashboard
  route's chunk, and a plugin's browser code loads only on a dashboard that
  uses it. Go Dashbin measured ECharts, tree-shaken to line and bar, at
  172 KiB gzipped in that chunk.
- **The frame wire's decoder is the browser's** (chapter 3): a frame arrives
  as typed arrays, and a column of numbers is a `Float64Array` the chart
  reads without a copy into objects.
- **The api's types are generated from the hub's handlers** (chapter 11), never
  written by hand.
- **Fonts are embedded and subset**, never fetched from a CDN: Golos Text and
  JetBrains Mono, which cover Cyrillic.
- **Everything reads design tokens, `--db-*`, and nothing else** (below).

## A dashboard, as stored

**A dashboard is one document**: its range, zone, variables and rows of
panels, each panel with its own query or a reference to a shared one.

```json
{
  "name": "Revenue",
  "range": "now-30d .. now",
  "zone": "Europe/Moscow",
  "refresh": "5m",
  "variables": [
    { "name": "site", "from": { "label": "site", "read": "postgres.sql" }, "multi": true, "default": "all" }
  ],
  "rows": [
    { "height": 2, "panels": [
      { "width": 2, "title": "Total", "viz": "number",
        "query": { "reads": [ { "read": "postgres.sql", "on": "site=$site", "sql": "…",
                                "calculate": { "values": { "revenue": { "op": "sum" } }, "by": [] } } ] },
        "thresholds": [ { "below": 100000, "state": "warn" } ] },
      { "width": 1, "title": "Orders today", "viz": "number", "repeat": "site",
        "query": { "shared": "orders-today" } }
    ] },
    { "height": 4, "panels": [
      { "width": 6, "title": "Revenue by day", "viz": "line", "query": { "shared": "revenue-by-day" } }
    ] }
  ]
}
```

- **It is one row of TinyStore SQL**: the document, beside the columns a
  listing reads — name, version, who changed it and when — so a list of two
  hundred dashboards does not read two hundred documents. Panels are not rows
  of their own; there is one truth, the document.
- **What it references is a small table written in the same transaction**:
  which shared queries the dashboard uses. Deleting a shared query a dashboard
  still uses is refused, as Go Dashbin refused deleting a query a panel drew.
- **A save names the version it was made from.** When someone else saved in
  between, the save is refused and says who, rather than one editor quietly
  overwriting the other.
- **Every saved version is kept** in a history table, so a dashboard is shown
  as it was and restored.
- **A save is announced on a TinyStore channel** after it commits, and open
  tabs and a screen on the wall redraw. The channel is a nudge and not the
  truth: a tab that reconnects or comes back into focus compares its version
  with the hub's and loads the document when it moved, so a lost message
  costs nothing.
- **A listing answers with a shape that cannot be sent back**: listed
  dashboards carry a panel count and no panels, so a client that sends a
  listing back cannot delete every panel, as Go Dashbin learned.
- **The document is what an export is.** Exporting a dashboard writes this
  document with the shared queries it names; importing checks that their
  plugins and instances exist, by name and label.

A flow is stored the same way (chapter 7): one document with its version,
its history and its references, on the same machinery, with a schema of its
own.

## A panel's query: its own, or shared

**A panel's query is its own by default**, inside the panel in the document.
It becomes shared only on purpose, and the two behave differently:

| What is done | A panel's own query | A shared query |
|---|---|---|
| the panel is copied | the query is copied with it | the copy names the same query |
| the query is edited | only this panel changes | every panel and flow naming it changes; the editor lists them, and a flow among them asks to confirm |
| the panel is deleted | its query goes with it | the query stays |
| the query is deleted | — | refused while anything names it |
| an alert is made from the panel | the flow gets its own copy, so editing the panel does not move the alert | the flow names the same query |
| "make it shared" | the query becomes a document of its own, and the panel names it | — |

Ten sites need no sharing: they are one panel's query with a variable or a
repeat. Sharing is for one query on several dashboards, or a panel and an
alert that must move together.

## The grid

**A dashboard is rows of panels on six columns**, as the design brief and Go
Dashbin have it: six is the whole width, three a half, two a third, one a
narrow number. A row has a height; a panel in it has a width, and the widths
of a row add up to six at most.

- **Rows, not coordinates.** Panels cannot overlap, the order on a phone is
  the order of the rows, and dragging moves a panel within its row or to
  another.
- **On a narrow screen a row that does not fit wraps; on a phone every panel
  takes the whole width**, in reading order. The column count is CSS, never an
  inline style: an inline custom property outranks every media query, and Go
  Dashbin's six columns stayed six on a phone.
- **Editing is on a desktop.** A phone reads a dashboard and does not
  rearrange it.

## Range, zone, refresh and the address

- **One range for the whole dashboard**, written relatively (`now-30d ..
  now`) and resolved by the hub once per refresh (chapter 5), so every panel
  sees the same instant.
- **The dashboard has a zone**, which "today", a day's bucket and the axis's
  dates follow (chapter 5's `zone`); a person in another zone sees Moscow's
  days on a Moscow dashboard, said in the header.
- **Refresh is the dashboard's** — off, or every so many minutes — and it
  **stops while the tab is hidden**, resuming when it is seen. A dashboard
  left open in a background tab does not query ten production databases all
  night.
- **The address holds the range and the variables**, so a link opens what its
  sender saw, and Back undoes a zoom. Dragging across a chart's time axis
  narrows the dashboard's range, not the panel's.

## Variables

A variable is a dashboard's own parameter (chapter 2), shown above the panels
as a selector, and it feeds every panel's query that names it.

| Where its values come from | Example |
|---|---|
| the values of a label on the instances a read reaches | `site`: kazan, msk, spb… — the first user's switch |
| a fixed list | `currency`: RUB, USD |
| a query's answer | `status`: the distinct statuses of payments |

- **`all` is a value written out**, and so is a variable holding several:
  `site=$site` with `all` reaches every site's instance, the eleventh included
  (chapter 5).
- **A viewer picks only what the variable allows**, checked on the hub before
  anything runs (chapter 5); the selector in the browser is a courtesy.
- **A variable changed reruns only the panels that use it.**
- **A flow never reads a dashboard's variables** (chapter 5): an alert made
  from a panel takes their values as its own parameters.

## A panel repeated, a panel combined

**Repeat** draws one panel per value of a variable, or per instance a
selector matches: `"repeat": "site"` lays out `orders-today` once per site,
each titled with its site, in its row, at the panel's own width, wrapping. An
eleventh site appears without editing the dashboard.

**Combined** is one panel over several instances. Drawn without a
calculation, `revenue-by-day` on `site=$site` is a line per site, each frame
named by its instance's label. A total over sites is a calculation
(`revenue: sum` by nothing), and the kernel refuses it when a site is missing
rather than handing the panel nine-tenths of the revenue (chapter 5).

## A panel draws as frames arrive

Each (read, instance) answers on its own and a calculation answers per group
(chapter 5), so a panel draws in parts: nine sites' lines appear while the
tenth is still running. A panel shows, for each instance or group, one of:

| State | What the panel shows |
|---|---|
| running | its part drawing in, or a quiet placeholder |
| done | drawn |
| empty | "no data for this period", or 0 where the calculation says `empty: zero` |
| cut by its cap | a table shows the rows and "first 10 000 of more"; a chart, a number and a gauge refuse, saying to aggregate in the query or raise the cap (chapter 3) |
| failed, or a group incomplete | the instance's name and the safe message: "msk: unavailable, the agent has been away for 3 minutes"; never a number for that group |
| matched nothing | "no instance matches `site=kazn`", never an empty chart |

## Laying out frames

What a drawer does with frames is layout, never arithmetic that means
something:

- **The series rule is chapter 3's**, used to lay frames out: one `Time`
  column, each number column a series, every other column a dimension, each
  distinct set of dimensions one series. `time, site, count` from one instance
  draws a line per site.
- **Instances are series too**: a frame from `pg-kazan` names its series by
  the instance's label, so ten frames are ten lines, each on its own times. A
  stacked bar places them on their shared keys and leaves a gap where one site
  had nothing, never sliding its values along.
- **Units are read from the column** (chapter 3): bytes as KiB and MiB, a
  rate per second, money in its currency with its minor units, formatted by
  the browser's `Intl` in the person's language. A chart never puts two units
  on one axis; a second unit is a second axis or a refusal.
- **Times are drawn in the dashboard's zone**, from microseconds.
- **A threshold colours a value the kernel gave**; comparing it with a number
  is the same comparison an alert makes, not a second calculation.

## Built-in visualizations

| Visualization | For |
|---|---|
| table | anything; always offered |
| number | one value, which the query's calculation makes: "show as one number" writes `sum`, `last` or another operation into the panel's query |
| line | values over time |
| bar | values by category, or counts per time bucket; vertical, horizontal, stacked |
| scatter | two numbers against each other |
| status | a state from thresholds: operational, degraded, down |
| donut, funnel | allowed, never suggested (below) |
| text | a note in Markdown, no query |
| button | a button flow's click (chapter 7) |

Every chart offers itself as a table, which is also what a screen reader is
given. Charts are ECharts, tree-shaken to what each uses, chosen in Go Dashbin
for annotations on a time axis and a real click API; whether a line of 50 000
points is fast enough there is not measured yet.

## Suggestions

**After a query runs, the editor suggests how to draw it**, cheapest first:
Go Dashbin's suggestion, ported to TypeScript beside the layout rules, with
its weights. Choosing a way to draw is presentation, so it stays a drawer's.

- **The weights are Draco's**, learned once and published, and only their
  ratios mean anything: copied as a group, ours placed by a stated rule and
  never between theirs, none lifted out of its context — their colour penalty
  is not about a crowded axis, and using it as one sent a good bar chart to a
  table.
- **Some visualizations are allowed rather than argued for.** A funnel says
  its stages nest and a donut says its slices are one whole, and a frame proves
  neither, so they are offered when the shape permits and never chosen.
- **A plugin's visualization is allowed, never argued**: it says which frames
  it accepts, and appears in the list below the built-ins when it does, since
  its cost cannot be weighed against Draco's.
- **The suggestion says why** in a plain line — "these look like counts per
  bucket, which read better as bars" — and is a choice the person makes, not
  one made for them. Go Dashbin's gate carries over: the shapes read off a real
  dashboard must keep their suggestions.

## Thresholds and states

One list of thresholds serves a number, a status, a table's cells and a
line's bands: `below 1 %` ok, `1–5 %` warn, `above 5 %` critical, coloured by
the theme's state tokens. **An alert can be made from a panel**: its query,
its calculation and a threshold become an alert flow (chapter 7) with the
dashboard's variable values as its own parameters and a window the person
picks, so the alert compares the very number the panel shows.

## Interactions

**A click on a chart is an event with its values**: the bar's category, the
point's time and value, the row's columns. A panel maps it to one of:

- **set a variable**: a click on `kazan`'s bar sets `site=kazan`, and every
  panel follows;
- **open a dashboard** with variables and a range from the event:
  `date = {{event.time}}`;
- **press a button**: a button flow's action with params from the event,
  under chapter 7's rules — its id is the operation, its permission is
  `interact`.

No graph of nodes, no code: what needs more is a plugin's action.

## A plugin's visualization

A plugin may carry visualizations in its browser bundle (chapter 6). One is
a function that draws into an element, whenever its frames, options or size
change:

```js
import { visualization, number } from 'dashbin/browser'

export const heatmap = visualization({
  options: { cell: number({ default: 12, title: 'Cell size' }) },
  accepts: ({ frames }) => frames.every((f) => f.time && f.numbers.length === 1),
}, (element, { frames, options, size, zone, emit }) => {
  const canvas = element.querySelector('canvas') ?? element.appendChild(document.createElement('canvas'))
  canvas.width = size.width
  canvas.height = size.height
  // each frame's time and number columns are typed arrays: f.time.values, f.numbers[0].values
  canvas.onclick = (event) => emit('select', { time: 0, value: 0 })   // what was clicked
})
```

- **Its options are declared in code**, with the same builders as a plugin's
  settings, so the panel's inspector builds the form; nothing repeats the code
  in a manifest.
- **The built-ins are drawn through the same contract**: frames, options,
  size, zone and the theme in; `select`, a range and a failure out. A panel
  has one path, whoever drew it.

**A plugin's visualization runs in a sandboxed frame, not in the page.**
JavaScript in the dashboard's page can call the api as whoever is looking at
it — the owner included — so code an admin installed for its pictures would
otherwise hold every viewer's account. So each one runs in an `iframe` with
`sandbox="allow-scripts"` and no same origin: no cookie, no api, and a
content policy that lets it reach nothing on the network. The page posts it
the frames as typed arrays, the options, the size and the theme's tokens as
values; it posts back its events, which the page checks before acting on.
A visualization draws; it does not fetch. What twenty such frames cost a
browser is not measured yet.

## Themes

**Tokens are the design system**, as Go Dashbin's frontend contract has it:
every component reads `--db-*` and nothing else, so a theme, a dashboard's
overrides and a plugin's frame all work by giving tokens values.

- **A theme is data, not CSS**: the default light and dark ship in the
  binary, and a theme is imported and exported as its tokens. The light theme
  is a theme of its own, not an inversion.
- **A dashboard overrides a few tokens** — an accent, a radius — and inherits
  the rest; it never copies a whole theme.
- **The accent is one value**: a chart's palette follows it through
  `color-mix`, so one colour restyles every chart.
- **A token registry is the gate**: a token with no default, a default with no
  token, a preset that leaves out what its siblings set, and a theme that
  invents a token all fail a test, as in Go Dashbin.

## Modes

- **Viewing**: the range, the variables, the panels, the time of the last
  refresh.
- **Editing**: handles to drag and resize, and an inspector on the right —
  data, calculation, visualization, options, thresholds, interactions —
  rather than a dialog per property. Nothing asks through the browser's own
  `confirm` or `prompt`, which some embedded browsers suppress.
- **Focus**: the same dashboard, signed in, with the application's chrome
  hidden, for a big screen; it redraws when the dashboard is saved elsewhere.

## The terminal

**`dashbin show` draws a dashboard in a terminal**, for someone on SSH, a
cron job or a bot that posts a picture to a chat:

```
$ dashbin show revenue --var site=all --range now-7d

Revenue · all sites · 20–27 Oct · Europe/Moscow
┌ Total ──────────┐ ┌ Orders today ───┐
│  1 240 000 ₽    │ │      312        │
│  incomplete: msk│ │                 │
└─────────────────┘ └─────────────────┘
┌ Revenue by day ────────────────────────────────┐
│ 200k ┤        ⣀⡠⠤⠒⠉⠉⠑⠢⢄       kazan            │
│ 100k ┤⣀⠤⠔⠒⠉⠁         ⠈⠑⠤⣀     spb              │
│    0 ┼────────────────────    msk: unavailable  │
└────────────────────────────────────────────────┘
```

- **It is a second drawer of the same frames.** It asks the hub's run
  endpoint as a dashboard does, with `--range` and `--var` as the address
  carries them, and gets the same numbers and the same refusals; it computes
  nothing.
- **It draws once and exits** in the first version; `--json` and `--csv`
  print the frames themselves for a script. `--watch`, full screen and
  refreshing, comes later.
- **Rows follow the terminal's width** by the phone's rule: a row that does
  not fit wraps, a narrow terminal stacks panels.
- **Drawing is ratatui's**: a line in Braille dots, bars, sparklines, tables.
  Braille and block characters pass every terminal and every SSH session;
  real images through Sixel or kitty's protocol, where a terminal has them,
  are later, with the characters as the fallback.
- **A plugin's visualization is shown as its table**, with a note that it
  draws in the browser: no plugin's JavaScript runs in a terminal.
- **It signs in with a personal token** (chapter 10), as any client of the
  api does.
- **What it adds to the binary is measured** before it ships in the one file;
  if it weighs too much, `show` is a build of its own.

## Chats, after the first version

**A chat is a third drawer**, reached through a messenger's plugin — the
official `telegram` first (chapter 6). In the first version the bot only
sends what alerts send. After it, it answers, and what it answers is the
kernel's, the same for any messenger whose plugin turns its updates into
messages:

- **An account is linked once, on purpose.** The account page gives a link,
  `t.me/<bot>?start=<code>`, with a code used once; the bot receives it and
  binds that Telegram user to the account. A person without a link is nobody
  to the hub, and no account is ever made from a chat.
- **Commands read what the browser reads.** `/show revenue site=kazan`
  answers with the panels as a text card — numbers, states, "incomplete: msk"
  — from the kernel's calculations, laid out as the terminal lays out a
  number, and a button that opens the dashboard; `/alerts` lists what is
  firing. A chart in a chat is a picture drawn on the hub, which is later.
- **A button under an alert's message** — silence for an hour, open the
  dashboard — is checked against the linked account's rights, as a button on
  a dashboard is checked for `interact`.
- **A Mini App is this application in Telegram's own browser**, in its phone
  layout. It signs in without a password: Telegram hands it `initData` signed
  with a key made from the bot's token, **the kernel** checks that signature
  with the token it holds, refuses one older than a few minutes, finds the
  account linked to that Telegram user and starts an ordinary session. The
  check is the kernel's and never a plugin's, so that no plugin's code ever
  decides who someone is.
- **A Mini App needs the hub at a public HTTPS address**, which a phone can
  reach; chapter 8's tunnel gives one. A hub reachable only on a LAN keeps the
  bot, its commands and its buttons, and has no Mini App.

## Against four scenarios

### Revenue of ten sites, one switch

The dashboard above. `site` takes its values from the `site` label of the
instances `postgres.sql` reaches, `all` by default. The line draws ten sites
as they answer; `msk`'s agent is away, so nine lines draw and the panel names
the tenth. The total's one group depends on `msk`, and the kernel refuses it;
the card says which site is missing. The owner picks `kazan`: every panel
reruns for one site, the address now says `site=kazan`, and the link sent to a
colleague opens the same. A viewer cannot type `site=secret-prod` into the
address and get anything.

### An alert from the total

The owner makes an alert from the total card with `site=all` and yesterday as
its window. The flow gets its own copy of the card's query and calculation,
so the alert compares exactly the number the card shows; when `msk` is away
the card says "incomplete" and the alert's subject holds its state, then
goes to no data, by the same rule.

### node_exporter, collected

A panel reads the `node` dataset (chapter 7) through `datasets.series` on
`site=$site`, `rate` per step. The editor suggests a line, since a time and a
rate are values over time; the axis says "per second", in the unit the
column carries. Ten minutes the agent was away are a gap in the line, not a
slope across it.

### A heatmap from a plugin, and the same dashboard over SSH

An admin installs a plugin carrying `heatmap`; the panel's suggestion list
shows it under the built-ins, and it draws in its sandboxed frame, never
seeing the viewer's session. Dmitry opens the same dashboard with
`dashbin show` over SSH: the heatmap is a table with its note, the other
panels draw in characters, and the numbers are the browser's.

## Forks

After two reviews:

1. **ECharts for the built-in charts**, measured in Go Dashbin. Kept, unless a
   line of 50 000 points is slow; uPlot, which reads typed arrays natively and
   weighs about a tenth, is the other way.
2. **A plugin's visualization in a sandboxed frame.** Kept.
3. **Drawing rules in the browser** — changed: the kernel computes, a drawer
   lays out. Totals, sums per site, last values and every number an alert
   might also want are the query's calculation (chapter 5); the browser keeps
   the series layout, units, zones, colours and the suggestions.
4. **Every panel's query saved** — changed: a panel's query is its own, inside
   the document, and becomes shared only on purpose, with the rules of the
   table above.
5. **The first version leaves out** comparing with the previous period, a
   formula across reads, a theme editor, presentation links for a TV, folders
   of dashboards, annotations on a time axis. Kept.
6. **A visualization declares its options in code.** Kept.
7. **Refresh stops in a hidden tab, and the address holds the range and the
   variables.** Kept.

Added: a dashboard is one document in one row, with a version checked on
save, its history and its references beside it, and a channel as a nudge;
rows of panels instead of coordinates, for a review to look at; the terminal
as a second drawer, `dashbin show` in the first version and `--watch` later;
a chat as a third, after the first version, with commands, buttons under
alerts and a Mini App whose sign-in the kernel checks.

## What it asks of other chapters

- Chapter 3: the series rule lays out a chart and shapes a dataset; the kernel
  pivots nothing for a chart.
- Chapter 4: dashboards and flows as documents, their versions, histories and
  references.
- Chapter 5: calculations; the terminal as a caller of the run endpoint.
- Chapter 6: a plugin's browser bundle and its visualizations, declared with
  the settings' builders; a messenger's plugin turning its updates into the
  kernel's messages.
- Chapter 7: an alert made from a panel; a button's click with params from an
  event; flows stored on the same machinery.
- Chapter 10: `interact`, who may edit a dashboard, a personal token for the
  terminal, a sandboxed frame as the boundary for plugin code in the browser,
  linking a chat's user to an account, and signing in from a Mini App.
- Chapter 11: a dashboard's document with its version, its history, the
  address's parameters, and the run endpoint `--json` reads.
- Chapter 12: `dashbin show` as a mode of the binary, and its weight.

## Not checked yet

- A line of 50 000 points in ECharts, and the same in uPlot, on the frame
  wire's typed arrays.
- Twenty sandboxed frames on one dashboard: memory, time to first draw, and
  frames posted to them.
- Decoding a refresh of twenty panels on the page's thread, against a worker.
- The dashboard route's weight with the decoder, the grid and the charts.
- What ratatui and its terminal backend add to the binary, and a line of
  50 000 points drawn in Braille.
