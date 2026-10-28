# Round 1j: meaning and results before the bottleneck

Status: harness prepared; no learning experiment run.

Round 1h trained a student on titles and passed its single keep probability
to the look. That tests late fusion of two decisions. It does not test
whether an aspect of a name that is insufficient to judge a title becomes
useful together with the actual result. This round tests that distinction.

The primary hypothesis is early fusion: keep the existing per-slot word
vectors and lexical properties until the model also reads the result.
A linear model with explicit semantic/result interactions is a second
architecture, because axis-aligned trees need not exploit word-vector
geometry efficiently. No new teacher, paid service, database query, private
data, or additional corpus is involved.

## Frozen comparisons

All models use round 1g's 93 public databases, its family grouping, five
folds and the same three draws. The targets are exactly the existing three
teachers' mean keep. Opus, Sonnet and Gemini never train or select a model.
The old databases are development data; this is not a fresh held-out test.

| Name | Input | Learner |
|---|---|---|
| baseline | round 1e's numeric features | original 150 trees, four leaves |
| capacity | the same numeric features | 400 trees, 15 leaves |
| late | numeric features and round 1h's scalar score | 400 trees, 15 leaves |
| early | numeric features, per-slot semantic features and result representation | 400 trees, 15 leaves |
| joint | early input, and semantic/result products | regularised logistic regression |

After that first comparison failed to improve consistently, a second pass
adds residual learning. A numeric baseline is retained, and a ridge-100
logistic correction reads numbers, the late scalar, or the early joint
features. The correction learns against baseline margins obtained by
three inner folds strictly within each outer training fold. Its outer-test
offset is the baseline fitted on the complete outer training fold. This
tests correction of a useful nonlinear model, rather than replacing it by
a weaker linear one. All three controls are reported. These are follow-up
development experiments, not an independent confirmation.

A third development pass tests the literal learned-representation
bottleneck: round 1h's compiled title student is frozen, and the look
receives either the sum of its 600 tree contributions, all contributions,
or their fixed 64-dimensional random projection. The same compiled
encoder is used for all three controls. This differs from feeding generic
word vectors directly: the representation was already learned from titles.
No counting labels fit the encoder or projection.

Prepare this extra input with:

    docker run --rm --cpus=4 -v <repo>:/src -w /src/lemmesee lemmesee-joint python round-1j/latent.py

Run its controls with joint.py run --configs baseline capacity
encoded_scalar encoded_64 encoded_600.

Candidates without a teaching label remain eligible at evaluation; only
their training rows are excluded. Removing an unlabelled candidate from
the evaluation pool would change the task.

The lexical properties and 16-dimensional word vectors are the frozen
public-schema resources already built by rounds 1g and 1h. They never use
the counting judges' labels. Result representation preserves such things
as the magnitude and spelling of numeric strings and how labels are
written; it does not add a rule derived from a counting judge's verdict.
The joint model's regularisation is fixed before the experiment.

Every candidate is scored out of family. Feature normalisation is fitted
on the training fold only. The primary measure is mean within-database AUC
against each independent judge. Kept and harmful among the five are
secondary diagnostics, counted both with no penalties and with the existing
axis rule. Resample families for paired confidence intervals. Report every
configuration, not just the winner. Existing per-panel labels cannot
establish the quality of a newly assembled five as a set.

## Reproduction

Build the image from the repository root:

    docker build -t lemmesee-joint -f lemmesee/round-1j/Dockerfile lemmesee/round-1j

Check the harness without fitting on the corpus:

    docker run --rm -v <repo>:/src:ro -w /src/lemmesee/round-1j lemmesee-joint python -m unittest -v

After the harness is committed, prepare the frozen inputs and run in Linux:

    docker run --rm --cpus=4 -v <repo>:/src -w /src/lemmesee lemmesee-joint python round-1j/joint.py prepare
    docker run --rm --cpus=4 -v <repo>:/src -w /src/lemmesee lemmesee-joint python round-1j/joint.py run

Inputs and learned artifacts go under the ignored corpus/round-1j/.
The manifest records hashes of the source files and prepared arrays, Git
revision and dirty paths, dependency versions, Linux environment, and fold
assignment. The loader reads existing public snapshots and aggregates
only; it connects to no database and makes no network request.

Packed tree thresholds retain float64. The old float32 threshold format
can move a threshold across an adjacent float32 vector feature and change
the chosen leaf. LMJ2 checks all models against the library reader.

Repository AGENTS.md forbids committing without a request, while measure
requires the harness to be committed before measuring a round. Ask for
authorization for this round's commit after its source and checks are
reviewable; do not run a learning comparison before then.
