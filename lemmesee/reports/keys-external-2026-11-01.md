# Keys on the databases others measured theirs on, 1 November 2026

Round 1p, as [its specification](../round-1p/README.md) sets it: round 1b's
key model [as reported](keys-2026-10-28.md), on the databases the SchemaPile
paper tested its foreign-key models on — Spider, BIRD and CTU's relational
repository — beside that paper's own T5 model run here on the same pairs and
Gemini 3.8 Flash asked the same questions; then its candidates widened on
SchemaPile-Perm's validation split and the chosen variant measured once.
Its harness is at research commit `c1163da`. Raw output in
[data/keys-external-2026-11-01/](data/keys-external-2026-11-01/).

| Question | Answer | What follows |
|---|---|---|
| In the paper's task, two tables known to hold one key, where does a model under a megabyte stand? | Round 1b's model: recall at one 0.854 / 0.848 / 0.775 on Spider / BIRD / CTU (768, 381, 1 267 pairs). SchemaPile's T5 run here on the same pairs: 0.957 / 0.934 / 0.929 constrained, 0.930 / 0.869 / 0.903 greedy; Gemini 3.8 Flash 0.987 / 0.953 / 0.985. The widened variant: **0.913 / 0.900 / 0.886** | the widened variant is the model to carry |
| And in round 1b's task, every column, no table named? | **F1 0.775 against the rules' 0.547** over 392 databases, [+0.186, +0.279]; Spider 0.919, BIRD 0.876, CTU 0.650. Over every declared key, the widened variant's F1 0.736 against Gemini's 0.891 | Gemini is the ceiling here; the gap is recall |
| Can a model trained on related pairs tell related tables from unrelated ones? | T5's best answer is as sure for a table with no key as for one with: AUC 0.798 against round 1b's 0.889 and the widened variant's 0.930 | the pair task hides the open task's hard part |
| Where does round 1b's model lose? | Coverage, not ranking: of the pairs its candidates hold it is right on 656 of 661, 323 of 330 and 982 of 1 042; the rest are keys that are their own table's key, targets with no primary key, keys into composite or non-key columns, and declared types of different kinds | widen the candidates |
| What does widening buy? | A table's own key as a source and a pseudo key for a target with no primary key, chosen on validation: the paper's task on validation 0.797 → 0.853, F1 over every declared validation key 0.686 → 0.690; once on test F1 0.761 against the rules' 0.420, the hard cases 0.871 | widened variant `both` |
| How big is it, as a file? | `keys.bin` 938.3 KiB for both stages, read back and checked: no key drawn differently from the libraries on 712 774 pairs | the size is measured, not estimated |
| And how small can it go? | 25 first-stage trees, 23 KiB beside the graph stage's 177: F1 0.748 against 0.760 on these databases, the paper's task within 0.008; on validation 0.655 against 0.690 | about 200 KiB is enough off SchemaPile |
| Against Nexus's XGBoost on its sets? | BIRD's 11 development databases: F1 0.787 at our threshold, 0.796 at the best, against its 0.10; TPC-DS 0.525 and 0.824 against 0.44; TPC-H is in SchemaPile-Perm whole, 8 of 8 tables, and says nothing | a more careful version of their baseline, measured on their sets |

## The databases

| | databases | declaring a key | novel | tables | one-column keys declared | the paper's pairs, kept |
|---|---|---|---|---|---|---|
| Spider, `test_database/` | 206 | 204 | 162 | 1 053 | 911 | 768 of 842 |
| BIRD, train and dev | 80 | 76 | 67 | 597 | 606 | 381 of 404 |
| CTU, the whole repository | 142 | 118 | 114 | 1 990 | 1 854 | 1 267 of 1 390 |

A pair is dropped when its two tables occur together in one SchemaPile-Perm
schema (74, 23 and 123) or pass 8 000 characters (none). The paper filtered
against all of SchemaPile and its instruction data and kept 437 / 242 / 558;
ours are larger, and its own figures are on its sets.

