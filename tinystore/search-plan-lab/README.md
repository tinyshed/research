# search-plan-lab

The round of [search-plan-2026-10-16](../reports/search-plan-2026-10-16.md):
whether FTS5's ranked query tails are its storage or the query it was given,
measured with an analyzer written for TinyStore rather than the earlier
lab's. Nothing here is a product.

| File           | What it is                                                                     |
|----------------|--------------------------------------------------------------------------------|
| `src/analyze`  | the analyzer under test: NFKC, stress marks off, bigrams, stems by script       |
| `src/plan`     | the five query plans, over FTS5 or Tantivy alike                               |
| `src/fts`      | the index in FTS5, as TinyStore opens a file                                   |
| `src/tant`     | the same tokens in Tantivy, the control                                        |
| `src/typo`     | a typo's candidates: an FST of the index's terms and a Levenshtein automaton   |
| `src/probe`    | what each piece adds to a stripped executable                                  |
| `fetch.sh`     | NoMIRACL at the pinned revision into `<corpus>`, checked against corpus.sha256 |
| `run.sh`       | the round in the Linux container                                               |
| `sizes.sh`     | the executables' sizes in the Linux container                                  |

```sh
sh tinystore/search-plan-lab/fetch.sh tinystore/bench/corpus/nomiracl
MSYS_NO_PATHCONV=1 docker run --rm --cpus 2 --memory 6g -v <research>:/src \
  -v search-plan-cargo:/usr/local/cargo/registry -v search-plan-target:/target \
  -v search-plan-perf:/perf -e CARGO_TARGET_DIR=/target rust:1.99 \
  sh -c 'sh /src/tinystore/search-plan-lab/run.sh && sh /src/tinystore/search-plan-lab/sizes.sh'
```
