# The key model on fixed features, one protocol and a held-out set, 7 November 2026

Round 1s, as [its specification](../round-1s/README.md) sets it: round 1p's
widened key model `both` ([report](keys-external-2026-11-01.md)) fitted again
with its letter trigrams in a fixed order, measured where rounds 1p to 1r
measured it beside the saved model scored both ways, read by a reader written
in Rust and timed in the Linux container, the language models asked again
under one protocol, and the frozen model evaluated once on a public set no
round had read. Its harness is at research commit `6807b66`; the
specification was written before any fit, each later addition marked there
with when it was made. Raw output in
[data/keys-clean-2026-11-07/](data/keys-clean-2026-11-07/).

Every F1 below is over every one-column key the databases declare, as round
1p counted it against Gemini, unless it says *over candidates*. Every
interval is a 95 % percentile interval of 1 000 resamples of databases
(external, held-out) or of repositories (SchemaPile-Perm); one of a
difference resamples both arms together.

| Question | Answer | What follows |
|---|---|---|
| Was the trigram order a real defect? | Yes. Built twice in process order, validation's features differ on 99 327 of 674 751 pairs; with the trigrams sorted, on none. The order can move 15 % of training pairs and 31 % of the external databases' pairs. Round 1p's saved `both`, scored under five hash seeds, reads **0.731 to 0.737** externally; the 0.736 the paper carries is one draw | the paper's figures move to the refit's |
| Does the refit change the model's numbers? | No more than the order did. The refit at round 1p's seed: externally **0.731** [0.657, 0.806] against 0.734 for round 1p's `both` scored here under hash seed 0, −0.004 [−0.015, +0.006]; on SchemaPile-Perm's test 0.736 against 0.735, +0.001 [−0.003, +0.005]; the pair task 0.913 / 0.898 / 0.886, as before | the paper's model is the refit; its numbers barely move |
| How far does a refit move? | Seeds 1 and 2: 0.728 and **0.678** externally. Seed 2 draws 655 false keys on CTU's `Bench`, against 98: without that one database the three seeds read 0.767–0.774, and their macro F1 0.803–0.811 | the pooled F1 is one database's hostage; report the macro F1 and `Bench` apart |
| Does the widening still beat the base? | On SchemaPile-Perm's test, yes: 0.736 against the refit base's 0.714, +0.022 [+0.012, +0.031]. **Off SchemaPile, no**: 0.731 against 0.738, −0.008 [−0.020, +0.004]; on CTU the base is ahead, −0.029 [−0.053, −0.007]. The pair task still separates them, 0.913 / 0.898 / 0.886 against 0.854 / 0.845 / 0.783 | the paper's claim that the widening adds off SchemaPile does not stand in the open setting |
| Does a reader in Rust compute what the libraries do? | Yes. On 712 774 external and 1 101 364 test pairs, no pair's hashed features differ; margins within 1.9e-6 (first stage) and, on one pair in each set, 0.12 and 0.16 (second); the same 2 534 and 13 904 keys drawn | the model is a file and 1 600 lines of Rust with two dependencies |
| How fast and how small is it? | In the Linux container, one thread: a median of **1.5 ms** a database (99th percentile 137 ms), about 48 µs a candidate pair, the model loaded in 6.2 ms; **5.2 MiB** resident for the median database. CTU's `Bench`, 622 512 pairs, takes 26 s and 298 MiB | small and fast for a schema of tens of tables; the largest needs the first stage scored as pairs are built |
| Under one protocol, how far are the language models? | Every prompt answered by every model: **0.834 to 0.890** against the refit's 0.731 (Gemini 3 Flash 0.834, 3.5 Flash-Lite 0.859, Haiku 5.5 0.873, 3.8 Flash 0.890); the refit minus each, −0.103 to −0.160, every interval below zero. The gap is recall, −0.196 to −0.275; precision differs by −0.035 to +0.019, three of four intervals across zero | the band holds under one protocol; Haiku through the API reads 0.873 where the agents read 0.888 |
| What do they cost? | At list price, a whole-schema prompt costs a median $0.0003 (Haiku 5.5) to $0.0027 (Gemini 3.8 Flash) and takes a median 0.95 to 5.3 s through the proxy; all 398 cost $0.30 to $3.27. All of this round's Haiku requests cost $0.95 | cheap enough that cost is not the small model's case; reading no network is |
| The cascade with the models round 1q named? | On round 1q's prompts, 0.751 to 0.792 against its first pass's 0.732; on the refit's, 0.739 to 0.774 against 0.731. Asking about the whole schema is ahead of every cascade by +0.08 to +0.12, each interval above zero | round 1q's reading stands: a cascade is bounded by its first pass |
| **On a set nothing read, once?** | LiveSQLBench-Base-Full-v1, 22 PostgreSQL databases built in 2025: **F1 0.185** [0.095, 0.280], precision 0.681, recall 0.107; the pair task 0.667 [0.552, 0.775], the name score alone 0.698. Its candidates hold 272 of the 299 keys | the model does not carry to a set named in another house style; the paper must say so first |
| And the language models on it, asked after that reading? | **0.896 to 0.924**, above their own 0.834–0.890 on Spider, BIRD and CTU; the frozen model minus each, −0.71 to −0.74, recall −0.76 to −0.80. In the pair task, which shows two tables' names only, the two fast Gemini models are where the frozen model is, 0.660 and 0.670 against 0.667; Haiku 0.789, Gemini 3.8 Flash 0.881 | the set is hard for a model of names and easy for a model given the whole schema |

