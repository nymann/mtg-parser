# MTG Card Text Parser — Rust Project Plan

## Goal

Build a high-performance parser for Magic: The Gathering Oracle text. Parse into a syntactic AST, lower to a semantic IR via an attribute pass. Replace an existing Lark/Earley parser that is too slow.

The architectural priority is a **testing setup and grammar-evolution workflow** that scales: as the grammar grows from handling 5% of cards to 95%, we need to know immediately when a change breaks something, and the full test suite must stay fast (<1s for the inner loop, <30s for the full suite excluding the corpus pass).

Python bindings will be added once the core is stable. Architecture should keep this in mind but not be driven by it.

## Architecture

Three layers, strictly separated (ports and adapters):

1. **Parser** (`mtg-grammar`): Card text → syntactic AST. Pure CFG.
2. **Attribute pass** (`mtg-semantic`): Syntactic AST → semantic IR. Resolves references, validates types, normalizes effects.
3. **Adapters** (`mtg-scryfall`, `mtg-py` later): Outer layers that bring data in (Scryfall) or expose the core out (Python).

The core (`mtg-grammar`, `mtg-semantic`) has zero knowledge of where text comes from or where results go. Hexagonal/ports-and-adapters shape.

### Ports

- `Parser`: `fn parse(text: &str) -> Result<SyntacticAst, ParseError>`
- `Lowering`: `fn lower(ast: SyntacticAst) -> Result<Ir, SemanticError>`
- `Unparser`: `fn unparse(ast: &SyntacticAst) -> String`

These are the public API. Everything else is implementation detail.

### Adapters

- `mtg-scryfall`: Fetches cards from the Scryfall API, normalizes Oracle text, feeds it into the core.
- `mtg-corpus`: Test harness that composes the Scryfall adapter with the core ports.
- `mtg-py` (later): PyO3 binding.

## Tech stack

- **Rust 2021**
- **`pest`** for the grammar
- **`serde`** for AST/IR serialization
- **`proptest`** for property-based tests
- **`insta`** for snapshot tests
- **`criterion`** for benchmarks
- **`reqwest` + `tokio`** for Scryfall fetching (adapter layer only)
- **`pyo3` + `maturin`** later, for Python bindings

## Repository layout

    mtg-parser/
    ├── Cargo.toml                  # workspace
    ├── ARCHITECTURE.md
    ├── crates/
    │   ├── mtg-grammar/            # core: parser + syntactic AST
    │   │   ├── src/
    │   │   │   ├── lib.rs
    │   │   │   ├── grammar.pest
    │   │   │   ├── ast.rs
    │   │   │   └── unparse.rs
    │   │   └── tests/
    │   ├── mtg-semantic/           # core: attribute pass + IR
    │   │   ├── src/
    │   │   │   ├── lib.rs
    │   │   │   ├── ir.rs
    │   │   │   └── lower.rs
    │   │   └── tests/
    │   ├── mtg-scryfall/           # adapter: Scryfall API client
    │   │   ├── src/
    │   │   │   ├── lib.rs
    │   │   │   ├── client.rs
    │   │   │   └── cache.rs
    │   │   └── tests/
    │   └── mtg-corpus/             # adapter: corpus test harness
    │       ├── src/
    │       └── tests/
    ├── xtask/                      # custom dev commands
    │   └── src/main.rs
    └── benches/

## The testing strategy

The test suite is organized into **tiers** by speed. Each tier has a clear purpose and a strict speed budget.

### Tier 0: Compile-time checks (<5s)

- `pest` grammar compiles without ambiguity warnings.
- Rust code compiles. Clippy is clean.
- Runs on every `cargo check`.

### Tier 1: Inner-loop tests (<1s total)

Run on every save during development. Goal: never wait more than a second for feedback.

- **Unit tests per grammar rule.** For each non-terminal in the grammar, hand-written tests that assert input → expected AST.
- **Unit tests per lowering rule.** Hand-constructed ASTs → expected IR.
- **Curated golden tests.** ~20 representative cards covering different patterns, as `insta` snapshots. Reviewed with `cargo insta review`.

If Tier 1 creeps over a second, that blocks merging. Slow tests move to Tier 2.

### Tier 2: Property tests (<10s)

Run before every commit.

- **Round-trip property test.** Generate random valid ASTs via `proptest`; assert `parse(unparse(ast)) == ast`. 1000 cases.
- **Lowering totality.** Every AST the parser produces must successfully lower to an IR (no panics).
- **IR invariants.** E.g., every `Target` must have a resolvable type; every `Cost` must be well-formed.

### Tier 3: Corpus test (manual / CI, ~minutes)

Run on demand and in CI. Not part of the inner loop.

