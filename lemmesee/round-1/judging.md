# Judging

Round 1's measure is what people keep and whether what they keep means what
it shows. Keeping alone is not enough: a sum of every order's amount is kept
gladly and is harmful. The form asks both, and the pilot measures the form as
much as the rules.

## What a judge sees

- **The panel**, drawn on the fixture, with its title, its line of reasons,
  its flags and its SQL.
- **The database**: its tables, columns and keys to browse, and its domain
  note — the source's own description for a public fixture, annotated
  SchemaPile's domain for a generated one, marked as a model's guess.
- **Not** which configuration proposed the panel, its rank or its penalties,
  and not the truth sheet of [traps.md](traps.md).

## The form

For every candidate:

| | Question | Answers |
|---|---|---|
| 1 | Would you keep this panel on a first overview of this database? | keep · drop |
| 2 | Does the picture mean what it seems to? | yes · no · cannot tell from what I have |
| 3 | If not, what is wrong? | wrong population: statuses, deleted rows · counted twice through a join · units or currencies mixed · a level summed · the wrong time · a code or identifier treated as a number · other, in words |
| 4 | What would settle it? | nothing more · more rows would · someone who knows the business must say |
| 5 | How sure are you? | sure · unsure |

The time from showing to answering is recorded; a skip is allowed and
recorded. **A panel is harmful when it is kept and its meaning is false** —
by the judge's answer to 2, and on a trap by the truth sheet.

After each fixture: **up to five questions you would want answered first
about this database that no panel answered**, in words. Each is later sorted
into one a template could express but the order did not show, and one no
template expresses: the generator's recall, which no ranking can fix.

## The protocol

- **The pool** of a fixture is the union of the top five of O, A, B1, B2 and
  M, and three more drawn with a seed from the rest of the pool, abstained
  candidates included: about 10 to 15 candidates, each once.
- **Blind and shuffled**: each judge sees the fixtures in an order of their
  own and the candidates of each in an order of their own, from a seed that
  is recorded.
- **Two judges, apart**, at least one of whom did not write the rules. Their
  answers to 1 and 2 are compared as given; where they differ they then agree
  on an answer, recorded beside the two. Measures use the agreed answer,
  agreement the original two.
- **The tool** is a local page or a terminal screen like `see`'s own, which
  reads candidates and drawings from local files and writes
  `decisions.jsonl`, and sends nothing anywhere.
- **About 120 judgments a judge**, ten fixtures of about twelve candidates;
  how long they take is one of the pilot's results.

## The measures

For each configuration, and for each engine where the configurations' top
five differ between engines:

| Measure | How |
|---|---|
| kept at five | the share of the top five kept, pooled over fixtures and for each kind — public, generated, trap — with an exact 95 % interval |
| harmful among shown | harmful panels over all top-five panels |
| harmful among kept | harmful over kept top-five panels: chapter 14's line, with its exact one-sided 95 % upper bound |
| harmful and confident | harmful panels the configuration ranked first or second |
| coverage | the share of fixtures given five panels, and of candidates not abstained |
| abstaining | of abstained candidates, the share that were harmful or dropped, where abstaining was right, and the share kept with a true meaning, where it hid a good panel |
| risk against coverage | harmful among kept against the share of candidates shown, as a threshold on the penalty moves |
| NDCG at five | gain 2 for kept with a true meaning, 1 for kept with a meaning nobody could tell, 0 otherwise; secondary, and harm is never folded into a gain |
| traps | caught, missed and not reached, as [traps.md](traps.md) counts them |
| recall of the generator | the share of the judges' unanswered questions a template could express |
| judging | median and 90th percentile seconds a judgment, minutes a fixture; agreement on 1 and on 2 as Cohen's kappa with the raw share beside it, since kappa misleads when one answer dominates |

## What the pilot can and cannot show

**It can show** whether the form works, how long judging takes and how far
two people agree; which failures occur, in which tier, and whether a ranking
or only a person could fix them; whether B1 changes what A shows, and by how
much on MySQL; how the rules stand beside Metabase and beside the schema's
own order; and which traps are caught.

**It cannot show chapter 14's lines.** With none harmful, at least 59 kept
guessed panels are needed to bound the harmful share below 5 % at 95 %, 93
with one and 124 with two; ten fixtures with five panels a configuration give
at most 50, fewer of them kept. The report gives the bound it reached, never
a pass. Nor can it rank configurations that differ little: differences come
with their intervals, and no winner is named where they overlap.

**It sizes round 2**: from the pilot's keep and harm rates, how many
fixtures and candidates make the lines showable for each engine and each kind
of fixture.
