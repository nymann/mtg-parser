# Architecture

Three layers, strictly separated (ports & adapters):

1. `mtg-grammar` — Card text → syntactic AST. Pure CFG via `pest`.
2. `mtg-semantic` — Syntactic AST → semantic IR. Attribute pass that
   resolves references, validates types, and normalizes effects.
3. Adapters (`mtg-scryfall`, `mtg-corpus`, later `mtg-py`) — Outer layers
   that bring data in or expose the core out. The core has no knowledge
   of where text comes from or where results go.

## Public ports

- `mtg_grammar::parse(text: &str) -> Result<Statement, ParseError>`
- `mtg_grammar::unparse(stmt: &Statement) -> String`
- `mtg_semantic::lower(ast: &Statement) -> Result<CardEffect, SemanticError>`

## Semantic IR

The IR is what downstream consumers (game engines, deck analyzers,
training data, eventually the Python binding) actually look at. The
syntactic AST captures the *shape* of the text; the IR captures the
*meaning*. Two surface forms that mean the same thing should lower to
the same IR.

Example: `{1}{1}{R}` and `{2}{R}` have different syntactic ASTs but
both lower to `ManaValue { generic: 2, red: 1, .. }`.

Today the IR is:

- `CardEffect::ManaCost(ManaValue)` — per-color and generic counters,
  with a `total()` helper for the mana value.
- `CardEffect::DestroyTargetCreature` — placeholder. As the grammar
  grows, this collapses into something like `Effect::Destroy { target:
  TargetSpec::Creature }` with explicit target-shape data.

### Lowering contract

Lowering is **total** over every AST the parser can produce. There is
no "partial lowering" — if the parser accepts a card, the lowering
must succeed. `SemanticError` is uninhabited today; real variants
arrive once lowering does reference resolution or type validation.

Tier 2 enforces this with a 1000-case property test in
`crates/mtg-semantic/tests/prop.rs`.

## Canonical form

The unparser emits Scryfall-style Oracle text. Every value the grammar
can produce has exactly one canonical printed form; that string is the
one the unparser emits.

### Mana costs

- Each symbol is a curly-brace token: `{2}`, `{R}`, `{C}`.
- Symbols are concatenated with **no whitespace** between them. The
  parser rejects `"{2} {R}"`.
- Generic mana uses decimal digits with no leading zeros.
- Color codes are single uppercase letters: `W`, `U`, `B`, `R`, `G`, and
  `C` for colorless. Lowercase letters are not accepted as input.

Examples:

| AST | Canonical form |
|-----|----------------|
| `Generic(2), Red, Red` | `{2}{R}{R}` |
| `White, Blue, Black, Red, Green` | `{W}{U}{B}{R}{G}` |
| `Generic(0)` | `{0}` |

### Sentence-form effects

- Sentences begin with a capital letter.
- Sentences end with a `.`.
- Tokens within a sentence are separated by a single ASCII space.

Examples:

| AST | Canonical form |
|-----|----------------|
| `DestroyTargetCreature` | `Destroy target creature.` |

The parser accepts case-insensitive sentence text (`"DESTROY target
creature."` parses), but the unparser always emits the canonical
capitalization.

### The round-trip contract

For every AST `a` the parser can produce:

```
parse(unparse(a)) == a
```

This is enforced by a `proptest` over 1000 generated ASTs in
`crates/mtg-grammar/tests/prop.rs`. A round-trip failure is the
clearest signal of either grammar ambiguity or unparser drift, so
failures there block progress until the underlying disagreement is
resolved (rather than papered over by tweaking the unparser).

## Test tiers

| Tier | Budget | Scope |
|------|--------|-------|
| 0    | <5s    | `cargo check`, clippy, grammar compiles |
| 1    | <1s    | Hand-written unit tests per rule, curated snapshots |
| 2    | <10s   | Property tests (round-trip, lowering totality, IR invariants) |
| 3    | minutes | Full Scryfall corpus diff against `corpus_status.json` |
| 4    | on-demand | `criterion` benchmarks |
| 5    | one-off | Differential test vs. the Python/Lark parser during migration |

Tier 1 staying under 1s is a hard rule. Slow tests move to a higher tier.
