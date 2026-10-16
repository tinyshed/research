# Search: the query, not the store — 16 October 2026

[search-2026-10-16](search-2026-10-16.md) measured FTS5's ranked top-10 at
hundreds of milliseconds against Tantivy's few, with every question sent as
an OR of all its words. This round asks whether that tail is FTS5's storage
or the query it was given, and checks the rest of a proposed design for
TinyStore's search on the same judged corpus: an analyzer that needs no
dictionary, stems chosen by script, and typo candidates from an FST. Its
code is new, [search-plan-lab](../search-plan-lab/README.md); nothing of the
earlier lab is used. Nothing here is a product promise.

| Question | Answer | What follows |
|---|---|---|
| Is FTS5's tail its storage or its query? | Mostly the query: leaving out the words in more than 5 % of the documents cuts FTS5's p99 from 33–65 ms to 2–3 ms in Russian and English and from 51 to 9 ms in the mixed collection, at nDCG@10 within −5 % to +1 % | The statistics' stop words go into the planner, no list a language |
| Does it reach Tantivy? | No: Tantivy's own `or` is 2–18 times faster at 0.2–0.9 ms, and 7–30 times on the same pruned plan; FTS5's recall@10 is higher on ru, en and mixed | FTS5 stays the store; the gap is the price of one file and its transactions |
| Every word required, or tiers that drop the most frequent? | Neither on natural questions: AND leaves 512 of 600 empty, tiers lose 5–76 % of nDCG | AND belongs to keyword queries, which this corpus does not have |
| Bigrams instead of ICU4X's dictionaries? | ICU is better for Chinese (+14 % nDCG) and Japanese (+3 to +8 %), bigrams for Thai under OR (+4 %); ICU costs 2.28 MiB of executable, bigrams none | Bigrams by default, ICU an owner's choice |
| English's stemmer for every Latin word? | No: worse than no stem in English, German, French and Spanish under both plans; Russian's stemmer gains 7–12 % | Cyrillic stemmed, Latin as written, a language hint optional |
| Typo candidates from an FST? | They win back 66–92 % of what one typo a question lost, and cost nothing when there is no typo; one FST a script, or a Chinese index's bigrams cost 16 ms a query | One FST a script, built from FTS5's vocabulary |
| What does it add to an executable? | 0.37 MiB: the analyzer 0.24, the FST and its automata 0.13 | — |

## The plans

FTS5 and Tantivy index the same tokens in the same four fields — a title's
and a body's words as written, and their stems where these differ — and run
the same plans. The questions are NoMIRACL's: 200 a language chosen by the
SHA-256 of their id, 600 in the mixed collection of 88,175 Russian, English
and Chinese passages. Each cell is two rounds, A B A B, each after a warm pass
of its own.

| Mixed, 88,175 passages | p50 ms | p99 ms | nDCG@10 | Recall@10 | empty |
|---|---|---|---|---|---|
| FTS5, `or` | 8.59; 8.81 | 51.29; 52.76 | 0.5123 | 0.7034 | 0 |
| FTS5, `and` | 0.18; 0.18 | 4.98; 4.94 | 0.0324 | 0.0302 | 512 |
| FTS5, `adaptive` | 0.68; 0.68 | 11.90; 10.30 | 0.3825 | 0.5120 | 0 |
| FTS5, `pruned-or` | 2.34; 2.42 | 8.84; 8.90 | 0.5152 | 0.7110 | 0 |
| FTS5, `adaptive-pruned` | 0.44; 0.44 | 1.94; 2.04 | 0.3865 | 0.5136 | 0 |
| Tantivy, `or` | 0.11; 0.12 | 0.71; 0.78 | 0.5151 | 0.6656 | 0 |
| Tantivy, `pruned-or` | 0.08; 0.08 | 0.30; 0.32 | 0.5199 | 0.6740 | 0 |

| `pruned-or` against `or`, FTS5 | p99 ms, `or` | p99 ms, `pruned-or` | nDCG@10, `or` | nDCG@10, `pruned-or` |
|---|---|---|---|---|
| ru | 36.81; 33.10 | 2.03; 2.02 | 0.6223 | 0.6004 |
| en | 63.70; 65.41 | 3.07; 3.10 | 0.5588 | 0.5382 |
| zh | 8.80; 8.43 | 3.45; 3.76 | 0.3658 | 0.3481 |
| mixed | 51.29; 52.76 | 8.84; 8.90 | 0.5123 | 0.5152 |

`pruned-or` asks for any word but those in more than `cut` of the documents,
or the rarest word when every word is that frequent. A hypothesis for why it
works, which this round does not separate: FTS5's bm25 counts the documents
of each phrase by reading its whole doclist, every query, so a frequent word
costs its doclist whatever the `LIMIT`; and a word in a twentieth of the
documents carries little of the rank.

FTS5 needs the documents a term is in to choose. A term in more than 0.5 % of
them comes from a table read at open — 2,710 terms of the mixed index in
190–200 ms, a scan of the whole vocabulary — and any other from `fts5vocab`
by its term, inside the timed query. A store would keep that table beside the
index rather than build it at every open.

