You are the grind repair agent. The autonomous refactor phase hit a gate failure at iteration 10 and the orchestrator handed you the error so the outer loop can continue without human input.

## Error

```text
cargo test failed with exit status: 101
    |             ^^^^^^ `Statement::NamedSourceDealsDamage` does not have this field
    |
    = note: available fields are: `event`

error[E0559]: variant `Statement::NamedSourceDealsDamage` has no field named `recipients`
   --> crates/mtg-semantic/tests/prop.rs:251:13
    |
251 |             recipients: DamageRecipients::AnyTarget,
    |             ^^^^^^^^^^ `Statement::NamedSourceDealsDamage` does not have this field
    |
    = note: available fields are: `event`

error[E0559]: variant `Statement::NamedSourceDealsDamage` has no field named `source_name`
   --> crates/mtg-semantic/tests/prop.rs:254:13
    |
254 |             source_name: "Earthquake".to_string(),
    |             ^^^^^^^^^^^ `Statement::NamedSourceDealsDamage` does not have this field
    |
    = note: available fields are: `event`

error[E0559]: variant `Statement::NamedSourceDealsDamage` has no field named `amount`
   --> crates/mtg-semantic/tests/prop.rs:255:13
    |
255 |             amount: DamageAmount::Variable(Variable::X),
    |             ^^^^^^ `Statement::NamedSourceDealsDamage` does not have this field
    |
    = note: available fields are: `event`

error[E0559]: variant `Statement::NamedSourceDealsDamage` has no field named `recipients`
   --> crates/mtg-semantic/tests/prop.rs:256:13
    |
256 |             recipients: DamageRecipients::List(vec![
    |             ^^^^^^^^^^ `Statement::NamedSourceDealsDamage` does not have this field
    |
    = note: available fields are: `event`

For more information about this error, try `rustc --explain E0559`.
error: could not compile `mtg-grammar` (test "prop") due to 12 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `mtg-semantic` (test "prop") due to 12 previous errors
```

## Current git status

```text
 M crates/mtg-grammar/src/ast.rs
 M crates/mtg-grammar/src/grammar.pest
 M crates/mtg-grammar/src/parse.rs
 M crates/mtg-grammar/src/unparse.rs
```

## Mission

Diagnose and fix the failure so the grind loop can keep going. Prefer the smallest patch that makes the gate green. If you cannot repair it safely, exit non-zero and the orchestrator will discard the working tree and treat this iteration as a no-op.

## Rules

1. Do not weaken or disable existing tests.
2. Do not bypass deterministic gates (tier-2, corpus, audit).
3. Run `cargo fmt --all` before exiting successfully.
4. Keep edits tightly scoped to the failure.