## 1. The defect

`keys.py` and `widen.py` keep `list(sg)[:12]` of a stem's set of letter
trigrams. A set of strings iterates in an order Python's per-process hash
seed decides, so for a stem with more than twelve trigrams the twelve kept
differ from one process to the next; the training features are built by a
pool of fifteen workers, each with its own seed. `letters.py` makes the set
iterate sorted, so the line keeps the twelve smallest by code point, and
every process that builds features imports it; round 1b's and 1p's files are
not edited. ([check.json](data/keys-clean-2026-11-07/check.json))

| | candidate pairs | whose stem has over twelve trigrams | columns | whose stem has over twelve |
|---|---|---|---|---|
| training | 6 206 601 | 956 307 (15.4 %) | 330 392 | 44 195 (13.4 %) |
| validation | 674 751 | 101 551 (15.0 %) | 39 504 | 5 519 (14.0 %) |
| Spider, BIRD and CTU | 712 774 | 219 645 (30.8 %) | 15 644 | 2 948 (18.8 %) |

Validation's features built twice, each time by a pool of new workers: in
process order 99 327 pairs differ between the two builds; sorted, none.

**There was no Rust port.** Round 1p's report says a Rust port takes the
trigrams sorted; no Rust code of the key model was in this repository or in
Dashbin's when this round began. Round 1p's check of the packed file ran in
Python.

## 2. The refit

Four fits as round 1p fitted `both` — five folds by repository, the first
stage XGBoost on the graphics card, 829 trees, the second LightGBM, 400 —
with the trigrams sorted; seeds 1 and 2 set as `seeds.py` set them.
On validation (6 781 pairs of tables in the pair task, 7 633 declared keys):

| | threshold | F1 over candidates | over every declared key, P / R / F1 | the pair task, column and referred column | column only, as round 1p |
|---|---|---|---|---|---|
| **refit `both`, 20261027** | 0.837 | 0.739 | 0.729 / 0.658 / 0.692 | 0.853 | 0.855 |
| refit `both`, seed 1 | 0.856 | 0.740 | 0.736 / 0.654 / 0.692 | 0.853 | 0.854 |
| refit `both`, seed 2 | 0.806 | 0.742 | 0.713 / 0.678 / 0.695 | 0.852 | 0.854 |
| refit base, 20261027 | 0.775 | 0.763 | 0.716 / 0.670 / 0.692 | 0.794 | 0.795 |
| round 1p's `both`, process order, hash seed 0 | 0.809 | 0.738 | 0.710 / 0.673 / 0.691 | 0.852 | 0.853 |
| round 1b's model, process order, hash seed 0 | 0.832 | 0.764 | 0.743 / 0.645 / 0.691 | 0.796 | 0.797 |

Round 1p's `both` under hash seeds 0 to 4 reads 0.689 to 0.691 over every
declared validation key. The refit at 20261027 replaces it in the paper, as
the specification fixed before it was fitted.

## 3. SchemaPile-Perm's test

2 115 schemas declaring a key, 13 455 keys, 12 013 pairs in the pair task;
the hard cases 753 keys. ([scored-*.json](data/keys-clean-2026-11-07/))

