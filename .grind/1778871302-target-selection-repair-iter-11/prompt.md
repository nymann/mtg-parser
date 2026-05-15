You are selecting the next autonomous grammar refactor target for `mtg-parser`.
This is iteration 11 of the outer grind loop.

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
- `unparse-templates`
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
crates/mtg-grammar/src/grammar.pest        loc=1545  surface-count=407
crates/mtg-grammar/src/ast.rs              loc=1662  surface-count=63
crates/mtg-grammar/src/parse.rs            loc=3796  surface-count=176
crates/mtg-grammar/src/unparse.rs          loc=2584  surface-count=121
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
ca1b4de Refactor destroy hotspot iteration 10
eb1645d Repair grind refactor iteration 9
1387f24 Refactor destroy hotspot iteration 8
348f814 Refactor unparse-templates hotspot iteration 7
2fa4bcc Refactor unparse-templates hotspot iteration 6
1e64d0a Refactor prevention hotspot iteration 5
2027119 Repair grind refactor iteration 4
ce8eba7 Refactor prevention hotspot iteration 3
757a96e Refactor damage hotspot iteration 2
5c0be72 Refactor damage hotspot iteration 1
211a49b grammar: support card Paralyze
bb5e310 grammar: support card Orcish Oriflamme
```
