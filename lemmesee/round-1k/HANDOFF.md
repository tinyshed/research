# Handoff: lemmesee after round 1k, 29 October 2026

For whoever continues without this session's context. Read first
`../../AGENTS.md`, `../teaching.md`, `../plan.md`, the report
[ceiling-2026-10-29.md](../reports/ceiling-2026-10-29.md) and this round's
README. Everything about the owner's production databases (hosts, containers,
table names, the owner's judgments) is kept only locally, in
`corpus/prod/HANDOFF-prod.md`; none of it may be written to the repository.

## What is established (in round 1k's report, pushed)

- The look's five without a person has reached the model judges' ceiling:
  data (GitTables) and features (a name's head word, a column's spread) move
  it by less than 0.1 of a panel.
- The measure: Sonnet and Gemini now label all 93 databases; the threshold is
  0.12 of a panel for a change of one panel and about 0.22 for a change of
  model. Comparing fives pairwise is two to three times worse than counting.
- The teachers Jev (fast, keep alone) and Luna (GPT-6 Luna through Codex, a
  chance from 0 to 100) replace DeepSeek and GLM at no loss.
- One click by a person (dropping the worst panel) keeps +0.17 of a panel,
  z ≈ 4, more than any model.

## What was done after the report (not in it; to be written up as round 1l)

1. **`see` on the owner's living databases**, `live.py`: the pool is built on
   the database's host through `docker exec psql` in a read-only session
   (0.5 s a statement), the look is taught on the 93 public databases alone,
   and a blind page with charts is made for judging. The owner and the
   owner's bot said the same thing independently: neighbouring timelines of
   activity repeat each other; the money panel and the health panel were
   found only by round 1's rules, the ranker places them 27th to 47th, and
   the look reads its ten best.
2. **Product judges** `pmgemini` and `pmsonnet` (`product.py`, the role: the
   analyst of what the database serves, building the screen they open every
   week) labelled all 93 databases. `product_count.py` writes
   `corpus/round-1k/product/product-judges.json`:
   - they agree with the first judges on 84 % of decisions and keep fewer
     (Sonnet 24 → 20 %, Gemini 28 → 18 %): they drop counts of rows over time
     and by breakdowns, while Sonnet keeps more sums and money;
   - the look's five with the axis rule keeps 1.90 of five by them (2.88 by
     the first judges); round 1's rules 0.95, round 1c's ranker 1.52;
   - **the best five the pool holds keeps 3.56**: the generator makes the
     product's panels, and the ranker and the look place them low;
   - changes to the choice (at most two timelines of activity, one time a
     measure, a seat for money): only the seat for money keeps more,
     +0.09 [0.00, +0.18];
   - their "instead" asks for revenue, sales, spend, active, rate, weekly
     sales, wins by team, average rating, graduation rate; some of these are
     shares, which the generator cannot make.
