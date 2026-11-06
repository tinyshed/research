
## connected

- ten agents connected through the proxy (2 ms after the first look)

## round_trip

- site1: median 3 ms of 30, the last 200 null rows
- site2: median 4 ms of 30, the last 200 null rows
- site3: median 3 ms of 30, the last 200 null rows
- site4: median 3 ms of 30, the last 200 null rows
- site5: median 3 ms of 30, the last 200 null rows
- site6: median 2 ms of 30, the last 200 null rows
- site7: median 2 ms of 30, the last 200 null rows
- site8: median 2 ms of 30, the last 200 null rows
- site9: median 2 ms of 30, the last 200 null rows
- site10: median 2 ms of 30, the last 200 null rows
- every site: median 3 ms of 300
- the same read from the host straight to postgres: median 3 ms of 30

## wrong_token

- token-site1 saying it is site2: closed: this token is site1's, and the agent says it is site2
- token-nobody saying it is site1: refused before the socket: HTTP error: 401 Unauthorized
- still connected: 10 agents, site1 and site2 among them: true

## cancel

- site1, postgres: 1 running while asked; ended 4 ms after the client left
- site6, mysql: 1 running while asked; ended 4 ms after the client left
- site9, mariadb: 1 running while asked; ended 4 ms after the client left

## lost_hub

- running before the cut: 1
- cut: 
- the statement ended 27031 ms after the cut
- the read answered 503 after 42613 ms in all: "unavailable": "the agent went away"
- the hub let site2 go as "silent" at 41.8 s after the cut
- mended: 
- site2 back 3058 ms after the mend, 45373 ms after the cut
- running after the reconnect, a call sent again would show: 0

## overload

- the hub's liveness: bytes
- agent5's link to the hub: eth0 at 1 Mbit/s
- at 10 s: site1 answers in 5 ms; site5's small read 504 in 5003 ms: "timeout": "the agent did not answer in time"
- at 20 s: site1 answers in 5 ms; site5's small read 504 in 5002 ms: "timeout": "the agent did not answer in time"
- at 30 s: site1 answers in 5 ms; site5's small read 504 in 5002 ms: "timeout": "the agent did not answer in time"
- at 40 s: site1 answers in 5 ms; site5's small read 504 in 5002 ms: "timeout": "the agent did not answer in time"
- at 50 s: site1 answers in 5 ms; site5's small read 504 in 5002 ms: "timeout": "the agent did not answer in time"
- at 60 s: site1 answers in 4 ms; site5's small read 504 in 5001 ms: "timeout": "the agent did not answer in time"
- at 70 s: site1 answers in 5 ms; site5's small read 504 in 5003 ms: "timeout": "the agent did not answer in time"
- at 80 s: site1 answers in 5 ms; site5's small read 504 in 5002 ms: "timeout": "the agent did not answer in time"
- at 85 s: site1 answers in 4 ms; site5's small read 200 in 6 ms: null rows
- the big read answered 200 after 84585 ms: null rows
- ten agents again 1 ms

## memory

- agent1 before: 4780 KiB resident, 4780 KiB at its peak so far
  - 200 in 56 ms: null rows
  - 200 in 26 ms: null rows
  - 200 in 61 ms: null rows
  - 200 in 61 ms: null rows
- agent1 after four reads of 10 000 rows at once: 5388 KiB resident, 11700 KiB at its peak
- agent6 before: 4580 KiB resident, 4580 KiB at its peak so far
  - 200 in 74 ms: null rows
  - 200 in 26 ms: null rows
  - 200 in 74 ms: null rows
  - 200 in 68 ms: null rows
- agent6 after four reads of 10 000 rows at once: 5248 KiB resident, 12408 KiB at its peak

## hub_restart

- restart: agent-probe-hub-1
- ten agents again 770 ms after the restart began
- each agent's return, seconds after the restart began: 1.14, 1.16, 1.17, 1.20, 1.22, 1.23, 1.26, 1.27, 1.29, 1.30

## proxy_reload

- reload with the same config, forced: 10 agents disconnected within 5 s; ten again 1 ms; caddy said "{\"level\":\"info\",\"ts\":1793992543.0580988,\"msg\":\"adapted config to JSON\",\"adapter\":\"caddyfile\"}"
- reload with a changed config: 10 agents disconnected within 5 s; ten again 1 ms; caddy said "{\"level\":\"info\",\"ts\":1793992548.4454215,\"msg\":\"adapted config to JSON\",\"adapter\":\"caddyfile\"}"
- reload with back: 10 agents disconnected within 5 s; ten again 4339 ms; caddy said "{\"level\":\"info\",\"ts\":1793992553.8498664,\"msg\":\"adapted config to JSON\",\"adapter\":\"caddyfile\"}"
