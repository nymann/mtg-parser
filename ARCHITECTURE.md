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

## The add-card orchestrator

`cargo xtask add-card [--set CODE] [--max-iterations N] [--dry-run]
[--allow-dirty]` is the find-next-card workflow with every deterministic
step automated. Only the creative step — extending the grammar to cover
a new pattern — is delegated to a fresh `claude -p` subagent.

If `--set` is omitted, the loop walks the tracked sets newest-first
and auto-advances to the next paper expansion (via the same logic as
`corpus-advance`) once the current set is fully covered. The loop
terminates with `SessionEndReason::CorpusComplete` when no more paper
expansion sets exist in Scryfall.

### Loop

For each iteration up to `--max-iterations`:

1. `find_next_failing_card` over the current set. If `--set` was
   explicit and the set is exhausted, stop with `AllPass`. If
   auto-advance is active, register the next paper expansion and
   continue with that as the new current set.
2. Snapshot the card and the current corpus pass count into
   `.add-card/<unix-ts>-<slug>/`.
3. Build the prompt (current `grammar.pest`, `ast.rs`, `lower.rs` inline
   plus the card, the round-trip error, and the constraint list) and
   write it to `prompt.md`.
4. `--dry-run` stops here without touching the working tree.
5. Promote the generated test (write it without `#[ignore]`).
6. Invoke `claude -p --dangerously-skip-permissions`, streaming output
   to `transcript.txt`.
7. `cargo xtask test --tier 2` as the test gate.
8. `cargo xtask corpus` as the regression gate (it exits non-zero on
   any previously-passing card that now fails).
9. `git commit` with a structured message:

       grammar: support card <name>

       Card: <name> (<set>)
       New passes: <count>
       Status: <total_pass>/<total>

Anything that goes wrong between steps 5 and 9 surfaces to the human:
the working tree is left as-is and the log dir keeps everything that
went into the run.

### What the orchestrator does *not* automate

- Architectural decisions (new AST node vs. attribute on existing).
- Regressions in `corpus_status.json` — stop-the-line.
- Property test failures unrelated to the new pattern.
- Snapshot test review.

### Prompt contract

Baked into the prompt and enforced by the gates afterwards:

- Don't modify the unparser. Round-trip failures signal grammar issues,
  not unparser issues.
- Don't add a special-case rule for one card. Generalize.
- Don't touch existing grammar rules unless necessary; additive is safer.
- Don't disable or modify existing tests.
- Stay within scope (`grammar.pest`, `ast.rs`, `lower.rs`, the generated
  test). Don't touch the adapters or xtask.
- If you can't solve it, say so. Don't ship a hack.

Inspired by [argentum-press/scripts/fix_parser_gaps.py](https://github.com/nymann/argentum-press)
— same deterministic-around-claude shape, simpler scope (one playbook,
one prompt template).
