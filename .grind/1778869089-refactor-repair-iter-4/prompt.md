You are the grind repair agent. The autonomous refactor phase hit a gate failure at iteration 4 and the orchestrator handed you the error so the outer loop can continue without human input.

## Error

```text
cargo test failed with exit status: 101
help: there is a variant with a similar name
    |
276 -         Just(Statement::PreventAllCombatDamageThisTurn),
276 +         Just(Statement::PreventDamageThisTurn { effect: /* value */ }),
    |

For more information about this error, try `rustc --explain E0599`.
error: could not compile `mtg-grammar` (test "prop") due to 2 previous errors
warning: build failed, waiting for other jobs to finish...
error[E0599]: no variant named `PreventNextDamageThatWouldBeDealtToRecipientThisTurn` found for enum `ModalMode`
   --> crates/mtg-semantic/tests/prop.rs:146:24
    |
146 |             ModalMode::PreventNextDamageThatWouldBeDealtToRecipientThisTurn {
    |                        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ variant not found in `ModalMode`

error[E0599]: no variant or associated item named `PreventAllCombatDamageThisTurn` found for enum `Statement` in the current scope
   --> crates/mtg-semantic/tests/prop.rs:271:25
    |
271 |         Just(Statement::PreventAllCombatDamageThisTurn),
    |                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ variant or associated item not found in `Statement`
    |
help: there is a variant with a similar name
    |
271 -         Just(Statement::PreventAllCombatDamageThisTurn),
271 +         Just(Statement::PreventDamageThisTurn { effect: /* value */ }),
    |

error[E0599]: no variant named `PreventNextDamageThatWouldBeDealtToRecipientThisTurn` found for enum `Statement`
   --> crates/mtg-semantic/tests/prop.rs:273:24
    |
273 |             Statement::PreventNextDamageThatWouldBeDealtToRecipientThisTurn {
    |                        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ variant not found in `Statement`

error[E0599]: no variant named `IfYouDoPreventNextDamageThatWouldBeDealtToRecipientThisTurn` found for enum `Statement`
   --> crates/mtg-semantic/tests/prop.rs:278:24
    |
278 |             Statement::IfYouDoPreventNextDamageThatWouldBeDealtToRecipientThisTurn {
    |                        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ variant not found in `Statement`

error: could not compile `mtg-semantic` (test "prop") due to 4 previous errors
```

## Current git status

```text
 M crates/mtg-grammar/src/ast.rs
 M crates/mtg-grammar/src/parse.rs
 M crates/mtg-grammar/src/unparse.rs
```

## Mission

Diagnose and fix the failure so the grind loop can keep going. Prefer the smallest patch that makes the gate green. You are free to edit, replace, or discard the currently modified files from the failed iteration. In most cases you should repair the patch in place; if the patch is unsalvageable, restore the affected files to the last committed state and exit successfully. If you cannot make either choice safely, exit non-zero and the orchestrator will discard the working tree and treat this iteration as a no-op.

## Rules

1. Do not weaken or disable existing tests.
2. Do not bypass deterministic gates (tier-2, corpus, audit).
3. You may run focused tests while debugging, but the orchestrator owns the final full gates and commit after you exit successfully.
4. Do not leave uncertainty about ownership of modified files: either make them part of the repair or restore them.
5. Keep edits tightly scoped to the failure.
