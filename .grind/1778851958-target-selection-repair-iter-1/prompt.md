You are selecting the next autonomous grammar refactor target for `mtg-parser`.
This is iteration 1 of the outer grind loop.

Pick one narrow effect-frame theme for the next `refactor-hotspot` run. Do not
choose broad `grammar-core` unless every narrower theme is exhausted.

## Allowed Themes

- `damage` — damage amounts, sources, recipients, damage-linked life gain.
- `destroy` — destroy/tap/sacrifice/attach target/all/list action frames.
- `prevention` — prevent/replacement effect amount and recipient frames.
- `keyword-abilities` — keyword ability variants and keyword data axes.
- `triggered-abilities` — event + optional condition + effect-list factoring.
- `unparse-templates` — reusable rendering/template slots.
- `parser-boilerplate` — parser mechanics only, no grammar/AST shape change.

Do not choose an exhausted theme.

## Exhausted Themes

(none)


## Preference Rules

1. Prefer a theme where repeated sentence-shaped rules can become one
   phenomenon-shaped rule plus data axes.
2. Prefer themes that reduce grammar, AST, parse, and unparse coupling together.
3. Avoid tiny common-substring deduplication unless it is part of a real frame.
4. If the recent commits already worked one theme and it is still yielding
   meaningful commits, you may continue it. If it is producing only small helper
   shuffles, switch.
5. Return exactly one line at the end: `theme: <allowed-theme>`.

## Current Grammar Surface

```text
crates/mtg-grammar/src/grammar.pest        loc=1510  surface-count=397
crates/mtg-grammar/src/ast.rs              loc=1491  surface-count=56
crates/mtg-grammar/src/parse.rs            loc=3714  surface-count=166
crates/mtg-grammar/src/unparse.rs          loc=2350  surface-count=92
```

## Git Status

```text
 M xtask/src/grind.rs
 M xtask/src/main.rs
```

## Current Diff Stat

```text
 xtask/src/grind.rs | 387 ++++++++++++++++++++++++++++++++++++++++++++++++-----
 xtask/src/main.rs  |  19 +--
 2 files changed, 365 insertions(+), 41 deletions(-)
```

## Recent Commits

```text
883d0fe Refactor damage hotspot iteration 14
6a17a9b Refactor damage hotspot iteration 13
c91a71b Refactor damage hotspot iteration 12
6f3f027 Refactor damage hotspot iteration 11
3c2093a Refactor damage hotspot iteration 9
0156048 Refactor damage hotspot iteration 8
b07f61b Refactor damage hotspot iteration 7
c2c9b1d Refactor damage hotspot iteration 4
851353c Refactor damage hotspot iteration 3
2d0a018 Refactor damage hotspot iteration 1
23fb40a Emit SessionStarted from grind so TUI panels render
98baf56 Refactor grammar-core hotspot iteration 5
```
