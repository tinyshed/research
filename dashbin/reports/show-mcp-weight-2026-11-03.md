# What `dashbin show` and `dashbin mcp` add to the binary

**3 November 2026.** Chapter 9 left open what drawing a dashboard in a
terminal would add to the one file, and chapter 11 what an MCP server would.
Measured before slice 4 is written, with [show-probe](../show-probe/): each
variant beside a plain one that prints alike.

| Variant        | What it links                                                                                                       | Bytes     |
|----------------|---------------------------------------------------------------------------------------------------------------------|-----------|
| `plain`        | a number, a table and a line printed with `format!`                                                                 | 414 392   |
| `with_ratatui` | the same drawn by `ratatui-core` 0.1.2 and `ratatui-widgets` 0.3.2 into a buffer, `terminal_size` 0.4 for the width | 672 504   |
| `config_only`  | TinyStore 0.6.0's `config-load`, one setting declared as data                                                       | 1 401 560 |
| `with_mcp`     | the same, and `tinystore::mcp` serving a tool that reads and one that writes on stdin                               | 1 450 776 |

Built with `weigh.sh` in `dashbin-musl:1.99.1` (`tools/musl.Dockerfile` of
dashbin's `rust` branch at `7a1ba63`): `x86_64-unknown-linux-musl`, Rust
1.99.1, release with `lto = "fat"`, `codegen-units = 1`, `strip = true`.

- **ratatui adds 258 112 bytes, 252 KiB**: a bordered number, a Braille line
  with its axis and legend, bars, a table. No backend is linked: the widgets
  draw into a `Buffer` that is printed cell by cell in the terminal's sixteen
  colours, so `crossterm` waits for `--watch`.
- **TinyStore's MCP server adds 49 216 bytes, 48 KiB**, beside a config it
  shares its JSON with.
- Both are measured alone; inside dashbin, which already links `serde_json`
  and Unicode tables, each costs at most this. Dashbin's binary is
  7 016 400 bytes at `7a1ba63`, so the two together are about 4 % of it.

**What it decides.** `dashbin show` draws with ratatui's widgets, not a
drawer of Dashbin's own; `dashbin mcp` is TinyStore's server, not a JSON-RPC
of Dashbin's own. Neither becomes a build of its own.