| | over candidates, model / rules [difference] | novel | complete | over every declared key | the pair task / covered | first stage alone | name score alone | hard cases (rules 0.544) |
|---|---|---|---|---|---|---|---|---|
| **refit `both`** | 0.764 / 0.420 [+0.306, +0.383] | 0.747 | 0.806 | **0.736** | **0.905** / 0.929 | 0.733 | 0.442 | 0.871 |
| refit base | 0.767 / 0.442 [+0.284, +0.365] | 0.751 | 0.813 | 0.714 | 0.840 / 0.871 | 0.716 | 0.553 | 0.881 |
| round 1p's `both`, process order | 0.763 / 0.420 | 0.745 | 0.809 | 0.735 | 0.907 / 0.929 | 0.729 | 0.442 | 0.871 |
| round 1p's `both`, sorted | 0.762 / 0.420 | 0.745 | 0.809 | 0.735 | 0.907 / 0.929 | 0.731 | 0.442 | 0.871 |
| round 1b's model, process order | 0.775 / 0.442 | 0.755 | 0.820 | 0.720 | 0.839 / 0.871 | 0.718 | 0.553 | 0.874 |

Over every declared key, resampling repositories: refit `both` minus round
1p's `both` +0.001 [−0.003, +0.005]; refit `both` minus refit base +0.022
[+0.012, +0.031]; refit base minus round 1b's model −0.006 [−0.012, 0.000];
round 1p's `both` sorted minus in process order 0.000 [−0.002, +0.001]. The
first stage alone and the name score alone are each at their own validation
threshold. The order matters little here: SchemaPile's names are shorter.

## 4. Spider, BIRD and CTU

398 databases declaring a key, 3 371 keys.
([summary.json](data/keys-clean-2026-11-07/summary.json))

| | P / R / F1 [95 %] | Spider | BIRD | CTU | macro F1 | without `Bench` |
|---|---|---|---|---|---|---|
| round 1p's `both`, process order, hash seed 0 | 0.844 / 0.650 / 0.734 [0.655, 0.816] | 0.871 | 0.837 | 0.619 | 0.810 | 0.777 |
| the same, hash seeds 0 to 4 | F1 0.731 to 0.737 | | | | | |
| round 1p's `both`, sorted | 0.844 / 0.645 / 0.731 [0.652, 0.814] | 0.872 | 0.836 | 0.612 | | |
| **refit `both`, 20261027** | 0.851 / 0.640 / **0.731** [0.657, 0.806] | 0.869 | 0.830 | 0.609 | 0.804 | 0.767 |
| refit `both`, seed 1 | 0.850 / 0.637 / 0.728 [0.641, 0.813] | 0.872 | 0.825 | 0.604 | 0.804 | 0.774 |
| refit `both`, seed 2 | 0.698 / 0.659 / 0.678 [0.547, 0.813] | 0.871 | 0.831 | 0.534 | 0.811 | 0.774 |
| round 1b's model, process order | 0.848 / 0.627 / 0.721 [0.624, 0.810] | 0.859 | 0.831 | 0.599 | | |
| refit base | 0.849 / 0.653 / 0.738 [0.665, 0.812] | 0.855 | 0.826 | 0.638 | 0.798 | 0.775 |

| difference in F1 | Spider | BIRD | CTU | all |
|---|---|---|---|---|
| refit `both` minus round 1p's, process order | | | | −0.004 [−0.015, +0.006] |
| refit `both` minus round 1p's, sorted | | | | −0.001 [−0.013, +0.009] |
| refit `both` minus refit base | +0.014 [0.000, +0.031] | +0.004 [−0.012, +0.023] | −0.029 [−0.053, −0.007] | −0.008 [−0.020, +0.004] |
| refit base minus round 1b's model | | | | +0.017 [−0.003, +0.042] |

**`Bench` decides the pooled figure.** CTU's `Bench` holds 678 tables and
362 of the 3 371 keys; seed 2 draws 655 false keys there against 98 for the
fit at 20261027, and no other database moves by more than 8. The pooled F1
of the three seeds spreads over 0.678–0.731, without `Bench` over
0.767–0.774.

**Over candidates, against the rules**: refit `both` 0.755 against 0.521,
[+0.188, +0.279], over 3 177 keys in 397 databases (Spider 0.901, BIRD 0.852,
CTU 0.630); refit base 0.793 against 0.547, [+0.200, +0.294], over its 2 959.

**The pair task**, recall at one, the whole database:

| | Spider (768) | BIRD (381) | CTU (1 267) |
|---|---|---|---|
| refit `both` | 0.913 [0.870, 0.951] | 0.898 [0.814, 0.955] | 0.886 [0.815, 0.954] |
| refit `both`, two tables only | 0.913 | 0.882 | 0.890 |
| the name score alone, its candidates | 0.876 | 0.808 | 0.835 |
| refit `both`, seeds 1 / 2 | 0.915 / 0.911 | 0.895 / 0.903 | 0.892 / 0.890 |
| refit base | 0.854 [0.792, 0.907] | 0.845 [0.763, 0.907] | 0.783 [0.685, 0.858] |
| round 1p's `both`, process order | 0.913 | 0.898 | 0.886 |

**The threshold** (F1 at 0.05 to 0.95): the refit's best is 0.732 at 0.80,
its own 0.837 gives 0.731; at 0.05 recall is 0.768. **Related or not**, on
round 1p's 2 416 related and 2 197 unrelated pairs: AUC 0.931, 3.6 % of
unrelated pairs accepted at 80 % of related; the refit base 0.888 and 8.1 %;
T5, round 1p's, 0.798.

## The reader in Rust

`round-1s/reader/`: 1 600 lines, `serde` and `serde_json` its only
dependencies. It reads `keys.bin` as round 1p's `pack.py` writes it (the
refit's: 960 819 bytes, SHA-256 `56a7719b…ba9`) and a schema as a line of
`hidden.jsonl`, and computes the targets, candidates and features of
`widen.py`'s `both`, hashes them as scikit-learn's `FeatureHasher`, walks the
first stage over the present features, computes the graph stage's nineteen
features in NumPy's precision there (the first stage's scores f32 and their
sums f32, as NumPy 2 promotes them), walks the second stage, and keeps one
target a column.

**Checked three ways.** Its first, line-for-line path writes every pair's
features by name: on eight databases (2 243 pairs, CTU's Mooney_Family and
tpce among them) they equal `widen.py`'s, name for name and value for value.
`reader selfcheck` compares its fast path with that path on every one of
round 1p's 428 databases: no pair, graph feature or key differs. Against the
libraries ([parity.json](data/keys-clean-2026-11-07/parity.json)):

| | pairs | hashed features differing | first stage, largest margin difference | second stage, largest; pairs over 1e-4 | keys drawn, Python / Rust; differing |
|---|---|---|---|---|---|
| Spider, BIRD and CTU | 712 774 | 0 | 1.9e-6 | 0.119; 1 | 2 534 / 2 534; 0 |
| SchemaPile-Perm's test | 1 101 364 | 0 | 1.9e-6 | 0.163; 1 | 13 904 / 13 904; 0 |

**Time and memory**, in the Linux container (`rust:1.99.1`, digest
`09027f87…0137`, Linux 6.18.33.2 on WSL2, one processor of the Ryzen 7 7700
by `--cpuset-cpus`), a release build, one thread, 17:43–17:56 UTC with no
other process of this round running; the time is from a parsed schema to its
keys, three warm runs and the median of 21, five after one for a schema
slower than a second (`Bench` and `Bench5`) and for the test split's.
([rust-bench-external.json](data/keys-clean-2026-11-07/rust-bench-external.json),
[rust-bench-test.json](data/keys-clean-2026-11-07/rust-bench-test.json))

| | schemas | median | 90th | 99th | largest | a pair, median | all, one after another |
|---|---|---|---|---|---|---|---|
| Spider, BIRD and CTU | 428 | 1.5 ms | 17 ms | 137 ms | 26.1 s (`Bench`: 678 tables, 622 512 pairs) | 48 µs | 30.4 s; 4.3 s without `Bench` |
| SchemaPile-Perm's test | 4 320 | 0.8 ms | 27 ms | 370 ms | 1.29 s (55 tables, 30 085 pairs) | 48 µs | 78 s |

