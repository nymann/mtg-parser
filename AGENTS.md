# AGENTS.md

Instructions for AI coding agents. Three rules, two workflows, and a pointer.

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

## Query Magic rules with qmd

When a grammar change depends on Magic rules vocabulary or semantics, query the
local rules collection instead of guessing. Use:

```sh
qmd -c i-forgot-the-collection query "<rules question or oracle phrase>"
```

Use the results to name AST/rule concepts after Comprehensive Rules terms when
possible. This is especially useful for damage, prevention/replacement effects,
activated abilities, keyword actions, timing restrictions, and zone changes.

## Investigate add-card runs from the logs

`cargo xtask add-card` writes one directory per card under `.add-card/`. When
debugging what happened, start with the newest relevant directory and inspect:

- `card.json` for the exact card and normalized oracle text.
- `focused_before_agent.txt` and `focused_after_agent.txt` for the focused
  generated-test failure and whether the card repair solved it.
- `grammar_audit.md` / `grammar_audit.json` for new pest rules, oracle-word
  overlap, and skeleton-neighbour signals.
- `generalization_report.txt` for the agent's declared path; if it says
  `missing`, check `transcript.ndjson` because Codex event-shape changes can
  hide the final block from the extractor.
- `downstream-repair-attempt-1/tier2_failure.txt` when present; classify the
  failure before drawing conclusions. A semantic prop compile error, unrelated
  xtask/TUI compile break, and old-card grammar regression mean different next
  actions.
- `diff.patch` and `commit_message.txt` for the final committed shape.

Useful commands:

```sh
ls -lt .add-card | head
cargo xtask grammar-audit --diff HEAD~1..HEAD --oracle-text "<normalized oracle text>"
rg -n "GENERALIZATION_PATH|error\[|FAILED|panicked|parse:" .add-card/<run>/
```

## Latest add-card calibration lessons

The `Rock Hydra` run (`.add-card/1778882961-rock_hydra`) showed what the new
logs are good for:

- `grammar_audit` gave a useful drift signal: nine new pest rules, with
  `counter_amount` flagged as a block-candidate because its RHS duplicated the
  shape of `draw_count`. Treat duplicated quantity-like grammar as a prompt to
  look for a shared abstraction or a wider existing rule.
- The focused generated tests passed, but tier 2 caught an old-card regression:
  `Clockwork Beast` no longer parsed `Put up to X +1/+0 counters...`. Focused
  tests prove the new card works; tier 2 proves the widened grammar still
  preserves older cards.
- `generalization_report.txt` said `missing`, but `transcript.ndjson` contained
  the required `GENERALIZATION_PATH` block. If this happens, the extractor is
  probably missing a Codex JSON event shape such as `item.completed.item.text`.
- Downstream repair logs need classification before conclusions. In recent
  runs, downstream repair covered at least three different classes: semantic
  property-generator follow-up, unrelated xtask/TUI compile break, and real
  old-card grammar regression.

Near-term workflow follow-ups:

- Fix Codex assistant-text extraction so `response.md`,
  `generalization_report.txt`, and commit bodies capture
  `GENERALIZATION_PATH`.
- Record a machine-readable downstream-repair reason, for example
  `semantic_prop_compile`, `infra_compile`, or `old_card_regression`.
- Make skeleton-neighbour findings more actionable when they identify duplicate
  quantity grammar such as `number_word | variable_name`.

## See also

- `ARCHITECTURE.md` — three-layer design, IR contract, test tiers.
- `xtask/src/add_card.rs` — `WORKFLOW_BLOCK` and `CONSTRAINTS_BLOCK`
  encode the operative ruleset for any agent extending the grammar
  via `cargo xtask add-card`.
