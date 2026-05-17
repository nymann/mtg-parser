# Semantic Lowering Plan (Phase 4)

This is the deep-dive on Phase 4 of `codex-agentic-plan-phases.html`. The
phases doc names this layer "Semantic Lowering" and describes its job as
"map AST into semantic IR and preserve meaning across equivalent surface
forms." This plan decides what that IR actually looks like and how the
lowering pass is shaped.

It does not change the existing phase boundaries. Grammar, parser, and
unparse phases stay as defined in the phases doc and `ARCHITECTURE.md`.

## Goal

Lower syntactic `Statement`s (from `mtg-grammar`) into a semantic IR
that a rules engine can execute. The IR captures *meaning*, not surface
form: two ASTs that mean the same thing lower to the same IR.

## Principle

The IR is **compositional, not enumerated**. New effects are built from
a small fixed set of primitives, not added as new IR variants.

Three structural rules follow from this:

1. **Atomic primitives over named effect variants.** No
   `CardEffect::Scry`, `CardEffect::Surveil`, `CardEffect::Mill`. Those
   are compositions of the same primitives (`Gather`, `Select`, `Move`)
   with parameter variation.
2. **Named bins thread referents.** Every step that produces a
   collection names its output. Every step that consumes a collection
   references it by name. English "it" / "that card" / "those cards"
   are resolved during lowering by binding to the most recent
   compatible bin.
3. **Lowering is the single seat of Magic knowledge.** Zone
   transitions, referent resolution, last-known-information rules, and
   effect normalization all live in the lowering pass. Grammar stays
   shape-focused.

## Why Decompose Instead Of Enumerate

We already pay attention to enumeration-vs-generalization at the
grammar layer (see `AGENTS.md` "Generalize existing rules before adding
new ones"). The same axis applies to the IR.

Enumerated IR (one variant per concept) has the same failure mode
itemfive's grammar hit: a cross-product of trigger × action × referent
contexts blows up the variant count. A compositional IR with named bins
absorbs that cross-product into a small primitive set, with lowering
choosing how to compose.

Three concrete payoffs:

- **Referent resolution centralises.** "When this creature dies, return
  it to your hand" lowers to `Gather { source: SourceFromGraveyard,
  store_as: "it" }` followed by `Move { from: "it", destination: Hand
  }`. The zone-transition knowledge ("after Dies the source is in
  graveyard") lives in one lowering rule, not threaded through grammar
  parameters.
- **IR growth is sublinear in coverage.** Adding scry, surveil, mill,
  explore, dig, peek… is all the same primitive set with different
  compositions. No new IR types per concept.
- **Engine portability becomes real.** Argentum (wingedsheep's Kotlin
  rules engine) already uses Gather/Select/Move with named bins. If our
  IR matches the shape, Argentum can consume our output directly.
  Different engines converging on similar primitives is not coincidence
  — these are what the rules actually require to execute.

## The Split: Syntactic AST vs Semantic IR

The round-trip contract (`parse(unparse(ast)) == ast`) pins the
syntactic AST to named variants. The unparser needs to emit "Scry 2."
back, which means `Statement` must carry that named distinction.

The IR has no round-trip contract. Per `ARCHITECTURE.md`: "lowering is
total over every AST the parser can produce" — but there is no
`unlower` going back the other way. That freedom is what lets the IR
be compositional.

Concrete example:

```text
"Scry 2."
    │
    │ mtg-grammar::parse
    ▼
Statement::Scry { count: 2 }        ← named, round-trips
    │
    │ mtg-semantic::lower
    ▼