| | |
|---|---|
| loading `keys.bin`, 960 819 bytes | 6.2 ms |
| reading a schema's JSON | median 5 µs; `Bench`'s 1.2 ms |
| peak resident set: the process alone; the model loaded | 2.5 MiB; 4.6 MiB |
| the same, the median database by pairs (CTU's Walmart, 30 pairs) | 5.2 MiB |
| the same, `Bench` | 298 MiB |
| the binary; stripped | 888 664 bytes; 752 184 |

A pair costs about 48 µs, nearly all of it the first stage's 829 trees
(`Bench`: features 0.6 s, the first stage 24 s, the graph stage 0.1 s, the
second 1.7 s, on Windows). The reader holds every pair's features until the
first stage has scored them all, which is what `Bench`'s 298 MiB is; scoring
each column's pairs as they are built would hold one column's. A dense row
that marked absent features by NaN was four times slower on this processor
than marking present ones by a generation.

## 5. The language models, one protocol

| | identifier | served as | reasoning, by the response's count | asked through |
|---|---|---|---|---|
| Gemini 3.5 Flash-Lite | `gemini-3.5-flash-lite` | `gemini-3.5-flash-lite` | none counted, in any request | CLIProxyAPI v8.0.28 (`0d18d3b`), Antigravity |
| Gemini 3 Flash | `gemini-3-flash` | `gemini-3-flash` | none counted, in any request | the same |
| Gemini 3.8 Flash | `gemini-3.8-flash-high` | `gemini-3.8-flash-n` | in every request; median 500 tokens a schema prompt, 287 a pair prompt | the same |
| Claude Haiku 5.5 | `claude-haiku-5-5` | `claude-haiku-5-5`, every reply | adaptive at its default effort; in 395 of 398 schema prompts, median 248 tokens | Anthropic's Messages API, SDK 2.6.0 |

Round 1p's and round 1q's prompts byte for byte (their SHA-256 in
[prompt-hashes.txt](data/keys-clean-2026-11-07/prompt-hashes.txt)); one user
message, no system prompt, temperature 0, at most 32 768 output tokens, four
requests in flight; 7 November 2026. **Claude Haiku 5.5 refuses any
temperature but its default**, so it was asked with none. **The reasoning
setting**: the proxy at v8.0.28 sets one only from a suffix in parentheses or
from the request's own fields (`internal/thinking/apply.go`, `applyThinking`:
"no config found, passthrough"), and these requests carry neither, so each
identifier runs at Antigravity's default for it, which the counts above show.
The proxy refused part of Gemini 3.5 Flash-Lite's and Gemini 3 Flash's pair
prompts with 503s and 429s, answered on later runs at two in flight; the API
refused 216 of Haiku's requests at four in flight with 403 "Request not
allowed", all answered at one in flight. **Nothing is left unanswered**: 398
whole-schema prompts, 2 636 pair prompts and 207 + 229 cascade prompts for
each model; Gemini 3 Flash's pair task is reported for the first time.

**Over every declared key**, 398 databases:

| | P / R / F1 [95 %] | Spider | BIRD | CTU | macro | novel | refit minus it: F1 | precision | recall | earlier run |
|---|---|---|---|---|---|---|---|---|---|---|
| refit `both` | 0.851 / 0.640 / 0.731 | 0.869 | 0.830 | 0.609 | 0.804 | 0.708 | | | | |
| Gemini 3 Flash | 0.832 / 0.836 / 0.834 [0.722, 0.913] | 0.913 | 0.858 | 0.783 | 0.905 | 0.824 | −0.103 [−0.178, −0.033] | +0.019 [−0.022, +0.063] | −0.196 [−0.322, −0.075] | 0.846 |
| Gemini 3.5 Flash-Lite | 0.875 / 0.843 / 0.859 [0.761, 0.926] | 0.912 | 0.889 | 0.819 | 0.911 | 0.855 | −0.128 [−0.193, −0.069] | −0.024 [−0.083, +0.035] | −0.204 [−0.329, −0.080] | 0.844 |
| Claude Haiku 5.5 | 0.835 / 0.915 / 0.873 [0.808, 0.924] | 0.916 | 0.894 | 0.844 | 0.916 | 0.869 | −0.142 [−0.204, −0.091] | +0.017 [−0.021, +0.057] | −0.275 [−0.357, −0.207] | 0.888, as agents |
| Gemini 3.8 Flash | 0.886 / 0.894 / 0.890 [0.806, 0.949] | 0.926 | 0.917 | 0.862 | 0.934 | 0.885 | −0.160 [−0.221, −0.107] | −0.035 [−0.067, −0.003] | −0.254 [−0.353, −0.171] | 0.891 |

The earlier runs are round 1p's and 1q's replies, read by the same code.
Asked again at temperature 0, Gemini 3 Flash moves by −0.012 and Flash-Lite
by +0.015: the band's order is not stable to a rerun at that resolution.
Through the API Haiku 5.5 reads 0.873 against the agents' 0.888, recall
0.915 against 0.942.

**The pair task**, 2 416 pairs, recall at one:

| | Spider (768) | BIRD (381) | CTU (1 267) |
|---|---|---|---|
| refit `both` | 0.913 [0.870, 0.951] | 0.898 [0.814, 0.955] | 0.886 [0.815, 0.954] |
| Gemini 3 Flash | 0.953 [0.929, 0.973] | 0.900 [0.843, 0.942] | 0.928 [0.891, 0.950] |
| Gemini 3.5 Flash-Lite | 0.952 [0.926, 0.975] | 0.908 [0.855, 0.947] | 0.933 [0.898, 0.955] |
| Claude Haiku 5.5 | 0.978 [0.958, 0.993] | 0.948 [0.912, 0.974] | 0.964 [0.930, 0.986] |
| Gemini 3.8 Flash | 0.988 [0.977, 0.997] | 0.950 [0.910, 0.979] | 0.987 [0.977, 0.995] |

**The cascades**, round 1q's band — drawn at or above the first pass's
threshold, asked in [0.2, threshold) — with the models its specification
named and Haiku:

| first pass | its F1 alone | Gemini 3 Flash | Gemini 3.5 Flash-Lite | Claude Haiku 5.5 | Gemini 3.8 Flash |
|---|---|---|---|---|---|
| `both, roles`, round 1q's prompts (2 040 questions, 207 databases) | 0.732 | 0.751 | 0.753 | 0.771 | 0.792 |
| cascade minus first pass | | +0.019 [−0.035, +0.080] | +0.021 [−0.030, +0.080] | +0.038 [+0.002, +0.086] | +0.060 [+0.031, +0.097] |
| whole schema minus cascade | | +0.083 [+0.032, +0.133] | +0.106 [+0.047, +0.153] | +0.102 [+0.040, +0.154] | +0.098 [+0.056, +0.151] |
| refit `both`, threshold 0.837 (2 301 questions, 229 databases) | 0.731 | 0.743 | 0.739 | 0.763 | 0.774 |
| cascade minus first pass | | +0.012 [−0.043, +0.073] | +0.008 [−0.054, +0.073] | +0.033 [−0.007, +0.081] | +0.043 [+0.008, +0.087] |
| whole schema minus cascade | | +0.091 [+0.037, +0.139] | +0.120 [+0.053, +0.172] | +0.110 [+0.044, +0.166] | +0.117 [+0.067, +0.165] |

Round 1q's cascade with Haiku as agents read 0.787; its intervals were over
the 369 databases with statistics, these over all 398.

