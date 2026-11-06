# latency on MySql, 100 runs each, median and 90th percentile

- connect and authenticate: (1.5752ms, 1.9366ms)
- a read whole, (b) prepared statement: (4.7293ms, 5.1828ms)
- a read whole, (a) prepared SET, statement as text: (5.5639ms, 5.9912ms)
- the statement alone on an open session, (b) prep + exec: (1.5599ms, 1.7889ms)
- the statement alone on an open session, (a) set + text: (2.7848ms, 3.1209ms)
- the statement alone on an open session, text, values written in: (1.1362ms, 1.4185ms)
