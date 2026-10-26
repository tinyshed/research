# 1. Product and promises

**Status: partly decided, 27 October 2026.** The two rules below are
decided. What Dashbin is, for whom, and what it is not, are open.

## Memory: 64 MiB

**Dashbin without third-party plugins fits a 64 MiB container**, the hub
with its five TinyStore engines under load. Measured on a prototype
([memory and allocators](../reports/memory-and-allocators-2026-10-24.md)):
9.4 MiB idle, about 22 after heavy load and flat from there; in a 32 MiB
container the default jemalloc was killed and the tuned one lived at the
limit, so the promise is 64, and 32 is not promised before signing in with
Argon2 is measured. JavaScript plugins run in the same process and are
counted against a budget the admin sets (chapter 6); a process plugin's
memory is its own, shown beside the hub's.

## Choosing technology

**The newest that passes through what the user already has, and whose gain
is measured.** Each part is a test a choice has to pass:

- **The newest.** Of two that do the same job, the newer standard: Rust 2024,
  rustls, HTTP/2 to the browser, jiff's time zones, the web's own `fetch`
  and `Request` inside plugins (chapter 6).
- **That passes through what the user already has.** A choice that needs
  something the user may not have — an open UDP port, a second port, a proxy
  that understands it, a runtime installed beside Dashbin — loses to one that
  does not. The agent's WebSocket is the example: QUIC and WebTransport are
  newer, and Caddy, nginx and Cloudflare Tunnel do not pass them to a hub
  (chapter 8).
- **Whose gain is measured.** A dependency is weighed in the binary and in
  memory against what it buys: Arrow lost to our own frame wire on weight
  (chapter 3), mimalloc lost on memory
  ([memory and allocators](../reports/memory-and-allocators-2026-10-24.md)).

"Newest" alone is not a reason, and neither is a habit: the architecture is
as plain as it can be, and a new technology earns its place in it the same
way an old one does.

## Open

- What Dashbin is and for whom: one place for business data and monitoring,
  the first user's ten sites, and the self-hosted operator.
- What it is not.