The cut moves speed against quality, mixed, FTS5, `pruned-or`:

| cut | p99 ms | nDCG@10 | Recall@10 |
|---|---|---|---|
| 1 % | 1.83; 1.78 | 0.4723 | 0.6596 |
| 2 % | 3.97; 3.88 | 0.5017 | 0.6929 |
| 5 % | 8.84; 8.90 | 0.5152 | 0.7110 |
| 10 % | 14.19; 13.95 | 0.5145 | 0.7077 |
| 20 % | 26.21; 25.59 | 0.5109 | 0.7029 |

`and` and the tiers lose on these questions because a natural question holds
words its answer does not: 145 to 199 of 200 have no passage with every word.
The tiers lose most in Chinese, 0.0871 against 0.3658, where every bigram is a
word that must match. Keyword queries, a product's name or two words of a
note, were not measured; they are where `and` would be expected to win.

The two engines do not score alike, so equal tokens give different ranks:
FTS5 has the higher nDCG@10 in Russian (0.6223 against 0.5947), Tantivy in
Chinese (0.4002 against 0.3658), and FTS5 the higher recall@10 everywhere but
Chinese, where they are level.

## Scripts written without spaces

| FTS5, `or`; `pruned-or` | nDCG@10, bigrams | nDCG@10, ICU4X | nDCG@10, words alone | index MiB, bigrams; ICU4X |
|---|---|---|---|---|
| zh | 0.3658; 0.3481 | 0.4179; 0.3921 | 0.0153 | 28.3; 17.5 |
| ja | 0.6018; 0.5784 | 0.6223; 0.6266 | 0.1929 | 29.8; 16.9 |
| th | 0.5980; 0.5172 | 0.5774; 0.5373 | 0.3121 | 31.1; 14.6 |

Bigrams are overlapping pairs of graphemes, so that a Thai vowel stays with
its consonant; ICU4X is `WordSegmenter::new_auto`, dictionaries for Chinese
and Japanese and an LSTM for Thai, whose index took 25.5 s to build against
3.1 s. Unicode's word rules alone leave a Chinese question one token, and 186
of 200 find nothing. Pruning costs bigrams more in Thai (−14 %) than ICU's
words (−7 %): a frequent bigram may still be most of a word.

## Stems

| FTS5, nDCG@10, `or`; `pruned-or` | as written | English's stems | the language's stems | its stems, accents kept |
|---|---|---|---|---|
| ru | 0.5570; 0.5590 | — | 0.6223; 0.6004 | — |
| en | 0.5892; 0.5921 | 0.5588; 0.5382 | — | — |
| de | 0.4412; 0.4424 | 0.4262; 0.4282 | 0.4494; 0.4149 | 0.4582; 0.4269 |
| fr | 0.4207; 0.4118 | 0.4176; 0.3834 | 0.4324; 0.4129 | 0.4251; 0.3986 |
| es | 0.6016; 0.5871 | 0.5765; 0.5641 | 0.6190; 0.5715 | 0.6217; 0.5799 |

Russian's stemmer gains 12 % under `or` and 7 % under `pruned-or`. English's,
the default for Latin script, loses to no stem in all four Latin languages,
English included. A language's own stemmer gains 2–3 % under `or`, and
nothing or less under `pruned-or`. Folding accents gives no verdict. Differences under 0.03
on 200 questions were not tested for significance.

## Typos

One typo in each question's longest word of five letters or more — a letter
replaced, dropped, doubled or swapped, chosen by the question's id. A word no
document holds, as written or as a stem, takes up to eight terms of the index
within one edit (five to eight letters) or two (nine or more), a swap one
edit.

| FTS5, `pruned-or`, nDCG@10 | clean | one typo | one typo, candidates | clean, candidates | p50 ms, typo with candidates |
|---|---|---|---|---|---|
| ru | 0.6004 | 0.4571 | 0.5671 | 0.6004 | 1.26; 1.34 |
| en | 0.5382 | 0.3595 | 0.5242 | 0.5382 | 1.18; 1.23 |
| mixed, one FST | 0.5152 | 0.3946 | 0.4747 | 0.5152 | 18.58; 18.95 |
| mixed, an FST a script | 0.5152 | 0.3946 | 0.4747 | 0.5152 | 2.68; 2.60 |

Candidates win back 77 % of the loss in Russian, 92 % in English and 66 % in
the mixed collection, whose Chinese typos fall inside bigrams, which are
under five letters and take none. A clean question loses nothing. With one
FST for every script, a Russian or English word of nine letters, allowed two
edits, walked every Chinese bigram, two characters within two edits of
anything; one FST a script took the mixed p50 from 18.6 ms to 2.6 ms, the
answers the same. An FST of 154,678 terms is 0.67 MB and builds in 90 ms; of
977,180, 4.5 MB in 360–400 ms.

## Memory, index and executable

| Mixed | index MiB | build s | build HWM MiB | query HWM MiB |
|---|---|---|---|---|
| FTS5 | 72.4 | 9.4 | 95 | 15; 15 |
| Tantivy | 48.6 | 5.9 | 281 | 46; 46 |

