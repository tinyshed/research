# Zero-configuration multilingual Search experiment

Research only. No Core/API changes. Source SQLite revision remains the pinned
TinyStore Rust revision `76b844c`; this independent CLI uses its bundled SQLite
3.53.2 through rusqlite 0.40.2. Search is a distinct product requirement from
general secondary indexing; the earlier recommendation against a universal
Index Engine does not reject a high-level Search collection.

Registered before timing:

- Shared ICU4X segmentation + NFKC/lowercase/ё folding, with controlled stemming
  variants, used by a real FTS5 tokenizer callback and Tantivy Tokenizer.
- Default-auto prototype: deterministic script dispatch (Russian Snowball for
  Cyrillic, English for Latin), ICU complex-script segmentation otherwise.
  It is a candidate with explicitly limited morphology coverage, not a claim
  that every Cyrillic/Latin language is Russian/English.
- Controls: ICU only, language-detected stemming, colocated language-tagged
  RU/EN/DE/FR/ES stem alternatives, Jieba Chinese segmentation, native Unicode61.
- Natural MIRACL/NoMIRACL judged passages and queries for RU/EN/ZH, kept separate
  per language, plus a declared small adversarial/mixed-language gold fixture.
  Report pooled-corpus retrieval and judged-pool reranking separately; never
  call unjudged documents confirmed false positives.
- Full-text positions/frequencies and top-10 BM25 on both engines; same analysis,
  term OR query and source fields, but scoring formulas need not rank identically.
  Set-equality checks are separate from quality metrics.
- Binary variants are the same reachable CLI with optional dependencies and
  identical release/strip/LTO settings. Dictionary/model data embedded in ICU,
  Whatlang/Jieba is included. Disk source/index/WAL and RAM are separate.
- Timings use serial fresh processes in the existing disk-backed Linux Docker
  lab (2 CPU quota, 6 GiB); two passes in opposite candidate order. No concurrent
  build/tests or older geo sweep. Corpus is fetched, ignored, SHA-pinned.

Commands and exact source/corpus revisions are recorded with raw measurements
under `../reports/data/search-2026-10-16/`. `corpus.py`, `sizes.sh`, `run.py`
and the dated report provide reproduction. SQL-trigger maintenance, rollback,
reader registration and reopen are correctness probes, not a public Search API.