- Pull Scryfall Oracle text via `mtg-scryfall`.
- Attempt to parse every card.
- Write `corpus_status.json`: per-card pass/fail with error message.
- Compare against the previous run's `corpus_status.json` (committed).
- **Three categories of change**:
  - **New passes**: cards that now parse. Good.
  - **New failures**: cards that previously parsed but no longer do. CI fails.
  - **Same failures**: the "remaining work" backlog.

The committed `corpus_status.json` is the source of truth for "what does the parser currently handle." Every grammar change updates it.

#### Implementation note: data-driven, not macro-expanded

The corpus test is a single `#[test]` that loops over all cards loaded from a JSON file, collects failures into a `Vec<(CardName, Error)>`, and asserts the vec is empty at the end. This is cheaper than macro-generating N test functions (no compile-time blowup, no per-test runner overhead) and reports every failing card in one run.

Per-card focused tests (the ones produced by `next-card`) are separate — those exist so the *active* card you're working on shows up as a discrete failure in your editor. Bulk regression checking is the data-driven loop.

### Tier 4: Benchmarks (on demand)

`cargo bench` via `criterion`. Tracks parse latency per card and full-corpus throughput.

### Tier 5: Differential test against Lark (one-off, during migration)

Until the Rust parser supersedes Lark, a script that runs both on the same inputs and reports disagreements. Lives in `xtask`.

## The grammar-evolution workflow

This is the day-to-day loop, designed so regressions are impossible to miss.

### The "find next card" loop (Scryfall-driven)

The idea: Scryfall is our source of failing tests. We point at a set, walk through its cards, and let the first round-trip failure tell us what grammar to write next.

#### Step 1: Pick a set

`cargo xtask next-card --set <code>` (e.g., `--set lea` for Limited Edition Alpha, `--set neo` for Kamigawa Neon Dynasty). Defaults to the oldest unsupported set if no `--set` is given.

The command fetches all cards in the set via the Scryfall API (cached locally), then iterates through them in order.

#### Step 2: Find the first failing card

For each card, the command runs the round-trip assertion:

```rust
let text = normalize(card.oracle_text);
let ast = parse(&text)?;          // may fail here
let reprinted = unparse(&ast);
let ast2 = parse(&reprinted)?;
assert_eq!(ast, ast2);            // or may fail here
```

If either step fails, this is the next card to tackle. The command:

1. Prints the card name, set, and Oracle text.
2. Prints the failure (parse error, or AST diff).
3. **Generates a failing test file** in `mtg-grammar/tests/generated/` with the card name, Oracle text, and a `#[test]` that runs the round-trip assertion. Marked `#[ignore]` initially so it doesn't break the suite — but flips on once you start working on it.

If every card in the set passes, the command moves to the next set.

#### Step 3: Promote the test

Move the generated test from `tests/generated/` to `tests/` (or fold it into an existing test module), remove `#[ignore]`. Now it's a real failing test in the suite.

#### Step 4: Extend the grammar

Modify `grammar.pest` and `ast.rs`. Make the test pass. Tier 1 still runs in <1s.

#### Step 5: Run Tier 2

`cargo xtask test --tier 2`. Property tests. If round-tripping fails on generated ASTs, the grammar has new ambiguity or the unparser is out of sync. Fix before proceeding.

#### Step 6: Add lowering

Extend `lower.rs` to handle the new AST shape. Add unit tests for the lowering.

#### Step 7: Add a snapshot test

Pick one or two cards exemplifying the new pattern. Add to the curated golden set. `cargo insta review` to accept.

#### Step 8: Run the corpus test

`cargo xtask corpus`. It will:

1. Parse every card.
2. Diff against the committed `corpus_status.json`.
3. Fail loudly if any previously-passing card now fails.
4. Show the count of newly-passing cards.
5. Update `corpus_status.json` if everything checks out.

#### Step 9: Commit

The commit includes:
- Grammar/lowering changes
- New unit tests (the promoted generated test)
- Updated snapshots (if any)
- Updated `corpus_status.json`

The diff on `corpus_status.json` is the proof of progress.

### Why this loop works

- **Scryfall is the test generator.** We don't have to imagine what cards exist; the real card database tells us what we don't handle yet, in a deterministic order.
- **Round-trip is the assertion.** A card passes when text → AST → text → AST is a fixed point. That's a strong correctness signal: it means the parser, AST design, and unparser all agree.
- **Each new test is a real card.** Not a synthetic example. When it passes, a real card works.
- **The set ordering gives natural progression.** Starting from Alpha and moving forward roughly corresponds to grammar complexity: early cards are simpler, modern cards are gnarlier. You can also target specific sets to hit a mechanic (e.g., `--set neo` for ninjutsu).

### The orchestrator

The find-next-card loop has a lot of deterministic shape (fetch, generate, test, diff, commit) and one non-deterministic step (write the grammar rule). An orchestrator does everything deterministic and delegates only the creative step to Claude Code. This makes the workflow auditable and dramatically reduces the amount of manual back-and-forth.

