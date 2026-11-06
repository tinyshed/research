
## overload

- the hub's liveness: messages
- agent5's link to the hub: eth0 at 1 Mbit/s
- at 10 s: site1 answers in 5 ms; site5's small read 504 in 5002 ms: "timeout": "the agent did not answer in time"
- at 20 s: site1 answers in 5 ms; site5's small read 504 in 5001 ms: "timeout": "the agent did not answer in time"
- at 30 s: site1 answers in 5 ms; site5's small read 504 in 5002 ms: "timeout": "the agent did not answer in time"
- at 40 s: site1 answers in 5 ms; site5's small read 504 in 5002 ms: "timeout": "the agent did not answer in time"
- at 45 s: site1 answers in 5 ms; site5's small read 503 in 1 ms: "unavailable": "the agent is away"
- the big read answered 503 after 41568 ms: "unavailable": "the agent went away"
- the hub let "site5" go as "silent" at 41.6 s
- ten agents again 32007 ms
