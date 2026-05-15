You are the grind repair agent. The autonomous refactor phase hit a gate failure at iteration 6 and the orchestrator handed you the error so the outer loop can continue without human input.

## Error

```text
cargo test failed with exit status: 101
test kudzu::round_trip ... ok
test mana_short::round_trip ... ok
test mesa_pegasus::round_trip ... ok
test lure::round_trip ... ok
test mana_flare::round_trip ... ok
test mind_twist::round_trip ... ok
test meekstone::round_trip ... ok
test manabarbs::round_trip ... ok
test lord_of_the_pit::round_trip ... ok
test natural_selection::round_trip ... ok
test nevinyrral_s_disk::round_trip ... ok
test living_artifact::round_trip ... ok
test nettling_imp::round_trip ... ok
test nether_shadow::round_trip ... ok
test lich::round_trip ... ok
test mana_vault::round_trip ... ok

failures:

---- creature_bond::round_trip stdout ----

thread 'creature_bond::round_trip' (460647875) panicked at crates/mtg-grammar/tests/generated/creature_bond.rs:11:40:
parse: Pest(Error { variant: ParsingError { positives: [that_player_damage_recipient, you_damage_recipient, that_permanent_damage_recipient, that_permanents_controller], negatives: [] }, location: Pos(108), line_col: Pos((2, 92)), inner: ErrorInner { path: None, line: "When enchanted creature dies, this Aura deals damage equal to that creature's toughness to the creature's controller.", continued_line: None, parse_attempts: None } })
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    creature_bond::round_trip

test result: FAILED. 128 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling mtg-grammar v0.0.0 (/Users/knj/code/github.com/nymann/mtg-parser/crates/mtg-grammar)
   Compiling mtg-semantic v0.0.0 (/Users/knj/code/github.com/nymann/mtg-parser/crates/mtg-semantic)
   Compiling mtg-corpus v0.0.0 (/Users/knj/code/github.com/nymann/mtg-parser/crates/mtg-corpus)
   Compiling xtask v0.0.0 (/Users/knj/code/github.com/nymann/mtg-parser/xtask)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 11.27s
     Running unittests src/lib.rs (target/debug/deps/mtg_corpus-b42c7474b776c6c5)
     Running unittests src/lib.rs (target/debug/deps/mtg_grammar-3100d00b1d4910f5)
     Running tests/generated.rs (target/debug/deps/generated-3ac3dc5a04bd71d7)
error: test failed, to rerun pass `-p mtg-grammar --test generated`
```

## Current git status

```text
 M crates/mtg-grammar/src/ast.rs
 M crates/mtg-grammar/src/grammar.pest
 M crates/mtg-grammar/src/lib.rs
 M crates/mtg-grammar/src/parse.rs
 M crates/mtg-grammar/src/unparse.rs
 M crates/mtg-grammar/tests/prop.rs
 M crates/mtg-semantic/tests/prop.rs
?? .grind/1778847864-refactor-repair-iter-5/
```

## Mission

Diagnose and fix the failure so the grind loop can keep going. Prefer the smallest patch that makes the gate green. If you cannot repair it safely, exit non-zero and the orchestrator will discard the working tree and treat this iteration as a no-op.

## Rules

1. Do not weaken or disable existing tests.
2. Do not bypass deterministic gates (tier-2, corpus, audit).
3. Run `cargo fmt --all` before exiting successfully.
4. Keep edits tightly scoped to the failure.
