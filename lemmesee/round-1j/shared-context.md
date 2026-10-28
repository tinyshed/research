# A shared context for lemmesee's tasks

Proposal, not a measured result. Written after the early-fusion and
residual experiments, in response to Dmitry's question about a common
semantic space.

## The architectural change

The existing stages pass decisions and hand-built features forward:
guessed keys, column roles, a question score, a title's keep score, and a
result's keep score. Passing a richer frozen representation is useful to
test, but is not joint learning: the representation still cannot change
when a downstream task discovers an error.

A shared contextual encoder would instead read one schema graph and feed
several task-specific heads. Losses from the heads train that same encoder.
The heads remain independently measurable.

    names, types, declarations, available profiles
                           |
                 typed schema graph
                           |
              shared contextual encoder
                 /         |          \
          key/role heads  question head  result/keep head
                           |
               constrained question compiler
                           |
                a budgeted look and choice

Only schema objects and bounded aggregate descriptions are nodes. A graph
of every stored row is outside this design's runtime and memory budgets.

## Two representations, with different jobs

The symbolic layer keeps table and column identity, declared and guessed
relations, cardinality evidence, row grain, a measure's expression and
aggregation, population and filters, time role, unit, and provenance.
Unknown stays unknown; a predicted key does not become declared evidence.
User confirmations belong here and survive a change of model.

The learned layer gives each column and question a small contextual vector.
For example, the representation of amount may depend on neighboring
currency and balance columns and on the table it belongs to. A shared
encoder can make that context available to every head.

## Practices to borrow

- Multi-task learning: one representation trained by several related
  objectives. See [Caruana, 1997](https://doi.org/10.1023/A:1007379606734).
- Contextual column typing: neighboring columns can disambiguate the same
  values. [Sato](https://arxiv.org/abs/1911.06311) combines table context and
  structured prediction rather than typing every column in isolation.
- Relation-aware schema encoding: preserve table/column relationships in
  the encoder. [RAT-SQL](https://arxiv.org/abs/1911.04942) is an example in
  text-to-SQL; its task and runtime are not lemmesee's.
- Structured prediction: the final assignment must respect compatibility
  constraints. A compiler checks grain and aggregation independently of
  the neural heads' confidence.

## A discriminating next experiment

Compare the same encoder three ways, holding its input and parameter
budget fixed:

1. Encode a column independently; fit each task separately.
2. Contextualise it with its table and declared neighbors; still fit tasks
   separately.
3. Use that same contextual encoder with jointly trained key, role and
   question/result heads.

This separates context from parameter sharing. A larger model alone would
not establish either benefit.

Use the existing declaration labels for key and enum tasks, and existing
teacher labels for question/result heads. Missing labels mask a task's
loss; they are not negatives. When a key declaration is the label, hide
that declaration from the input. Split by schema family before deriving
any learned features. Balance tasks so that millions of key labels do not
drown out the relatively few real-result labels.

Measure every head, not just the final keep count: a better keep head
must not hide worse key predictions. Count newly assembled dashboards as
sets with judges that taught no part of the candidate. Keep the present
numeric look as the control. Compare packed size and actual inference
memory only after the quality comparison passes.

The uncertain part is the supervision: a shared encoder cannot invent a
unit, the meaning of a status, or a business goal absent from its inputs.
Those remain questions for a person and explicit abstentions.
