# AGENTS.md

Instructions for AI coding agents. Three rules and a pointer.

## Generalize existing rules before adding new ones

This project's defining failure mode is one-rule-per-card explosion in
`crates/mtg-grammar/src/{grammar.pest,ast.rs,parse.rs,unparse.rs}`. Two
existing rules that differ only in one axis (verb, subject, recipient,
quantity, polarity) are a refactor waiting to happen — fold the axis
into one rule and add the new card's variant as data. Three such rules
is a stronger signal. The corpus regression gate catches breakage from
widening a rule, so you do not need to be additive to be safe. If you
must add a new pest rule, write one sentence in your final message
explaining why no existing rule could be widened.

## Do not hand-edit generated test files

`crates/mtg-grammar/tests/generated/` and
`crates/mtg-grammar/tests/generated_patterns/` are owned by
`cargo xtask add-card`. The next orchestrator run will overwrite any
hand-edits. To change the shape of a generated test, change the
template in `xtask/src/add_card.rs`.

## A failing round-trip test points at the grammar, not the unparser

The round-trip law `parse(unparse(ast)) == ast` is enforced by
`crates/mtg-grammar/tests/prop.rs` over 1000 generated ASTs. When a
round-trip test fails after your change, the unparser is rarely the
problem — the bug is grammar ambiguity or a missing AST distinction.
Do not make the unparser asymmetric to dodge the failure.

## See also

- `ARCHITECTURE.md` — three-layer design, IR contract, test tiers.
- `xtask/src/add_card.rs` — `WORKFLOW_BLOCK` and `CONSTRAINTS_BLOCK`
  encode the operative ruleset for any agent extending the grammar
  via `cargo xtask add-card`.
