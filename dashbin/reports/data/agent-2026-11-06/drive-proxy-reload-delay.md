## connected

- ten agents connected through the proxy (17735 ms after the first look)

## proxy_reload

- reload with the same config, forced: 10 agents disconnected within 5 s; ten again 30363 ms; caddy said "{\"level\":\"info\",\"ts\":1793992625.100782,\"msg\":\"adapted config to JSON\",\"adapter\":\"caddyfile\"}"
- reload with a changed config: 10 agents disconnected within 5 s; ten again 30655 ms; caddy said "{\"level\":\"info\",\"ts\":1793992660.8673337,\"msg\":\"adapted config to JSON\",\"adapter\":\"caddyfile\"}"
- reload with a config that delays closing streams: 10 agents disconnected within 5 s; ten again 30739 ms; caddy said "{\"level\":\"info\",\"ts\":1793992696.9281144,\"msg\":\"adapted config to JSON\",\"adapter\":\"caddyfile\"}"
- reload with that config changed: 0 agents disconnected within 5 s; ten again 1 ms; caddy said "{\"level\":\"info\",\"ts\":1793992733.051793,\"msg\":\"adapted config to JSON\",\"adapter\":\"caddyfile\"}"
- reload with back: 0 agents disconnected within 5 s; ten again 1 ms; caddy said "{\"level\":\"info\",\"ts\":1793992738.431073,\"msg\":\"adapted config to JSON\",\"adapter\":\"caddyfile\"}"