The passages' text is 69.1 MiB. Both indexes are merged into one segment
before they are read. A build's peak holds the corpus as well; a query's is a
process that holds the index's ids and the questions, and Tantivy's counts the
pages of its memory-mapped files it touched, which SQLite's page cache of
1 MiB does not.

| Stripped Linux x86-64 executable, probe | bytes | over SQLite with FTS5, MiB |
|---|---|---|
| SQLite with FTS5, as TinyStore links it | 2,344,360 | 0 |
| and the analyzer | 2,596,648 | 0.241 |
| and the FST with its automata | 2,731,912 | 0.370 |
| and ICU4X's segmenter with its data | 5,120,840 | 2.648 |
| and Tantivy | 6,125,064 | 3.606 |

## What this round does not show

- Natural questions only. Keyword queries, prefixes typed as one searches and
  phrases were not measured.
- Typos are synthetic, one a question.
- Reads only: no writes, no index between merges, no concurrent readers, one
  query at a time, on one machine.
- No significance test; two rounds agree to the last digit on quality, so
  their spread is latency alone.
- Vectors were not measured.
- A million documents: the earlier round's FTS5 store answered its `or` at a
  p99 of 1.1–1.5 s there, and Tantivy at 47–68 ms; whether `pruned-or` keeps
  its gain at that size was not measured.
- The earlier round found a document's text able to spell a stem, `s_ru_кошк`
  matching `кошка`, since its stems and words share one column. Here stems
  have columns of their own and a token holds only letters, marks and digits,
  so no text spells one; a word written as its own stem, `кошк`, matches its
  forms by design.

## What follows

- **The design for TinyStore's book**: FTS5 holds the postings in the store's
  file; the analyzer is NFKC, stress marks off, ё as е, bigrams for Han, kana
  and Thai, Russian's stems for Cyrillic and Latin as written; the planner
  leaves out words in more than 5 % of the documents, from a table of
  frequent terms kept beside the index; a typo's candidates come from an FST a
  script, built again from FTS5's vocabulary in the background. 0.37 MiB.
- **The owner's choices**: ICU4X behind a feature for Chinese and Japanese,
  +2.28 MiB over the design; a cut other than 5 %; a language hint that turns a
  Latin script's stems on.
- **Rounds still owed**: vectors on MIRACL — embeddings computed once outside
  TinyStore, int8 against float32 by exhaustive search, and the two lists
  fused by rank against words alone; keyword queries; writes and an index
  between merges; a million documents.

## Environment and reproduction

Linux 6.18.33.2-microsoft-standard-WSL2 in Docker Desktop's WSL2 VM on an AMD
Ryzen 7 7700 (8 cores, 16 threads), the container limited to 2 CPUs and 6 GiB
(cgroup `200000 100000`, `6442450944`); Rust 1.99.0; rusqlite 0.40.2 with its
bundled SQLite 3.53.2, the pin of TinyStore's `rust` at `63ca65e`, opened as
TinyStore opens a file: 4 KiB pages, WAL, `synchronous=full`, a 1 MiB cache;
Tantivy 0.26.2; ICU4X's `icu_segmenter` 2.3.0; rust-stemmers 1.2.0; fst 0.4.7;
levenshtein_automata 0.2.1. The executables' sizes are a release build with
thin LTO and one codegen unit, symbols stripped.

NoMIRACL at revision `ecd08778d0426a5ca28ac99763b0c9ddc2c78e68`, every file
checked against [corpus.sha256](../search-plan-lab/corpus.sha256); Russian,
English and Chinese are the files the earlier round pinned, byte for byte. A
passage id seen twice is kept once; a question whose judged passages are not
all present is left out; a passage nobody judged counts as not relevant.

Harness `064637d` ran [run.sh](../search-plan-lab/run.sh) from 08:46:11 to
09:05:15 UTC and [sizes.sh](../search-plan-lab/sizes.sh) after it; harness
`94c9a58` ran [typos-by-script.sh](../search-plan-lab/typos-by-script.sh) from
09:07:55 to 09:11:59 UTC on the indexes run.sh built. Every pass's table is in
[the measurements](search-plan-measurements-2026-10-16.md), and the raw lines
and every question's ten passages in
[data/search-plan-2026-10-16](data/search-plan-2026-10-16/).

```sh
sh tinystore/search-plan-lab/fetch.sh tinystore/bench/corpus/nomiracl
MSYS_NO_PATHCONV=1 docker run --rm --cpus 2 --memory 6g -v <repo>:/src \
  -v search-plan-cargo:/usr/local/cargo/registry -v search-plan-target:/target \
  -v search-plan-perf:/perf -e CARGO_TARGET_DIR=/target rust:1.99 \
  sh -c 'sh /src/tinystore/search-plan-lab/run.sh && sh /src/tinystore/search-plan-lab/sizes.sh \
    && sh /src/tinystore/search-plan-lab/typos-by-script.sh'
python tinystore/search-plan-lab/summarize.py tinystore/reports/data/search-plan-2026-10-16
```