**Tokens, time and price.** Tokens as each response counts them, output
including reasoning; seconds from sending to the whole body, through the
proxy for Gemini, at four in flight; price at each provider's list price
read on 7 November 2026 (Google AI Studio's page, Gemini 3 Flash as
`gemini-3-flash-preview`, Gemini 3.8 Flash at its price until 31 December
2026; Anthropic's, Haiku 5.5 to 100 000 prompt tokens). Through the proxy
the owner pays a subscription, not these prices; Haiku's were paid.

| whole-schema prompts, 398 | input tokens | output (reasoning) | seconds, median / 90th / largest | price, median a schema | all 398 |
|---|---|---|---|---|---|
| Gemini 3 Flash | 316 233 | 88 310 (0) | 0.95 / 1.5 / 8.0 | $0.0005 | $0.42 |
| Gemini 3.5 Flash-Lite | 316 233 | 83 292 (0) | 0.97 / 1.5 / 5.5 | $0.0004 | $0.30 |
| Claude Haiku 5.5 | 455 418 | 296 121 (170 911) | 2.1 / 5.6 / 98 | $0.0003 | $0.30 |
| Gemini 3.8 Flash | 316 233 | 809 839 (721 902) | 5.3 / 27 / 242 | $0.0027 | $3.27 |

| the cascade's own databases | cascade prompts: input / output tokens, price | whole-schema prompts of the same databases: input / output, price |
|---|---|---|
| round 1q's, 207 databases, Gemini 3.8 Flash | 277 740 / 250 781, $1.15 | 239 244 / 700 529, $2.81 |
| the same, Claude Haiku 5.5 | 410 697 / 74 459, $0.16 | 350 614 / 229 850, $0.26 |
| the same, Gemini 3.5 Flash-Lite | 277 740 / 14 340, $0.12 | 239 244 / 60 635, $0.22 |
| the refit's, 229 databases, Gemini 3.8 Flash | 293 102 / 215 833, $1.03 | 248 801 / 715 122, $2.87 |

A cascade's prompt is longer than the whole-schema prompt of its database —
the schema and the questions — and its answer shorter; it costs about half.
The pair prompts cost $0.24 (Haiku), $0.39 (Flash-Lite), $0.52 (3 Flash)
and $4.23 (3.8 Flash) for 2 636. This round's Haiku requests on Spider,
BIRD and CTU cost $0.89, and $0.06 more on the held-out set.

## 6. The held-out set, once

**LiveSQLBench-Base-Full-v1** (BIRD's team, released in 2025 and built to
be uncontaminated; CC BY 4.0 by its card's metadata, CC BY-SA 4.0 by its text), chosen by the owner among three candidates.
The frozen model — the refit `both` at 20261027, `keys.bin` SHA-256
`56a7719b12838148e959c67723d00bee8749a012852bde298d1eaacdc5ed6ba9`,
threshold 0.8373612937315371 — was written to
[frozen.json](data/keys-clean-2026-11-07/frozen.json) at 16:20 UTC; then the
22 `*_schema.txt` files were fetched at Hugging Face revision `e33469b`
([hashes](data/keys-clean-2026-11-07/heldout-schema.sha256)), read as round
1p reads Spider's and CTU's, and the model run once, in Python and in Rust.
Every line of every table was read: 244 tables, 1 942 columns, 243 primary
keys, 329 foreign-key rows, 299 one-column keys. All 22 databases are novel;
no pair of tables occurs together in a SchemaPile-Perm schema; 285 pairs in
the pair task.
([heldout-result.json](data/keys-clean-2026-11-07/heldout-result.json),
[heldout-score.json](data/keys-clean-2026-11-07/heldout-score.json))

| | |
|---|---|
| over every declared key, P / R / F1 | 0.681 [0.467, 0.845] / 0.107 [0.053, 0.169] / **0.185** [0.095, 0.280]; macro 0.162 |
| keys drawn | 47, 32 of them declared; Rust drew the same 47 |
| over candidates, against the rules | 272 of the 299 keys among the candidates; model 0.681 / 0.118 / 0.201, the rules none right of 95 guesses |
| the pair task, recall at one | 0.667 [0.552, 0.775]; two tables only 0.691; the name score alone 0.698; 13 pairs refer to a column other than the target's key |

**The model does not carry to this set.** Its candidates hold 91 % of the
keys, as on the external databases' 94 %, and its precision is in the range
it had there; recall falls from 0.640 to 0.107. That the set's naming style —
`amenities.houseid` → `households.housenum`, keys ending in `ref` — is the
reason is a hypothesis this round does not test: nothing was changed or
looked into after the reading, as the specification fixed.

**The language models on it**, asked at the owner's request after the
frozen model's reading, from 18:04 UTC, under §5's protocol, Haiku one
request at a time; round 1p's prompts cut by its own `gemini.py cut` from the
same `hidden.jsonl` and `bench.jsonl` (hashes in
[heldout-prompt-hashes.txt](data/keys-clean-2026-11-07/heldout-prompt-hashes.txt)).
Every prompt was answered.
([heldout-lm.json](data/keys-clean-2026-11-07/heldout-lm.json))

| | P / R / F1 [95 %] | macro | frozen minus it: F1 | precision | recall | the pair task (285) | price, all 22 / all pairs |
|---|---|---|---|---|---|---|---|
| the frozen model | 0.681 / 0.107 / 0.185 [0.095, 0.280] | 0.162 | | | | 0.667 [0.552, 0.775] | |
| Gemini 3 Flash | 0.928 / 0.866 / 0.896 [0.834, 0.945] | 0.852 | −0.711 [−0.800, −0.615] | −0.247 [−0.470, −0.077] | −0.759 [−0.843, −0.660] | 0.660 [0.527, 0.782] | $0.03 / $0.06 |
| Gemini 3.5 Flash-Lite | 0.931 / 0.863 / 0.896 [0.833, 0.943] | 0.852 | −0.711 [−0.799, −0.615] | −0.251 [−0.474, −0.080] | −0.756 [−0.837, −0.658] | 0.670 [0.544, 0.782] | $0.03 / $0.05 |
| Claude Haiku 5.5 | 0.927 / 0.890 / 0.908 [0.838, 0.957] | 0.875 | −0.723 [−0.830, −0.611] | −0.246 [−0.472, −0.080] | −0.783 [−0.873, −0.664] | 0.789 [0.673, 0.897] | $0.02 / $0.04 |
| Gemini 3.8 Flash | 0.938 / 0.910 / 0.924 [0.869, 0.965] | 0.909 | −0.739 [−0.832, −0.643] | −0.257 [−0.481, −0.086] | −0.803 [−0.879, −0.717] | 0.881 [0.784, 0.951] | $0.55 / $1.22 |

Given the whole schema, every model does better here than on Spider, BIRD
and CTU, and better than the frozen model by 0.7. Given two tables' names
only, the two fast Gemini models do no better than the frozen model, and the
two that think, Haiku and Gemini 3.8 Flash, do. Whether a language model
helped write LiveSQLBench's schemas is not known here; that they read
naturally to one is a hypothesis.

## Limits

- Three fits of `both`, one of the base; the widening's value off SchemaPile
  is measured on one fit of each.
- The language models' training data are unknown; Spider and BIRD are on the
  web. LiveSQLBench was built in 2025 against contamination; the language
  models were asked on it after the frozen model's reading, on 22 databases.
- Latency is through the proxy and four requests at once, not a provider's
  own; Haiku at its default temperature, the others at 0.
- The held-out set is 22 databases from one team in one style; its interval
  is wide, and it says nothing of databases built otherwise.
- `both, roles` and round 1r's `both, unique, decimal` were not refitted;
  their figures stand on the trigrams in process order.

## What the paper's marks are closed by

The `[needs: …]` and `[author: …]` marks of `papers/foreign-keys/draft.md`:

| mark | closed by |
|---|---|
| §3, the widened model rescored or refitted with sorted trigrams | §1–§4: refitted; 0.731 externally, 0.736 on test |
| §4, the reasoning setting each identifier implies | §5: the proxy's passthrough, and the reasoning counted |
| §4, Haiku's model version | §5: `claude-haiku-5-5` through the API; the agents' version of round 1q was not recorded |
| §5.1, test F1 over every declared key, base and widened | §3: 0.714 and 0.736 (round 1b's model as saved 0.720) |
| §5.1, the base model on the same test pairs | §3: 0.840 against 0.905 |
| §5.3, intervals on the precision and recall differences | §5 |
| §5.3, a per-database macro average | §4 and §5 |
| §5.3, F1 over every declared key for the widened model's three fits | §4: 0.731, 0.728, 0.678; without `Bench` 0.767–0.774 |
| §5.8, whole-schema characters and tokens on the cascade's databases | §5's last tables |
| §5.10, the first stage alone and the name score over every declared key on test | §3: 0.733 and 0.442 against both stages' 0.736 |
| §5.10, the reader's time a schema and resident memory in Rust | [the reader](#the-reader-in-rust) |
| §6, Haiku through the API at temperature 0 | §5: through the API; temperature cannot be set on Haiku 5.5 |
| §6, tokens, price and latency a schema for each model | §5 |
| §5.2, an inclusion-dependency baseline; §5.4, a names-only variant; §5.9, the owner's adjudication; §5.11, Nexus's undirected F1; the `[author: …]` marks of provenance, availability and the use of AI | not closed here |

And one the draft has no mark for: its abstract's and conclusion's figures
were measured on development data; §6's held-out reading is the only one
that is not, and there the gap to the language models is 0.71 to 0.74, not
0.10 to 0.16.

## Environment and reproduction

| | |
|---|---|
| research commit | `6807b66` |
| models | the refits in `corpus/round-1s/models/`; round 1p's `both` and round 1b's model as saved |
| machine | AMD Ryzen 7 7700, RTX 5070 12 GiB, Windows 11 Pro 26300 |
| Python | 3.13.15; XGBoost 3.4.1 (first stage on the graphics card), LightGBM 4.7.0, scikit-learn 1.9.1, numpy 2.5.4, scipy 1.18.1; anthropic 2.6.0 for Haiku |
| Rust | rustc 1.99.1; in the container `rust:1.99.1`, Linux 6.18.33.2 (WSL2), one processor |
| language models | as §5's first table, 7 November 2026 |
| held-out set | LiveSQLBench-Base-Full-v1 at Hugging Face revision `e33469b1d1134dac377deda2643c0bda945cdfc9` |
| run, UTC | `check` 12:33; fits 12:34–13:09; Gemini asked from 12:43; frozen 16:20; scorings 16:20–16:55; Haiku 16:24–17:12; held-out scored 17:06; a first container run 17:12–17:41, stopped unread; the container's reported run 17:43–17:56; the language models on the held-out set 18:04–18:22 |

The specification's run section has the commands in order.