**CTU was read twice.** Its server refused this machine's connections at
first, so the round began on the 48 databases earlier rounds had dumped
whole; reached later through a tunnel, all 142 were dumped as structure
only, and the 48 parse to identical schemas both ways. Every number here is
on the 142. Two databases reference each other's tables (`ctu_crossSchema1`
and `2`); those keys are not within one schema and are left out. One
database, `Bench`, has 678 tables and 362 declared keys.

**BIRD's schemas were read from the archives**, each file's first 32 MiB, or
the whole file when SQLite could not read its schema from that (30 of 80); on
the 42 databases already unpacked the two readings are identical.

## Round 1b's task

Each column keeps its best target and draws it past round 1b's threshold,
0.832; keys among the model's candidates, the difference's interval from
1 000 resamples of databases.

| | databases | keys | rules P / R / F1 | model P / R / F1 | F1, model − rules |
|---|---|---|---|---|---|
| Spider | 199 | 801 | 0.945 / 0.562 / 0.705 | 0.905 / 0.933 / 0.919 | [+0.164, +0.271] |
| BIRD | 75 | 544 | 0.924 / 0.491 / 0.641 | 0.888 / 0.864 / 0.876 | [+0.139, +0.358] |
| CTU | 118 | 1 614 | 0.790 / 0.286 / 0.420 | 0.790 / 0.552 / 0.650 | [+0.156, +0.331] |
| all | 392 | 2 959 | 0.873 / 0.398 / 0.547 | 0.849 / 0.712 / 0.775 | [+0.186, +0.279] |
| novel only | 319 | 2 600 | 0.855 / 0.378 / 0.525 | 0.832 / 0.684 / 0.751 | [+0.177, +0.281] |

Spider and BIRD are easier than SchemaPile-Perm's test (0.774), as its
complete subset (0.819) said a schema that declares its keys would be. CTU is
harder, and not because of `Bench`: against every key CTU declares, F1 is
0.598 with it and 0.680 without it. Four of its
databases give the model nothing — Mooney_Family's 134 keys, tpce's 48,
Mesh's 33, Dunur's 32. The first three are relational-learning sets whose
keys are named for the role a row plays (`brother.name1` → `person.name`),
the kind round 1b's hard cases found hardest; tpce declares its identifiers
`decimal(11,0)`, which no candidate of the integer, text or UUID kinds takes.

## The paper's task

Recall at one: the column and the referred column both right; a pair no
candidate answers is wrong. Intervals from 1 000 resamples of databases.

| | Spider (768) | BIRD (381) | CTU (1 267) |
|---|---|---|---|
| the name score alone | 0.848 | 0.832 | 0.783 |
| round 1b's model, the two tables only | 0.852 | 0.835 | 0.772 |
| round 1b's model, the whole database | 0.854 [0.792, 0.907] | 0.848 [0.763, 0.913] | 0.775 [0.676, 0.853] |
| **widened, the whole database** | **0.913** [0.871, 0.949] | **0.900** [0.817, 0.959] | **0.886** [0.814, 0.954] |
| T5-base fine-tuned on SchemaPile, greedy, run here | 0.930 | 0.869 | 0.903 |
| T5, constrained to the two tables' columns, run here | 0.957 | 0.934 | 0.929 |
| Gemini 3.8 Flash, the paper's prompt | 0.987 | 0.953 | 0.985 |
| *the paper, on its sets of 437 / 242 / 558, read off its figure:* | | | |
| T5-base, eight beams | 0.92 | 0.83 | 0.91 |
| StarCoder 15.5B, constrained | 0.97 | 0.94 | 0.97 |
| GPT-3.5 | 0.88 | 0.82 | 0.86 |

**T5 was run by a forward pass written here** on torch alone, its weights
read as saved: on the model card's own example it writes the card's answer,
and scores the right column pair at −0.13 against −10.4 for the wrong one.
Its greedy figures on Spider and BIRD land within a few points of the paper's
beam-search ones on sets that differ, the nearest check of the reproduction
there is.

