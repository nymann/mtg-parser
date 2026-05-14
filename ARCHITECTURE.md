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
- `mtg_semantic::lower(ast: Statement) -> Result<Ir, SemanticError>` (later)

## Canonical form

The unparser emits Scryfall-style Oracle text:

- Mana symbols are curly-brace tokens with no whitespace between them
  (`{2}{R}{R}`).
- Sentence-form effects are capitalized and terminated with a period
  (`Destroy target creature.`).

Round-trip: `parse(unparse(ast)) == ast` for every AST the parser
produces. This is the contract the grammar and unparser must keep.

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
