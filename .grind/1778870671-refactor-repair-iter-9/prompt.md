You are the grind repair agent. The autonomous refactor phase hit a gate failure at iteration 9 and the orchestrator handed you the error so the outer loop can continue without human input.

## Error

```text
cargo test failed with exit status: 101
    |                                              ^^^^^^^^^^^^^^^^^^^^^^^ variant not found in `Statement`

error: could not compile `mtg-semantic` (test "unit") due to 5 previous errors
error[E0599]: no variant named `DestroyTargetPermanents` found for enum `Statement`
   --> crates/mtg-semantic/tests/prop.rs:238:25
    |
238 |         Just(Statement::DestroyTargetPermanents {
    |                         ^^^^^^^^^^^^^^^^^^^^^^^ variant not found in `Statement`

error[E0599]: no variant named `DestroyTargetPermanents` found for enum `Statement`
   --> crates/mtg-semantic/tests/prop.rs:328:24
    |
328 |             Statement::DestroyTargetPermanents {
    |                        ^^^^^^^^^^^^^^^^^^^^^^^ variant not found in `Statement`

error[E0599]: no variant named `DestroyTargetPermanents` found for enum `Statement`
   --> crates/mtg-semantic/tests/prop.rs:333:24
    |
333 |             Statement::DestroyTargetPermanents {
    |                        ^^^^^^^^^^^^^^^^^^^^^^^ variant not found in `Statement`

error[E0599]: no variant named `DestroyAll` found for enum `Statement`
   --> crates/mtg-semantic/tests/prop.rs:338:54
    |
338 |             .prop_map(|permanent_types| { Statement::DestroyAll { permanent_types } }),
    |                                                      ^^^^^^^^^^
    |
help: there is a variant with a similar name
    |
338 -             .prop_map(|permanent_types| { Statement::DestroyAll { permanent_types } }),
338 +             .prop_map(|permanent_types| { Statement::Destroy { permanent_types } }),
    |

error[E0599]: no variant named `DestroyAllBasicLands` found for enum `Statement`
   --> crates/mtg-semantic/tests/prop.rs:346:54
    |
346 |             .prop_map(|basic_land_type| { Statement::DestroyAllBasicLands { basic_land_type } }),
    |                                                      ^^^^^^^^^^^^^^^^^^^^ variant not found in `Statement`

error: could not compile `mtg-semantic` (test "prop") due to 5 previous errors
```

## Current git status

```text
 M crates/mtg-grammar/src/ast.rs
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
