# search-fields: two fields and prefixes, in the lab's layout and in one made for them

Written on 1 November 2026, before any time was taken.
[search-fast-2026-11-01](../../../reports/search-fast-2026-11-01.md) measured
one field and designed, without measuring, a layout for several and for
prefixes: one dictionary row a word, lists by rowid, a byte of length a
document a field. This round measures it.

## The question

The search lab keeps a body as two fields, its words as written and their
stems, and asks every form of a word in each: the words as written, the stem
in the stems' field, and the stem among the words where it differs. Its keys
carry their field as text, `b|word` and `bs|word`, and its reader asks four
fields a form, two of them a title MS MARCO has not. A prefix — the last word
of a query still being typed — is a range of keys a field.

1. Over two fields, how near Tantivy does each layout come, and does the
   native one keep `fast2`'s gain over the lab's?
2. With a prefix expanded to its 32 most frequent words, as fast?
3. What does each layout cost in bytes a posting, both fields counted?

## What is built

All from MS MARCO passage v1 as search-msmarco prepared it, and once more
with every token as written (`MSMARCO_TOKENS=surface`): 8,841,823 passages,
345,553,319 postings of stems and 353,745,695 of words as written.

- **`lab`**: compact1's tables over both fields, `fields-lab.db`, read by the
  lab's own reader, compact.rs's `Compact`, its window executor reading ahead
  8, as search-msmarco's `compact` arm.
- **`native`**: `fields-native.db`, from the lab's tables, the same
  postings: `words` (a word, its number, its documents in each field),
  `lists` and `dirs` by rowid, number × 4 + field, the blocks bit-packed in
  one blob a list and the directory 16 bytes a block, as `fast2`; `lengths`
  a byte a document a field. Its reader is `fast4-cache`'s executor; the
  arm `native-nocache` reads every directory again a query.
- **`tantivy`**: the same two fields, a term query a form, boosted 1.0 and
  0.75 as the lab weighs its fields; Tantivy's BM25 keeps a field's length
  in a byte.

A prefix: the last word of each sample query with four letters or more, cut
to its first half and three letters at least; the 32 words of `b` that begin
with it and hold the most documents, ties by word, each in every engine's
own dictionary, every one a word with no stem of its own. `lab` scans its
keys from `b|` and the prefix; `native` its words; Tantivy its term
dictionary.

## Measures

- p50 and p99 of each arm, warm, one query at a time, the sample's 1,097
  queries and the prefix set, four rounds in one order and then the other,
  each against Tantivy's of the same round, as search-fast measured.
- Bytes a posting by object, both fields' postings together.
- `native` against `lab-exhaustive`, the lab's reader scoring every posting:
  every top ten's scores within 1e-9, documents differing only where scores
  tie. Tantivy's BM25 differs by its lengths, and is not checked for answers.

A layout's effect is the difference between arms in one run; why is a
hypothesis until a second measurement separates it.

## Run

```sh
B=/target/release/msmarco
docker run --rm -v <repo>:/src:ro -v search-msmarco-target:/target -v search-msmarco-perf:/perf \
  -e MSMARCO_TOKENS=surface rust:1.99 sh -c "$B prepare /src/tinystore/bench/corpus/msmarco /perf/msmarco-surface 6"
docker run --rm -v <repo>:/src:ro -v search-msmarco-target:/target -v search-msmarco-perf:/perf rust:1.99 sh -c "
  $B fields-queries /src/tinystore/bench/corpus/msmarco /perf/msmarco
  $B fields-build lab /perf/msmarco /perf/msmarco-surface /perf/msmarco/fields-lab.db
  $B fields-build native /perf/msmarco/fields-lab.db /perf/msmarco/fields-native.db
  $B fields-build tantivy /perf/msmarco /perf/msmarco-surface /perf/msmarco/fields-tantivy"
sh tinystore/bench/search-msmarco/fields/timing.sh
```
