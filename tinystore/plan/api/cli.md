# The command line

`tinystore` is the one command a person types: it serves a store's directory
to other processes and machines, shows what a store holds and logs, backs it
up, and runs the nodes of a network. Proposed on 18 October from the Go
release candidates' command, which the owner liked, and from what the Rust
rewrite added since; reviewed by three newcomers the same night
([Rounds](#rounds)), and accepted by the owner, the tagline left out of the
help as too much of an advertisement. [Today](#today) says what is built.

## Principles

- **A command reads as a sentence**: a verb, the directory, then flags —
  `tinystore serve ./data`, `tinystore logs ./data -f`. The directory may come
  after the flags too, or with `--dir`; without one a command that only reads
  uses the current directory. `serve`, `witness`, `stop` and `restore` need
  one. Two paths read from → to: `backup ./data backup.zip`, `restore
  backup.zip ./data2`.
- **A terminal gets colour and words; a pipe gets plain lines; a script asks
  `--json`.** Colour only on a terminal, never under `NO_COLOR` or
  `TERM=dumb`. Scripts read `--json`, never the human lines. `serve`'s and
  `witness`'s lines are their log: pretty on a terminal, JSON anywhere else,
  as a store's logger prints.
- **Help is not an error.** `tinystore` alone and `tinystore help` print the
  commands and exit 0; `tinystore help <command>` and `<command> --help` print
  one command's help, its flags grouped and an example of each use.
- **A mistake names the next command.** One line naming what is wrong, a
  second with the nearest word, the flag that fixes it or the command to run
  first, exit 2. A failure while running is one line too, exit 1.
- **Reading never stops the store.** `status`, `logs`, `kv`, `sql`, `backup`
  and `mcp` read a directory beside the process that serves it
  (`Store::open_beside`), without its lock and without writing a byte, so
  they work on a live system.
- **A dangerous step says what it changes and refuses when unsure.**
  `node promote` refuses while a node it took its files from still serves
  them.
- **Help and the parser read one table**, `help.rs`, so that a flag the help
  names is one the command takes, and a misspelt one is told its nearest.

## Help

```text
$ tinystore
tinystore 0.1.0

Look
  status [dir]                          what the store holds, who serves it, how its node stands
  logs [dir]                            the application's logs, and new ones as they arrive with -f
  kv [dir] get <bucket> <key>           one value; list <bucket> [branch] for a bucket's keys
  sql [dir] <db> "<query>"              a query's rows; reads only unless --write
  search list [dir]                     the store's search indexes; drop <dir> <name> drops one

Run
  serve <dir>                           share the store with programs here and elsewhere, until Ctrl+C
  stop <dir>                            stop the server of a directory, its work finished
  backup <dir> <zip>                    the whole store in one checked zip, while it runs
  restore <zip> <dir>                   a backup into an empty directory
  mcp [dir]                             let an AI agent read the store
  token new [admin|data|read]           a token line for serve's --tokens

Nodes
  node status [dir]                     the node's files, its links and how far behind its copies are
  node promote <dir> [file]             make a copy the home, once its old home is gone
  node demote <dir> [file]              make an old home's files copies again
  witness <dir> --listen <host>:<port>  decide which node writes a file when its home goes quiet

Develop
  migrate <dir> <db> <migrations>       apply a database's migration files
  schema <dir> [db]                     a database's tables, columns and indexes

tinystore help <command> · --version · --json for scripts · docs: tinyshed.org/tinystore
```

## serve

```sh
tinystore serve ./data                       # programs on this machine, until Ctrl+C
tinystore serve ./data --listen tls://0.0.0.0:7443 --tls-cert cert.pem --tls-key key.pem --tokens tokens.txt
tinystore serve ./data --node-listen 0.0.0.0:7444 --from us.internal:7444 --node-config node.toml
```

On a terminal it prints what it serves and how to reach it, the settings it
took, then one line an event, and on Ctrl+C how it stopped:

```text
$ tinystore serve ./data --node-listen 0.0.0.0:7444 --from us.internal:7444 --node-config node.toml

  tinystore 0.1.0 · ./data

  ● local    programs on this machine     pipe tinystore-5434ed53 · found through ./data/server/SERVE
  ○ remote   off                          --listen tls://<host>:<port> serves other machines
  ● node     listen 0.0.0.0:7444          advertised as eu.internal:7444 · TLS
             copies us.internal:7444      kv.db, sql/app.db (node.toml)
                                          failover witness.internal:7445 · synchronous with eu2.internal:7444

  kv 36 KiB · jobs 60 KiB · search 2.1 MiB · sql/app 1.2 MiB
  ready in 14 ms · Ctrl+C stops · tinystore logs ./data -f · tinystore status ./data

11:02:11  node       listening on 0.0.0.0:7444
11:02:11  linked     us.internal:7444
11:02:11  copied     kv.db · caught up
11:02:14  client     tinystore-js/0.1.0 bun/1.4.2 · 1 connected
11:02:40  warn       us.internal:7444 · the node said nothing for too long
11:02:41  linked     us.internal:7444
11:02:58  client     left · tinystore-js/0.1.0 bun/1.4.2 · 0 connected
^C
11:03:02  stopping   interrupted · 0 clients
11:03:02  stopped    in 0.3s
```

With remote serving the second line reads `● remote  tls://0.0.0.0:7443  2 tokens: admin, data`.
A directory that holds no store yet says `new store` beside its name, so that a
misspelt path shows at once. A person's terminal names a known event by a word
and the fields that matter; any other line is its level, message and fields.
Anywhere else, a pipe, a sidecar or a file, each line is one JSON object, its
keys in the console's order: `time`, `level`, `stream`, `msg`, then the
fields. The lines go to stderr, which the protocol leaves alone.

`--listen tls://` refuses to start without `--tls-cert`, `--tls-key` and
`--tokens`; a second `serve` of a held directory refuses, naming the process
that holds it. `serve --help` groups its flags:

| Group  | Flag                           | Meaning                                                                         |
|--------|--------------------------------|---------------------------------------------------------------------------------|
| local  | (on by default)                | a socket or a named pipe that `server/SERVE` names, for programs on the machine |
|        | `--stdio`                      | one client on stdin and stdout, for a program that starts the server itself     |
| remote | `--listen tls://<host>:<port>` | serve other machines over TLS                                                   |
|        | `--tls-cert <pem>`             | the certificate chain it answers with                                           |
|        | `--tls-key <pem>`              | its private key                                                                 |
|        | `--tokens <file>`              | who may connect: `admin`, `data` and `read` tokens, a line each                 |
| node   | `--node-listen <host>:<port>`  | where other nodes copy from this one                                            |
|        | `--from <host>:<port>`...      | the nodes it copies from, tried in order                                        |
|        | `--node-config <file.toml>`    | the rest of what a node is, below                                               |
| limits | `--memory <size>`              | what the engines may hold at once; 1 GiB with `--listen`                        |
|        | `--idle <span>`                | stop once no client has been connected this long; never by default              |
| store  | `--durability full\|os`        | how far a commit goes before it returns; each engine's own by default           |
|        | `--keep-free <size>`           | the disk the store keeps free; 1 GiB by default                                 |
|        | `--encryption-key-file <file>` | the store's key, kept apart from its data                                       |
|        | `--self-metrics`               | measure the store into its own metrics, as `tinystore_` names                   |
|        | `--no-system-metrics`          | with `--self-metrics`, leave out the machine it runs on                         |
| output | `--log-file <file>`            | JSON lines to a file as well                                                    |
|        | `--quiet`                      | the banner, warnings and errors only                                            |

The SDKs pass two more, which help leaves out: `--local` for a sidecar they
started, and `--clock` for a private server's test clock.

A node's rarer settings live in a TOML file, so that a node is one file in
version control, each line with its comment; a path in it is read from the
file's own directory, a setting it does not know is refused with its nearest,
and the banner prints what it took:

```toml
# node.toml
files = ["kv.db", "sql/app.db"]    # what it copies; every file but its own logs when absent
token-file = "/etc/tinystore/node-token"
catalog = false                     # true on the one node that keeps the catalog
advertise = "eu.internal:7444"      # where other nodes reach it; needed when it listens on 0.0.0.0
synchronous-with = ["eu2.internal:7444"]
failover = "witness.internal:7445"  # the witness that judges it
insecure = false                    # plain TCP beyond this machine, which anyone on the way reads

[tls]
cert = "/etc/tinystore/node.pem"
key = "/etc/tinystore/node.key"
roots = "/etc/tinystore/ca.pem"
```

A link beyond loopback with no `[tls]` is refused unless `insecure = true`,
and a node on `0.0.0.0` with a catalog refuses to start without `advertise`,
rather than tell other nodes to reach it at 0.0.0.0. `files` without `--from`
is refused too: it names what a node copies. The SDKs write the node config
they start a sidecar with into `<dir>/server/node.toml`.

## witness

```sh
tinystore witness ./witness --listen 0.0.0.0:7445 --token-file token --tls-cert w.pem --tls-key w.key
```

A witness keeps each file's lease and no data; nodes name it as `failover` in
their `node.toml`. It prints the leases it grants as they change, and
`--insecure` lets nodes beyond this machine reach it over plain TCP.

## status

```text
$ tinystore status ./data
● ./data  served by tinystore 0.1.0 · pid 37420 · up 12m

  kv         36 KiB   home
  jobs       60 KiB   home
  search    2.1 MiB   home
  sql/app   1.2 MiB   copy of us.internal:7444 · at commit 412
  records    24 KiB   this node's own
  ─────────────────
  total     3.4 MiB

  node from us.internal:7444 · 2 nodes copy from it
```

A directory no server holds says `○ ./data  not served`, exit 0; one whose
`SERVE` names a server that does not answer says so. Whether a server serves
is found by reaching it and checking its proof, which the server logs as a
client that came and left. `--json` prints one document of the same.

## logs

```text
$ tinystore logs ./data --since 1h --level warn
11:02:14.502 WARN  api  slow request  requestId=7f3a ms=1200
11:02:15.911 ERROR api  payment failed  orderId=981 reason="card declined"
```

| Flag             | Meaning                                                |
|------------------|--------------------------------------------------------|
| `-f`             | follow new lines as they come                          |
| `-n <count>`     | how many of the last lines to print first              |
| `--level <lvl>`  | `trace`, `debug`, `info`, `warn` or `error`, and above |
| `--since <span>` | lines from the last span, as `1h`                      |
| `--grep <text>`  | lines that hold the text, any case                     |
| `--stream <s>`   | one stream                                             |
| `--json`         | one JSON object a line                                 |

Without `-n` or `--since` it prints the last twenty lines. Following reads the
store again every half second, opened afresh so that it meets streams made
since, from ten seconds before the newest line printed, since another
process's line may commit a second late, and prints each line once.

## kv and sql

```sh
tinystore kv ./data get sessions 42
tinystore kv ./data list sessions 42          # the keys under the branch 42
tinystore sql ./data app "select count(*) from users"
tinystore sql ./data app "delete from users where id = 7" --write   # only when no server holds ./data
```

Both read beside a running server; `sql` refuses a statement that writes
unless `--write`, which needs the store's own lock. A bucket keeps a value of
the type its program chose, an integer as one, text as its bytes, anything
compound as JSON, and nothing says which: `kv` shows JSON spread over lines,
a JSON string or text as it reads, an integer, and other bytes as hex, and an
encrypted bucket's values opened with the store's key. A database the store
does not hold is named with those it does.

## stop, backup, restore, mcp

```sh
tinystore stop ./data
tinystore backup ./data backup.zip            # beside a running server too
tinystore restore backup.zip ./data2          # into an empty directory
tinystore token new admin >> tokens.txt
claude mcp add tinystore -- tinystore mcp ./data
```

`stop` reaches the server `SERVE` names, checks its proof, asks
`server.stop`, and waits until `SERVE` is gone; with no server it says so,
exit 1. `mcp` speaks the Model Context Protocol on stdin and stdout with
read-only tools: `status`, `logs`, `kv_get`, `kv_list`, `jobs_list`,
`sql_query`, `search_list`, a list two hundred items at most. It reads beside
a running server or alone, the store opened again at each call. `jobs_list`
lists a queue's jobs that have ids, by a prefix, or its failed ones, as a
queue's list does. `backup` takes `--include <name>` for a file of the
program's own and `--include-encryption-key`. `token new` prints a token line,
`data` unless told.

## Nodes

```sh
tinystore node status ./data
tinystore node promote ./data --from us.internal:7444 --node-config node.toml
tinystore node demote ./data        # an old home back: its files become copies again
```

`node promote` first asks each node `--from` names, with the token and TLS of
`--node-config`, which files it serves; one that still serves a file being
promoted is its home alive, or a copy of it, and promoting would make a second
home, so it refuses and says so. `--force` promotes anyway, for a home known
to take no more writes. Without `--from` it asks none and says so, since a
stopped node keeps no record of where it took its files from.

## Develop

```sh
tinystore migrate ./data app ./migrations   # apply what is new, checked against what ran
tinystore schema ./data app                  # tables, columns and indexes
```

## Words

| Word    | Meaning                                                                        |
|---------|--------------------------------------------------------------------------------|
| home    | the one node that writes a file; every other node holds a copy                 |
| copy    | a file a node takes from its home, page for page, and reads                    |
| commit  | one write the home made; a copy counts how many it lacks                       |
| stream  | a named series of log lines or events, as `api`                                |
| catalog | the file one node keeps that names each file's home                            |
| witness | a small process that decides which node writes a file once its home goes quiet |

## Exit codes

| Code | Meaning                                       |
|------|-----------------------------------------------|
| 0    | done                                          |
| 1    | it failed while running: the line says why    |
| 2    | the command line was wrong: the line says how |

## Today

Built, on 18 October, everything above: the help and each command's help from
one table; `serve` with its banner, `--node-config`, `--log-file` and
`--quiet`, and a person's lines named by a word; `status`, `logs`, `kv`, `sql`,
`schema` and `search list` beside a running server; `sql --write`, `migrate`
and `search drop` on a directory no one holds; `stop`; `token new`; `mcp`;
`node status`, `node promote` with its check and `node demote`; `witness`;
`backup` beside a server; and in the core `Store::open_beside`,
`replication::served_by`, and the pipe's `Connect::greeted`, told a client's
name once it is welcomed.

## Decided

The owner accepted the proposal on 18 October, and with it what was open:
fourteen `--node-*` flags went, for `--node-listen`, `--from` and
`--node-config`; `promote` and `demote` moved under `node`; and `stop` asks
`server.stop` through a client of the protocol the command writes by hand,
rather than signal the process, which Windows gives no way to do without
`unsafe`.

## Rounds

**18 October, three newcomers** (Haiku, no context: a free review, bare calls
read cold with six tasks, a blind comparison of the doubtful choices). Taken:
`--listen tls://`, `--tls-cert` and `--tls-key` stay, since `tls://` says what
the port speaks and one path a flag cannot be swapped (both who compared);
`--node` was read as a node's name, so it is `--node-listen` (both); a
witness is its own command, verb then directory (both); `promote` alone read
as shipping a build, so it is `node promote`, and it refuses while the old
home answers (both); a node's rare settings go in a file whose settings the
banner prints (the comparison); `kv` and `sql` for a person, reading beside a
live server and `sql` read-only unless `--write` (the comparison); `stop`
stays, since a deploy script cannot press Ctrl+C; `advertise` stays, the word
Consul, Nomad and Kafka use, and a node on 0.0.0.0 refuses without it.
Asked: whether `backup` works while `serve` holds the directory — it will,
beside it; how slow requests show in logs — `--since` and `--level`.
The free review added: what `--listen` does with no certificate (refuses),
`serve` on a misspelt path (its banner says `new store`), `stop` needing its
directory, from → to for two paths, words defined, levels listed, `--grep`'s
matching, `--log-file` rather than `--log`, commits rather than entries, the
banner naming `server/SERVE` and the remote's tokens, lines for a client that
left and a copy caught up, `--stdio`'s banner on stderr, `--version`, a way to
make a token. Kept against it: `home` and `copy` rather than primary and
replica, the owner's words; `failover` and `synchronous-with`, named by the
owner on 18 October; `search list`, built.

## Writing, accepted 26 October

Issue #32: a server with only a shell wanted one ping's round trip a second
from six targets in metrics, kept a month, and the command line could neither
write metrics nor read them. Its author installed Python and a 14 MB wheel,
and wrote a 100-line daemon to ingest and a second script to read back.
Proposed the same day, read by three newcomers and then by GPT
([Rounds](#rounds-26-october)), and accepted by the owner with GPT's
changes. Nothing in this section is built yet; [Phases](#phases) gives the
order.

**It is not a fifth SDK.** The command line takes what a shell needs from
each engine, a write, a read, a list, a delete of one item, and streams; no
workers, transactions, handlers or bulk deletes. It stays a person's tool
first, coloured and aligned on a terminal, and a script's or an agent's
under `--json`.

### Principles it adds

- **One grammar**: `tinystore <thing> <dir> <verb> <name> [args] [flags]`,
  as `kv` reads. The thing is the store's word in the SDKs, `metrics`,
  `events`, `jobs`, `files`, `channels`, with `kv` keeping its released
  name; the verb is the SDK's verb, so a guide's call is the command.
- **A write goes through a server**, the protocol call an SDK makes, with its
  checks and its errors; the command line keeps no write path of its own.
- **A write needs a store**: a directory given explicitly that holds `LOCK`,
  checked before any server starts. The path is resolved once, and that path
  is what a started server is given.
- **Nothing waits unasked**: a command that reads stdin refuses a terminal; a
  command that never ends takes `--for <span>`; no prompt and no pager.
- **stdout is the data.** Everything else goes to stderr, each line starting
  `tinystore <command>:`, and as JSON lines under `--json`, so that
  `--json 2>&1 | jq` holds.
- **The exit code is the truth**, and a reader that closes the pipe, `| head`,
  ends the command quietly: that error alone, not every failed write.
- **Output is bounded, and the data says so**: `--json` is NDJSON whose last
  line is `{"next": "…"}` when more is there, and `--after` reads on.
- **A write prints one line of what it did**, and help says which writes are
  safe to send again.
- **Times in JSON are whole and UTC.** A person's lines are local, and a read
  prints the window it read first, with its zone.

### Commands

```sh
tinystore metrics ./data ingest < scrape.txt                       # whole: every sample or none
ping … | awk '…' | tinystore metrics ./data ingest -f --keep 30d   # a stream, until stdin ends
tinystore metrics ./data names
tinystore metrics ./data read ping_rtt_ms --where target=poland --since 1h
tinystore metrics ./data read ping_rtt_ms --from 14:30 --to 14:35
tinystore metrics ./data aggregate ping_rtt_ms --since 1h --every 10m --op max --by target
tinystore metrics ./data delete ping_rtt_ms target=poland          # one series, by its exact labels

tinystore events ./data append audit role.changed user:=42 role=admin
my-worker 2>&1 | tinystore events ./data append worker            # each line an event
tinystore events ./data list audit --since 1h --where user=42

tinystore kv ./data set flags beta on --ttl 1h
tinystore kv ./data set counters hits 42 --as int
tinystore kv ./data delete sessions 42
tinystore jobs ./data add emails '{"to": "ada@example.com"}' --id welcome-42 --delay 1m
tinystore jobs ./data list emails --state failed
tinystore jobs ./data cancel emails welcome-42
tinystore files ./data put avatars users/42/avatar.png ./avatar.png
tinystore files ./data get avatars users/42/avatar.png ./avatar.png
tinystore files ./data list avatars users/42
tinystore files ./data delete avatars users/42/avatar.png
tinystore channels ./data publish chat-events '{"kind": "reload"}'
tinystore channels ./data subscribe chat-events --for 1m
tinystore sql ./data app "delete from users where id = 7" --write  # through a running server too

tinystore lock ./data hold cron nightly-backup -- ./backup.sh       # later
tinystore config ./data explain chat                                # later
tinystore config ./data update chat model.max_tokens=2048           # later: live settings alone

tinystore completion bash|zsh|fish|powershell
tinystore help --all
tinystore status ./data --json                                      # every name the store holds
```

A person's terminal reads them as lines and tables:

```text
$ tinystore metrics ./data read ping_rtt_ms --where target=poland --from 14:32 --to 14:33
ping_rtt_ms{target="poland"}   26 Oct 14:32–14:33 Europe/Moscow · 46 samples
  14:32:00   23.1
  14:32:01   22.9
  14:32:17   24.0

$ tinystore metrics ./data aggregate ping_rtt_ms --since 1h --every 10m --op max --by target
ping_rtt_ms max every 10m   26 Oct 13:40–14:40 Europe/Moscow
          finland   poland   sweden
  13:40      31.2     24.0     40.1
  13:50      30.8   1012.5     39.7
```

`aggregate` draws buckets as rows and series as columns, and falls back to a
line each when the columns do not fit.

### metrics ingest

Prometheus' text decides its timestamps' unit by its last line, `# EOF` for
OpenMetrics' seconds and anything else for milliseconds, and a text is read
alone, so a stream cut into texts would lose both its unit and its `# TYPE`
lines. Hence two forms:

- `metrics ingest` reads stdin to its end and writes it whole, every sample
  or none, as the SDKs' `ingestPrometheus`: a scrape or a file, in either
  format. It is strict by being whole.
- `metrics ingest -f` follows a stream that does not end, as `logs -f`
  follows lines; Prometheus' text alone. A streaming form of the library's
  own reader keeps `# TYPE`, `# HELP` and `# UNIT` across the stream, gives a
  line without a time the time it was read, and refuses a line alone; the
  command sends the series it hands back with `metrics.ingest` every second,
  or at 10,000 lines. A line it cannot read goes to stderr with its number
  and is skipped, and the command exits 1 at the end if any was. The reader
  is not copied into the command line.
- Without `-f`, stdin still open after 5 seconds says `still reading stdin: a
  stream that does not end needs -f`.
- `--kind ping_lost_total=counter` gives a name its kind where the input has
  no `# TYPE`; a name with neither is a gauge, said once on stderr. `--keep`
  sets the names' keep, 30 days when none is given; `--label k=v` labels
  every series.

What a repeat changes:

| Repeat                                     | What happens                                    |
|--------------------------------------------|-------------------------------------------------|
| a batch sent again after a lost connection | the same times and values: nothing changes      |
| the same stream after the command restarts | safe only when the stream carries its own times |
| a write whose answer was lost              | sent again unchanged, never with new times      |

### Values

Event fields are `k=v` for text and `k:=v` for JSON, as httpie writes them:
nothing is guessed from a value's look, so `zip=02134` stays text. `kv set`
writes text unless `--as int|json`, float and bytes later. `-` reads a value
from stdin, since Windows PowerShell 5.1 drops the quotes inside a JSON
argument. `--id` and `--at` belong to one event given in arguments and are
refused with stdin.

`jobs add` prints the id it was given, and says a job without one cannot be
cancelled, since `add` returns no id. `files get` writes to the path it is
given, never stdout, since PowerShell's `>` re-encodes binary output as text;
an existing file is refused without `--force`, and the file is written beside
and renamed.

### Who writes

| Who holds `./data`                    | A write                                                 |
|---------------------------------------|---------------------------------------------------------|
| no one                                | starts a sidecar, as the SDKs do, and writes through it |
| `tinystore serve` or an SDK's sidecar | finds it through `server/SERVE` and writes through it   |
| a program that opened it embedded     | refuses, exit 1, saying what to do                      |

A sidecar is needed because six `ping | ingest -f` at once would otherwise
lock five of them out. One the command starts says so on stderr, `started a
server for ./data; it stops 30 s after the last client`; several commands
starting one at once leave one, the rest connecting to it; it refuses to start
as a user who does not own the directory, since `sudo … ingest` would leave
files the program cannot open. A host that runs `serve` itself gets a way to
refuse rather than start one, named when built.

A program that holds the directory embedded is refused in words:

```text
tinystore metrics ingest: a program holds ./data inside its own process, so no other process can write to it
  open it in that program with a sidecar, or let `tinystore serve ./data` hold it
```

The core says only `another store holds it` today; telling an embedded
program from a second writer needs `LOCK` to say who holds it. Reading beside
it works as `logs` does, and the handbook's `embedded` says first that
nothing else can write while it runs.

### Completion

`tinystore completion <shell>` prints a few lines that ask a hidden
`tinystore __complete <words>` on each Tab, so the four shells complete alike
from the table help and the parser read: commands, verbs, flags, a flag's
values, and the names a store holds, read beside it, a metric's labels and
their values among them. It creates no store, writes nothing, waits for no
server, and offers nothing when anything fails.

### Agents

`help --all` prints every command with its examples at once, and `status
--json` names everything the store holds, the names completion offers. A flag
is no security boundary for an agent with a shell, so writing through `mcp`
waits for permissions by operation, and comes last.

### Decided

| Question                                   | Decided                                                                |
|--------------------------------------------|------------------------------------------------------------------------|
| who writes when no one holds the directory | a sidecar the command starts, as above                                 |
| an event field's type                      | `k=v` text, `k:=v` JSON                                                |
| a `kv set` value's type                    | text, `--as int\|json`                                                 |
| reading events                             | `events list`; `logs` stays the application's log tail                 |
| `sql --write`                              | kept: released, and an exception to writing by a verb, said as one     |
| names                                      | the SDKs': `names`, `--keep`, `hold`, not `list`, `--retention`, `run` |
| `--strict`                                 | none: `ingest` whole is strict, `ingest -f` skips and says             |
| gaps in a person's `read`                  | only when asked, `--gaps 5s`; the command guesses no step              |
| `TINYSTORE_DIR`                            | none for writes                                                        |
| embedded listening as a sidecar            | no                                                                     |
| `jobs retry`, a publish's listener count   | not added to the engines for the command line                          |
| deleting metrics                           | one series by its exact labels, as the SDK; no `--all`                 |

### Phases

1. A client of the protocol in the command line, built from the library's
   generated messages, which already write and read both ways: connecting to
   a server, starting a sidecar against others starting one, timeouts, a
   connection lost, exit codes, the closed pipe, and a test of a write that
   committed whose answer was lost.
2. Metrics and events: `names`, `read`, `aggregate`, `delete`, `append` and
   `list` first, then `ingest` and `ingest -f`.
3. kv, jobs, files, channels and `sql --write` through a server; completion,
   `help --all`, the names in `status --json`; lock and config; writing
   through `mcp` last, after its permissions.

### Rounds, 26 October

**Three newcomers** (Haiku, no context but the text: a free review, twenty
bare commands read cold, a blind comparison of seven choices). Taken: a time
stamped when read and kept across repeats; the window printed with its zone;
a gauge by default said on stderr; `files get` to a path; `-` for stdin; a
line for each write; an explicit directory for writes; a sidecar that says
it started and checks its user; `k=v` and `k:=v`; `files list` and `delete`,
`metrics delete`, `config explain`, `jobs cancel`; `events list` (two of
three). The least sure readings were `kv set flags beta true` (bucket, key or
value, and what `true` becomes) and `config update chat …` (what `chat` is).
Kept against them: the SDKs' words; a sidecar rather than the command's own
`LOCK` (the blind comparison chose the lock, citing no daemon).

**GPT**, reading the proposal against the code: a stream cut into texts
loses its unit and its kinds (taken: the two forms of `ingest`); a repeat is
safe only within one batch's times (taken: the table); `--kind` by name for a
plain `awk` (taken); a delete of a whole name (answered: a delete is one
series, as the SDK's); a flag is no security boundary for `mcp` (taken:
permissions first); `is_dir` is not a store (taken: `LOCK`, before a server
starts); `jobs add --id` and its id printed, `files get` refusing to
overwrite, the truncation in the data, the closed pipe alone silenced (all
taken); its order of phases (taken). Its advice: do not build a fifth SDK.

While the proposal was written, `tinystore sql … | head -2` on Windows
panicked with exit 101, `failed printing to stdout` on `os error 232`: no
command handles a closed pipe. Linux is expected to do the same, since Rust
ignores SIGPIPE, and is not yet checked.
