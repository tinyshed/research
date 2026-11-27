# What the small open models would cost through OpenRouter

Collected at about 16:00 UTC on 26 November 2026 from OpenRouter's public
list of models (`https://openrouter.ai/api/v1/models`, no key; saved beside
this file as `openrouter-models.json`). No request was sent to any model and
nothing was paid. Prices are list prices in USD a million tokens, input /
output, of the cheapest listed route; a provider's own limits may differ.

## Which of the local models are hosted

None of the three. The list has no Qwen 3.5 under 9B and no Gemma 4 under
26B:

| local tag (Ollama, Q4_K_M) | on OpenRouter | nearest hosted |
|---|---|---|
| `qwen3.5:2b-q4_K_M` | **not hosted** | `qwen/qwen3.5-9b` |
| `qwen3.5:4b-q4_K_M` | **not hosted** | `qwen/qwen3.5-9b` |
| `gemma4:12b-it-q4_K_M` | **not hosted** | `google/gemma-4-26b-a4b-it`, `google/gemma-4-31b-it` |

## Tokens

Input, counted from round 1w's prompt files at 3.06 bytes a token, which is
what Qwen 3.5 and Gemma 4 took on the pilot's 46 replies (2.95 to 3.07).
Output, two cases from round 1w's saved replies on the same prompts: a model
that does not reason (Gemini 3.5 Flash-Lite's replies) and one that reasons
at length (DeepSeek V4.1 Flash's, 94 % of it reasoning). The worst case is
every reply at the protocol's limit of 32 768 tokens.

| set | schemas | input tokens | the largest schema | output, no reasoning | output, long reasoning | output, worst case |
|---|---|---|---|---|---|---|
| plain A | 718 | 1 652 825 | 72 584 | 584 037 | 5 897 819 | 23 527 424 |
| style 3 | 100 | 163 450 | 16 808 | 58 498 | 795 762 | 3 276 800 |
| the two clean sets | 818 | 1 816 275 | 72 584 | 642 535 | 6 693 581 | 26 804 224 |
| 398 external | 398 | 333 021 | 72 116 | 89 853 | 1 555 100 | 13 041 664 |
| LiveSQLBench | 22 | 23 595 | 1 622 | 8 098 | 194 699 | 720 896 |

The largest schema of every set is about 72 600 tokens, so it fits every
model below (131 072 or 262 144 tokens of context). It would not fit
`microsoft/phi-4` (16 384), which is left out for that. Second largest on
plain A: about 35 000.

## Prices and the estimate

| model | context | output limit | $ a million, in / out | clean sets, no reasoning | clean sets, long reasoning | clean sets, worst case | external + LiveSQLBench, no reasoning | the same, long reasoning | takes a reasoning setting |
|---|---|---|---|---|---|---|---|---|---|
| `qwen/qwen3.5-9b` | 262 144 | 235 929 | 0.10 / 0.25 | $0.34 | $1.86 | $6.88 | $0.06 | $0.47 | yes |
| `google/gemma-4-26b-a4b-it` | 262 144 | 235 929 | 0.09 / 0.30 | $0.36 | $2.17 | $8.20 | $0.06 | $0.56 | yes |
| `google/gemma-4-31b-it` | 262 144 | 16 384 | 0.20 / 0.40 | $0.62 | $3.04 | $11.08 | $0.11 | $0.77 | yes |
| `openai/gpt-oss-20b` | 131 072 | 117 964 | 0.03 / 0.13 | $0.14 | $0.92 | $3.54 | $0.02 | $0.24 | yes |
| `ibm-granite/granite-4.2-8b` | 131 072 | 117 964 | 0.06 / 0.15 | $0.21 | $1.11 | $4.13 | $0.04 | $0.28 | yes |
| `liquid/lfm-3-1.7b` | 262 144 | 8 192 | 0.08 / 0.20 | $0.27 | $1.48 | $5.51 | $0.05 | $0.38 | yes |
| `mistralai/ministral-3b-2512` | 131 072 | 104 857 | 0.10 / 0.10 | $0.25 | $0.85 | $2.86 | $0.05 | $0.21 | no |
| `mistralai/ministral-8b-2512` | 262 144 | 209 715 | 0.15 / 0.15 | $0.37 | $1.28 | $4.29 | $0.07 | $0.32 | no |
| `google/gemma-3-12b-it` | 131 072 | 16 384 | 0.05 / 0.15 | $0.19 | $1.09 | $4.11 | $0.03 | $0.28 | no |
| `meta-llama/llama-3.2-3b-instruct` | 131 072 | 117 964 | 0.05 / 0.33 | $0.30 | $2.30 | $8.94 | $0.05 | $0.60 | no |

Notes on the list:

- `google/gemma-4-31b-it` and `google/gemma-3-12b-it` are capped at 16 384
  output tokens and `liquid/lfm-3-1.7b` at 8 192, under the protocol's
  32 768; a reply cut there counts as no key, as everywhere.
- `google/gemma-4-26b-a4b-it:free`, `google/gemma-4-31b-it:free` and
  `liquid/lfm-3-1.7b:free` are listed at no charge (output capped at 32 768
  and 8 192); free routes are rate-limited and may log prompts, so they are
  not counted on here.
- `openai/gpt-oss-20b:batch` (0.024 / 0.112) and
  `mistralai/ministral-8b-2512:batch` (0.075 / 0.075) exist. A batch holds
  its worst case on the key at submit (round 1w); a request at a time does
  not.
- A hosted model is the publisher's weights at full precision as its
  provider serves them. The local tags are 4-bit quantisations (Q4_K_M). The
  two are not the same model, and the difference is itself a measurement.

## Totals

All ten models on the two clean sets: **$3.05** if none reasons, **$16.10**
if every one reasoned as long as DeepSeek did, $59.54 at the worst case.
The three that matter most (`qwen/qwen3.5-9b`, `google/gemma-4-26b-a4b-it`,
`openai/gpt-oss-20b`) on the clean sets: **$0.84 to $4.95**; with the 398
external databases and LiveSQLBench, **$0.98 to $6.22**. Worst case for
those three on the clean sets, $18.62.

## The split proposed

**Through OpenRouter, every schema, no sample** (a request at a time, each
model's default reasoning as round 1w's protocol, and once more with
reasoning off where the route takes the setting):

1. `qwen/qwen3.5-9b`: the same family as our two local Qwen tags and the
   largest of it that fits the 12 GB card (`qwen3.5:9b-q4_K_M`, 6.6 GB), so
   it is the one model that can stand both hosted and local.
2. `google/gemma-4-26b-a4b-it`: the Gemma 4 a reviewer would name; it does
   not fit the card (16 to 19 GB as 4-bit files), so hosted is the only way
   to give it a figure. It answers "what if the user had a bigger card".
3. `openai/gpt-oss-20b`: the other small open model a reviewer would name;
   about 13 GB locally, over the card.
4. Optional, for the bottom of the size axis: `liquid/lfm-3-1.7b` and
   `ibm-granite/granite-4.2-8b`, about $0.5 to $2.6 more.

**Kept local**:

- `qwen3.5:2b`, `qwen3.5:4b` and `gemma4:12b` **with reasoning off, every
  schema**: they are not hosted, so their F1 can only be local, and the
  pilot shows it is cheap: a median of 0.2, 2.4 and 3.9 s a schema on its
  twelve small schemas, an hour or two of the card for all three on the two
  clean sets (an estimate; plain A's schemas are larger).
- Their **default-reasoning** setting only on a sample: on the pilot the 2b
  model reasoned until the window was full on all six prompts it was given
  (about 64 s each, no answer) and the 4b model on two of four (about 99 s
  each), which is a finding of its own and not worth 15 hours of
  the card.
- `qwen3.5:9b-q4_K_M` on **a sample of about 50 schemas** (drawn by the
  README's seed and bands): seconds a schema, peak memory of the card and
  the host, and the agreement, key by key, between the hosted
  full-precision model and the local 4-bit one on those same schemas.

What this buys: the paper's size axis gets real F1 at 9B, 20B and 26B on all
818 clean schemas for a few dollars, and the claim about a local machine
rests on measured time and memory, with the quantisation's cost measured and
not assumed.
