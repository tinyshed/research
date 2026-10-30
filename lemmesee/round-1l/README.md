# Round 1l: the ranker taught the product's way

Status: run on 30 October 2026 as a development probe; not reported, the
harness not yet committed. No ranker is kept: the product's question moves
the money panels up on the owner's databases and nothing on the 93 public
ones by the product judges.
[Reported](../reports/screen-2026-10-30.md) with rounds 1l to 1o.

Round 1k found the look's fives poor for a product: on the owner's two
databases the money panel and the health panel were made by the generator
but placed 27th to 47th by round 1c's ranker, and the look reads its ten
best; on the 93 public databases the product judges keep 1.9 of the look's
five, while 3.0 to 3.6 are there to be chosen. Teaching the look by the
product's question did not help: the teachers asked it choose no better
than before, and the look cannot follow what they see
([handoff](../round-1k/HANDOFF.md), point 4).

This round asks whether the ranker, which decides what the look is given,
moves the product's panels up when its teachers are asked the product's
question.

## Teachers

Round 1c's 480 SchemaPile-Perm schemas in its 80 batches of six, its prompt's
role replaced (`teach.py`): the analyst responsible for what the database
serves, building the screen they open every week to see how it is going —
who and what is active and how much, what it earns and costs, what succeeds
and fails, how that changes — with an inventory of a table, technical
bookkeeping and the same activity counted again from another table left
out. The answer's form is round 1c's, so its `learn.py` reads it.

| Name | Model | Route |
|---|---|---|
| `luna` | GPT-6 Luna | Codex on a ChatGPT Plus subscription, behind the CLIProxyAPI |
| `deepseek` | DeepSeek V4.1 Flash | OpenRouter |
| `jev` | Jev 1.13 | OpenRouter's decisions endpoint (`jevrank.py`) |

Jev answers a question put to it and cannot write ten, so it is asked of
the generator's own: for each schema, the fifty best by round 1c's ranker,
each alone beside the schema's text, whether the same analyst would put it
on that screen, as a chance. Its first five by chance stand as a teacher's
first five and its next five as the rest of its ten. A question outside the
fifty is one Jev did not choose, so it leans on round 1c's ranker there.

gpt-oss-120b through Antigravity was tried first and dropped: past ten
batches the free route answered 429, and its questions read weaker.

None of them counts: the product judges are Gemini (`pmgemini`) and Sonnet
(`pmsonnet`), and Haiku (`pmhaiku`) where a pool is new.

## The rankers

`ranker.py` fits round 1c's LambdaRank, its features, labels (3 in a
teacher's first five, 2 in its ten, 1 sharing base and measure or base and
both dimensions, the larger of two teachers'), its 600 trees of seven leaves
and its five folds by repository, once for each set of teachers, in one
environment:

| Set | Teachers |
|---|---|
| first teachers | Haiku and Gemini, round 1c's own; the refit matches the shipped ranker |
| luna and deepseek | the two that write the product's questions |
| jev | Jev alone |
| luna, deepseek and jev | the product's three |
| all five | the five together |
| votes of the three, of the five | the same teachers by consensus: their labels summed, 0 none, 1 for 1–2, 2 for 3–5, 3 for 6 and more, so what two teachers put first outranks what one did; the sets above take the largest label, so one teacher's whim counts as much as all agreeing |

## How it is measured

| Measure | What |
|---|---|
| `cv` | each set's five out of fold against every teacher it did not see, round 1c's precision, recall and NDCG@5 |
| `judged` | on the 93 public databases, each set's five and its AUC against the panels the first and product judges already saw, with the share of its five they saw |
| `pool`, `count` | each set's ten and five run on the local copies of the 93 databases and judged anew by one pair for all, so no five is counted by judges that saw only some of it |
| `live` | on the owner's databases, each set's ten best and where the panels the owner and the owner's bot asked for fall; printed, never written |

Development comparisons on databases already looked at.

## What it found

