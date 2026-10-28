# Round 1h: a student of names, taught where names are many

**Status: run on 28 October 2026, taught almost wholly by Jev as the
OpenRouter balance ran out; [reported](../reports/names-2026-10-28.md): the
student learns titles on SchemaPile, and the look reading it does no
better by the judges.** Round 1g found that the look predicts its teachers on
a database it has not seen at an AUC of 0.81 however it is built, that the
teachers' own five would keep a panel more, and that what separates them is
what names mean — a card number summed, a code broken down by, a technical
date — which 93 databases with data are too few to teach. Names are what
SchemaPile has most of: 22 989 schemas, and no rows to speak of. This round
teaches what a panel's name is worth where names are many, and what its
result shows where results exist.

## What is built

1. **Panels by title alone**: SchemaPile-Perm schemas that look like an
   application — round 1c's filter, one per repository — none whose tables
   overlap a judged database's by 40 % or more; round 1c's question and
   ranker on each, its thirty best questions, distinct, as titles.
2. **Their labels**: each panel asked by title of the three teachers the look
   already has — Jev, each panel alone with the schema's tables; DeepSeek
   and GLM, a schema's thirty titles at once with its tables, columns and
   keys, through OpenRouter's batch endpoint where a model offers it. No
   judge that counts is asked.
3. **What a word means, three ways**: round 1g's lexicon of 4 000 words;
   **vectors** of the 20 000 commonest words of SchemaPile-Perm's names from
   a small sentence-embedding model run offline on a graphics card, cut to
   16 numbers each by principal components and kept as bytes; and **a
   lexicon of values** — the commonest values in SchemaPile-Perm's columns,
   each said by DeepSeek to be a meaningful label or an opaque code.
4. **The student of names**: boosted trees on a panel's question —
   round 1c's ranker's numbers, the lexicon's, the vectors' of each place's
   words, and the keep rate of each word in each place counted on the
   schemas taught — learned on the teachers' mean keep by title, folds by
   repository. Its score of a panel is a number the look reads.
5. **The look**, round 1g's, reading the student's score and the values'
   lexicon besides its own numbers.

## How it is measured

As round 1g: 93 databases in 90 families, five folds, three draws; AUC
within a database against the teachers and against the judges that never
teach, on the 50 they answered; kept of five; and the best look's five
under round 1g's each-axis-once rule judged as a set by Sonnet and Gemini
beside round 1g's best and round 1's. The student of names never sees a
judged database, so it needs no folds of its own there; on SchemaPile it
is measured by repository folds against its teachers.

## Files

| Path | What |
|---|---|
| `names.py` | the schemas chosen, their panels by title, the teachers asked and gathered |
| `vectors.py` | the words' vectors on the graphics card, cut and packed |
| `values.py` | the commonest values and the lexicon of them |
| `student.py` | the student of names, taught and measured by repository folds, packed |

Round 1g's `diagnose.py` and `sets.py` count the look. Outputs live in
`lemmesee/corpus/round-1h/`, which git ignores.