## Related or not

For each kept pair, one table of the same database with no declared key to
or from its first table, chosen at random: 2 416 related and 2 197
unrelated pairs. T5's score is its best log-likelihood over all column pairs;
ours the best score of any column of the first table into the second, nothing
where there is no candidate.

| | AUC, T5 | AUC, round 1b's | AUC, widened | unrelated accepted at 80 % of related: T5 / round 1b's / widened |
|---|---|---|---|---|
| Spider | 0.825 | 0.914 | 0.958 | 27.9 % / 1.1 % / 1.1 % |
| BIRD | 0.766 | 0.914 | 0.941 | 43.6 % / 1.1 % / 1.1 % |
| CTU | 0.796 | 0.872 | 0.917 | 34.5 % / 17.6 % / 8.7 % |
| all | 0.798 | 0.889 | 0.930 | 36.2 % / 8.1 % / 3.1 % |
| novel only | 0.797 | 0.889 | 0.931 | 35.9 % / 8.6 % / 3.3 % |

## Where round 1b's model loses

Each kept pair by whether the model's candidates hold the answer, the first
reason they do not:

| | Spider | BIRD | CTU |
|---|---|---|---|
| covered, right | 656 | 323 | 982 |
| covered, wrong | 5 | 7 | 60 |
| the key is its table's own one-column key | 25 | 23 | 140 |
| the declared types are of different kinds | 34 | 0 | 0 |
| the target declares no primary key | 22 | 11 | 8 |
| the target's key is not of a key kind | 6 | 0 | 48 |
| the target's key spans several columns | 3 | 11 | 26 |
| the referred column is not the target's key | 17 | 6 | 3 |

Of what it can answer it is right 99.2, 97.9 and 94.2 % of the time.

## The widened candidates

Four variants fitted as graph.py's final fits round 1b's model, measured on
validation only:

| Variant | training pairs | F1 over candidates | precision | recall over every declared key | F1 over every declared key | the paper's task on validation | covered |
|---|---|---|---|---|---|---|---|
| reported | 4 625 401 | 0.762 | 0.756 | 0.629 | 0.686 | 0.797 | 0.830 |
| own key | 5 752 137 | 0.755 | 0.732 | 0.658 | 0.693 | 0.822 | 0.843 |
| pseudo key | 5 036 330 | 0.752 | 0.750 | 0.641 | 0.691 | 0.830 | 0.864 |
| **both** | 6 206 601 | 0.737 | 0.710 | 0.671 | 0.690 | **0.853** | 0.878 |

**The rule as first written would have chosen none**: it compared F1 over
each variant's own candidates, and each widening puts more declared keys —
the harder ones — into its own recall's denominator. Read again after the
first two variants and before the last two, with F1 over all 7 636 keys the
validation schemas declare, every variant passes and `both` has the highest
recall at one. Both readings are in the specification.

Once, on SchemaPile-Perm's test: F1 0.761 against the rules' 0.420 over its
wider candidates, [+0.303, +0.381]; novel 0.744; complete 0.808; the paper's
task built on test schemas 0.909, 92.9 % covered. The hard cases: 0.871
against the rules' 0.544, round 1b's 0.875.

On these databases the widened variant covers 707, 354 and 1 182 of the kept
pairs and is right on 99.2, 96.9 and 95.0 % of them; in round 1b's task its F1
is 0.903 / 0.861 / 0.641, 0.760 in all, over its wider candidates.

**The graph stage**, the widened variant's first stage alone at its own
validation threshold against both stages: Spider 0.906 against 0.903, BIRD
0.865 against 0.861, CTU 0.610 against 0.640 with precision 0.637 against
0.780; all 0.738 against 0.759. It pays where names are hard.

## Against Gemini 3.8 Flash in round 1b's task

The schema as the model sees it, written out table by table, and the
question for every key, one prompt a database; scored against every
one-column key declared, so a key no candidate covers counts against our
model too. One of 398 replies was not readable JSON and counts as no keys.

