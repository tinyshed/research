# latency on MariaDb, 100 runs each, median and 90th percentile

- connect and authenticate: (1.4876ms, 1.7712ms)
- a read whole, (b) prepared statement: (4.4463ms, 4.8754ms)
- a read whole, (a) prepared SET, statement as text: (4.9184ms, 5.3065ms)
- the statement alone on an open session, (b) prep + exec: (1.5759ms, 1.843ms)
- the statement alone on an open session, (a) set + text: (1.9564ms, 2.1582ms)
- the statement alone on an open session, text, values written in: (1.1063ms, 1.2752ms)