Inspired by the existing `parser-fix` script in [argentum-press/scripts](https://github.com/nymann/argentum-press/tree/main/scripts).

#### Orchestrator flow

`cargo xtask grammar-fix [--set <code>] [--max-iterations N]`:

1. **Fetch next failing card** (deterministic). Run `next-card`. If none, exit success.
2. **Generate failing test** (deterministic). Write `tests/generated/<card_slug>.rs` with the card name, Oracle text, and round-trip assertion. Promote it (un-`#[ignore]`).
3. **Snapshot state** (deterministic). Record git HEAD, current `corpus_status.json` pass count.
4. **Delegate to Claude Code** (non-deterministic). Invoke with a tightly-scoped prompt:
   - The failing card name and Oracle text.
   - The specific parse/round-trip error message.
   - The current `grammar.pest` and relevant `ast.rs` / `lower.rs` sections.
   - Explicit constraints (see Prompt Contract below).
5. **Run Tier 1** (deterministic). `cargo xtask test`. If it fails, hand the failure back to Claude Code with one retry. If still failing, surface to the human.
6. **Run Tier 2** (deterministic). Property tests. Same retry policy.
7. **Run corpus diff** (deterministic). `cargo xtask corpus`. If any previously-passing card now fails, surface to human — this is a real regression, not something to paper over.
8. **Commit** (deterministic). Structured commit message:

       grammar: support <pattern> (e.g., "tapped-entry triggers")

       Card: <name> (<set>)
       New passes: <count>
       Status: <total_pass>/<total>

9. **Loop or stop** (deterministic). If `--max-iterations` not reached and time/budget allows, go to step 1.

#### Prompt contract with Claude Code

Constraints baked into the orchestrator's prompt:

- **Don't modify the unparser to make round-trip pass.** If round-trip fails after your grammar change, that's a signal the AST design is wrong or there's new ambiguity — think about it, don't paper over it.
- **Don't add a special-case rule for one card.** If the pattern looks specific to one card, ask yourself what general pattern it's an instance of. Special-case rules are a code smell.
- **Don't touch existing grammar rules unless necessary.** Additive changes are safer than modifications. If you must modify, explain why in a comment.
- **Don't disable or modify existing tests.** Generated tests and existing tests are the contract. If one needs updating, surface that as a question, don't just rewrite it.
- **Stay within scope.** Modify only `grammar.pest`, `ast.rs`, `lower.rs`, and the generated test file. Don't touch `mtg-scryfall`, `mtg-corpus`, or xtask code.
- **If you can't solve it, say so.** Better to surface "I don't see how to extend the grammar for this without restructuring X" than to produce a hack.

#### What the orchestrator does NOT automate

- **Architectural decisions.** "Should this be a new AST node or an attribute on an existing one?" goes to the human.
- **Regressions in `corpus_status.json`.** A previously-passing card breaking is a stop-the-line event. Human triage.
- **Property test failures that aren't obviously about the new pattern.** If proptest finds an ambiguity unrelated to today's card, that's a separate bug.
- **Snapshot test changes.** `cargo insta review` stays human.

#### Logging and replay

Every orchestrator run writes to `.grammar-fix/<timestamp>/`:

- `card.json` — the card the run targeted
- `prompt.md` — exactly what was sent to Claude Code
- `response.md` — what came back
- `diff.patch` — the resulting code change
- `result.json` — pass/fail at each step

This makes failures debuggable and lets you replay or audit a run later. Also gives you a corpus of (failing card, working grammar fix) pairs that could feed future tooling.

### What stops this from degrading

Three guardrails:

1. **The corpus test's regression check.** You cannot break a card that used to pass without CI screaming. The committed `corpus_status.json` is a contract.
2. **The tier discipline.** Tier 1 must stay under 1s. If it doesn't, that blocks merging.
3. **Property tests in Tier 2.** Ambiguity sneaks in invisibly otherwise. Proptest finds it the moment it appears.

## Scryfall integration (`mtg-scryfall` adapter)

### Responsibilities

- Fetch cards by set via the Scryfall API (`/cards/search?q=set:<code>`).
- Optionally fetch the full Oracle bulk data dump for the corpus test.
- Cache locally with ETag/last-modified handling.
- Normalize Oracle text: strip reminder text in parens, normalize Unicode (em-dashes, etc.), handle split/transform/adventure cards.
- Expose iterators by set or over all cards.

### Interface

```rust
pub struct ScryfallClient { /* ... */ }

impl ScryfallClient {
    pub fn new(cache_dir: &Path) -> Result<Self>;
    pub fn cards_in_set(&self, set_code: &str) -> Result<Vec<Card>>;
    pub fn all_cards(&self) -> Result<impl Iterator<Item = Card>>;
    pub fn refresh(&mut self) -> Result<()>;
}

pub struct Card {
    pub name: String,
    pub set_code: String,
    pub collector_number: String,
    pub oracle_text: String,
    pub layout: Layout,
}
```

### Caching

Set queries cached as JSON files keyed by set code. Bulk data cached separately. Don't re-fetch on every test run. Provide `cargo xtask refresh-corpus` for explicit updates.

### Rate limiting

Scryfall asks for 50–100ms between requests. The client enforces this. For bulk operations, prefer the bulk data dump over individual queries.

## `cargo xtask` — the developer workflow CLI

- `cargo xtask test` — Tier 1 only. <1s. Default for inner loop.
- `cargo xtask test --tier 2` — Tier 1 + property tests.
- `cargo xtask test --all` — Everything except the corpus.
- `cargo xtask next-card [--set <code>]` — Find the next failing card from Scryfall and generate a failing test for it.
- `cargo xtask grammar-fix [--set <code>] [--max-iterations N]` — Run the full orchestrated find-next-card → fix → test → commit loop. Inspired by [argentum-press/scripts](https://github.com/nymann/argentum-press/tree/main/scripts) `parser-fix`.
- `cargo xtask corpus` — Run the corpus test, diff against committed status, fail on regressions, update status on success.
- `cargo xtask corpus --update` — Force-update `corpus_status.json` even if regressions exist. Requires explicit reviewer approval.
- `cargo xtask refresh-corpus` — Re-fetch Scryfall data.
- `cargo xtask diff-lark` — Differential test against Lark (during migration).
- `cargo xtask bench` — Run benchmarks.

## Milestones

### M1: Scaffolding and tiered test infrastructure

- Cargo workspace with `mtg-grammar`, `mtg-semantic` empty crates.
- `xtask` crate with `test` and `test --tier 2` commands.
- Minimal grammar: mana costs and "destroy target creature".
- A handful of Tier 1 unit tests.
- A single Tier 2 property test (round-trip).
- CI config running both tiers.
- **Exit criteria**: `cargo xtask test` passes in <1s. `cargo xtask test --tier 2` passes in <10s.

### M2: Scryfall adapter and `next-card` workflow

- `mtg-scryfall` crate fetches and caches cards by set.
- `cargo xtask next-card` working: walks a set, finds the first round-trip failure, generates a failing test.
- `mtg-corpus` crate runs the parser against the full corpus, produces `corpus_status.json`.
- Initial `corpus_status.json` committed.
- `cargo xtask corpus` with regression detection.
- **Exit criteria**: `next-card --set lea` produces a failing test for a real Alpha card. `cargo xtask corpus` detects regressions.

### M3: Unparser and full round-trip discipline

- Implement `unparse` over the current AST.
- Property test covers parse/unparse round-tripping.
- Document the canonical form in `ARCHITECTURE.md`.
- **Exit criteria**: round-trip property test passes 1000 cases.

### M4: Semantic IR and lowering

- Define `ir.rs` types.
- Implement `lower` for the current grammar coverage.
- Tier 1 lowering unit tests.
- Tier 2 lowering totality property test.
- **Exit criteria**: every AST the parser produces lowers without error.

### M5: Grammar growth phase (manual)

Iterate the find-next-card workflow by hand. Start with `--set lea` and work forward. Each iteration is a small commit that:

1. Promotes a generated failing test to the real suite.
2. Extends grammar and lowering.
3. Increases the corpus pass rate.
4. Does not regress any previously-passing card.

Doing this manually first builds intuition for what the orchestrator's prompt contract needs to enforce.

### M5.5: Orchestrator

Once the manual workflow feels solid (say, ~20 cards added by hand and the patterns are clear), build `cargo xtask grammar-fix`. Use the existing [argentum-press parser-fix script](https://github.com/nymann/argentum-press/tree/main/scripts) as a reference for orchestration shape and prompt design.

- **Exit criteria**: orchestrator can autonomously add support for at least 5 consecutive cards from a set without human intervention, with all guardrails respected. Failures surface to human cleanly with full context.

### M6: Python bindings (deferred)

Brief note: once the core is stable, add `mtg-py` crate with PyO3 bindings. Mirrors the `Parser` and `Lowering` ports. The adapter pattern means this is a thin wrapper. Detailed plan when we get there.

## Open questions

1. Should `corpus_status.json` be checked in as one file or split per set? Start with one; split if it gets unwieldy.
2. How aggressive should Oracle text normalization be? Recommend: strip reminder text, normalize Unicode, leave everything else alone. Document the rules.
3. Should the corpus test allow "intentionally unsupported" cards (un-sets, conspiracy)? Probably yes, via an allowlist. Defer until annoying.
4. What's the canonical form for the unparser? Match Scryfall's Oracle text conventions and document.