| | keys declared | round 1b's P / R / F1 | widened | Gemini | widened − Gemini |
|---|---|---|---|---|---|
| Spider | 911 | 0.905 / 0.820 / 0.861 | 0.911 / 0.835 / 0.872 | 0.913 / 0.963 / 0.937 | [-0.090, -0.040] |
| BIRD | 606 | 0.890 / 0.777 / 0.830 | 0.896 / 0.785 / 0.837 | 0.935 / 0.949 / 0.942 | [-0.160, -0.052] |
| CTU | 1854 | 0.790 / 0.481 / 0.598 | 0.784 / 0.513 / 0.621 | 0.868 / 0.832 / 0.850 | [-0.333, -0.140] |
| all | 3371 | 0.850 / 0.626 / 0.721 | 0.848 / 0.649 / 0.736 | 0.893 / 0.888 / 0.891 | [-0.209, -0.106] |
| novel only | 2966 | 0.832 / 0.600 / 0.697 | 0.831 / 0.626 / 0.714 | 0.888 / 0.880 / 0.884 | [-0.231, -0.113] | |

Spider and BIRD are on the web and very likely in Gemini's training data.

## Against Nexus

Nexus (Cong et al., arXiv 2602.08186, February 2026) trains an XGBoost on
SchemaPile-Perm's metadata and reports its F1 on TPC-H, TPC-DS and BIRD's
development databases pooled, for an undirected join graph at a threshold
tuned on each set (its Table 3). The widened variant on the same sets,
over declared one-column keys:

| | tables | in Perm | keys | at our threshold P / R / F1 | at the set's best | Nexus's XGBoost |
|---|---|---|---|---|---|---|
| TPC-H (CTU's `tpch`) | 8 | 8 | 7 | 1.000 / 1.000 / 1.000 | 1.000 | 0.35 |
| TPC-DS (CTU's `tpcds`) | 24 | 0 | 102 | 0.949 / 0.363 / 0.525 | 0.824 | 0.44 |
| BIRD's 11 development databases | 75 | 5 | 104 | 0.911 / 0.692 / 0.787 | 0.796 | 0.10 |

TPC-H's eight tables are all in SchemaPile-Perm, so its 1.000 is memory. Nexus
reports GPT-4o alone at 0.78 / 0.81 / 0.56 and Nexus with GPT-4o at 0.88 /
0.89 / 0.83.

## The file

`pack.py` writes both stages as round 1b sized them — an inner node 11
bytes, a leaf 4 — and reads them back into arrays, as a reader in Rust
would hold them:

| | trees | inner nodes | leaves |
|---|---|---|---|
| first stage | 829 | 51 398 | 52 227 |
| second stage | 400 | 12 000 | 12 400 |
| `keys.bin` | | | 960 819 bytes, 938.3 KiB |

On all 712 774 pairs of the widened variant on these databases, the packed
first stage's margins are within 2.2e-5 of XGBoost's and the second stage's
within 5.5e-7 of LightGBM's on the same features; end to end, carried
through the graph features, 11 pairs differ by more than 1e-4, and of the
2 590 keys drawn none is drawn differently. LightGBM's
thresholds are doubles; the file keeps the largest f32 not above each, which
an f32 feature compares against exactly.

## Size against quality

The widened variant fitted again with fewer first-stage trees, the graph
stage of 400 trees each time:

| first-stage trees | first stage, KiB | F1 over every declared validation key | the paper's task, validation | F1, round 1b's task, external | the paper's task, Spider / BIRD / CTU |
|---|---|---|---|---|---|
| 25 | 23 | 0.655 | 0.846 | 0.748 | 0.909 / 0.892 / 0.885 |
| 50 | 46 | 0.659 | 0.848 | 0.750 | 0.911 / 0.898 / 0.882 |
| 100 | 91 | 0.669 | 0.849 | 0.755 | 0.911 / 0.895 / 0.889 |
| 200 | 182 | 0.676 | 0.850 | 0.755 | 0.911 / 0.898 / 0.884 |
| 400 | 365 | 0.685 | 0.851 | 0.754 | 0.914 / 0.898 / 0.886 |
| 829 | 756 | 0.690 | 0.853 | 0.760 | 0.913 / 0.900 / 0.886 |

Off SchemaPile the curve is nearly flat: a first stage of 23 KiB, with the
graph stage's 177, is within 0.012 of the whole model in round 1b's task and
within 0.008 in the paper's. The learning rate stayed at 0.05 for every size, which favours the larger.

## Three seeds

The widened variant fitted twice more, its negatives kept and both stages'
random states set by the seed, everything else as chosen:

| seed | F1 over every declared validation key | the paper's task, validation | F1, round 1b's task, external (P / R) | the paper's task, Spider / BIRD / CTU |
|---|---|---|---|---|
| 20261027, the chosen fit | 0.690 | 0.853 | 0.760 (0.848 / 0.689) | 0.913 / 0.900 / 0.886 |
| 1 | 0.688 | 0.853 | 0.760 (0.837 / 0.696) | 0.914 / 0.892 / 0.885 |
| 2 | 0.694 | 0.854 | 0.744 (0.844 / 0.665) | 0.911 / 0.892 / 0.888 |

The paper's task moves by under a point between fits; the open task's F1 off
SchemaPile by 0.016, about the gap between the reported and widened
variants there, so that gap is not a finding.

## What was found besides

- **Round 1b's letter grams depend on the process.** `keys.py` takes twelve
  trigrams of a set in iteration order, which Python randomises per process
  for strings; a name of more than ten letters gets a different twelve each
  run. Scored four times, the same model's external F1 moved between 0.8910
  and 0.8914. The Rust port takes them sorted.
- **Retrained from scratch** at the same configuration, round 1b's model is at
  test F1 0.768 against the reported 0.774 and external F1 0.892 against
  0.892 on CTU's first 48 databases.
- **80 of round 1b's 173 false keys** on the first 334 databases were Gemini's
  too, and read as keys the schemas did not declare (`staff.store_id` →
  `store`, `City.Country_id` → `Country`); labels are declarations, so
  precision here is a floor. A sheet for marking them by hand is in the
  paper's folder.

## Limits

- One fit of each variant, three of the chosen one; the variants differ on
  validation by less than 0.01 of F1 over every key, within what a refit
  moves, while the paper's task separates them by more than refits do.
- Gemini and T5 see what the paper's prompt shows, names only, in the pair
  task; our model also reads types and keys.
- The paper's sets are smaller and filtered against more; its own figures are
  read off a bar chart.
- CTU's tpce would be answerable if `decimal(p,0)` were read as an integer;
  that change was seen after the results and is not made.

## Environment and reproduction

| | |
|---|---|
| research commit | `c1163da` |
| models | round 1b's at `1524905`, `corpus/round-1p/models-1524905/`; the variants in `corpus/round-1p/variants/` |
| machine | AMD Ryzen 7 7700, RTX 5070 12 GiB, driver 610.88, Windows 11 Pro 26300 |
| Python | 3.13.15; XGBoost 3.4.1, LightGBM 4.7.0, scikit-learn 1.9.1, numpy 2.5.4; T5 on torch 2.11.0+cu128 and tokenizers 0.23.1 |
| Spider | `spider_data.zip`, SHA-256 `00636695dabed6b5f4b8328a16b13e069a2f16591d5efcce57660669c85b121b` |
| CTU | `mysqldump` 8.4.13 `--no-data` of relational.fel.cvut.cz, hashes in `corpus/round-1p/ctu-schema.sha256` |
| T5 | `tdoehmen/t5-schemapile-fk` at `cccb88fad028269594b6e33f2df4a9f571a51a3e` |
| Gemini | `gemini-3.8-flash-high` through a CLIProxyAPI, served as `gemini-3.8-flash-n`, temperature 0, 1 November 2026 |

The specification's run section has the commands in order.