**The writers ask what they asked before.** Of Luna's 4 800 questions the
generator says 69 %, of DeepSeek's 76 %, against Gemini's 83 %. Out of fold,
round 1c's ranker, never taught by them, picks their questions nearly as well
as a ranker taught by them: NDCG@5 against Luna 0.503 and 0.532, against
DeepSeek 0.552 and 0.565, precision equal. DeepSeek's five falls in Gemini's
ten 0.63 of the time and in Haiku's 0.66, against 0.72 and 0.73 for Haiku
and Gemini.

**Jev agrees with no one**: its five in any writer's ten 0.29 to 0.37. It
prefers sums, statuses and breakdowns by a parent's key.

**On the 93 public databases, by the judges' existing panels** (round 1j's
frozen set; a five's or ten's panels the judges never saw are left out, 2 to
10 % of them for the writers, 15 to 20 % for Jev; host, round 1b's
environment), the difference from round 1c's teachers' fit, per panel kept
by the product judges, bootstrapped over families:

| Taught by | in its five | in its ten | AUC |
|---|---|---|---|
| luna and deepseek | +0.007 [−0.016, +0.030] | +0.014 [−0.002, +0.031] | +0.030 [−0.004, +0.068] |
| jev | −0.056 [−0.102, −0.009] | −0.017 [−0.046, +0.014] | −0.062 [−0.126, −0.000] |
| luna, deepseek and jev | +0.005 [−0.023, +0.033] | +0.009 [−0.011, +0.030] | +0.029 [−0.010, +0.074] |
| all five | +0.007 [−0.018, +0.035] | +0.007 [−0.008, +0.024] | +0.034 [−0.000, +0.073] |
| votes of the three | −0.008 [−0.030, +0.014] | +0.006 [−0.011, +0.024] | −0.002 [−0.039, +0.036] |
| votes of the five | −0.007 [−0.024, +0.011] | +0.008 [−0.007, +0.025] | +0.005 [−0.025, +0.035] |

By the first judges every new fit loses 0.02 to 0.14 a panel. The refit of
round 1c's teachers scores as the shipped ranker to three places. With so
few panels unseen, judging the new pools anew could not change this, and
was not done.

**On the owner's two databases** (places only; what was asked for stays on
the owner's machine) the product's teachers move the money up and the
health down. On one, the two money panels the owner's bot asked for stand
27th and 47th by round 1c's ranker; 7th and 52nd by Luna and DeepSeek's,
1st and 8th by the three's, and Jev's alone puts one 2nd. The health panel
moves from 21st to 37th and below. On the other, the fives fill with money:
Jev's with five sums by parents' keys, the writers' with one sum over two of
its times.

## Reproduction

From the research repository's lemmesee folder:

    python -I round-1l/teach.py batches corpus/round-1c/teacher corpus/round-1l/teacher
    TEACHER_KEY=<key> python -I round-1l/teach.py ask corpus/round-1l/teacher <endpoint> gpt-6-luna luna 4
    TEACHER_KEY=<key> python -I round-1l/teach.py ask corpus/round-1l/teacher https://openrouter.ai/api/v1 deepseek/deepseek-v4.1-flash deepseek 40
    JUDGE_KEY=<key> round-1b/.venv/Scripts/python -I round-1l/jevrank.py ask corpus/round-1l/teacher corpus/round-1c/learn 50 40

In the Linux container of round 1j:

    docker run --rm --cpus=4 -v <repo>:/src -w /src/lemmesee lemmesee-joint python round-1l/ranker.py build corpus/round-1c/teacher corpus/round-1l/teacher corpus/round-1l/learn
    docker run --rm --cpus=4 -v <repo>:/src -w /src/lemmesee lemmesee-joint python round-1l/ranker.py cv corpus/round-1l/learn
    docker run --rm --cpus=4 -v <repo>:/src -w /src/lemmesee lemmesee-joint python round-1l/ranker.py fit corpus/round-1l/learn
    docker run --rm --cpus=4 -v <repo>:/src -w /src/lemmesee lemmesee-joint python round-1l/ranker.py judged corpus/round-1l/learn corpus/see corpus/round-1j

The pool runs on the host, beside round 1's database containers, in round
1b's environment:

    round-1b/.venv/Scripts/python -I round-1l/ranker.py pool corpus/round-1l/learn corpus/see corpus/round-1j corpus/round-1l/look

The teachers' answers, the rankers and the pools stay in the ignored corpus.
