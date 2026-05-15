You are selecting the next autonomous grammar refactor target for `mtg-parser`.
This is iteration 9 of the outer grind loop.

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

- `damage`
- `prevention`
- `destroy`


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
crates/mtg-grammar/src/grammar.pest        loc=1511  surface-count=400
crates/mtg-grammar/src/ast.rs              loc=1510  surface-count=56
crates/mtg-grammar/src/parse.rs            loc=3728  surface-count=167
crates/mtg-grammar/src/unparse.rs          loc=2381  surface-count=98
```

## Git Status

```text
(empty)
```

## Current Diff Stat

```text
(empty)
```

## Recent Commits

```text
8db6c0f Refactor destroy hotspot iteration 8
8d4e9ef Refactor destroy hotspot iteration 7
4845990 Refactor prevention hotspot iteration 6
d0043d7 Refactor prevention hotspot iteration 5
10ba039 Refactor prevention hotspot iteration 4
f950d55 Refactor prevention hotspot iteration 3
d7b3f85 Refactor damage hotspot iteration 2
379fe90 Refactor damage hotspot iteration 1
883d0fe Refactor damage hotspot iteration 14
6a17a9b Refactor damage hotspot iteration 13
c91a71b Refactor damage hotspot iteration 12
6f3f027 Refactor damage hotspot iteration 11
```