3. **Product teachers**: `pmjev` is done (`jevfast.py --product`, $0.07 for
   every panel). `pmlunap` (Luna with the product's form and a chance) was
   running at handoff and has since finished. To run it again, what is done
   is skipped:

       CLIPROXY_KEY=<key> PRODUCT_CHANCE=1 python -I round-1k/product.py ask corpus/round-1d/look <endpoint> gpt-6-luna pmlunap 4
       python -I round-1k/product.py gather corpus/round-1d/look pmlunap      # and the same for 1e, 1f

4. **The look taught by the product teachers: the hypothesis failed**
   (30 October, in `lemmesee-joint`, the harness not yet committed). A bug in
   `jevfast.py` (the teacher's name overwritten in a loop) had written
   `pmjev`'s answers to `basketball-11-15-*`, `gosales-3-33-*` and
   `birdretailcomplains-4-31-*`; they are renamed `pmjev-*` and the bug is
   fixed. `product_look.py` (the look, each axis once, against the first
   teachers):

   | The look taught by | first judges | product judges |
   |---|---|---|
   | the first teachers | 2.89 | 1.89 |
   | Jev and Luna | +0.07 [−0.03, +0.18] | +0.09 [+0.01, +0.18] |
   | `pmjev` and `pmlunap` | −0.75 [−1.02, −0.49] | −0.16 [−0.40, +0.08] |
   | all four | −0.15 [−0.30, −0.00] | +0.10 [−0.03, +0.24] |

   `product_teachers.py`, the teachers' own five without the look, by the
   product judges: Jev and Luna 2.45, the product teachers 2.38 (keeping only
   10 %), all four 2.65, the product judges themselves 3.04. The product's
   question gives the teachers almost nothing, and the gap of about 0.66
   between the teachers and the look is the look's own: its 127 numbers do not
   hold what the teachers tell a product's panel by (as round 1g found). On
   the living databases a second gap comes before the look: the ranker places
   the money and health panels 27th to 47th.
5. **`see`'s architecture for every kind of store** is written into
   `../../dashbin/plan/11-api.md` ("One question for every kind of store"): a
   snapshot reader for each kind of store, the analytical question as one
   plan, a compiler to each query language, and the look and the choice
   reading results alone; the first version covers relational databases and
   TinyStore.

## What next, in order

1. Done: `pmlunap` finished and `product_look.py` run; the look does not gain
   (point 4 above).
   Done: round 1c's ranker on the product's question, in
   [round 1l](../round-1l/README.md) (30 October, not committed). Luna and
   DeepSeek write nearly the questions Haiku and Gemini wrote; Jev, scoring
   the generator's questions, agrees with no one; on the 93 databases no
   ranker keeps more by the product judges; on the living databases the money
   rises into the ten and the health falls. gpt-oss through Antigravity was
   dropped (429).
   Done: [round 1m](../round-1m/README.md) (30 October, not committed): a
   bench of 12 living databases with planted stories (their specs written by
   Haiku agents) and the five as a screen of roles (pulse, money, health over
   time, speed, people) over all the generator's questions. Over Luna and
   DeepSeek's ranker the screen shows over time 16 of 29 stories against round
   1c's 10, +6 [+3, +9]; on the owner's busiest database it holds four of the
   five panels the owner's bot asked for.
   The owner judged the screen blind on 30 October: the closest of four on
   the busiest database, its people panel not wanted; on the other database
   none of the four, beside the owner's own product dashboard, eleven panels
   of which the generator can say three (round 1m's README).
2. Started: [round 1n](../round-1n/README.md), numbers first (30 October,
   not committed): people, the thing done (a status's done values read from
   the database), who did it, who did it twice, the shares, the funnel, the
   new in 7 and 30 days, a month's money, a week's failures. On the database
   the owner keeps a dashboard for, six of its eight numbers come out exactly;
   on the bench it finds people and the thing in 8 schemas of 12. Next: judge
   the numbers, money by kind, percentiles (p95 of a delay).
3. A second button: carrying a person's answer over by the columns' roles
   (carrying it over by shared axes was tried and hurts).
4. The move to Rust and `dashbin see` v1; checks by people on 10 to 15 public
   databases.
5. Not checked: leakage from SchemaPile into round 1c's ranker (15 % of our
   databases' tables have a near copy there); for `gemini-3.8-flash-high`
   cliproxy serves `gemini-3.8-flash-n`.

## Rules easy to break

- The repository is public and written in English: reports, READMEs, code
  and handoffs alike. Russian belongs only in the ignored corpus.
- Commits: one Conventional Commits line, no body and no trailers; a round is
  `test(spike):`, a report `docs:`; commit and push only when asked.
- A judge that taught does not count; judge agents are Haiku, or Sonnet at
  medium effort, never Opus.
- A new judge's name must not begin with an older judge's: `labels()` takes
  `<judge>-*.json`, and `gemini-pm-1.json` would replace `gemini`'s answers.
- Keys are read into one command's environment and never printed.
- The owner's production data is sent to no judge and no teacher and never
  enters the repository; only counts leave the machine.
