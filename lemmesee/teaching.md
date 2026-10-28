# How lemmesee teaches and judges

lemmesee ships no language model: what `see` carries is rules, small models
of a few dozen kilobytes, and the bytes they are packed into. Language
models are used offline only, as **teachers** that write what the small
models learn from and as **judges** that say how good a result is. These
are the rules for both, from rounds 1c and 1d; a round that breaks one says
so and why.

## Teachers

- **A teacher sees what `see` would see, and nothing of ours.** For
  questions: a schema as tables, columns, types and keys, with a line on
  what the database holds; tier S only, unless the round is about
  statistics. Never the generator's candidates, the rules' choices or
  another teacher's answer.
- **One prompt asks the same thing every time**: the ten questions a person
  just given the database would want answered first, the first five for the
  first screen, each as base, measure, time, breakdown, filter and joins.
  The prompt is kept with the round.
- **Two teachers or more**, of different families where possible, on part of
  the same schemas, so their agreement is measured: it is the nearest thing
  to a ceiling until people judge.
- **Answers are kept as they came**, beside the report, with the model's name
  and the date; they are not reproducible by asking again.

## Judges

- **Blind and shuffled.** A judge sees a panel's title and the first rows its
  database returned — never its SQL, whose style tells makers apart, never
  which configuration chose it, never another judge's answer.
- **The form is round 1's**: keep on a first overview, true or false; does the
  picture mean what it seems to, yes, no or cannot tell; and if not, what is
  wrong — plus the line that a single bar, an empty result or a column of
  blanks is not worth a chart.
- **A panel's name is unique across everything ever packed**: the whole
  fixture's name, the pass and a number. Four letters of a fixture once gave
  two databases one set of names and one set of answers.
- **The judge that taught a model never counts it.** One judge's answers
  train; the others only count, each on the databases where it answered
  every panel of every five compared.
- **Every count says its spread**: kept of five averaged over databases, each
  difference bootstrapped over databases, 10 000 times, and the judges'
  agreement on keep, pair by pair.

## Which models, and how they are asked

| Use | Model | Asked through |
|---|---|---|
| labels at scale | Jev, TypeSafe's decision model | OpenRouter's decisions endpoint, each panel alone, a probability for each yes-or-no question; 845 panels for a few cents |
| a judge that teaches, or a second camp | Claude Sonnet at medium effort — **never Opus**, whose limits a round of judging exhausted | an agent reading one packet and writing one file; a few at a time |
| judges that count | Gemini Flash; DeepSeek; GLM | an OpenAI-style endpoint — a CLIProxyAPI for Gemini, which `agy` serves too slowly, OpenRouter for the others — prompts cut by database so each fits a command line, many at once |
| teachers of questions | Gemini Flash, Claude Haiku | the same endpoint, or an agent reading one batch of six schemas |

- **A key is read from where it is kept into the environment of the one
  command that uses it**, never printed, never written to a file, a report or
  the repository; a tunnel to it is closed when the command is done.
- **A reply is read for its last whole answer**: a model may write two.
- **An empty reply is a failure to ask again**, not an answer.

## Learning

- **Folds by repository or by database**, never a schema or a database on
  both sides; the score a second stage reads is out of fold.
- **A held-out set is chosen before it is looked at and measured once**:
  round 1c's second wave of 240 teacher schemas, round 1c's two final
  databases.
- **A fixture looked at is a development fixture from then on**; a pilot
  fixture is never one.
- **Measured against the teachers and the judges**, and against the rules it
  replaces on the same schemas; a learned part that does not beat them is
  not carried.
- **Every model is packed and its bytes reported**, with a reader that scores
  without its library and the largest difference from it.

## Production databases

Dmitry's databases are measured on, never learned from. Read-only: through
a tunnel, a session that cannot write, a time limit on every statement,
nothing reset; statistics as numbers only, no values; results drawn for him
alone into an ignored folder; nothing about them — no address, host, key,
table or column name — in the repository.