Composite([                          ← atomic, executes
    Gather { source: TopOfLibrary(2), store_as: "scried" },
    Select { from: "scried", mode: ChooseUpTo(2),
             store_selected: "to_bottom",
             store_remainder: "to_top" },
    Move   { from: "to_bottom", destination: LibraryBottom },
    Move   { from: "to_top", destination: LibraryTop,
             order: ControllerChooses },
])
```

Both directions of the parser stay clean. The IR doesn't have to
round-trip; the AST doesn't have to enumerate compositions.

## Primitive Categories

The primitive set is split into two groups. Imperative primitives drive
collection / selection / movement. Non-imperative primitives carry
effects that aren't a sequence of operations.

### Imperative collection primitives

Borrowed in shape from Argentum (`LibraryPatterns.kt`). Names are
illustrative; final Rust types should match `mtg-semantic`
conventions.

- **Gather** — produce a named collection from a source.
  - Axes: `source` (zone + filter + amount + player ref), `store_as`
    (bin name).
- **Select** — split a collection by player choice.
  - Axes: `from` (input bin), `selection_mode` (ChooseExactly,
    ChooseUpTo, ChooseAtLeast, All), `store_selected` (bin name),
    `store_remainder` (bin name), optional labels for prompts.
- **Move** — move a collection to a destination.
  - Axes: `from` (input bin), `destination` (zone + placement +
    ordering + player ref).

### Non-imperative effect primitives

These cover effects that imperative composition cannot express. Each
category is its own primitive shape; we are not forcing them into the
Gather/Select/Move mold.

- **Replacement** — replace one event with another. Source/recipient
  filter, replaced event, replacement event (which may itself be a
  Composite).
- **Continuous** — modify game state for a duration. Filter,
  modification (PT delta, ability grant, type/subtype add, color add,
  etc.), duration, layer.
- **Triggered** — observe an event and fire a Composite. Trigger
  condition, optional intervening-if, effect Composite. Referent
  bindings established here are visible to the inner Composite.
- **Static** — passive effect with no event. Filter, modification,
  layer.

The exact shape of each non-imperative primitive is open — see
"Open Decisions" below. The goal at this stage is the *category*
boundary, not the field-by-field schema.

## Named Bins And Referent Resolution

Every `Gather` and every `Select` *names* its output. Every consuming
step references by name. This is how the IR layer answers the "it" /
"that card" / "those cards" problem.

Lowering's job is to translate English referents into bin references:

| English referent | Resolution rule |
|------------------|-----------------|
| "it" after a Dies trigger | Bind to a `Gather { source: SourceFromGraveyard, store_as: "it" }` introduced by the trigger context |
| "it" after an Attacks trigger | Bind to the source object on battlefield (already named by the trigger context) |
| "that card" after an effect that moved a card | Bind to the bin produced by the prior `Move` or `Gather` |
| "those cards" after a Gather | Bind to the gather's `store_as` |
| "the chosen one" after a Select | Bind to `store_selected` |

The lowering rules table is finite. Once it's encoded, every English
phrasing that uses these referents resolves uniformly — no
parameter threading through grammar rules.

## Recognition Tags (Optional)

The execution engine does not need to know that a particular Composite
"is a scry." But debuggers, audit tools, and AI training pipelines
do.

Lowering may attach a recognition tag alongside a Composite:

```rust
TaggedEffect {
    tag: Some(Pattern::Scry { count: 2 }),
    effect: Composite([...]),
}
```

- Execution ignores `tag`.
- Audit/debug tools use `tag` to render "Scry 2" in human-readable
  form.
- Tags are not part of the IR semantic contract — two effects with
  different tags but identical Composites are semantically equal.

This is optional infrastructure. We do not need to ship it in the
first lowering. Defer until a concrete consumer (debugger, training
data emitter) needs it.

## Lowering Contract

The lowering pass `lower: Statement -> Result<CardEffect,
SemanticError>` must satisfy:

- **Totality.** `lower` succeeds for every `Statement` the parser can
  produce. Per `ARCHITECTURE.md`: "there is no partial lowering — if
  the parser accepts a card, the lowering must succeed." `SemanticError`
  is reserved for future reference-resolution and type-validation
  failures that don't exist yet.
- **No reverse direction.** No `unlower`. The IR is forward-only.
- **Normalization.** Two ASTs with the same meaning must lower to the
  same IR. Example: `Statement::ManaCost([Generic(1), Generic(1),
  Red])` and `Statement::ManaCost([Generic(2), Red])` lower to the
  same `ManaValue { generic: 2, red: 1 }`. Surface variation collapses.
- **Property test target.** A new tier-2 property test in
  `crates/mtg-semantic/tests/prop.rs`: generate 1000 ASTs, assert
  `lower(ast).is_ok()`. This matches the existing grammar prop test
  shape.

## Validation Strategy

Design bottom-up. Sketch the primitive categories before any specific
lowering. Validate against a small set of composite-heavy cards before
going wide.

Suggested validation cards (in order):

1. **Scry 2.** Exercises Gather + Select + Move + Move. The canonical
   composition that motivates the design.
2. **Surveil 2.** Same shape as scry with one destination changed.
   Confirms the primitive set composes both, not just one.
3. **"Mill three cards."** No Select step. Confirms composition is
   genuinely optional, not load-bearing.
4. **"When this creature dies, return it to your hand."** Cross-zone
   referent resolution. Confirms the named-bin protocol carries "it"
   through a Triggered → Composite boundary.
5. **"Draw two cards, then discard a card."** Two-stage composition
   with intermediate player choice on the discard. Confirms Select
   works from non-library sources.

If the primitive set cannot express any of these without bespoke
extension, the design needs another iteration before going wide.

## Phase 4 Maturity Updates

The concept registry (`grammar-concepts/<concept>.toml`) currently has
a `semantic_lowering` maturity field with no concrete criteria. Phase 4
needs a defined gate.

Proposed `[maturity]` gate criteria for `semantic_lowering = "green"`:

1. AST variant(s) for the concept have a lowering rule producing a
   Composite or non-imperative primitive.
2. Named bins are correctly bound for every referent the AST exposes.
3. Semantic equivalence fixture: for every grammar fixture's accepted
   examples, all of them lower to byte-identical IR.
4. Property test (`lower` on generated ASTs) does not regress.

Proposed registry schema additions:

```toml
[artifacts]
# existing fields...
semantic_fixtures = "semantic-fixtures/<concept>.json"
lowering_snapshot = "lowering-snapshots/<concept>.json"
```

The lowering snapshot is the canonical IR for the concept's grammar
fixture examples. Like AST snapshots, regenerated via `--update` and
diffed in PR review.

A new `xtask` command parallel to `concept-ast-test`:

```sh
cargo xtask concept-lower <concept> [--update] [--json]
```

Same shape as the AST snapshot runner. Without `--update`, fails if
the snapshot is missing or differs.

## Open Decisions

These are decisions left to make during implementation, not in this
plan.

1. **Adopt Argentum's exact shape or Rust-native equivalents?**
   Argentum's Kotlin types (`CardSource.TopOfLibrary`,
   `CardDestination.ToZone`, `SelectionMode.ChooseUpTo`) carry specific
   nesting choices that may or may not fit Rust idioms. The shape
   (axes they encode) is what we want; the literal types may not be.
   Recommendation: design Rust-native types that capture the same axes.
   Defer "wire-compatible-with-Argentum" until a real consumer asks
   for it.

2. **Continuous and replacement effect schema.** The primitive
   *category* is settled; the field-by-field shape is not. Replacement
   effects in particular have edge cases (multiple replacement effects
   on the same event, optional vs mandatory replacement, replacement
   self-reference) that deserve their own design pass before code.

3. **Public crate or internal use?** `mtg-semantic` is currently a
   `pub` crate but its IR is reached via `mtg_semantic::lower`. Whether
   external consumers (Argentum, future Python binding) should depend
   on the IR types directly, or only on a stable serialised form, is
   open.

4. **Recognition tag coverage.** Optional infrastructure; deferred.
   When we do add it, decide whether to tag every concept or only
   composites that mean a single named effect.

5. **Error type design.** `SemanticError` is currently uninhabited.
   First real variant arrives when lowering does reference resolution
   or type validation. Defer the enum design until the first failure
   mode is concrete.

## Out Of Scope

This plan does not decide:

- Grammar or AST changes. Phase 4 consumes Phase 2/3 outputs as-is.
- Rules engine implementation. We design the IR; engines (Argentum,
  others) execute it.
- Card enablement. Phase 5 still gates on full layer maturity per the
  phases doc.
- The orchestrator's lowering workflow. `xtask` commands sketched
  above are illustrative; actual command design happens in
  `xtask/src/`.

## See Also

- `ARCHITECTURE.md` — three-layer design, lowering contract, test
  tiers
- `codex-agentic-plan-phases.html` — the five-phase pipeline this
  plan sits inside
- `AGENTS.md` — generalization rules that apply to the IR layer the
  same way they apply to grammar
