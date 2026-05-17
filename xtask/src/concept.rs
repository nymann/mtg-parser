use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use mtg_corpus::normalize_oracle_text;
use mtg_grammar::{parse as parse_card_text, parse_pest_rule};
use mtg_scryfall::ScryfallClient;
use serde::{Deserialize, Serialize};

use crate::console_sink::ConsoleSink;
use crate::flow::{
    AgentProvider, FlowEvent, FlowSink, IterationOutcomeSummary, NoteLevel, SessionEndReason,
};
use crate::grammar_query as grammar_query_engine;
use crate::grammar_query::GrammarQuery;
use crate::paths::{
    corpus_sets_path, grammar_concept_log_root, grammar_concepts_dir, grammar_fixtures_dir,
    grammar_pest_path, repo_root,
};
use crate::refactor_hotspot;
use crate::rules_context;

const DISCOVER_USAGE: &str = "\
cargo xtask concept-discover --query TEXT [--concept NAME] [--set CODE] [--limit N]

Creates a grammar-first discovery run under .grammar-concept-runs/.
This command searches qmd rules context and cached/refreshable corpus text.
It does not edit grammar, parser, generated card tests, or corpus status.
";

const GROW_USAGE: &str = "\
cargo xtask concept-grow --query TEXT --concept NAME [--rule RULE] [--set CODE] [--limit N] [--force]

Runs the grammar-first loop end to end:
- qmd rules search, corpus clustering, axes, and grammar-neighbor discovery
- choose a candidate PEST rule
- write grammar-concepts/<concept>.toml and grammar-fixtures/<concept>.toml
- run grammar fixtures and update [maturity].pest_grammar

This command still stops at grammar maturity. It does not edit parser, unparser,
lowering, generated card tests, or corpus status.
";

const GRAMMAR_TEST_USAGE: &str = "\
cargo xtask concept-grammar-test CONCEPT [--json]
cargo xtask concept-grammar-test --fixture PATH [--json]

Runs grammar-fixtures/<concept>.toml at PEST-rule level.
This command does not require AST construction, unparse, lowering, or card tests.
";

const PARSE_USAGE: &str = "\
cargo xtask concept-parse CONCEPT [--json]
cargo xtask concept-parse --fixture PATH [--json]

Runs Phase 2 parser/AST readiness checks for the accepted examples in a
grammar fixture. This uses the full mtg_grammar::parse entrypoint, not only the
PEST rule-level fixture gate. Counterexamples are not full-parser rejects; they
remain Phase 1 concept-boundary evidence.
";

const AST_TEST_USAGE: &str = "\
cargo xtask concept-ast-test CONCEPT [--update] [--json]
cargo xtask concept-ast-test --fixture PATH [--update] [--json]

Compares Phase 2 AST snapshots for accepted grammar fixture examples. With
--update, writes ast-fixtures/<concept>.json from the current parser output.
Without --update, fails if the snapshot is missing or differs.
";

const PHASE2_MAP_USAGE: &str = "\
cargo xtask concept-phase2-map [--json] [--all]

Inventories Phase 2 readiness for grammar concepts. By default the denominator
is concepts whose [maturity].pest_grammar is grammar_fixture_green. With --all,
also includes non-green concepts in the per-concept listing while keeping the
grammar-green denominator separate.
";

const PHASE2_GRIND_USAGE: &str = "\
cargo xtask concept-phase2-grind [--agent codex|claude] [--max-iterations N]
                                 [--concept NAME] [--dry-run]
                                 [--allow-dirty] [--no-commit]
                                 [--ui console|tui]

Autonomous Phase 2 loop. Each iteration builds a concept-phase2-map report,
picks a grammar-green concept that is not AST-snapshot green, then either
records a missing AST snapshot for parse-green concepts or asks an agent to mend
the parser/AST surface for parse/snapshot failures. It gates each accepted
change with concept-parse, concept-ast-test, and cargo check -p xtask.
";

const ROADMAP_USAGE: &str = "\
cargo xtask concept-roadmap [--json] [--kind all|actions|abilities|effects]

Inventories rulebook-derived concept candidates from resources/rules:
- 701 keyword actions
- 702 keyword abilities
- major 600-family effect sections

This is the product roadmap denominator. It complements concept-map-existing,
which only measures cleanup coverage of the current legacy grammar.pest file.
";

const GRAMMAR_QUERY_USAGE: &str = "\
cargo xtask concept-grammar-query --query TEXT [--rule NAME] [--limit N] [--json]

Queries grammar.pest for candidate rules, dependencies, reverse dependencies,
and duplicate/similar RHS shape drift.
";

const MATURITY_USAGE: &str = "\
cargo xtask concept-maturity CONCEPT [--json] [--update]

Reports grammar-first maturity from grammar-concepts/ and grammar-fixtures/.
With --update, writes [maturity].pest_grammar and blockers back to the concept file.
";

const MAP_EXISTING_USAGE: &str = "\
cargo xtask concept-map-existing [--json] [--no-expand-deps]

Inventories current grammar.pest rules against committed grammar-concepts/*.toml.
Concept files own rules through [concept].pest_rules. By default, ownership
expands through PEST dependencies until shared grammar primitives. The report
shows mapped rules, unmapped legacy rules, and likely owner suggestions by
rule-name prefix.
";

const GRIND_USAGE: &str = "\
cargo xtask concept-grind [--agent codex|claude] [--max-iterations N]
                          [--concept NAME] [--target-rule RULE] [--query TEXT]
                          [--repair-attempts N] [--dry-run]
                          [--allow-dirty] [--no-commit] [--ui console|tui]

Autonomous grammar-first loop. Each iteration maps existing grammar, selects the
next concept gap, runs a read-only boundary agent, runs a PEST patch agent,
gates grammar fixtures and grammar debt, repairs failures when allowed, updates
maturity, and commits. It does not run add-card or try to make cards pass.
";

const GRIND_LOOP_USAGE: &str = "\
cargo xtask concept-grind-loop [--agent codex|claude] [--batch-size N]
                               [--max-batches N] [--dry-run]
cargo xtask concept-grind-loop --resume PATH

Runs concept-grind in fixed-size batches. After each batch, an agent reviews
metrics and quality artifacts, decides whether the previous optimization
experiment passed or failed, reverts failed experiment commits, proposes the
next experiment, applies it, compiles, and repeats. By default it keeps
running until interrupted; --max-batches is an optional fuse. When an
experiment commit is created, the loop re-execs itself through --resume before
the next batch so wrapper changes take effect.
";

const PHASE_LOOP_USAGE: &str = "\
cargo xtask concept-phase-loop [--agent codex|claude]
                               [--phase2-batch-size N]
                               [--phase2-max-batches N]
                               [--phase2-max-commits N]
                               [--repeat-stop-after N]
                               [--phase1-batch-size N]
                               [--phase1-max-batches N]
                               [--dry-run]

Runs Phase 2 in small committed batches until objective quality stop signals
fire: no AST-green progress, repeated commits for the same concept, new
top-level Statement enum variants, AST failures becoming the majority of
remaining non-green concepts, or all Phase 2 concepts going green. Optional
max flags are fuses for scheduled/manual runs, not normal stop goals. When a
Phase 2 stop signal fires, it writes a summary and hands off to
concept-grind-loop for Phase 1 concept work.
";

const PHASE_STATUS_USAGE: &str = "\
cargo xtask concept-phase-status [--json]

Prints a quick continue/inspect/stopped verdict for the latest phase-loop run
using Phase 2 map metrics, latest batch summary, stop reasons, and whether a
phase loop/grind process is still running.
";

pub fn discover(args: &[String]) -> ExitCode {
    match parse_discover_options(args).and_then(run_discover) {
        Ok(report) => {
            println!(
                "Wrote grammar concept discovery run: {}",
                report.run_dir.display()
            );
            println!("  query          : {}", report.query);
            println!("  concept        : {}", report.concept);
            println!("  corpus examples: {}", report.corpus_examples);
            println!("  qmd queries    : {}", report.rules_queries);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub fn grow(args: &[String]) -> ExitCode {
    match parse_grow_options(args).and_then(run_grow) {
        Ok(report) => {
            println!("concept: {}", report.concept);
            println!("run    : {}", report.run_dir.display());
            println!("rule   : {}", report.rule);
            println!("concept file: {}", report.concept_file.display());
            println!("fixture file: {}", report.fixture_file.display());
            println!(
                "fixture: {} ({} case(s), {} failure(s))",
                if report.fixture_passed {
                    "pass"
                } else {
                    "fail"
                },
                report.fixture_cases,
                report.fixture_failures
            );
            println!("maturity: {}", report.maturity_state);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub fn grammar_test(args: &[String]) -> ExitCode {
    match parse_grammar_test_options(args).and_then(run_grammar_test) {
        Ok(report) => {
            if report.json {
                match serde_json::to_string_pretty(&report.result) {
                    Ok(json) => println!("{json}"),
                    Err(e) => {
                        eprintln!("error: {e:#}");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                print_fixture_report(&report.result);
            }
            if report.result.passed {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub fn parse_concept(args: &[String]) -> ExitCode {
    match parse_phase2_options(args, PARSE_USAGE).and_then(run_concept_parse) {
        Ok(report) => {
            if report.json {
                match serde_json::to_string_pretty(&report) {
                    Ok(json) => println!("{json}"),
                    Err(e) => {
                        eprintln!("error: {e:#}");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                print_parse_report(&report);
            }
            if report.passed {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!("concept-parse: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub fn ast_test(args: &[String]) -> ExitCode {
    match parse_phase2_options(args, AST_TEST_USAGE).and_then(run_ast_test) {
        Ok(report) => {
            if report.json {
                match serde_json::to_string_pretty(&report) {
                    Ok(json) => println!("{json}"),
                    Err(e) => {
                        eprintln!("error: {e:#}");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                print_ast_report(&report);
            }
            if report.passed {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!("concept-ast-test: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub fn phase2_map(args: &[String]) -> ExitCode {
    match parse_phase2_map_options(args).and_then(run_phase2_map) {
        Ok(report) => {
            if report.json {
                match serde_json::to_string_pretty(&report) {
                    Ok(json) => println!("{json}"),
                    Err(e) => {
                        eprintln!("error: {e:#}");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                print_phase2_map_report(&report);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("concept-phase2-map: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub fn phase2_grind(args: &[String]) -> ExitCode {
    match parse_phase2_grind_options(args) {
        Ok(options) => {
            let mut sink = ConsoleSink::new();
            match run_phase2_grind(options, &mut sink) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("concept-phase2-grind: {e:#}");
                    ExitCode::FAILURE
                }
            }
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}

pub fn phase_loop(args: &[String]) -> ExitCode {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print!("{PHASE_LOOP_USAGE}");
        return ExitCode::SUCCESS;
    }
    match parse_phase_loop_options(args).and_then(run_phase_loop) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("concept-phase-loop: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub fn phase_status(args: &[String]) -> ExitCode {
    match parse_phase_status_options(args).and_then(run_phase_status) {
        Ok(report) => {
            if report.json {
                match serde_json::to_string_pretty(&report) {
                    Ok(json) => println!("{json}"),
                    Err(e) => {
                        eprintln!("error: {e:#}");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                print_phase_status_report(&report);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("concept-phase-status: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub fn roadmap(args: &[String]) -> ExitCode {
    match parse_roadmap_options(args).and_then(run_roadmap) {
        Ok(report) => {
            if report.json {
                match serde_json::to_string_pretty(&report) {
                    Ok(json) => println!("{json}"),
                    Err(e) => {
                        eprintln!("error: {e:#}");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                print_roadmap_report(&report);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("concept-roadmap: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub fn grammar_query(args: &[String]) -> ExitCode {
    match parse_grammar_query_options(args).and_then(run_grammar_query) {
        Ok(report) => {
            if report.json {
                match serde_json::to_string_pretty(&report.report) {
                    Ok(json) => println!("{json}"),
                    Err(e) => {
                        eprintln!("error: {e:#}");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                print_grammar_query_report(&report.report);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub fn maturity(args: &[String]) -> ExitCode {
    match parse_maturity_options(args).and_then(|options| {
        let json = options.json;
        run_maturity(options).map(|report| (report, json))
    }) {
        Ok((report, json)) => {
            if json {
                match serde_json::to_string_pretty(&report) {
                    Ok(json) => println!("{json}"),
                    Err(e) => {
                        eprintln!("error: {e:#}");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                println!("concept: {}", report.concept);
                println!("state  : {}", report.state);
                println!(
                    "concept file: {}",
                    display_optional_path(&report.concept_file)
                );
                println!(
                    "fixture file: {}",
                    display_optional_path(&report.fixture_file)
                );
                for blocker in &report.blockers {
                    println!("blocker: {blocker}");
                }
                if report.updated {
                    println!("updated: maturity written to concept file");
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub fn map_existing(args: &[String]) -> ExitCode {
    match parse_map_existing_options(args).and_then(|options| {
        let json = options.json;
        run_map_existing(options).map(|report| (report, json))
    }) {
        Ok((report, json)) => {
            if json {
                match serde_json::to_string_pretty(&report) {
                    Ok(json) => println!("{json}"),
                    Err(e) => {
                        eprintln!("error: {e:#}");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                print_map_existing_report(&report);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub fn grind(args: &[String]) -> ExitCode {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print!("{GRIND_USAGE}");
        return ExitCode::SUCCESS;
    }
    let options = match parse_grind_options(args) {
        Ok(options) => options,
        Err(e) => {
            eprintln!("concept-grind: {e:#}");
            return ExitCode::FAILURE;
        }
    };
    let mut sink = ConsoleSink::new();
    match run_with_sink(options, &mut sink) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("concept-grind: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub fn grind_loop(args: &[String]) -> ExitCode {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print!("{GRIND_LOOP_USAGE}");
        return ExitCode::SUCCESS;
    }
    match parse_grind_loop_options(args).and_then(run_grind_loop) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("concept-grind-loop: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub(crate) fn run_with_sink(
    options: ConceptGrindOptions,
    sink: &mut dyn FlowSink,
) -> Result<ExitCode> {
    run_grind(options, sink)?;
    Ok(ExitCode::SUCCESS)
}

pub(crate) fn run_phase2_with_sink(
    options: Phase2GrindOptions,
    sink: &mut dyn FlowSink,
) -> Result<ExitCode> {
    run_phase2_grind(options, sink)?;
    Ok(ExitCode::SUCCESS)
}

#[derive(Debug, Clone)]
struct DiscoverOptions {
    query: String,
    concept: Option<String>,
    sets: Vec<String>,
    limit: usize,
}

#[derive(Debug)]
struct GrowOptions {
    discover: DiscoverOptions,
    rule: Option<String>,
    force: bool,
}

#[derive(Debug)]
struct GrowRun {
    concept: String,
    run_dir: PathBuf,
    rule: String,
    concept_file: PathBuf,
    fixture_file: PathBuf,
    fixture_passed: bool,
    fixture_cases: usize,
    fixture_failures: usize,
    maturity_state: String,
}

#[derive(Debug)]
struct DiscoverRun {
    query: String,
    concept: String,
    run_dir: PathBuf,
    corpus_examples: usize,
    rules_queries: u32,
}

#[derive(Debug)]
struct GrammarTestOptions {
    concept: Option<String>,
    fixture: Option<PathBuf>,
    json: bool,
}

#[derive(Debug)]
struct GrammarTestRun {
    result: FixtureRunResult,
    json: bool,
}

#[derive(Debug)]
struct Phase2Options {
    concept: Option<String>,
    fixture: Option<PathBuf>,
    update: bool,
    json: bool,
}

#[derive(Debug)]
struct Phase2MapOptions {
    json: bool,
    include_all: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Phase2GrindOptions {
    agent: AgentProvider,
    max_iterations: u32,
    concept: Option<String>,
    dry_run: bool,
    allow_dirty: bool,
    no_commit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConceptPhaseLoopOptions {
    agent: AgentProvider,
    phase2_batch_size: u32,
    phase2_max_batches: Option<u32>,
    phase2_max_commits: Option<u32>,
    repeat_stop_after: u32,
    phase1_batch_size: u32,
    phase1_max_batches: Option<u32>,
    dry_run: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct PhaseLoopBatchSummary {
    batch: u32,
    before: PhaseLoopMapSummary,
    after: PhaseLoopMapSummary,
    commits: Vec<PhaseLoopCommitSummary>,
    stop_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PhaseLoopMapSummary {
    parse_green_concepts: usize,
    ast_green_concepts: usize,
    parse_failed_concepts: usize,
    ast_failed_concepts: usize,
    grammar_green_concepts: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct PhaseLoopCommitSummary {
    sha: String,
    concept: Option<String>,
    subject: String,
    added_statement_variants: Vec<String>,
}

#[derive(Debug)]
struct PhaseStatusOptions {
    json: bool,
}

#[derive(Debug, Serialize)]
struct PhaseStatusReport {
    verdict: PhaseStatusVerdict,
    reasons: Vec<String>,
    running_processes: Vec<String>,
    current: PhaseLoopMapSummary,
    latest_phase_loop_dir: Option<PathBuf>,
    latest_batch_summary_path: Option<PathBuf>,
    latest_batch: Option<PhaseStatusBatchReport>,
    json: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum PhaseStatusVerdict {
    Continue,
    Inspect,
    Stopped,
}

#[derive(Debug, Serialize)]
struct PhaseStatusBatchReport {
    batch: u32,
    ast_green_delta: i64,
    parse_green_delta: i64,
    commits: usize,
    concepts: Vec<String>,
    added_statement_variants: Vec<String>,
    stop_reasons: Vec<String>,
}

#[derive(Debug)]
struct RoadmapOptions {
    json: bool,
    kind: RoadmapKindFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoadmapKindFilter {
    All,
    Actions,
    Abilities,
    Effects,
}

#[derive(Debug, Serialize, Deserialize)]
struct ParseReport {
    concept: String,
    fixture_path: PathBuf,
    passed: bool,
    total: usize,
    failures: usize,
    cases: Vec<ParseCaseResult>,
    #[serde(skip)]
    json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ParseCaseResult {
    index: usize,
    rule: String,
    text: String,
    parsed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    ast: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct AstTestReport {
    concept: String,
    fixture_path: PathBuf,
    snapshot_path: PathBuf,
    updated: bool,
    passed: bool,
    total: usize,
    failures: usize,
    cases: Vec<AstCaseResult>,
    #[serde(skip)]
    json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AstCaseResult {
    index: usize,
    rule: String,
    text: String,
    matched: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_ast: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    actual_ast: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AstSnapshot {
    concept: String,
    fixture_path: PathBuf,
    cases: Vec<AstSnapshotCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AstSnapshotCase {
    index: usize,
    rule: String,
    text: String,
    ast: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct Phase2MapReport {
    total_concepts: usize,
    grammar_green_concepts: usize,
    concepts_reported: usize,
    parse_green_concepts: usize,
    ast_green_concepts: usize,
    parse_failed_concepts: usize,
    missing_snapshot_concepts: usize,
    ast_failed_concepts: usize,
    missing_fixture_concepts: usize,
    total_accepted_examples: usize,
    parsed_examples: usize,
    ast_snapshot_examples: usize,
    concepts: Vec<Phase2ConceptStatus>,
    #[serde(skip)]
    json: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Phase2ConceptStatus {
    concept: String,
    maturity: String,
    concept_file: PathBuf,
    fixture_path: PathBuf,
    snapshot_path: PathBuf,
    grammar_green: bool,
    accepted_examples: usize,
    parse_passed: bool,
    parse_failures: usize,
    ast_status: Phase2AstStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Phase2AstStatus {
    Pass,
    MissingSnapshot,
    Fail,
    ParseFailed,
    MissingFixture,
    NotGrammarGreen,
}

#[derive(Debug, Serialize)]
struct RoadmapReport {
    total_candidates: usize,
    action_candidates: usize,
    ability_candidates: usize,
    effect_candidates: usize,
    exact_concept_matches: usize,
    mentioned_by_concept: usize,
    missing_candidates: usize,
    grammar_green_candidates: usize,
    parse_green_candidates: usize,
    ast_green_candidates: usize,
    candidates_with_corpus_failures: usize,
    total_corpus_failure_hits: usize,
    candidates: Vec<RoadmapCandidateStatus>,
    #[serde(skip)]
    json: bool,
}

#[derive(Debug, Serialize)]
struct RoadmapCandidateStatus {
    kind: RoadmapCandidateKind,
    rule_ref: String,
    name: String,
    slug: String,
    rules_path: PathBuf,
    coverage: RoadmapCoverage,
    exact_concept: Option<String>,
    mentioned_concepts: Vec<String>,
    phase2_status: Option<Phase2AstStatus>,
    corpus_failure_hits: usize,
    corpus_failure_examples: Vec<CorpusFailureExample>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RoadmapCandidateKind {
    KeywordAction,
    KeywordAbility,
    EffectFamily,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RoadmapCoverage {
    ExactConcept,
    MentionedByConcept,
    Missing,
}

#[derive(Debug, Clone, Serialize)]
struct CorpusFailureExample {
    card: String,
    text: String,
}

#[derive(Debug)]
struct ConceptSearchEntry {
    name: String,
    maturity: String,
    text: String,
}

#[derive(Debug)]
struct GrammarQueryOptions {
    query: String,
    rule_names: Vec<String>,
    limit: usize,
    json: bool,
}

#[derive(Debug)]
struct GrammarQueryRun {
    report: grammar_query_engine::GrammarQueryReport,
    json: bool,
}

#[derive(Debug)]
struct MaturityOptions {
    concept: String,
    json: bool,
    update: bool,
    fresh_fixture: bool,
}

#[derive(Debug)]
struct MapExistingOptions {
    json: bool,
    expand_deps: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ConceptGrindOptions {
    agent: AgentProvider,
    max_iterations: u32,
    concept: Option<String>,
    target_rule: Option<String>,
    query: Option<String>,
    repair_attempts: u8,
    dry_run: bool,
    allow_dirty: bool,
    no_commit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConceptGrindLoopOptions {
    agent: AgentProvider,
    batch_size: u32,
    max_batches: Option<u32>,
    dry_run: bool,
    #[serde(skip)]
    resume: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConceptGrindLoopState {
    options: ConceptGrindLoopOptions,
    next_batch: u32,
    active_experiment: Option<GrindLoopExperiment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GrindLoopExperiment {
    id: String,
    hypothesis: String,
    implementation_request: String,
    success_metric: String,
    quality_checks: Vec<String>,
    #[serde(default)]
    commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GrindLoopReview {
    previous_decision: ExperimentDecision,
    previous_reason: String,
    next_experiment: Option<GrindLoopExperiment>,
}

#[derive(Debug, Serialize)]
struct GrindLoopExperimentHistoryEntry {
    batch: u32,
    experiment: Option<GrindLoopExperiment>,
    decision: ExperimentDecision,
    reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ExperimentDecision {
    Pass,
    Fail,
    None,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExistingGrammarMapReport {
    rule_count: usize,
    concept_count: usize,
    dependency_expansion: bool,
    shared_rule_count: usize,
    mapped_rule_count: usize,
    unmapped_rule_count: usize,
    concepts: Vec<ConceptRuleMap>,
    unmapped_rules: Vec<UnmappedGrammarRule>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ConceptRuleMap {
    concept: String,
    maturity: String,
    concept_file: PathBuf,
    declared_rules: Vec<String>,
    found_rules: Vec<RuleLocationSummary>,
    owned_rules: Vec<RuleLocationSummary>,
    missing_rules: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RuleLocationSummary {
    name: String,
    line: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct UnmappedGrammarRule {
    name: String,
    line: usize,
    suggested_concept: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConceptGap {
    concept: String,
    query: String,
    target_rule: String,
    target_line: usize,
    suggested_existing_owner: bool,
    reason: String,
}

#[derive(Debug)]
struct ConceptGrindGateFailure {
    label: String,
    output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BoundaryDecision {
    owner: BoundaryOwner,
    owner_raw: String,
    axes: String,
    examples_to_accept: String,
    counterexamples_to_reject: String,
    pest_patch_intent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
enum BoundaryOwner {
    Existing(String),
    New(String),
    Blocked(String),
}

#[derive(Debug, Default, Clone, Serialize)]
struct PlumbingCooldownState {
    cooled_target_rules: BTreeSet<String>,
    derivations: Vec<PlumbingCooldownDerivation>,
}

#[derive(Debug, Clone, Serialize)]
struct PlumbingCooldownDerivation {
    blocked_target_rule: String,
    blocked_target_expansion_tree: RuleExpansionNode,
    blocked_target_normalized_leaf_rules: Vec<String>,
    blocked_target_leaf_owning_concepts: BTreeMap<String, Vec<String>>,
    cooled_target_rules: Vec<PlumbingCooldownCandidate>,
    fallback_status: String,
}

#[derive(Debug, Clone, Serialize)]
struct PlumbingCooldownCandidate {
    target_rule: String,
    expansion_tree: RuleExpansionNode,
    normalized_leaf_rules: Vec<String>,
    leaf_owning_concepts: BTreeMap<String, Vec<String>>,
    relationship_type: PlumbingCooldownRelationship,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum PlumbingCooldownRelationship {
    Equal,
    Wrapper,
    Child,
}

#[derive(Debug, Clone, Serialize)]
struct RuleExpansionNode {
    rule: String,
    line: Option<usize>,
    pure_wrapper_or_alternation: bool,
    normalized_leaf_rules: Vec<String>,
    children: Vec<RuleExpansionNode>,
}

#[derive(Debug, Serialize)]
struct PlumbingCooldownSelection {
    exact_blocked_target_rules: Vec<String>,
    cooled_target_rules: Vec<String>,
    excluded_rules: Vec<String>,
    fallback_status: String,
}

#[derive(Debug, Clone, Serialize)]
struct PersistedBlockedExclusion {
    target_rule: String,
    normalized_blocked_reason: String,
    structural_exclusion_reason: String,
    matched_feature: String,
    evidence_rule_or_parent: String,
    source_run: PathBuf,
    source_iteration: u32,
}

#[derive(Debug, Serialize)]
struct CandidateBuild {
    gap: ConceptGap,
    audit: CandidateBuildAudit,
}

#[derive(Debug, Serialize)]
struct CandidateBuildAudit {
    persisted_exclusions: Vec<PersistedBlockedExclusion>,
    plumbing_cooldown_selection: PlumbingCooldownSelection,
    excluded_count: usize,
    excluded_rules: Vec<String>,
    structural_exclusion_reason: String,
    matched_feature: String,
    evidence_rule_or_parent: String,
    remaining_candidate_count: usize,
    selected_post_filter_candidate: Option<CandidateSelectionSummary>,
}

#[derive(Debug, Serialize)]
struct CandidateSelectionSummary {
    concept: String,
    query: String,
    target_rule: String,
    target_line: usize,
    reason: String,
}

#[derive(Debug, Serialize)]
struct SelectorContractReport {
    status: String,
    required_blocked_rules: Vec<String>,
    persisted_blocked_rules: Vec<String>,
    missing_rules: Vec<String>,
    exposed_rules: Vec<String>,
    missing_audit_fields: Vec<String>,
    candidate_build: Option<CandidateBuildAudit>,
}

#[derive(Debug, Serialize)]
struct ConceptGrindIterationSummary {
    iteration: u32,
    concept: String,
    query: String,
    target_rule: String,
    fixture_passed: bool,
    maturity_state: String,
    mapped_rule_count_before: usize,
    mapped_rule_count_after: usize,
    unmapped_rule_count_before: usize,
    unmapped_rule_count_after: usize,
    committed: bool,
}

#[derive(Debug, Serialize)]
struct NoPestConceptFastpathReport {
    fastpath_attempted: bool,
    fastpath_result: String,
    fallback_reason: Option<String>,
    patch_agent_started: bool,
    concept: String,
    original_target_rule: String,
    target_rule: String,
    example_rule: Option<String>,
    resolution_reason: Option<String>,
    mapped_pest_rules: Vec<String>,
    pest_patch_intent: String,
    eligible_shape: bool,
    target_rule_exists: bool,
    concept_path: PathBuf,
    fixture_path: PathBuf,
    generated_concept: bool,
    generated_fixture: bool,
    grammar_pest_changed: bool,
    quality_contract: Option<ConceptQualityContractReport>,
}

#[derive(Debug, Serialize)]
struct ConceptQualityContractReport {
    concept: String,
    fixture_command: Vec<String>,
    maturity_command: Vec<String>,
    fixture_result: FixtureRunResult,
    maturity_result: MaturityReport,
    passed: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ConceptQualityContractFailure {
    reason: &'static str,
    concept: String,
    fixture_command: Vec<String>,
    maturity_command: Vec<String>,
    fixture_result: FixtureRunResult,
    maturity_result: MaturityReport,
}

#[derive(Debug, Serialize)]
struct ConceptGrindMetrics {
    workflow: String,
    session_dir: PathBuf,
    started_unix_ms: u128,
    iterations: Vec<ConceptGrindIterationMetrics>,

    #[serde(skip)]
    active_steps: BTreeMap<(u32, u8), ActiveConceptGrindStep>,
}

#[derive(Debug)]
struct ActiveConceptGrindStep {
    label: String,
    started: Instant,
    started_unix_ms: u128,
}

#[derive(Debug, Serialize)]
struct ConceptGrindIterationMetrics {
    iteration: u32,
    steps: Vec<ConceptGrindStepMetric>,
}

#[derive(Debug, Serialize)]
struct ConceptGrindStepMetric {
    index: u8,
    label: String,
    started_unix_ms: u128,
    finished_unix_ms: u128,
    duration_ms: u128,
    ok: bool,
    summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct MaturityReport {
    concept: String,
    state: String,
    concept_file: Option<PathBuf>,
    fixture_file: Option<PathBuf>,
    blockers: Vec<String>,
    updated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    fixture_result: Option<FixtureRunResult>,
}

#[derive(Debug, Deserialize)]
struct ConceptDocument {
    concept: ConceptHeader,
    #[serde(default)]
    boundary: Option<ConceptBoundary>,
    #[serde(default)]
    axis: Vec<ConceptAxis>,
    #[serde(default)]
    example: Vec<ConceptExample>,
    #[serde(default)]
    counterexample: Vec<ConceptCounterexample>,
}

#[derive(Debug, Deserialize)]
struct ConceptHeader {
    name: String,
    #[serde(default)]
    rules_terms: Vec<String>,
    #[serde(default)]
    rules_queries: Vec<String>,
    #[serde(default)]
    pest_rules: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ConceptBoundary {
    #[serde(default)]
    includes: Vec<String>,
    #[serde(default)]
    excludes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ConceptAxis {
    name: String,
    #[serde(default)]
    values: Vec<String>,
    #[serde(default)]
    evidence: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ConceptExample {
    text: String,
}

#[derive(Debug, Deserialize)]
struct ConceptCounterexample {
    text: String,
}

#[derive(Debug, Serialize)]
struct RulesArtifact {
    query: String,
    rendered_markdown: String,
    always_loaded: Vec<PathBuf>,
    dynamic_hits: Vec<PathBuf>,
    notes: Vec<String>,
    query_logs: Vec<RulesQueryArtifact>,
}

#[derive(Debug, Serialize)]
struct RulesQueryArtifact {
    query: String,
    hits: Vec<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct CorpusClusterArtifact {
    query: String,
    sets: Vec<String>,
    query_terms: Vec<String>,
    total_cards_scanned: usize,
    total_matching_clauses: usize,
    examples: Vec<CorpusExample>,
    skeletons: Vec<SkeletonCount>,
}

#[derive(Debug, Serialize)]
struct CorpusExample {
    card: String,
    set: String,
    collector_number: String,
    clause: String,
    skeleton: String,
}

#[derive(Debug, Serialize)]
struct SkeletonCount {
    skeleton: String,
    count: usize,
}

#[derive(Debug, Serialize)]
struct AxisArtifact {
    concept: String,
    axes: Vec<AxisCandidate>,
}

#[derive(Debug, Serialize)]
struct AxisCandidate {
    name: &'static str,
    evidence: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DiscoveryMaturityArtifact {
    concept: String,
    pest_grammar: &'static str,
    blockers: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FixtureDocument {
    fixture: FixtureHeader,
    #[serde(default)]
    phase2: Option<FixturePhase2>,
    #[serde(default)]
    example: Vec<FixtureCase>,
    #[serde(default)]
    counterexample: Vec<FixtureCase>,
}

#[derive(Debug, Deserialize)]
struct FixtureHeader {
    concept: String,
    rule: String,
}

#[derive(Debug, Deserialize)]
struct FixturePhase2 {
    #[serde(default)]
    ast_shape: Option<FixtureAstShapeContract>,
}

#[derive(Debug, Deserialize)]
struct FixtureAstShapeContract {
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    forbid: Vec<String>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FixtureCase {
    text: String,
    #[serde(default)]
    rule: Option<String>,
    #[serde(default)]
    expect: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FixtureRunResult {
    concept: String,
    fixture_path: PathBuf,
    passed: bool,
    total: usize,
    failures: usize,
    grammar_drift: GrammarDriftSummary,
    cases: Vec<FixtureCaseResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GrammarDriftSummary {
    duplicate_rhs_shape_groups: usize,
    quantity_like_duplicate_rhs_shape_groups: usize,
    similar_rhs_shape_pairs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FixtureCaseResult {
    kind: String,
    rule: String,
    text: String,
    expected: String,
    matched: bool,
    passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn parse_discover_options(args: &[String]) -> Result<DiscoverOptions> {
    let mut query = None;
    let mut concept = None;
    let mut sets = Vec::new();
    let mut limit = 25usize;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => bail!("{DISCOVER_USAGE}"),
            "--query" => query = iter.next().cloned(),
            "--concept" => concept = iter.next().cloned(),
            "--set" => {
                let set = iter
                    .next()
                    .ok_or_else(|| anyhow!("--set requires a value"))?;
                sets.push(set.to_lowercase());
            }
            "--limit" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| anyhow!("--limit requires a value"))?;
                limit = raw
                    .parse()
                    .with_context(|| format!("parse --limit value {raw:?}"))?;
            }
            s if s.starts_with("--query=") => {
                query = Some(s["--query=".len()..].to_string());
            }
            s if s.starts_with("--concept=") => {
                concept = Some(s["--concept=".len()..].to_string());
            }
            s if s.starts_with("--set=") => {
                sets.push(s["--set=".len()..].to_lowercase());
            }
            s if s.starts_with("--limit=") => {
                let raw = &s["--limit=".len()..];
                limit = raw
                    .parse()
                    .with_context(|| format!("parse --limit value {raw:?}"))?;
            }
            other => bail!("unknown argument: {other}\n\n{DISCOVER_USAGE}"),
        }
    }
    let query = query.ok_or_else(|| anyhow!("--query is required\n\n{DISCOVER_USAGE}"))?;
    if query.trim().is_empty() {
        bail!("--query must not be empty");
    }
    if limit == 0 {
        bail!("--limit must be greater than zero");
    }
    if sets.is_empty() {
        sets = tracked_sets()?;
    }
    Ok(DiscoverOptions {
        query,
        concept,
        sets,
        limit,
    })
}

fn parse_grow_options(args: &[String]) -> Result<GrowOptions> {
    let mut query = None;
    let mut concept = None;
    let mut rule = None;
    let mut sets = Vec::new();
    let mut limit = 25usize;
    let mut force = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => bail!("{GROW_USAGE}"),
            "--force" => force = true,
            "--query" => query = iter.next().cloned(),
            "--concept" => concept = iter.next().cloned(),
            "--rule" => rule = iter.next().cloned(),
            "--set" => {
                let set = iter
                    .next()
                    .ok_or_else(|| anyhow!("--set requires a value"))?;
                sets.push(set.to_lowercase());
            }
            "--limit" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| anyhow!("--limit requires a value"))?;
                limit = raw
                    .parse()
                    .with_context(|| format!("parse --limit value {raw:?}"))?;
            }
            s if s.starts_with("--query=") => query = Some(s["--query=".len()..].to_string()),
            s if s.starts_with("--concept=") => concept = Some(s["--concept=".len()..].to_string()),
            s if s.starts_with("--rule=") => rule = Some(s["--rule=".len()..].to_string()),
            s if s.starts_with("--set=") => sets.push(s["--set=".len()..].to_lowercase()),
            s if s.starts_with("--limit=") => {
                let raw = &s["--limit=".len()..];
                limit = raw
                    .parse()
                    .with_context(|| format!("parse --limit value {raw:?}"))?;
            }
            other => bail!("unknown argument: {other}\n\n{GROW_USAGE}"),
        }
    }
    let query = query.ok_or_else(|| anyhow!("--query is required\n\n{GROW_USAGE}"))?;
    let concept = concept.ok_or_else(|| anyhow!("--concept is required\n\n{GROW_USAGE}"))?;
    if query.trim().is_empty() {
        bail!("--query must not be empty");
    }
    if concept.trim().is_empty() {
        bail!("--concept must not be empty");
    }
    if limit == 0 {
        bail!("--limit must be greater than zero");
    }
    if sets.is_empty() {
        sets = tracked_sets()?;
    }
    Ok(GrowOptions {
        discover: DiscoverOptions {
            query,
            concept: Some(concept),
            sets,
            limit,
        },
        rule,
        force,
    })
}

fn run_discover(options: DiscoverOptions) -> Result<DiscoverRun> {
    let concept = options
        .concept
        .clone()
        .unwrap_or_else(|| slug(&options.query).replace('-', "_"));
    let run_dir = grammar_concept_log_root().join(format!("{}-{concept}", unix_timestamp()));
    fs::create_dir_all(&run_dir).with_context(|| format!("create {}", run_dir.display()))?;

    let (rules_markdown, rules_search) =
        rules_context::render_rules_block_with_search(&options.query);
    let rules_artifact = RulesArtifact {
        query: options.query.clone(),
        rendered_markdown: rules_markdown,
        always_loaded: rules_search.always_loaded,
        dynamic_hits: rules_search.dynamic_hits,
        notes: rules_search.notes,
        query_logs: rules_search
            .query_logs
            .into_iter()
            .map(|log| RulesQueryArtifact {
                query: log.query,
                hits: log.hits,
                error: log.error,
            })
            .collect(),
    };
    write_json(run_dir.join("rules_context.json"), &rules_artifact)?;

    let corpus = build_corpus_cluster(&options.query, &options.sets, options.limit)?;
    write_json(run_dir.join("corpus_cluster.json"), &corpus)?;

    let axes = AxisArtifact {
        concept: concept.clone(),
        axes: infer_axes(&corpus.examples),
    };
    write_json(run_dir.join("axes.json"), &axes)?;

    let grammar_neighbors = build_grammar_query_report(&options.query, Vec::new(), options.limit)?;
    write_json(run_dir.join("grammar_neighbors.json"), &grammar_neighbors)?;

    let blockers = discovery_blockers(&rules_artifact, &corpus, &axes);
    let maturity = DiscoveryMaturityArtifact {
        concept: concept.clone(),
        pest_grammar: if blockers.is_empty() {
            "discovered"
        } else {
            "blocked"
        },
        blockers,
    };
    write_json(run_dir.join("maturity.json"), &maturity)?;
    write_concept_stub(
        run_dir.join("concept_stub.toml"),
        &concept,
        &options.query,
        &corpus,
        &axes,
    )?;

    Ok(DiscoverRun {
        query: options.query,
        concept,
        run_dir,
        corpus_examples: corpus.examples.len(),
        rules_queries: rules_artifact.query_logs.len() as u32,
    })
}

fn run_grow(options: GrowOptions) -> Result<GrowRun> {
    let discover = run_discover(options.discover.clone())?;
    let concept = discover.concept.clone();
    let concept_file = grammar_concepts_dir().join(format!("{concept}.toml"));
    let fixture_file = grammar_fixtures_dir().join(format!("{concept}.toml"));
    if !options.force {
        if concept_file.exists() {
            bail!(
                "{} already exists; pass --force to overwrite",
                concept_file.display()
            );
        }
        if fixture_file.exists() {
            bail!(
                "{} already exists; pass --force to overwrite",
                fixture_file.display()
            );
        }
    }

    let corpus = build_corpus_cluster(
        &options.discover.query,
        &options.discover.sets,
        options.discover.limit,
    )?;
    let axes = AxisArtifact {
        concept: concept.clone(),
        axes: infer_axes(&corpus.examples),
    };
    let grammar_report = build_grammar_query_report(
        &options.discover.query,
        options.rule.clone().into_iter().collect(),
        options.discover.limit,
    )?;
    let rule = choose_fixture_rule(&concept, options.rule.as_deref(), &grammar_report)?;
    let fixture = build_fixture_document(&concept, &rule, &corpus)?;

    fs::create_dir_all(grammar_concepts_dir()).context("create grammar-concepts")?;
    fs::create_dir_all(grammar_fixtures_dir()).context("create grammar-fixtures")?;
    write_grown_concept_file(
        &concept_file,
        &concept,
        &options.discover.query,
        &rule,
        &corpus,
        &axes,
        &grammar_report,
    )?;
    fs::write(&fixture_file, fixture)
        .with_context(|| format!("write {}", fixture_file.display()))?;

    let fixture_result = run_fixture_file(&fixture_file)?;
    let maturity = run_maturity(MaturityOptions {
        concept: concept.clone(),
        json: false,
        update: true,
        fresh_fixture: false,
    })?;

    Ok(GrowRun {
        concept,
        run_dir: discover.run_dir,
        rule,
        concept_file,
        fixture_file,
        fixture_passed: fixture_result.passed,
        fixture_cases: fixture_result.total,
        fixture_failures: fixture_result.failures,
        maturity_state: maturity.state,
    })
}

fn parse_grammar_query_options(args: &[String]) -> Result<GrammarQueryOptions> {
    let mut query = None;
    let mut rule_names = Vec::new();
    let mut limit = 25usize;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => bail!("{GRAMMAR_QUERY_USAGE}"),
            "--json" => json = true,
            "--query" => query = iter.next().cloned(),
            "--rule" => {
                let rule = iter
                    .next()
                    .ok_or_else(|| anyhow!("--rule requires a value"))?;
                rule_names.push(rule.clone());
            }
            "--limit" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| anyhow!("--limit requires a value"))?;
                limit = raw
                    .parse()
                    .with_context(|| format!("parse --limit value {raw:?}"))?;
            }
            s if s.starts_with("--query=") => {
                query = Some(s["--query=".len()..].to_string());
            }
            s if s.starts_with("--rule=") => {
                rule_names.push(s["--rule=".len()..].to_string());
            }
            s if s.starts_with("--limit=") => {
                let raw = &s["--limit=".len()..];
                limit = raw
                    .parse()
                    .with_context(|| format!("parse --limit value {raw:?}"))?;
            }
            other => bail!("unknown argument: {other}\n\n{GRAMMAR_QUERY_USAGE}"),
        }
    }
    let query = query.ok_or_else(|| anyhow!("--query is required\n\n{GRAMMAR_QUERY_USAGE}"))?;
    if query.trim().is_empty() {
        bail!("--query must not be empty");
    }
    if limit == 0 {
        bail!("--limit must be greater than zero");
    }
    Ok(GrammarQueryOptions {
        query,
        rule_names,
        limit,
        json,
    })
}

fn run_grammar_query(options: GrammarQueryOptions) -> Result<GrammarQueryRun> {
    let report = build_grammar_query_report(&options.query, options.rule_names, options.limit)?;
    Ok(GrammarQueryRun {
        report,
        json: options.json,
    })
}

fn parse_grammar_test_options(args: &[String]) -> Result<GrammarTestOptions> {
    let mut concept = None;
    let mut fixture = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => bail!("{GRAMMAR_TEST_USAGE}"),
            "--json" => json = true,
            "--fixture" => {
                fixture = Some(PathBuf::from(
                    iter.next()
                        .ok_or_else(|| anyhow!("--fixture requires a value"))?,
                ));
            }
            s if s.starts_with("--fixture=") => {
                fixture = Some(PathBuf::from(&s["--fixture=".len()..]));
            }
            other if other.starts_with('-') => {
                bail!("unknown argument: {other}\n\n{GRAMMAR_TEST_USAGE}");
            }
            positional => {
                if concept.replace(positional.to_string()).is_some() {
                    bail!("only one concept may be provided\n\n{GRAMMAR_TEST_USAGE}");
                }
            }
        }
    }
    if concept.is_none() && fixture.is_none() {
        bail!("concept or --fixture is required\n\n{GRAMMAR_TEST_USAGE}");
    }
    Ok(GrammarTestOptions {
        concept,
        fixture,
        json,
    })
}

fn run_grammar_test(options: GrammarTestOptions) -> Result<GrammarTestRun> {
    let fixture_path = match options.fixture {
        Some(path) => path,
        None => grammar_fixtures_dir().join(format!(
            "{}.toml",
            options.concept.expect("checked by parser")
        )),
    };
    let result = run_fixture_file(&fixture_path)?;
    Ok(GrammarTestRun {
        result,
        json: options.json,
    })
}

fn parse_phase2_options(args: &[String], usage: &str) -> Result<Phase2Options> {
    let mut concept = None;
    let mut fixture = None;
    let mut update = false;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => bail!("{usage}"),
            "--json" => json = true,
            "--update" => update = true,
            "--fixture" => {
                fixture = Some(PathBuf::from(
                    iter.next()
                        .ok_or_else(|| anyhow!("--fixture requires a value"))?,
                ));
            }
            s if s.starts_with("--fixture=") => {
                fixture = Some(PathBuf::from(&s["--fixture=".len()..]));
            }
            other if other.starts_with('-') => bail!("unknown argument: {other}\n\n{usage}"),
            positional => {
                if concept.replace(positional.to_string()).is_some() {
                    bail!("only one concept may be provided\n\n{usage}");
                }
            }
        }
    }
    if concept.is_none() && fixture.is_none() {
        bail!("concept or --fixture is required\n\n{usage}");
    }
    Ok(Phase2Options {
        concept,
        fixture,
        update,
        json,
    })
}

fn parse_phase2_map_options(args: &[String]) -> Result<Phase2MapOptions> {
    let mut json = false;
    let mut include_all = false;
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => bail!("{PHASE2_MAP_USAGE}"),
            "--json" => json = true,
            "--all" => include_all = true,
            other => bail!("unknown argument: {other}\n\n{PHASE2_MAP_USAGE}"),
        }
    }
    Ok(Phase2MapOptions { json, include_all })
}

pub(crate) fn parse_phase2_grind_options(args: &[String]) -> Result<Phase2GrindOptions> {
    let mut agent = AgentProvider::Codex;
    let mut max_iterations = 1u32;
    let mut concept = None::<String>;
    let mut dry_run = false;
    let mut allow_dirty = false;
    let mut no_commit = false;

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => bail!("{PHASE2_GRIND_USAGE}"),
            "--agent" => {
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow!("--agent requires a value"))?;
                agent = parse_agent_provider(value)?;
            }
            s if s.starts_with("--agent=") => {
                agent = parse_agent_provider(&s["--agent=".len()..])?;
            }
            "--max-iterations" => {
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow!("--max-iterations requires a value"))?;
                max_iterations = value
                    .parse()
                    .with_context(|| format!("--max-iterations value: {value:?}"))?;
            }
            s if s.starts_with("--max-iterations=") => {
                max_iterations = s["--max-iterations=".len()..]
                    .parse()
                    .with_context(|| format!("--max-iterations value: {s:?}"))?;
            }
            "--concept" => {
                concept = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--concept requires a value"))?
                        .to_string(),
                );
            }
            s if s.starts_with("--concept=") => {
                concept = Some(s["--concept=".len()..].to_string());
            }
            "--dry-run" => dry_run = true,
            "--allow-dirty" => allow_dirty = true,
            "--no-commit" => no_commit = true,
            "--ui" => {
                let _ = iter
                    .next()
                    .ok_or_else(|| anyhow!("--ui requires a value"))?;
            }
            s if s.starts_with("--ui=") => {}
            other => bail!("unknown argument: {other}\n\n{PHASE2_GRIND_USAGE}"),
        }
    }

    Ok(Phase2GrindOptions {
        agent,
        max_iterations,
        concept,
        dry_run,
        allow_dirty,
        no_commit,
    })
}

fn parse_phase_loop_options(args: &[String]) -> Result<ConceptPhaseLoopOptions> {
    let mut agent = AgentProvider::Codex;
    let mut phase2_batch_size = 5u32;
    let mut phase2_max_batches = None::<u32>;
    let mut phase2_max_commits = None::<u32>;
    let mut repeat_stop_after = 2u32;
    let mut phase1_batch_size = 5u32;
    let mut phase1_max_batches = None::<u32>;
    let mut dry_run = false;

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => bail!("{PHASE_LOOP_USAGE}"),
            "--agent" => {
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow!("--agent requires a value"))?;
                agent = parse_agent_provider(value)?;
            }
            s if s.starts_with("--agent=") => {
                agent = parse_agent_provider(&s["--agent=".len()..])?;
            }
            "--phase2-batch-size" => {
                phase2_batch_size = parse_next_u32(&mut iter, "--phase2-batch-size")?;
            }
            s if s.starts_with("--phase2-batch-size=") => {
                phase2_batch_size = parse_u32_flag_value("--phase2-batch-size", s)?;
            }
            "--phase2-max-batches" => {
                phase2_max_batches = Some(parse_next_u32(&mut iter, "--phase2-max-batches")?);
            }
            s if s.starts_with("--phase2-max-batches=") => {
                phase2_max_batches = Some(parse_u32_flag_value("--phase2-max-batches", s)?);
            }
            "--phase2-max-commits" => {
                phase2_max_commits = Some(parse_next_u32(&mut iter, "--phase2-max-commits")?);
            }
            s if s.starts_with("--phase2-max-commits=") => {
                phase2_max_commits = Some(parse_u32_flag_value("--phase2-max-commits", s)?);
            }
            "--repeat-stop-after" => {
                repeat_stop_after = parse_next_u32(&mut iter, "--repeat-stop-after")?;
            }
            s if s.starts_with("--repeat-stop-after=") => {
                repeat_stop_after = parse_u32_flag_value("--repeat-stop-after", s)?;
            }
            "--phase1-batch-size" => {
                phase1_batch_size = parse_next_u32(&mut iter, "--phase1-batch-size")?;
            }
            s if s.starts_with("--phase1-batch-size=") => {
                phase1_batch_size = parse_u32_flag_value("--phase1-batch-size", s)?;
            }
            "--phase1-max-batches" => {
                phase1_max_batches = Some(parse_next_u32(&mut iter, "--phase1-max-batches")?);
            }
            s if s.starts_with("--phase1-max-batches=") => {
                phase1_max_batches = Some(parse_u32_flag_value("--phase1-max-batches", s)?);
            }
            "--dry-run" => dry_run = true,
            other => bail!("unknown argument: {other}\n\n{PHASE_LOOP_USAGE}"),
        }
    }

    for (name, value) in [
        ("--phase2-batch-size", Some(phase2_batch_size)),
        ("--phase2-max-batches", phase2_max_batches),
        ("--phase2-max-commits", phase2_max_commits),
        ("--repeat-stop-after", Some(repeat_stop_after)),
        ("--phase1-batch-size", Some(phase1_batch_size)),
        ("--phase1-max-batches", phase1_max_batches),
    ] {
        if value == Some(0) {
            bail!("{name} must be greater than zero");
        }
    }

    Ok(ConceptPhaseLoopOptions {
        agent,
        phase2_batch_size,
        phase2_max_batches,
        phase2_max_commits,
        repeat_stop_after,
        phase1_batch_size,
        phase1_max_batches,
        dry_run,
    })
}

fn parse_phase_status_options(args: &[String]) -> Result<PhaseStatusOptions> {
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => bail!("{PHASE_STATUS_USAGE}"),
            "--json" => json = true,
            other => bail!("unknown argument: {other}\n\n{PHASE_STATUS_USAGE}"),
        }
    }
    Ok(PhaseStatusOptions { json })
}

fn parse_next_u32(iter: &mut std::slice::Iter<'_, String>, flag: &str) -> Result<u32> {
    let value = iter
        .next()
        .ok_or_else(|| anyhow!("{flag} requires a value"))?;
    value
        .parse()
        .with_context(|| format!("{flag} value: {value:?}"))
}

fn parse_u32_flag_value(flag: &str, arg: &str) -> Result<u32> {
    let prefix = format!("{flag}=");
    arg[prefix.len()..]
        .parse()
        .with_context(|| format!("{flag} value: {arg:?}"))
}

fn parse_roadmap_options(args: &[String]) -> Result<RoadmapOptions> {
    let mut json = false;
    let mut kind = RoadmapKindFilter::All;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => bail!("{ROADMAP_USAGE}"),
            "--json" => json = true,
            "--kind" => {
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow!("--kind requires a value"))?;
                kind = parse_roadmap_kind_filter(value)?;
            }
            s if s.starts_with("--kind=") => {
                kind = parse_roadmap_kind_filter(&s["--kind=".len()..])?;
            }
            other => bail!("unknown argument: {other}\n\n{ROADMAP_USAGE}"),
        }
    }
    Ok(RoadmapOptions { json, kind })
}

fn parse_roadmap_kind_filter(value: &str) -> Result<RoadmapKindFilter> {
    match value {
        "all" => Ok(RoadmapKindFilter::All),
        "actions" | "keyword-actions" | "701" => Ok(RoadmapKindFilter::Actions),
        "abilities" | "keyword-abilities" | "702" => Ok(RoadmapKindFilter::Abilities),
        "effects" | "effect-families" | "600" => Ok(RoadmapKindFilter::Effects),
        other => bail!("--kind must be all, actions, abilities, or effects; got {other:?}"),
    }
}

fn phase2_fixture_path(options: &Phase2Options) -> PathBuf {
    match &options.fixture {
        Some(path) => path.clone(),
        None => grammar_fixtures_dir().join(format!(
            "{}.toml",
            options.concept.as_ref().expect("checked by parser")
        )),
    }
}

fn ast_fixture_path(concept: &str) -> PathBuf {
    repo_root()
        .join("ast-fixtures")
        .join(format!("{concept}.json"))
}

fn repo_relative_path(path: &Path) -> PathBuf {
    path.strip_prefix(repo_root()).unwrap_or(path).to_path_buf()
}

fn run_concept_parse(options: Phase2Options) -> Result<ParseReport> {
    let fixture_path = phase2_fixture_path(&options);
    let doc = read_fixture_document(&fixture_path)?;
    let mut cases = Vec::new();
    for (index, case) in doc.example.iter().enumerate() {
        let rule = case
            .rule
            .as_deref()
            .unwrap_or(&doc.fixture.rule)
            .to_string();
        let parsed = parse_card_text(&case.text);
        match parsed {
            Ok(ast) => cases.push(ParseCaseResult {
                index: index + 1,
                rule,
                text: case.text.clone(),
                parsed: true,
                ast: Some(serde_json::to_value(ast).context("serialize AST")?),
                error: None,
            }),
            Err(error) => cases.push(ParseCaseResult {
                index: index + 1,
                rule,
                text: case.text.clone(),
                parsed: false,
                ast: None,
                error: Some(error.to_string()),
            }),
        }
    }
    let failures = cases.iter().filter(|case| !case.parsed).count();
    Ok(ParseReport {
        concept: doc.fixture.concept,
        fixture_path,
        passed: failures == 0,
        total: cases.len(),
        failures,
        cases,
        json: options.json,
    })
}

fn run_ast_test(options: Phase2Options) -> Result<AstTestReport> {
    let parse_report = run_concept_parse(Phase2Options {
        concept: options.concept.clone(),
        fixture: options.fixture.clone(),
        update: false,
        json: false,
    })?;
    let fixture_doc = read_fixture_document(&parse_report.fixture_path)?;
    let ast_shape = fixture_doc
        .phase2
        .as_ref()
        .and_then(|phase2| phase2.ast_shape.as_ref());
    let snapshot_path = ast_fixture_path(&parse_report.concept);
    let actual_snapshot = AstSnapshot {
        concept: parse_report.concept.clone(),
        fixture_path: repo_relative_path(&parse_report.fixture_path),
        cases: parse_report
            .cases
            .iter()
            .filter_map(|case| {
                Some(AstSnapshotCase {
                    index: case.index,
                    rule: case.rule.clone(),
                    text: case.text.clone(),
                    ast: case.ast.clone()?,
                })
            })
            .collect(),
    };

    if options.update {
        if !parse_report.passed {
            bail!(
                "cannot update AST snapshot because {} accepted example(s) failed full parse",
                parse_report.failures
            );
        }
        validate_ast_snapshot_shape(&actual_snapshot, ast_shape)?;
        if let Some(parent) = snapshot_path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        write_json(snapshot_path.clone(), &actual_snapshot)?;
        let total = actual_snapshot.cases.len();
        return Ok(AstTestReport {
            concept: parse_report.concept,
            fixture_path: parse_report.fixture_path,
            snapshot_path,
            updated: true,
            passed: true,
            total,
            failures: 0,
            cases: actual_snapshot
                .cases
                .into_iter()
                .map(|case| AstCaseResult {
                    index: case.index,
                    rule: case.rule,
                    text: case.text,
                    matched: true,
                    expected_ast: Some(case.ast.clone()),
                    actual_ast: Some(case.ast),
                    error: None,
                })
                .collect(),
            json: options.json,
        });
    }

    if !snapshot_path.exists() {
        bail!(
            "missing AST snapshot {}; run concept-ast-test --update first",
            snapshot_path.display()
        );
    }
    let expected: AstSnapshot = serde_json::from_str(
        &fs::read_to_string(&snapshot_path)
            .with_context(|| format!("read {}", snapshot_path.display()))?,
    )
    .with_context(|| format!("parse {}", snapshot_path.display()))?;
    let mut expected_by_index = BTreeMap::new();
    for case in expected.cases {
        expected_by_index.insert(case.index, case);
    }
    let mut cases = Vec::new();
    let mut actual_indices = BTreeSet::new();
    for actual in &parse_report.cases {
        actual_indices.insert(actual.index);
        let expected = expected_by_index.get(&actual.index);
        let snapshot_matched = actual.parsed
            && expected.is_some_and(|expected| {
                expected.text == actual.text
                    && expected.rule == actual.rule
                    && Some(&expected.ast) == actual.ast.as_ref()
            });
        let shape_error = actual
            .ast
            .as_ref()
            .and_then(|ast| ast_shape_case_error(ast, ast_shape));
        let matched = snapshot_matched && shape_error.is_none();
        cases.push(AstCaseResult {
            index: actual.index,
            rule: actual.rule.clone(),
            text: actual.text.clone(),
            matched,
            expected_ast: expected.map(|case| case.ast.clone()),
            actual_ast: actual.ast.clone(),
            error: if matched {
                None
            } else if let Some(shape_error) = shape_error {
                Some(shape_error)
            } else if !actual.parsed {
                actual.error.clone()
            } else {
                Some("AST snapshot mismatch".to_string())
            },
        });
    }
    for (index, expected) in expected_by_index {
        if actual_indices.contains(&index) {
            continue;
        }
        cases.push(AstCaseResult {
            index,
            rule: expected.rule,
            text: expected.text,
            matched: false,
            expected_ast: Some(expected.ast),
            actual_ast: None,
            error: Some("AST snapshot case missing from fixture".to_string()),
        });
    }
    cases.sort_by_key(|case| case.index);
    let failures = cases.iter().filter(|case| !case.matched).count();
    Ok(AstTestReport {
        concept: parse_report.concept,
        fixture_path: parse_report.fixture_path,
        snapshot_path,
        updated: false,
        passed: failures == 0,
        total: cases.len(),
        failures,
        cases,
        json: options.json,
    })
}

fn validate_ast_snapshot_shape(
    snapshot: &AstSnapshot,
    contract: Option<&FixtureAstShapeContract>,
) -> Result<()> {
    let Some(contract) = contract else {
        return Ok(());
    };
    let violations: Vec<String> = snapshot
        .cases
        .iter()
        .filter_map(|case| {
            ast_shape_case_error(&case.ast, Some(contract))
                .map(|error| format!("example #{}: {error}", case.index))
        })
        .collect();
    if violations.is_empty() {
        Ok(())
    } else {
        bail!("AST shape contract failed:\n{}", violations.join("\n"))
    }
}

fn ast_shape_case_error(
    ast: &serde_json::Value,
    contract: Option<&FixtureAstShapeContract>,
) -> Option<String> {
    let contract = contract?;
    let mut variants = BTreeSet::new();
    collect_ast_variant_keys(ast, &mut variants);

    let mut errors = Vec::new();
    if let Some(owner) = &contract.owner {
        if !variants.contains(owner) {
            errors.push(format!("expected AST owner `{owner}`"));
        }
    }
    for forbidden in &contract.forbid {
        if variants.contains(forbidden) {
            errors.push(format!("forbidden legacy AST variant `{forbidden}`"));
        }
    }
    if errors.is_empty() {
        None
    } else if let Some(note) = &contract.note {
        Some(format!("{} ({note})", errors.join("; ")))
    } else {
        Some(errors.join("; "))
    }
}

fn collect_ast_variant_keys(value: &serde_json::Value, out: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if map.len() == 1 {
                if let Some(key) = map.keys().next() {
                    if key.chars().next().is_some_and(char::is_uppercase) {
                        out.insert(key.clone());
                    }
                }
            }
            for child in map.values() {
                collect_ast_variant_keys(child, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_ast_variant_keys(item, out);
            }
        }
        _ => {}
    }
}

fn run_phase2_map(options: Phase2MapOptions) -> Result<Phase2MapReport> {
    let concept_files = read_concept_files()?;
    let total_concepts = concept_files.len();
    let mut concepts = Vec::new();

    for (concept_file, doc, maturity) in concept_files {
        let grammar_green = maturity == "grammar_fixture_green";
        if !grammar_green && !options.include_all {
            continue;
        }

        let concept = doc.concept.name.clone();
        let fixture_path = grammar_fixtures_dir().join(format!("{concept}.toml"));
        let snapshot_path = ast_fixture_path(&concept);
        if !fixture_path.exists() {
            concepts.push(Phase2ConceptStatus {
                concept,
                maturity,
                concept_file,
                fixture_path,
                snapshot_path,
                grammar_green,
                accepted_examples: 0,
                parse_passed: false,
                parse_failures: 0,
                ast_status: Phase2AstStatus::MissingFixture,
                first_error: Some("missing grammar fixture".to_string()),
            });
            continue;
        }

        if !grammar_green {
            let accepted_examples = read_fixture_document(&fixture_path)
                .map(|fixture| fixture.example.len())
                .unwrap_or(0);
            concepts.push(Phase2ConceptStatus {
                concept,
                maturity,
                concept_file,
                fixture_path,
                snapshot_path,
                grammar_green,
                accepted_examples,
                parse_passed: false,
                parse_failures: 0,
                ast_status: Phase2AstStatus::NotGrammarGreen,
                first_error: None,
            });
            continue;
        }

        let parse_report = run_concept_parse(Phase2Options {
            concept: Some(concept.clone()),
            fixture: None,
            update: false,
            json: false,
        })?;
        let first_parse_error = parse_report
            .cases
            .iter()
            .find_map(|case| case.error.as_ref().cloned());
        if !parse_report.passed {
            concepts.push(Phase2ConceptStatus {
                concept,
                maturity,
                concept_file,
                fixture_path,
                snapshot_path,
                grammar_green,
                accepted_examples: parse_report.total,
                parse_passed: false,
                parse_failures: parse_report.failures,
                ast_status: Phase2AstStatus::ParseFailed,
                first_error: first_parse_error,
            });
            continue;
        }

        if !snapshot_path.exists() {
            concepts.push(Phase2ConceptStatus {
                concept,
                maturity,
                concept_file,
                fixture_path,
                snapshot_path,
                grammar_green,
                accepted_examples: parse_report.total,
                parse_passed: true,
                parse_failures: 0,
                ast_status: Phase2AstStatus::MissingSnapshot,
                first_error: Some("missing AST snapshot".to_string()),
            });
            continue;
        }

        let ast_report = run_ast_test(Phase2Options {
            concept: Some(concept.clone()),
            fixture: None,
            update: false,
            json: false,
        })?;
        let ast_status = if ast_report.passed {
            Phase2AstStatus::Pass
        } else {
            Phase2AstStatus::Fail
        };
        let first_ast_error = ast_report
            .cases
            .iter()
            .find_map(|case| case.error.as_ref().cloned());
        concepts.push(Phase2ConceptStatus {
            concept,
            maturity,
            concept_file,
            fixture_path,
            snapshot_path,
            grammar_green,
            accepted_examples: parse_report.total,
            parse_passed: true,
            parse_failures: 0,
            ast_status,
            first_error: first_ast_error,
        });
    }

    let grammar_green_concepts = concepts
        .iter()
        .filter(|status| status.grammar_green)
        .count();
    let parse_green_concepts = concepts
        .iter()
        .filter(|status| status.grammar_green && status.parse_passed)
        .count();
    let ast_green_concepts = concepts
        .iter()
        .filter(|status| status.ast_status == Phase2AstStatus::Pass)
        .count();
    let parse_failed_concepts = concepts
        .iter()
        .filter(|status| status.ast_status == Phase2AstStatus::ParseFailed)
        .count();
    let missing_snapshot_concepts = concepts
        .iter()
        .filter(|status| status.ast_status == Phase2AstStatus::MissingSnapshot)
        .count();
    let ast_failed_concepts = concepts
        .iter()
        .filter(|status| status.ast_status == Phase2AstStatus::Fail)
        .count();
    let missing_fixture_concepts = concepts
        .iter()
        .filter(|status| status.ast_status == Phase2AstStatus::MissingFixture)
        .count();
    let total_accepted_examples = concepts
        .iter()
        .filter(|status| status.grammar_green)
        .map(|status| status.accepted_examples)
        .sum();
    let parsed_examples = concepts
        .iter()
        .filter(|status| status.grammar_green && status.parse_passed)
        .map(|status| status.accepted_examples)
        .sum();
    let ast_snapshot_examples = concepts
        .iter()
        .filter(|status| status.ast_status == Phase2AstStatus::Pass)
        .map(|status| status.accepted_examples)
        .sum();

    Ok(Phase2MapReport {
        total_concepts,
        grammar_green_concepts,
        concepts_reported: concepts.len(),
        parse_green_concepts,
        ast_green_concepts,
        parse_failed_concepts,
        missing_snapshot_concepts,
        ast_failed_concepts,
        missing_fixture_concepts,
        total_accepted_examples,
        parsed_examples,
        ast_snapshot_examples,
        concepts,
        json: options.json,
    })
}

fn run_phase2_grind(options: Phase2GrindOptions, sink: &mut dyn FlowSink) -> Result<()> {
    if !options.dry_run && !options.allow_dirty {
        ensure_clean_working_tree().context(
            "concept-phase2-grind requires a clean working tree; use --allow-dirty to override",
        )?;
    }

    let session_dir = grammar_concept_log_root().join(format!(
        "{}-phase2-grind",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system time before Unix epoch")?
            .as_secs()
    ));
    fs::create_dir_all(&session_dir)
        .with_context(|| format!("create {}", session_dir.display()))?;

    let baseline = run_phase2_map_fresh()?;
    sink.emit(FlowEvent::SessionStarted {
        workflow: "concept-phase2-grind".to_string(),
        set: "phase2".to_string(),
        max_iterations: options.max_iterations,
        baseline_corpus_passing: baseline.ast_green_concepts,
        baseline_corpus_total: baseline.grammar_green_concepts,
        baseline_grammar_rules: baseline.parse_green_concepts,
    });

    for iteration in 1..=options.max_iterations {
        let iteration_start = Instant::now();
        let iteration_dir = session_dir.join(format!("iteration-{iteration:03}"));
        fs::create_dir_all(&iteration_dir)
            .with_context(|| format!("create {}", iteration_dir.display()))?;
        let report = run_phase2_map_fresh()?;
        write_json(iteration_dir.join("phase2_map_before.json"), &report)?;

        let Some(candidate) = select_phase2_grind_candidate(&report, options.concept.as_deref())
        else {
            sink.emit(FlowEvent::SessionFinished {
                reason: SessionEndReason::AllPass,
            });
            return Ok(());
        };
        let concept = candidate.concept.clone();
        sink.emit(FlowEvent::WorkflowIterationStarted {
            index: iteration,
            max_iterations: options.max_iterations,
            title: concept.clone(),
            detail: format!(
                "status={:?} parse_green={}/{} ast_green={}/{}",
                candidate.ast_status,
                report.parse_green_concepts,
                report.grammar_green_concepts,
                report.ast_green_concepts,
                report.grammar_green_concepts
            ),
        });

        if options.dry_run {
            println!(
                "phase2 dry-run: next concept {} has status {:?}",
                concept, candidate.ast_status
            );
            sink.emit(FlowEvent::SessionFinished {
                reason: SessionEndReason::DryRunStop,
            });
            return Ok(());
        }

        match candidate.ast_status {
            Phase2AstStatus::MissingSnapshot => {
                sink.emit(FlowEvent::StepStarted {
                    index: 1,
                    total: 4,
                    label: "write AST snapshot".to_string(),
                });
                run_phase2_gate_command(
                    "concept-ast-test-update",
                    "cargo",
                    &["xtask", "concept-ast-test", &concept, "--update"],
                    &iteration_dir,
                )?;
                sink.emit(FlowEvent::StepFinished {
                    index: 1,
                    ok: true,
                    summary: Some("snapshot updated from parse-green examples".to_string()),
                });
            }
            Phase2AstStatus::ParseFailed | Phase2AstStatus::Fail => {
                sink.emit(FlowEvent::StepStarted {
                    index: 1,
                    total: 4,
                    label: "repair agent".to_string(),
                });
                let prompt = build_phase2_repair_prompt(&candidate, &report)?;
                fs::write(iteration_dir.join("repair_prompt.md"), &prompt).with_context(|| {
                    format!("write {}", iteration_dir.join("repair_prompt.md").display())
                })?;
                let outcome = refactor_hotspot::invoke_agent(
                    options.agent,
                    &prompt,
                    &iteration_dir.join("repair_transcript.ndjson"),
                    sink,
                )?;
                fs::write(
                    iteration_dir.join("repair_response.md"),
                    &outcome.assistant_text,
                )
                .with_context(|| {
                    format!(
                        "write {}",
                        iteration_dir.join("repair_response.md").display()
                    )
                })?;
                if !outcome.success {
                    bail!(
                        "{} repair agent exited with status {}; transcript: {}",
                        options.agent.label(),
                        outcome.exit_code,
                        iteration_dir.join("repair_transcript.ndjson").display()
                    );
                }
                sink.emit(FlowEvent::StepFinished {
                    index: 1,
                    ok: true,
                    summary: Some("repair agent completed".to_string()),
                });
            }
            Phase2AstStatus::Pass => {
                sink.emit(FlowEvent::SessionFinished {
                    reason: SessionEndReason::AllPass,
                });
                return Ok(());
            }
            Phase2AstStatus::MissingFixture | Phase2AstStatus::NotGrammarGreen => {
                bail!(
                    "selected concept {} is not actionable for Phase 2: {:?}",
                    concept,
                    candidate.ast_status
                );
            }
        }

        sink.emit(FlowEvent::StepStarted {
            index: 2,
            total: 4,
            label: "phase2 gates".to_string(),
        });
        run_phase2_concept_gates(&concept, &iteration_dir)?;
        sink.emit(FlowEvent::StepFinished {
            index: 2,
            ok: true,
            summary: Some("concept-parse, concept-ast-test, and cargo check passed".to_string()),
        });

        let after = run_phase2_map_fresh()?;
        write_json(iteration_dir.join("phase2_map_after.json"), &after)?;

        sink.emit(FlowEvent::StepStarted {
            index: 3,
            total: 4,
            label: "commit".to_string(),
        });
        let committed = if options.no_commit {
            false
        } else {
            commit_phase2_grind_iteration(&concept, iteration)?
        };
        sink.emit(FlowEvent::StepFinished {
            index: 3,
            ok: true,
            summary: Some(if committed {
                "committed Phase 2 advancement".to_string()
            } else {
                "no commit created".to_string()
            }),
        });

        sink.emit(FlowEvent::StepStarted {
            index: 4,
            total: 4,
            label: "metrics".to_string(),
        });
        sink.emit(FlowEvent::StepFinished {
            index: 4,
            ok: true,
            summary: Some(format!(
                "parse {}/{} -> {}/{}, ast {}/{} -> {}/{}",
                report.parse_green_concepts,
                report.grammar_green_concepts,
                after.parse_green_concepts,
                after.grammar_green_concepts,
                report.ast_green_concepts,
                report.grammar_green_concepts,
                after.ast_green_concepts,
                after.grammar_green_concepts
            )),
        });
        if committed {
            sink.emit(FlowEvent::IterationFinished {
                index: iteration,
                outcome: IterationOutcomeSummary::Committed {
                    new_passes: after
                        .ast_green_concepts
                        .saturating_sub(report.ast_green_concepts),
                    corpus_passing: after.ast_green_concepts,
                    corpus_total: after.grammar_green_concepts,
                    grammar_rules: after.parse_green_concepts,
                    duration_secs: iteration_start.elapsed().as_secs(),
                },
            });
        }
    }

    sink.emit(FlowEvent::SessionFinished {
        reason: SessionEndReason::MaxIterationsReached(options.max_iterations),
    });
    Ok(())
}

fn run_phase2_map_fresh() -> Result<Phase2MapReport> {
    let output = Command::new("cargo")
        .args(["xtask", "concept-phase2-map", "--json"])
        .current_dir(repo_root())
        .output()
        .context("cargo xtask concept-phase2-map --json")?;
    let text = command_output_text(&output);
    if !output.status.success() {
        bail!("fresh concept-phase2-map failed\n{text}");
    }
    serde_json::from_slice::<Phase2MapReport>(&output.stdout)
        .with_context(|| format!("parse fresh concept-phase2-map JSON\n{text}"))
}

fn run_concept_parse_fresh(concept: &str) -> Result<ParseReport> {
    let output = Command::new("cargo")
        .args(["xtask", "concept-parse", concept, "--json"])
        .current_dir(repo_root())
        .output()
        .with_context(|| format!("cargo xtask concept-parse {concept} --json"))?;
    parse_fresh_concept_parse_output(concept, &output)
}

fn parse_fresh_concept_parse_output(
    concept: &str,
    output: &std::process::Output,
) -> Result<ParseReport> {
    let text = command_output_text(&output);
    match serde_json::from_slice::<ParseReport>(&output.stdout) {
        Ok(report) => Ok(report),
        Err(error) if output.status.success() => Err(error)
            .with_context(|| format!("parse fresh concept-parse JSON for {concept}\n{text}")),
        Err(error) => bail!("fresh concept-parse failed for {concept}: {error}\n{text}"),
    }
}

fn select_phase2_grind_candidate<'a>(
    report: &'a Phase2MapReport,
    requested_concept: Option<&str>,
) -> Option<&'a Phase2ConceptStatus> {
    if let Some(requested) = requested_concept {
        return report
            .concepts
            .iter()
            .find(|status| status.concept == requested);
    }
    report
        .concepts
        .iter()
        .find(|status| status.ast_status == Phase2AstStatus::MissingSnapshot)
        .or_else(|| {
            report
                .concepts
                .iter()
                .find(|status| status.ast_status == Phase2AstStatus::ParseFailed)
        })
        .or_else(|| {
            report
                .concepts
                .iter()
                .find(|status| status.ast_status == Phase2AstStatus::Fail)
        })
}

fn build_phase2_repair_prompt(
    candidate: &Phase2ConceptStatus,
    report: &Phase2MapReport,
) -> Result<String> {
    let parse = run_concept_parse_fresh(&candidate.concept)?;
    let parse_json = serde_json::to_string_pretty(&parse).context("serialize parse report")?;
    Ok(format!(
        "\
You are working in the mtg-parser repository.

Goal: advance Phase 2 parser/AST coverage for concept `{concept}` without reducing output quality.

Current Phase 2 metrics:
- grammar-green concepts: {grammar_green}
- parse-green concepts: {parse_green}
- AST-snapshot-green concepts: {ast_green}

Concept artifacts:
- concept file: {concept_file}
- grammar fixture: {fixture_file}
- AST snapshot: {snapshot_file}

Current status: {status:?}
First error: {first_error}

Use the existing concept fixture as the behavioral contract. Make the smallest parser/AST/grammar integration change that makes accepted fixture examples parse through `mtg_grammar::parse` with the concept-owned AST shape. If the fixture has `[phase2.ast_shape]`, treat it as a hard contract: the `owner` variant is the desired concept shape, and `forbid` variants are legacy/card-centric shapes that must be merged away. Prefer generalizing existing rules and parser helpers over adding one rule per example. Do not edit generated tests or run `cargo xtask add-card`.

Allowed implementation areas:
- crates/mtg-grammar/src/grammar.pest
- crates/mtg-grammar/src/ast.rs
- crates/mtg-grammar/src/parse.rs
- grammar-fixtures/{concept}.toml only if an accepted example is malformed as a full oracle/card-text phrase

After editing, run:
- cargo xtask concept-parse {concept}
- cargo xtask concept-ast-test {concept} --update
- cargo xtask concept-ast-test {concept}
- cargo check -p xtask

Current concept-parse report:
```json
{parse_json}
```
",
        concept = candidate.concept,
        grammar_green = report.grammar_green_concepts,
        parse_green = report.parse_green_concepts,
        ast_green = report.ast_green_concepts,
        concept_file = candidate.concept_file.display(),
        fixture_file = candidate.fixture_path.display(),
        snapshot_file = candidate.snapshot_path.display(),
        status = candidate.ast_status,
        first_error = candidate.first_error.as_deref().unwrap_or("none"),
        parse_json = parse_json
    ))
}

fn run_roadmap(options: RoadmapOptions) -> Result<RoadmapReport> {
    let concept_entries = read_concept_search_entries()?;
    let phase2 = run_phase2_map(Phase2MapOptions {
        json: false,
        include_all: false,
    })
    .ok();
    let phase2_by_concept: BTreeMap<String, Phase2AstStatus> = phase2
        .as_ref()
        .map(|report| {
            report
                .concepts
                .iter()
                .map(|status| (status.concept.clone(), status.ast_status))
                .collect()
        })
        .unwrap_or_default();
    let corpus_failures = read_corpus_failure_examples()?;
    let mut candidates = read_rulebook_candidates(options.kind)?;

    for candidate in &mut candidates {
        annotate_roadmap_candidate(
            candidate,
            &concept_entries,
            &phase2_by_concept,
            &corpus_failures,
        );
    }
    candidates.sort_by(|left, right| {
        right
            .corpus_failure_hits
            .cmp(&left.corpus_failure_hits)
            .then_with(|| roadmap_kind_rank(left.kind).cmp(&roadmap_kind_rank(right.kind)))
            .then_with(|| left.rule_ref.cmp(&right.rule_ref))
    });

    let action_candidates = candidates
        .iter()
        .filter(|candidate| candidate.kind == RoadmapCandidateKind::KeywordAction)
        .count();
    let ability_candidates = candidates
        .iter()
        .filter(|candidate| candidate.kind == RoadmapCandidateKind::KeywordAbility)
        .count();
    let effect_candidates = candidates
        .iter()
        .filter(|candidate| candidate.kind == RoadmapCandidateKind::EffectFamily)
        .count();
    let exact_concept_matches = candidates
        .iter()
        .filter(|candidate| candidate.coverage == RoadmapCoverage::ExactConcept)
        .count();
    let mentioned_by_concept = candidates
        .iter()
        .filter(|candidate| candidate.coverage == RoadmapCoverage::MentionedByConcept)
        .count();
    let missing_candidates = candidates
        .iter()
        .filter(|candidate| candidate.coverage == RoadmapCoverage::Missing)
        .count();
    let grammar_green_candidates = candidates
        .iter()
        .filter(|candidate| {
            candidate
                .exact_concept
                .as_ref()
                .and_then(|name| concept_entries.iter().find(|entry| &entry.name == name))
                .is_some_and(|entry| entry.maturity == "grammar_fixture_green")
        })
        .count();
    let parse_green_candidates = candidates
        .iter()
        .filter(|candidate| {
            matches!(
                candidate.phase2_status,
                Some(Phase2AstStatus::MissingSnapshot)
                    | Some(Phase2AstStatus::Fail)
                    | Some(Phase2AstStatus::Pass)
            )
        })
        .count();
    let ast_green_candidates = candidates
        .iter()
        .filter(|candidate| candidate.phase2_status == Some(Phase2AstStatus::Pass))
        .count();
    let candidates_with_corpus_failures = candidates
        .iter()
        .filter(|candidate| candidate.corpus_failure_hits > 0)
        .count();
    let total_corpus_failure_hits = candidates
        .iter()
        .map(|candidate| candidate.corpus_failure_hits)
        .sum();

    Ok(RoadmapReport {
        total_candidates: candidates.len(),
        action_candidates,
        ability_candidates,
        effect_candidates,
        exact_concept_matches,
        mentioned_by_concept,
        missing_candidates,
        grammar_green_candidates,
        parse_green_candidates,
        ast_green_candidates,
        candidates_with_corpus_failures,
        total_corpus_failure_hits,
        candidates,
        json: options.json,
    })
}

fn read_rulebook_candidates(kind: RoadmapKindFilter) -> Result<Vec<RoadmapCandidateStatus>> {
    let mut candidates = Vec::new();
    if matches!(kind, RoadmapKindFilter::All | RoadmapKindFilter::Actions) {
        candidates.extend(read_numbered_rulebook_dir(
            "resources/rules/700-additional-rules/701-keyword-actions",
            RoadmapCandidateKind::KeywordAction,
        )?);
    }
    if matches!(kind, RoadmapKindFilter::All | RoadmapKindFilter::Abilities) {
        candidates.extend(read_numbered_rulebook_dir(
            "resources/rules/700-additional-rules/702-keyword-abilities",
            RoadmapCandidateKind::KeywordAbility,
        )?);
    }
    if matches!(kind, RoadmapKindFilter::All | RoadmapKindFilter::Effects) {
        candidates.extend(read_effect_family_candidates()?);
    }
    Ok(candidates)
}

fn read_numbered_rulebook_dir(
    rel_dir: &str,
    kind: RoadmapCandidateKind,
) -> Result<Vec<RoadmapCandidateStatus>> {
    let dir = repo_root().join(rel_dir);
    let mut candidates = Vec::new();
    if !dir.is_dir() {
        return Ok(candidates);
    }
    for entry in fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry.with_context(|| format!("read entry in {}", dir.display()))?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        let Some((rule_ref, name)) = stem.split_once('-') else {
            continue;
        };
        if name.chars().all(|ch| ch.is_ascii_digit() || ch == '.') {
            continue;
        }
        candidates.push(empty_roadmap_candidate(kind, rule_ref, name, path));
    }
    Ok(candidates)
}

fn read_effect_family_candidates() -> Result<Vec<RoadmapCandidateStatus>> {
    let effect_files = [
        "600-spells-abilities-and-effects/603-handling-triggered-abilities.md",
        "600-spells-abilities-and-effects/604-handling-static-abilities.md",
        "600-spells-abilities-and-effects/605-mana-abilities.md",
        "600-spells-abilities-and-effects/608-resolving-spells-and-abilities.md",
        "600-spells-abilities-and-effects/609-effects.md",
        "600-spells-abilities-and-effects/610-one-shot-effects.md",
        "600-spells-abilities-and-effects/611-continuous-effects.md",
        "600-spells-abilities-and-effects/614-replacement-effects.md",
        "600-spells-abilities-and-effects/615-prevention-effects.md",
    ];
    let mut candidates = Vec::new();
    for rel in effect_files {
        let path = repo_root().join("resources/rules").join(rel);
        let Some(stem) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        let Some((rule_ref, name)) = stem.split_once('-') else {
            continue;
        };
        candidates.push(empty_roadmap_candidate(
            RoadmapCandidateKind::EffectFamily,
            rule_ref,
            name,
            path,
        ));
    }
    Ok(candidates)
}

fn empty_roadmap_candidate(
    kind: RoadmapCandidateKind,
    rule_ref: &str,
    name: &str,
    path: PathBuf,
) -> RoadmapCandidateStatus {
    RoadmapCandidateStatus {
        kind,
        rule_ref: rule_ref.to_string(),
        name: name.replace('-', " "),
        slug: slug(name).replace('-', "_"),
        rules_path: repo_relative_path(&path),
        coverage: RoadmapCoverage::Missing,
        exact_concept: None,
        mentioned_concepts: Vec::new(),
        phase2_status: None,
        corpus_failure_hits: 0,
        corpus_failure_examples: Vec::new(),
    }
}

fn annotate_roadmap_candidate(
    candidate: &mut RoadmapCandidateStatus,
    concepts: &[ConceptSearchEntry],
    phase2_by_concept: &BTreeMap<String, Phase2AstStatus>,
    corpus_failures: &[CorpusFailureExample],
) {
    let needle_slug = candidate.slug.as_str();
    let needle_hyphen = needle_slug.replace('_', "-");
    let needle_words = needle_slug.replace('_', " ");

    candidate.exact_concept = concepts
        .iter()
        .find(|entry| entry.name == needle_slug)
        .map(|entry| entry.name.clone());
    candidate.mentioned_concepts = concepts
        .iter()
        .filter(|entry| {
            entry.name == needle_slug
                || entry.name.contains(needle_slug)
                || contains_ascii_phrase(&entry.text, needle_slug)
                || contains_ascii_phrase(&entry.text, &needle_hyphen)
                || contains_ascii_phrase(&entry.text, &needle_words)
        })
        .map(|entry| entry.name.clone())
        .collect();
    candidate.mentioned_concepts.sort();
    candidate.mentioned_concepts.dedup();
    candidate.coverage = if candidate.exact_concept.is_some() {
        RoadmapCoverage::ExactConcept
    } else if !candidate.mentioned_concepts.is_empty() {
        RoadmapCoverage::MentionedByConcept
    } else {
        RoadmapCoverage::Missing
    };
    candidate.phase2_status = candidate
        .exact_concept
        .as_ref()
        .and_then(|concept| phase2_by_concept.get(concept).copied());

    let corpus_needles = corpus_needles_for_candidate(candidate);
    candidate.corpus_failure_examples = corpus_failures
        .iter()
        .filter(|failure| {
            let text = failure.text.to_ascii_lowercase();
            corpus_needles
                .iter()
                .any(|needle| contains_ascii_phrase(&text, needle))
        })
        .take(5)
        .cloned()
        .collect();
    candidate.corpus_failure_hits = corpus_failures
        .iter()
        .filter(|failure| {
            let text = failure.text.to_ascii_lowercase();
            corpus_needles
                .iter()
                .any(|needle| contains_ascii_phrase(&text, needle))
        })
        .count();
}

fn corpus_needles_for_candidate(candidate: &RoadmapCandidateStatus) -> Vec<String> {
    let name = candidate.name.to_ascii_lowercase();
    let mut needles = vec![name.clone(), name.replace(' ', "")];
    if candidate.kind == RoadmapCandidateKind::EffectFamily {
        match candidate.rule_ref.as_str() {
            "603" => needles.extend([
                "when".to_string(),
                "whenever".to_string(),
                "at the beginning".to_string(),
            ]),
            "611" => needles.extend(["until end of turn".to_string(), "as long as".to_string()]),
            "614" => needles.push("instead".to_string()),
            "615" => needles.push("prevent".to_string()),
            _ => {}
        }
    }
    if candidate.kind == RoadmapCandidateKind::KeywordAbility && name.contains("landwalk") {
        needles.push("walk".to_string());
    }
    needles.retain(|needle| needle.len() >= 4);
    needles.sort();
    needles.dedup();
    needles
}

fn contains_ascii_phrase(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let mut start = 0;
    while let Some(offset) = haystack[start..].find(needle) {
        let absolute = start + offset;
        let before = haystack[..absolute].chars().next_back();
        let after = haystack[absolute + needle.len()..].chars().next();
        if !is_ascii_word_char(before) && !is_ascii_word_char(after) {
            return true;
        }
        start = absolute + needle.len();
    }
    false
}

fn is_ascii_word_char(ch: Option<char>) -> bool {
    ch.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn read_concept_search_entries() -> Result<Vec<ConceptSearchEntry>> {
    let mut entries = Vec::new();
    for (path, doc, maturity) in read_concept_files()? {
        let text = fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?
            .to_ascii_lowercase();
        entries.push(ConceptSearchEntry {
            name: doc.concept.name,
            maturity,
            text,
        });
    }
    Ok(entries)
}

fn read_corpus_failure_examples() -> Result<Vec<CorpusFailureExample>> {
    let path = repo_root().join("corpus_status.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let value: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?,
    )
    .with_context(|| format!("parse {}", path.display()))?;
    let Some(cards) = value.get("cards").and_then(|cards| cards.as_object()) else {
        return Ok(Vec::new());
    };
    let mut failures = Vec::new();
    for (card, status) in cards {
        if status.get("status").and_then(|s| s.as_str()) == Some("pass") {
            continue;
        }
        let Some(error) = status.get("error").and_then(|error| error.as_str()) else {
            continue;
        };
        if error.contains("empty oracle text") {
            continue;
        }
        if let Some(text) = parse_oracle_line_from_error(error) {
            failures.push(CorpusFailureExample {
                card: card.clone(),
                text,
            });
        }
    }
    Ok(failures)
}

fn parse_oracle_line_from_error(error: &str) -> Option<String> {
    let marker = "\n1 | ";
    let after = error.split(marker).nth(1)?;
    let line = after.lines().next()?.trim();
    (!line.is_empty()).then(|| line.to_string())
}

fn roadmap_kind_rank(kind: RoadmapCandidateKind) -> u8 {
    match kind {
        RoadmapCandidateKind::EffectFamily => 0,
        RoadmapCandidateKind::KeywordAction => 1,
        RoadmapCandidateKind::KeywordAbility => 2,
    }
}

fn read_fixture_document(path: &Path) -> Result<FixtureDocument> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

fn parse_maturity_options(args: &[String]) -> Result<MaturityOptions> {
    let mut concept = None;
    let mut json = false;
    let mut update = false;
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => bail!("{MATURITY_USAGE}"),
            "--json" => json = true,
            "--update" => update = true,
            other if other.starts_with('-') => {
                bail!("unknown argument: {other}\n\n{MATURITY_USAGE}")
            }
            positional => {
                if concept.replace(positional.to_string()).is_some() {
                    bail!("only one concept may be provided\n\n{MATURITY_USAGE}");
                }
            }
        }
    }
    let concept = concept.ok_or_else(|| anyhow!("concept is required\n\n{MATURITY_USAGE}"))?;
    Ok(MaturityOptions {
        concept,
        json,
        update,
        fresh_fixture: false,
    })
}

fn parse_map_existing_options(args: &[String]) -> Result<MapExistingOptions> {
    let mut json = false;
    let mut expand_deps = true;
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => bail!("{MAP_EXISTING_USAGE}"),
            "--json" => json = true,
            "--no-expand-deps" => expand_deps = false,
            other => bail!("unknown argument: {other}\n\n{MAP_EXISTING_USAGE}"),
        }
    }
    Ok(MapExistingOptions { json, expand_deps })
}

pub(crate) fn parse_grind_options(args: &[String]) -> Result<ConceptGrindOptions> {
    let mut agent = AgentProvider::Codex;
    let mut max_iterations = 1u32;
    let mut concept = None::<String>;
    let mut target_rule = None::<String>;
    let mut query = None::<String>;
    let mut repair_attempts = 1u8;
    let mut dry_run = false;
    let mut allow_dirty = false;
    let mut no_commit = false;

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => bail!("{GRIND_USAGE}"),
            "--agent" => {
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow!("--agent requires a value"))?;
                agent = parse_agent_provider(value)?;
            }
            s if s.starts_with("--agent=") => {
                agent = parse_agent_provider(&s["--agent=".len()..])?;
            }
            "--max-iterations" => {
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow!("--max-iterations requires a value"))?;
                max_iterations = value
                    .parse()
                    .with_context(|| format!("--max-iterations value: {value:?}"))?;
            }
            s if s.starts_with("--max-iterations=") => {
                max_iterations = s["--max-iterations=".len()..]
                    .parse()
                    .with_context(|| format!("--max-iterations value: {s:?}"))?;
            }
            "--concept" => {
                concept = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--concept requires a value"))?
                        .to_string(),
                );
            }
            s if s.starts_with("--concept=") => {
                concept = Some(s["--concept=".len()..].to_string());
            }
            "--target-rule" => {
                target_rule = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--target-rule requires a value"))?
                        .to_string(),
                );
            }
            s if s.starts_with("--target-rule=") => {
                target_rule = Some(s["--target-rule=".len()..].to_string());
            }
            "--query" => {
                query = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--query requires a value"))?
                        .to_string(),
                );
            }
            s if s.starts_with("--query=") => {
                query = Some(s["--query=".len()..].to_string());
            }
            "--repair-attempts" => {
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow!("--repair-attempts requires a value"))?;
                repair_attempts = value
                    .parse()
                    .with_context(|| format!("--repair-attempts value: {value:?}"))?;
            }
            s if s.starts_with("--repair-attempts=") => {
                repair_attempts = s["--repair-attempts=".len()..]
                    .parse()
                    .with_context(|| format!("--repair-attempts value: {s:?}"))?;
            }
            "--dry-run" => dry_run = true,
            "--allow-dirty" => allow_dirty = true,
            "--no-commit" => no_commit = true,
            "--ui" => {
                let _ = iter.next();
            }
            s if s.starts_with("--ui=") => {}
            other => bail!("unknown argument: {other}\n\n{GRIND_USAGE}"),
        }
    }

    if max_iterations == 0 {
        bail!("--max-iterations must be greater than zero");
    }
    if concept.as_deref().is_some_and(str::is_empty) {
        bail!("--concept must not be empty");
    }
    if query.as_deref().is_some_and(str::is_empty) {
        bail!("--query must not be empty");
    }
    if target_rule.as_deref().is_some_and(str::is_empty) {
        bail!("--target-rule must not be empty");
    }

    Ok(ConceptGrindOptions {
        agent,
        max_iterations,
        concept,
        target_rule,
        query,
        repair_attempts,
        dry_run,
        allow_dirty,
        no_commit,
    })
}

fn parse_grind_loop_options(args: &[String]) -> Result<ConceptGrindLoopOptions> {
    let mut agent = AgentProvider::Codex;
    let mut batch_size = 5u32;
    let mut max_batches = None::<u32>;
    let mut dry_run = false;
    let mut resume = None::<PathBuf>;

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => bail!("{GRIND_LOOP_USAGE}"),
            "--agent" => {
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow!("--agent requires a value"))?;
                agent = parse_agent_provider(value)?;
            }
            s if s.starts_with("--agent=") => {
                agent = parse_agent_provider(&s["--agent=".len()..])?;
            }
            "--batch-size" => {
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow!("--batch-size requires a value"))?;
                batch_size = value
                    .parse()
                    .with_context(|| format!("--batch-size value: {value:?}"))?;
            }
            s if s.starts_with("--batch-size=") => {
                batch_size = s["--batch-size=".len()..]
                    .parse()
                    .with_context(|| format!("--batch-size value: {s:?}"))?;
            }
            "--max-batches" => {
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow!("--max-batches requires a value"))?;
                max_batches = Some(
                    value
                        .parse()
                        .with_context(|| format!("--max-batches value: {value:?}"))?,
                );
            }
            s if s.starts_with("--max-batches=") => {
                max_batches = Some(
                    s["--max-batches=".len()..]
                        .parse()
                        .with_context(|| format!("--max-batches value: {s:?}"))?,
                );
            }
            "--resume" => {
                resume = Some(PathBuf::from(
                    iter.next()
                        .ok_or_else(|| anyhow!("--resume requires a value"))?,
                ));
            }
            s if s.starts_with("--resume=") => {
                resume = Some(PathBuf::from(&s["--resume=".len()..]));
            }
            "--dry-run" => dry_run = true,
            other => bail!("unknown argument: {other}\n\n{GRIND_LOOP_USAGE}"),
        }
    }

    if batch_size == 0 {
        bail!("--batch-size must be greater than zero");
    }
    if max_batches == Some(0) {
        bail!("--max-batches must be greater than zero");
    }

    Ok(ConceptGrindLoopOptions {
        agent,
        batch_size,
        max_batches,
        dry_run,
        resume,
    })
}

fn parse_agent_provider(value: &str) -> Result<AgentProvider> {
    match value {
        "codex" => Ok(AgentProvider::Codex),
        "claude" => Ok(AgentProvider::Claude),
        other => bail!("--agent must be 'codex' or 'claude', got {other:?}"),
    }
}

fn run_grind_loop(options: ConceptGrindLoopOptions) -> Result<()> {
    let (loop_dir, mut state) = if let Some(resume_dir) = options.resume.as_ref() {
        let state = read_grind_loop_state(resume_dir)?;
        if !state.options.dry_run {
            ensure_clean_working_tree()
                .context("concept-grind-loop --resume requires a clean working tree")?;
        }
        println!("concept-grind-loop log: {} (resumed)", resume_dir.display());
        (resume_dir.clone(), state)
    } else {
        if !options.dry_run {
            ensure_clean_working_tree()
                .context("concept-grind-loop requires a clean working tree")?;
        }
        let loop_dir = grammar_concept_log_root().join(format!("loop-{}", unix_timestamp()));
        fs::create_dir_all(&loop_dir).with_context(|| format!("create {}", loop_dir.display()))?;
        println!("concept-grind-loop log: {}", loop_dir.display());
        let state = ConceptGrindLoopState {
            options,
            next_batch: 1,
            active_experiment: None,
        };
        write_grind_loop_state(&loop_dir, &state)?;
        (loop_dir, state)
    };
    let options = state.options.clone();

    if options.dry_run {
        write_json(loop_dir.join("options.json"), &options)?;
        return Ok(());
    }

    let mut sink = ConsoleSink::new();
    let mut active_experiment = state.active_experiment.take();

    loop {
        let batch = state.next_batch;
        if options.max_batches.is_some_and(|max| batch > max) {
            break;
        }
        state.next_batch = batch;
        state.active_experiment = active_experiment.clone();
        write_grind_loop_state(&loop_dir, &state)?;

        let batch_dir = loop_dir.join(format!("batch-{batch:03}"));
        fs::create_dir_all(&batch_dir)
            .with_context(|| format!("create {}", batch_dir.display()))?;

        let batch_start = SystemTime::now();
        run_grind_loop_batch(&options, &batch_dir)?;
        let grind_dir = newest_grind_run_since(batch_start)?
            .ok_or_else(|| anyhow!("concept-grind batch did not create a grind-* run directory"))?;
        write_text(
            batch_dir.join("grind_run_path.txt"),
            &format!("{}\n", grind_dir.display()),
        )?;
        copy_if_exists(
            grind_dir.join("metrics.json"),
            batch_dir.join("metrics.json"),
        )?;

        let review_prompt = build_grind_loop_review_prompt(
            batch,
            &batch_dir,
            &grind_dir,
            active_experiment.as_ref(),
        )?;
        fs::write(batch_dir.join("review_prompt.md"), &review_prompt)
            .with_context(|| format!("write {}", batch_dir.join("review_prompt.md").display()))?;
        let review_outcome = refactor_hotspot::invoke_agent(
            options.agent,
            &review_prompt,
            &batch_dir.join("review_transcript.ndjson"),
            &mut sink,
        )?;
        fs::write(
            batch_dir.join("review_response.md"),
            &review_outcome.assistant_text,
        )
        .with_context(|| format!("write {}", batch_dir.join("review_response.md").display()))?;
        if !review_outcome.success {
            bail!(
                "{} review agent exited with status {}",
                options.agent.label(),
                review_outcome.exit_code
            );
        }
        let review = parse_grind_loop_review(&review_outcome.assistant_text)?;
        write_json(batch_dir.join("review.json"), &review)?;

        let mut should_reexec = false;
        if let Some(previous) = active_experiment.take() {
            match review.previous_decision {
                ExperimentDecision::Pass | ExperimentDecision::None => {
                    active_experiment = Some(previous);
                }
                ExperimentDecision::Fail => {
                    if let Some(commit) = &previous.commit {
                        git_revert_commit(commit)?;
                        run_loop_validation(&batch_dir)?;
                        should_reexec = options.max_batches.is_none_or(|max| batch < max);
                    }
                }
            }
        }

        if let Some(mut next) = review.next_experiment {
            let experiment_dir = batch_dir.join("next_experiment");
            fs::create_dir_all(&experiment_dir)
                .with_context(|| format!("create {}", experiment_dir.display()))?;
            write_json(experiment_dir.join("experiment.json"), &next)?;
            apply_grind_loop_experiment(options.agent, &next, &experiment_dir, &mut sink)?;
            run_loop_validation(&experiment_dir)?;
            let patch = git_diff()?;
            fs::write(experiment_dir.join("patch.diff"), &patch).with_context(|| {
                format!("write {}", experiment_dir.join("patch.diff").display())
            })?;
            if patch.trim().is_empty() {
                next.commit = None;
            } else {
                let commit = commit_current_experiment(&next)?;
                next.commit = Some(commit);
            }
            write_json(experiment_dir.join("experiment_applied.json"), &next)?;
            should_reexec = should_reexec
                || next.commit.is_some() && options.max_batches.is_none_or(|max| batch < max);
            active_experiment = Some(next);
        }

        state.next_batch = batch + 1;
        state.active_experiment = active_experiment.clone();
        write_grind_loop_state(&loop_dir, &state)?;
        if should_reexec {
            reexec_grind_loop(&loop_dir)?;
        }
    }

    Ok(())
}

fn run_phase_loop(options: ConceptPhaseLoopOptions) -> Result<()> {
    if !options.dry_run {
        ensure_clean_working_tree().context("concept-phase-loop requires a clean working tree")?;
    }
    let loop_dir = grammar_concept_log_root().join(format!("phase-loop-{}", unix_timestamp()));
    fs::create_dir_all(&loop_dir).with_context(|| format!("create {}", loop_dir.display()))?;
    write_json(loop_dir.join("options.json"), &options)?;
    println!("concept-phase-loop log: {}", loop_dir.display());

    if options.dry_run {
        println!("dry-run: would run Phase 2 batches, then concept-grind-loop on stop");
        return Ok(());
    }

    let mut phase2_commits = 0u32;
    let mut concept_commit_counts: BTreeMap<String, u32> = BTreeMap::new();
    let mut stop_reasons = Vec::new();

    let mut batch = 1u32;
    loop {
        if options.phase2_max_batches.is_some_and(|max| batch > max) {
            stop_reasons.push(format!(
                "Phase 2 max batches reached: {}",
                options.phase2_max_batches.expect("checked some")
            ));
            break;
        }
        let batch_dir = loop_dir.join(format!("phase2-batch-{batch:03}"));
        fs::create_dir_all(&batch_dir)
            .with_context(|| format!("create {}", batch_dir.display()))?;

        let before = run_phase2_map_fresh()?;
        write_json(batch_dir.join("phase2_map_before.json"), &before)?;
        let before_head = current_head()?;
        let before_statement_variants = statement_variants_at_ref("HEAD")?;

        run_phase_loop_phase2_batch(&options, &batch_dir)?;

        let after_head = current_head()?;
        let after = run_phase2_map_fresh()?;
        write_json(batch_dir.join("phase2_map_after.json"), &after)?;
        let after_statement_variants = statement_variants_at_ref("HEAD")?;

        let commits = phase_loop_commits_between(&before_head, &after_head)?;
        let mut batch_stop_reasons = Vec::new();
        if commits.is_empty() {
            batch_stop_reasons.push("Phase 2 batch created no commits".to_string());
        }
        if after.ast_green_concepts <= before.ast_green_concepts {
            batch_stop_reasons.push(format!(
                "Phase 2 AST-green did not improve: {} -> {}",
                before.ast_green_concepts, after.ast_green_concepts
            ));
        }
        if after.ast_failed_concepts > 0 && after.ast_failed_concepts >= after.parse_failed_concepts
        {
            batch_stop_reasons.push(format!(
                "AST failures are now the majority of remaining non-green concepts: ast_failed={} parse_failed={}",
                after.ast_failed_concepts, after.parse_failed_concepts
            ));
        }

        let mut commit_summaries = Vec::new();
        for commit in commits {
            phase2_commits += 1;
            if let Some(concept) = &commit.concept {
                let count = concept_commit_counts.entry(concept.clone()).or_default();
                *count += 1;
                if *count >= options.repeat_stop_after {
                    batch_stop_reasons.push(format!(
                        "concept {concept:?} was committed {count} time(s) in this Phase 2 cycle"
                    ));
                }
            }
            commit_summaries.push(commit);
        }

        let added_statement_variants: Vec<String> = after_statement_variants
            .difference(&before_statement_variants)
            .cloned()
            .collect();
        if !added_statement_variants.is_empty() {
            batch_stop_reasons.push(format!(
                "Phase 2 added top-level Statement variant(s): {}",
                added_statement_variants.join(", ")
            ));
            if let Some(last) = commit_summaries.last_mut() {
                last.added_statement_variants = added_statement_variants;
            }
        }

        if options
            .phase2_max_commits
            .is_some_and(|max| phase2_commits >= max)
        {
            batch_stop_reasons.push(format!(
                "Phase 2 commit budget reached: {phase2_commits}/{}",
                options.phase2_max_commits.expect("checked some")
            ));
        }
        if after.ast_green_concepts == after.grammar_green_concepts {
            batch_stop_reasons.push("all grammar-green concepts are Phase 2 AST-green".to_string());
        }

        let summary = PhaseLoopBatchSummary {
            batch,
            before: PhaseLoopMapSummary::from_report(&before),
            after: PhaseLoopMapSummary::from_report(&after),
            commits: commit_summaries,
            stop_reasons: batch_stop_reasons.clone(),
        };
        write_json(batch_dir.join("summary.json"), &summary)?;
        println!(
            "phase2 batch {batch}: ast_green {} -> {}, parse_green {} -> {}, commits {}",
            summary.before.ast_green_concepts,
            summary.after.ast_green_concepts,
            summary.before.parse_green_concepts,
            summary.after.parse_green_concepts,
            summary.commits.len()
        );

        if !batch_stop_reasons.is_empty() {
            stop_reasons = batch_stop_reasons;
            break;
        }
        batch += 1;
    }
    write_json(loop_dir.join("phase2_stop_reasons.json"), &stop_reasons)?;
    println!("phase2 stop:");
    for reason in &stop_reasons {
        println!("  - {reason}");
    }

    run_phase_loop_phase1(&options, &loop_dir)?;
    Ok(())
}

fn run_phase_status(options: PhaseStatusOptions) -> Result<PhaseStatusReport> {
    let current = PhaseLoopMapSummary::from_report(&run_phase2_map_fresh()?);
    let latest = latest_phase_loop_batch_summary()?;
    let running_processes = phase_loop_running_processes();
    let latest_batch = latest
        .as_ref()
        .map(|(_, _, summary)| PhaseStatusBatchReport::from_summary(summary));

    let mut reasons = Vec::new();
    if running_processes.is_empty() {
        reasons.push("no phase loop/grind process is running".to_string());
    }
    if current.ast_failed_concepts > 0
        && current.ast_failed_concepts >= current.parse_failed_concepts
    {
        reasons.push(format!(
            "AST failures dominate remaining non-green concepts: ast_failed={} parse_failed={}",
            current.ast_failed_concepts, current.parse_failed_concepts
        ));
    }
    if let Some(batch) = &latest_batch {
        if batch.ast_green_delta <= 0 {
            reasons.push(format!(
                "latest batch did not increase AST-green concepts: delta={}",
                batch.ast_green_delta
            ));
        }
        if !batch.added_statement_variants.is_empty() {
            reasons.push(format!(
                "latest batch added top-level Statement variant(s): {}",
                batch.added_statement_variants.join(", ")
            ));
        }
        reasons.extend(batch.stop_reasons.iter().cloned());
    }

    let verdict = if running_processes.is_empty() {
        PhaseStatusVerdict::Stopped
    } else if reasons.iter().any(|reason| {
        !reason.starts_with("no phase loop/grind process")
            && !reason.starts_with("latest batch did not increase")
    }) || latest_batch
        .as_ref()
        .is_some_and(|batch| batch.ast_green_delta <= 0 && batch.commits > 0)
    {
        PhaseStatusVerdict::Inspect
    } else {
        PhaseStatusVerdict::Continue
    };

    Ok(PhaseStatusReport {
        verdict,
        reasons,
        running_processes,
        current,
        latest_phase_loop_dir: latest.as_ref().map(|(dir, _, _)| dir.clone()),
        latest_batch_summary_path: latest.as_ref().map(|(_, path, _)| path.clone()),
        latest_batch,
        json: options.json,
    })
}

impl PhaseLoopMapSummary {
    fn from_report(report: &Phase2MapReport) -> Self {
        Self {
            parse_green_concepts: report.parse_green_concepts,
            ast_green_concepts: report.ast_green_concepts,
            parse_failed_concepts: report.parse_failed_concepts,
            ast_failed_concepts: report.ast_failed_concepts,
            grammar_green_concepts: report.grammar_green_concepts,
        }
    }
}

impl PhaseStatusBatchReport {
    fn from_summary(summary: &PhaseLoopBatchSummary) -> Self {
        let mut concepts = Vec::new();
        let mut added_statement_variants = Vec::new();
        for commit in &summary.commits {
            if let Some(concept) = &commit.concept {
                concepts.push(concept.clone());
            }
            added_statement_variants.extend(commit.added_statement_variants.iter().cloned());
        }
        concepts.sort();
        concepts.dedup();
        added_statement_variants.sort();
        added_statement_variants.dedup();
        Self {
            batch: summary.batch,
            ast_green_delta: summary.after.ast_green_concepts as i64
                - summary.before.ast_green_concepts as i64,
            parse_green_delta: summary.after.parse_green_concepts as i64
                - summary.before.parse_green_concepts as i64,
            commits: summary.commits.len(),
            concepts,
            added_statement_variants,
            stop_reasons: summary.stop_reasons.clone(),
        }
    }
}

fn grind_loop_state_path(loop_dir: &Path) -> PathBuf {
    loop_dir.join("loop_state.json")
}

fn read_grind_loop_state(loop_dir: &Path) -> Result<ConceptGrindLoopState> {
    let path = grind_loop_state_path(loop_dir);
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

fn write_grind_loop_state(loop_dir: &Path, state: &ConceptGrindLoopState) -> Result<()> {
    write_json(grind_loop_state_path(loop_dir), state)
}

fn reexec_grind_loop(loop_dir: &Path) -> Result<()> {
    println!(
        "concept-grind-loop re-exec: cargo xtask concept-grind-loop --resume {}",
        loop_dir.display()
    );
    #[cfg(unix)]
    {
        let error = Command::new("cargo")
            .args(["xtask", "concept-grind-loop", "--resume"])
            .arg(loop_dir)
            .current_dir(repo_root())
            .exec();
        Err(error).context("exec cargo xtask concept-grind-loop --resume")
    }
    #[cfg(not(unix))]
    {
        let status = Command::new("cargo")
            .args(["xtask", "concept-grind-loop", "--resume"])
            .arg(loop_dir)
            .current_dir(repo_root())
            .status()
            .context("spawn cargo xtask concept-grind-loop --resume")?;
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn run_grind_loop_batch(options: &ConceptGrindLoopOptions, batch_dir: &Path) -> Result<()> {
    let output = Command::new("cargo")
        .args([
            "xtask",
            "concept-grind",
            "--agent",
            options.agent.label(),
            "--max-iterations",
            &options.batch_size.to_string(),
        ])
        .current_dir(repo_root())
        .output()
        .context("cargo xtask concept-grind batch")?;
    let text = command_output_text(&output);
    fs::write(batch_dir.join("grind_output.txt"), &text)
        .with_context(|| format!("write {}", batch_dir.join("grind_output.txt").display()))?;
    if !output.status.success() {
        bail!("concept-grind batch failed\n{text}");
    }
    Ok(())
}

fn run_phase_loop_phase2_batch(options: &ConceptPhaseLoopOptions, batch_dir: &Path) -> Result<()> {
    let output = Command::new("cargo")
        .args([
            "xtask",
            "concept-phase2-grind",
            "--agent",
            options.agent.label(),
            "--max-iterations",
            &options.phase2_batch_size.to_string(),
        ])
        .current_dir(repo_root())
        .output()
        .context("cargo xtask concept-phase2-grind batch")?;
    let text = command_output_text(&output);
    fs::write(batch_dir.join("phase2_output.txt"), &text)
        .with_context(|| format!("write {}", batch_dir.join("phase2_output.txt").display()))?;
    if !output.status.success() {
        bail!("concept-phase2-grind batch failed\n{text}");
    }
    Ok(())
}

fn run_phase_loop_phase1(options: &ConceptPhaseLoopOptions, loop_dir: &Path) -> Result<()> {
    let phase1_dir = loop_dir.join("phase1");
    fs::create_dir_all(&phase1_dir).with_context(|| format!("create {}", phase1_dir.display()))?;
    let mut args = vec![
        "xtask".to_string(),
        "concept-grind-loop".to_string(),
        "--agent".to_string(),
        options.agent.label().to_string(),
        "--batch-size".to_string(),
        options.phase1_batch_size.to_string(),
    ];
    if let Some(max_batches) = options.phase1_max_batches {
        args.push("--max-batches".to_string());
        args.push(max_batches.to_string());
    }
    let output = Command::new("cargo")
        .args(&args)
        .current_dir(repo_root())
        .output()
        .context("cargo xtask concept-grind-loop")?;
    let text = command_output_text(&output);
    fs::write(phase1_dir.join("concept_grind_loop_output.txt"), &text).with_context(|| {
        format!(
            "write {}",
            phase1_dir.join("concept_grind_loop_output.txt").display()
        )
    })?;
    if !output.status.success() {
        bail!("concept-grind-loop failed\n{text}");
    }
    Ok(())
}

fn latest_phase_loop_batch_summary() -> Result<Option<(PathBuf, PathBuf, PhaseLoopBatchSummary)>> {
    let Some(loop_dir) = latest_phase_loop_dir()? else {
        return Ok(None);
    };
    let mut newest: Option<(u32, PathBuf)> = None;
    for entry in fs::read_dir(&loop_dir).with_context(|| format!("read {}", loop_dir.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(batch) = name
            .strip_prefix("phase2-batch-")
            .and_then(|suffix| suffix.parse::<u32>().ok())
        else {
            continue;
        };
        let summary_path = entry.path().join("summary.json");
        if !summary_path.exists() {
            continue;
        }
        if newest
            .as_ref()
            .is_none_or(|(current_batch, _)| batch > *current_batch)
        {
            newest = Some((batch, summary_path));
        }
    }
    let Some((_, summary_path)) = newest else {
        return Ok(None);
    };
    let summary: PhaseLoopBatchSummary = serde_json::from_str(
        &fs::read_to_string(&summary_path)
            .with_context(|| format!("read {}", summary_path.display()))?,
    )
    .with_context(|| format!("parse {}", summary_path.display()))?;
    Ok(Some((loop_dir, summary_path, summary)))
}

fn latest_phase_loop_dir() -> Result<Option<PathBuf>> {
    let root = grammar_concept_log_root();
    if !root.exists() {
        return Ok(None);
    }
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(&root).with_context(|| format!("read {}", root.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("phase-loop-") {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(UNIX_EPOCH);
        if newest
            .as_ref()
            .is_none_or(|(current, _)| modified > *current)
        {
            newest = Some((modified, entry.path()));
        }
    }
    Ok(newest.map(|(_, path)| path))
}

fn phase_loop_running_processes() -> Vec<String> {
    let Ok(output) = Command::new("ps")
        .args(["-axo", "pid,etime,command"])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| {
            (line.contains("concept-phase-loop")
                || line.contains("concept-phase2-grind")
                || line.contains("concept-grind-loop")
                || line.contains("codex exec"))
                && !line.contains("concept-phase-status")
        })
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn phase_loop_commits_between(
    before_head: &str,
    after_head: &str,
) -> Result<Vec<PhaseLoopCommitSummary>> {
    if before_head == after_head {
        return Ok(Vec::new());
    }
    let output = Command::new("git")
        .args([
            "log",
            "--reverse",
            "--format=%H%x00%s",
            &format!("{before_head}..{after_head}"),
        ])
        .current_dir(repo_root())
        .output()
        .context("git log phase loop commits")?;
    if !output.status.success() {
        bail!("git log failed\n{}", command_output_text(&output));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut commits = Vec::new();
    for line in text.lines() {
        let Some((sha, subject)) = line.split_once('\0') else {
            continue;
        };
        let concept = subject
            .strip_prefix("Advance Phase 2 concept ")
            .map(str::to_string);
        commits.push(PhaseLoopCommitSummary {
            sha: sha.to_string(),
            concept,
            subject: subject.to_string(),
            added_statement_variants: Vec::new(),
        });
    }
    Ok(commits)
}

fn statement_variants_at_ref(rev: &str) -> Result<BTreeSet<String>> {
    let output = Command::new("git")
        .args(["show", &format!("{rev}:crates/mtg-grammar/src/ast.rs")])
        .current_dir(repo_root())
        .output()
        .with_context(|| format!("git show {rev}:crates/mtg-grammar/src/ast.rs"))?;
    if !output.status.success() {
        bail!(
            "git show {rev}:ast.rs failed\n{}",
            command_output_text(&output)
        );
    }
    Ok(extract_statement_variants(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

fn extract_statement_variants(text: &str) -> BTreeSet<String> {
    let mut variants = BTreeSet::new();
    let mut in_statement = false;
    let mut depth = 0i32;
    for line in text.lines() {
        let trimmed = line.trim();
        if !in_statement {
            if trimmed == "pub enum Statement {" {
                in_statement = true;
                depth = 1;
            }
            continue;
        }
        if depth == 1
            && trimmed
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_uppercase())
        {
            if let Some(name) = trimmed
                .split(|ch: char| ch == '{' || ch == '(' || ch == ',' || ch.is_whitespace())
                .next()
                .filter(|name| !name.is_empty())
            {
                variants.insert(name.to_string());
            }
        }
        depth += trimmed.matches('{').count() as i32;
        depth -= trimmed.matches('}').count() as i32;
        if depth <= 0 {
            break;
        }
    }
    variants
}

fn newest_grind_run_since(since: SystemTime) -> Result<Option<PathBuf>> {
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for entry in
        fs::read_dir(grammar_concept_log_root()).context("read grammar concept log root")?
    {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("grind-") {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(UNIX_EPOCH);
        if modified < since {
            continue;
        }
        if newest
            .as_ref()
            .is_none_or(|(current, _)| modified > *current)
        {
            newest = Some((modified, entry.path()));
        }
    }
    Ok(newest.map(|(_, path)| path))
}

fn build_grind_loop_review_prompt(
    batch: u32,
    batch_dir: &Path,
    grind_dir: &Path,
    previous: Option<&GrindLoopExperiment>,
) -> Result<String> {
    let metrics = read_optional(grind_dir.join("metrics.json"))?;
    let output = read_optional(batch_dir.join("grind_output.txt"))?;
    let summaries = collect_iteration_summaries(grind_dir)?;
    let loop_dir = batch_dir
        .parent()
        .ok_or_else(|| anyhow!("batch dir {} has no parent", batch_dir.display()))?;
    let experiment_history = collect_grind_loop_experiment_history(loop_dir, batch)?;
    let experiment_history_json = serde_json::to_string_pretty(&experiment_history)?;
    let previous_json = match previous {
        Some(experiment) => serde_json::to_string_pretty(experiment)?,
        None => "null".to_string(),
    };

    Ok(format!(
        r#"You are reviewing an autonomous concept-grind optimization loop.

Goal: increase completed grammar-green concept throughput per hour without reducing concept quality or weakening downstream readiness.

Review batch {batch}. Decide whether the previous experiment passed or failed based on metrics and quality artifacts, then propose exactly one next experiment or null.

Rules:
- Judge by outcomes, not by a fixed allowlist of change types.
- If quality regressed, mark the previous experiment as fail.
- Do not propose prompt/context reduction unless the quality checks can prove no degradation.
- Do not propose skipping quality gates unless the resulting quality contract is demonstrably equivalent.
- The next experiment must include a measurable hypothesis and concrete implementation request.
- Review the prior experiment history before proposing the next experiment.
- Do not propose an experiment that is materially the same as a failed prior experiment unless the implementation_request explains the new evidence and the specific difference from that failed experiment.

Previous experiment:
```json
{previous_json}
```

Prior experiment history for this loop:
```json
{experiment_history_json}
```

Batch metrics:
```json
{metrics}
```

Iteration summaries:
```json
{summaries}
```

Batch command output:
```text
{output}
```

Return only JSON with this shape:
{{
  "previous_decision": "pass" | "fail" | "none",
  "previous_reason": "short reason",
  "next_experiment": {{
    "id": "exp-short-snake-case",
    "hypothesis": "measurable claim",
    "implementation_request": "specific code change to try",
    "success_metric": "metric that should improve after the next batch",
    "quality_checks": ["checks that must remain green"],
    "commit": null
  }}
}}

Use "next_experiment": null only if no responsible experiment is supported by this batch.
"#,
    ))
}

fn collect_grind_loop_experiment_history(
    loop_dir: &Path,
    current_batch: u32,
) -> Result<Vec<GrindLoopExperimentHistoryEntry>> {
    let mut history = Vec::new();
    for batch in 1..current_batch.saturating_sub(1) {
        let batch_dir = loop_dir.join(format!("batch-{batch:03}"));
        let decision_review_path = loop_dir
            .join(format!("batch-{:03}", batch + 1))
            .join("review.json");
        let experiment_path = batch_dir
            .join("next_experiment")
            .join("experiment_applied.json");
        let experiment_fallback_path = batch_dir.join("next_experiment").join("experiment.json");

        let review = if decision_review_path.exists() {
            let text = fs::read_to_string(&decision_review_path)
                .with_context(|| format!("read {}", decision_review_path.display()))?;
            Some(
                serde_json::from_str::<GrindLoopReview>(&text)
                    .with_context(|| format!("parse {}", decision_review_path.display()))?,
            )
        } else {
            None
        };
        let experiment = if experiment_path.exists() {
            read_grind_loop_experiment(&experiment_path)?
        } else if experiment_fallback_path.exists() {
            read_grind_loop_experiment(&experiment_fallback_path)?
        } else {
            None
        };

        history.push(GrindLoopExperimentHistoryEntry {
            batch,
            experiment,
            decision: review
                .as_ref()
                .map(|review| review.previous_decision)
                .unwrap_or(ExperimentDecision::None),
            reason: review
                .as_ref()
                .map(|review| review.previous_reason.clone())
                .unwrap_or_else(|| "not yet reviewed".to_string()),
        });
    }
    Ok(history)
}

fn read_grind_loop_experiment(path: &Path) -> Result<Option<GrindLoopExperiment>> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let experiment = serde_json::from_str::<GrindLoopExperiment>(&text)
        .with_context(|| format!("parse {}", path.display()))?;
    Ok(Some(experiment))
}

fn apply_grind_loop_experiment(
    agent: AgentProvider,
    experiment: &GrindLoopExperiment,
    experiment_dir: &Path,
    sink: &mut dyn FlowSink,
) -> Result<()> {
    let prompt = format!(
        r#"You are implementing one concept-grind workflow optimization experiment.

Experiment:
```json
{}
```

Constraints:
- Implement only this experiment.
- Preserve concept output quality; do not weaken gates, fixtures, maturity checks, or generated-test protections unless the experiment explicitly proves an equivalent quality contract.
- Do not run `cargo xtask add-card`.
- Keep the patch reviewable and scoped to workflow/tooling code.
- Do not commit; the wrapper will validate and commit.

Return:
CONCEPT_GRIND_LOOP_EXPERIMENT:
ID: {}
CHANGE: <summary>
QUALITY_RISK: <summary>
"#,
        serde_json::to_string_pretty(experiment)?,
        experiment.id
    );
    fs::write(experiment_dir.join("implement_prompt.md"), &prompt).with_context(|| {
        format!(
            "write {}",
            experiment_dir.join("implement_prompt.md").display()
        )
    })?;
    let outcome = refactor_hotspot::invoke_agent(
        agent,
        &prompt,
        &experiment_dir.join("implement_transcript.ndjson"),
        sink,
    )?;
    fs::write(
        experiment_dir.join("implement_response.md"),
        &outcome.assistant_text,
    )
    .with_context(|| {
        format!(
            "write {}",
            experiment_dir.join("implement_response.md").display()
        )
    })?;
    if !outcome.success {
        bail!(
            "{} experiment implementation agent exited with status {}",
            agent.label(),
            outcome.exit_code
        );
    }
    Ok(())
}

fn parse_grind_loop_review(response: &str) -> Result<GrindLoopReview> {
    let json = extract_json_object(response)
        .ok_or_else(|| anyhow!("review response did not contain a JSON object"))?;
    serde_json::from_str(json).context("parse grind-loop review JSON")
}

fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in text[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = start + offset + ch.len_utf8();
                    return Some(&text[start..end]);
                }
            }
            _ => {}
        }
    }
    None
}

fn collect_iteration_summaries(grind_dir: &Path) -> Result<String> {
    let mut summaries = Vec::<serde_json::Value>::new();
    for entry in fs::read_dir(grind_dir).with_context(|| format!("read {}", grind_dir.display()))? {
        let entry = entry?;
        let path = entry.path().join("summary.json");
        if !path.exists() {
            continue;
        }
        let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            summaries.push(value);
        }
    }
    serde_json::to_string_pretty(&summaries).context("serialize iteration summaries")
}

fn run_loop_validation(dir: &Path) -> Result<()> {
    run_logged_command("cargo_check_xtask", "cargo", &["check", "-p", "xtask"], dir)?;
    run_logged_command(
        "cargo_test_xtask_concept_grind",
        "cargo",
        &["test", "-p", "xtask", "concept_grind", "--", "--nocapture"],
        dir,
    )?;
    Ok(())
}

fn run_logged_command(label: &str, program: &str, args: &[&str], dir: &Path) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(repo_root())
        .output()
        .with_context(|| format!("run {label}"))?;
    let text = command_output_text(&output);
    fs::write(dir.join(format!("{label}.txt")), &text)
        .with_context(|| format!("write {}", dir.join(format!("{label}.txt")).display()))?;
    if !output.status.success() {
        bail!("{label} failed\n{text}");
    }
    Ok(text)
}

fn git_diff() -> Result<String> {
    let output = Command::new("git")
        .args(["diff", "--binary"])
        .current_dir(repo_root())
        .output()
        .context("git diff --binary")?;
    if !output.status.success() {
        bail!("git diff failed\n{}", command_output_text(&output));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn commit_current_experiment(experiment: &GrindLoopExperiment) -> Result<String> {
    let add = Command::new("git")
        .args(["add", "."])
        .current_dir(repo_root())
        .output()
        .context("git add experiment")?;
    if !add.status.success() {
        bail!("git add experiment failed\n{}", command_output_text(&add));
    }
    let message = format!(
        "Experiment concept-grind loop {}\n\nHypothesis: {}\nSuccess metric: {}\n",
        experiment.id, experiment.hypothesis, experiment.success_metric
    );
    let commit = Command::new("git")
        .args(["commit", "--no-verify", "-m", &message])
        .current_dir(repo_root())
        .output()
        .context("git commit experiment")?;
    if !commit.status.success() {
        bail!(
            "git commit experiment failed\n{}",
            command_output_text(&commit)
        );
    }
    current_head()
}

fn current_head() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_root())
        .output()
        .context("git rev-parse HEAD")?;
    if !output.status.success() {
        bail!(
            "git rev-parse HEAD failed\n{}",
            command_output_text(&output)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_revert_commit(commit: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["revert", "--no-edit", commit])
        .current_dir(repo_root())
        .output()
        .with_context(|| format!("git revert {commit}"))?;
    if !output.status.success() {
        bail!(
            "git revert {commit} failed\n{}",
            command_output_text(&output)
        );
    }
    Ok(())
}

fn read_optional(path: PathBuf) -> Result<String> {
    match fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok("(missing)\n".to_string()),
        Err(e) => Err(e).with_context(|| format!("read {}", path.display())),
    }
}

fn copy_if_exists(from: PathBuf, to: PathBuf) -> Result<()> {
    if from.exists() {
        fs::copy(&from, &to)
            .with_context(|| format!("copy {} to {}", from.display(), to.display()))?;
    }
    Ok(())
}

fn write_text(path: PathBuf, text: &str) -> Result<()> {
    fs::write(&path, text).with_context(|| format!("write {}", path.display()))
}

fn run_map_existing(options: MapExistingOptions) -> Result<ExistingGrammarMapReport> {
    let rules = grammar_query_engine::parse_grammar_file(grammar_pest_path())?;
    let rule_by_name: BTreeMap<String, &grammar_query_engine::GrammarRuleDefinition> =
        rules.iter().map(|rule| (rule.name.clone(), rule)).collect();
    let concept_files = read_concept_files()?;
    let mut concepts = Vec::new();
    let mut mapped_rules = BTreeSet::new();

    for (path, doc, maturity) in &concept_files {
        let mut found_rules = Vec::new();
        let mut missing_rules = Vec::new();
        for rule_name in &doc.concept.pest_rules {
            if let Some(rule) = rule_by_name.get(rule_name) {
                mapped_rules.insert(rule.name.clone());
                found_rules.push(RuleLocationSummary {
                    name: rule.name.clone(),
                    line: rule.line,
                });
            } else {
                missing_rules.push(rule_name.clone());
            }
        }
        let (owned_rule_names, expanded_missing_rules) =
            concept_owned_rule_names(&rules, &doc.concept.pest_rules, options.expand_deps);
        for missing in expanded_missing_rules {
            if !missing_rules.contains(&missing) {
                missing_rules.push(missing);
            }
        }
        let owned_rules: Vec<RuleLocationSummary> = owned_rule_names
            .iter()
            .filter_map(|rule_name| rule_by_name.get(rule_name))
            .map(|rule| RuleLocationSummary {
                name: rule.name.clone(),
                line: rule.line,
            })
            .collect();
        mapped_rules.extend(owned_rule_names);
        concepts.push(ConceptRuleMap {
            concept: doc.concept.name.clone(),
            maturity: maturity.clone(),
            concept_file: path.clone(),
            declared_rules: doc.concept.pest_rules.clone(),
            found_rules,
            owned_rules,
            missing_rules,
        });
    }

    let concept_names: Vec<String> = concept_files
        .iter()
        .map(|(_, doc, _)| doc.concept.name.clone())
        .collect();
    let unmapped_rules: Vec<UnmappedGrammarRule> = rules
        .iter()
        .filter(|rule| !mapped_rules.contains(&rule.name))
        .filter(|rule| !is_shared_grammar_stop_rule(&rule.name))
        .map(|rule| UnmappedGrammarRule {
            name: rule.name.clone(),
            line: rule.line,
            suggested_concept: suggest_concept_owner(&rule.name, &concept_names),
        })
        .collect();

    Ok(ExistingGrammarMapReport {
        rule_count: rules.len(),
        concept_count: concepts.len(),
        dependency_expansion: options.expand_deps,
        shared_rule_count: rules
            .iter()
            .filter(|rule| is_shared_grammar_stop_rule(&rule.name))
            .count(),
        mapped_rule_count: mapped_rules.len(),
        unmapped_rule_count: unmapped_rules.len(),
        concepts,
        unmapped_rules,
    })
}

const SHARED_GRAMMAR_STOP_RULES: &[&str] = &[
    "WHITESPACE",
    "abilities",
    "ability_word",
    "activated_ability",
    "activated_effect",
    "activated_effect_sentence",
    "activated_effect_statement",
    "activated_effects",
    "activated_then_effect",
    "additional_cost",
    "article",
    "basic_land_type",
    "basic_land_type_plural",
    "card_name",
    "card_text",
    "card_type",
    "card_type_plural",
    "choose_one",
    "color",
    "color_word",
    "colored_mana_symbol",
    "colors",
    "cost",
    "counter_amount",
    "counter_name",
    "creature_subtype",
    "creature_subtype_plural",
    "creature_type",
    "creature_type_plural",
    "generic",
    "keyword_ability",
    "land_subtype",
    "land_subtype_plural",
    "line",
    "mana_body",
    "mana_cost",
    "mana_symbol",
    "modal_choice",
    "modal_effect",
    "modal_mode",
    "number_word",
    "permanent_type",
    "permanent_type_plural",
    "phrase_statement",
    "proper_name",
    "sentence_chain",
    "sentence_statement",
    "source_object",
    "spell_type_choice",
    "static_ability",
    "static_effect",
    "subtype",
    "supertype",
    "tap_symbol",
    "that_object",
    "this_turn_period",
    "trigger_effect",
    "trigger_sentence",
    "triggered_ability",
    "unsigned_number",
    "variable_name",
    "where_clause",
    "zone",
];

fn concept_owned_rule_names(
    rules: &[grammar_query_engine::GrammarRuleDefinition],
    roots: &[String],
    expand_deps: bool,
) -> (BTreeSet<String>, Vec<String>) {
    let rule_names: BTreeSet<&str> = rules.iter().map(|rule| rule.name.as_str()).collect();
    let mut owned = BTreeSet::new();
    let mut missing = Vec::new();
    let mut stack = Vec::new();

    for root in roots {
        if rule_names.contains(root.as_str()) {
            owned.insert(root.clone());
            if expand_deps {
                stack.push(root.clone());
            }
        } else {
            missing.push(root.clone());
        }
    }

    while let Some(rule_name) = stack.pop() {
        for dependency in grammar_query_engine::direct_dependencies(rules, &rule_name) {
            if is_shared_grammar_stop_rule(&dependency) {
                continue;
            }
            if owned.insert(dependency.clone()) {
                stack.push(dependency);
            }
        }
    }

    (owned, missing)
}

fn is_shared_grammar_stop_rule(rule_name: &str) -> bool {
    SHARED_GRAMMAR_STOP_RULES.contains(&rule_name)
}

fn is_shared_plumbing_block_reason(reason: &str) -> bool {
    let reason = reason.trim().to_ascii_lowercase();
    reason.starts_with("shared") || reason.starts_with("shared/plumbing")
}

fn is_structural_block_reason(reason: &str) -> bool {
    let reason = reason.trim().to_ascii_lowercase();
    is_shared_plumbing_block_reason(&reason)
        || reason.contains("plumbing")
        || reason.contains("wrapper")
        || reason.contains("aggregator")
        || reason.contains("lexical")
}

fn plumbing_cooldown_selection(
    report: &ExistingGrammarMapReport,
    blocked_targets: &BTreeSet<String>,
    cooldown: &PlumbingCooldownState,
) -> PlumbingCooldownSelection {
    let non_cooled_candidate_exists = report.unmapped_rules.iter().any(|rule| {
        !blocked_targets.contains(&rule.name) && !cooldown.cooled_target_rules.contains(&rule.name)
    });
    let mut excluded_rules = blocked_targets.clone();
    let fallback_status = if non_cooled_candidate_exists {
        excluded_rules.extend(cooldown.cooled_target_rules.iter().cloned());
        "cooldown_active_non_cooled_candidate_available"
    } else if cooldown.cooled_target_rules.is_empty() {
        "no_cooled_candidates"
    } else {
        "fallback_to_cooled_candidates"
    };

    PlumbingCooldownSelection {
        exact_blocked_target_rules: blocked_targets.iter().cloned().collect(),
        cooled_target_rules: cooldown.cooled_target_rules.iter().cloned().collect(),
        excluded_rules: excluded_rules.into_iter().collect(),
        fallback_status: fallback_status.to_string(),
    }
}

const SELECTOR_CONTRACT_BLOCKED_RULES: &[&str] = &[
    "spell_type",
    "life_gain_amount",
    "permanent_controller",
    "permanent_object",
    "damage_amount",
    "colored_target_effect",
    "colored_target_action",
    "upper_alpha",
];

fn load_persisted_blocked_exclusions() -> Result<Vec<PersistedBlockedExclusion>> {
    let root = grammar_concept_log_root();
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut exclusions_by_rule = BTreeMap::<String, PersistedBlockedExclusion>::new();
    for run_entry in fs::read_dir(&root).with_context(|| format!("read {}", root.display()))? {
        let run_entry = run_entry?;
        let run_path = run_entry.path();
        if !run_path.is_dir() {
            continue;
        }
        let run_name = run_entry.file_name();
        let run_name = run_name.to_string_lossy();
        if !run_name.starts_with("grind-") {
            continue;
        }

        for iteration_entry in
            fs::read_dir(&run_path).with_context(|| format!("read {}", run_path.display()))?
        {
            let iteration_entry = iteration_entry?;
            let iteration_path = iteration_entry.path();
            if !iteration_path.is_dir() {
                continue;
            }
            let iteration_name = iteration_entry.file_name();
            let iteration_name = iteration_name.to_string_lossy();
            let Some(iteration_raw) = iteration_name.strip_prefix("iteration-") else {
                continue;
            };
            let source_iteration = iteration_raw.parse::<u32>().unwrap_or_default();
            let gap_path = iteration_path.join("gap.json");
            let decision_path = iteration_path.join("boundary_decision.json");
            if !gap_path.exists() || !decision_path.exists() {
                continue;
            }
            let gap_text = fs::read_to_string(&gap_path)
                .with_context(|| format!("read {}", gap_path.display()))?;
            let decision_text = fs::read_to_string(&decision_path)
                .with_context(|| format!("read {}", decision_path.display()))?;
            let gap: ConceptGap = serde_json::from_str(&gap_text)
                .with_context(|| format!("parse {}", gap_path.display()))?;
            let decision: BoundaryDecision = serde_json::from_str(&decision_text)
                .with_context(|| format!("parse {}", decision_path.display()))?;
            let BoundaryOwner::Blocked(reason) = decision.owner else {
                continue;
            };
            let exclusion = PersistedBlockedExclusion {
                target_rule: gap.target_rule.clone(),
                normalized_blocked_reason: slug(&reason).replace('-', "_"),
                structural_exclusion_reason: reason.clone(),
                matched_feature: classify_blocked_feature(&gap.target_rule, &reason),
                evidence_rule_or_parent: gap.target_rule,
                source_run: run_path.clone(),
                source_iteration,
            };
            let replace = exclusions_by_rule
                .get(&exclusion.target_rule)
                .is_none_or(|current| {
                    (current.source_run.as_path(), current.source_iteration)
                        <= (exclusion.source_run.as_path(), exclusion.source_iteration)
                });
            if replace {
                exclusions_by_rule.insert(exclusion.target_rule.clone(), exclusion);
            }
        }
    }

    Ok(exclusions_by_rule.into_values().collect())
}

fn classify_blocked_feature(target_rule: &str, reason: &str) -> String {
    let lower = reason.to_ascii_lowercase();
    if lower.contains("lexical") {
        "lexical_plumbing".to_string()
    } else if lower.contains("wrapper") {
        "wrapper_rule".to_string()
    } else if lower.contains("aggregator") {
        "aggregator_rule".to_string()
    } else if lower.contains("controller") {
        "controller_phrase".to_string()
    } else if lower.contains("amount") {
        "amount_wrapper".to_string()
    } else if lower.contains("plumbing") || lower.starts_with("shared") {
        "shared_plumbing".to_string()
    } else {
        target_rule.to_string()
    }
}

fn seed_selector_exclusions(
    report: &ExistingGrammarMapReport,
    persisted_exclusions: &[PersistedBlockedExclusion],
    blocked_targets: &mut BTreeSet<String>,
    plumbing_cooldown: &mut PlumbingCooldownState,
) -> Result<()> {
    for exclusion in persisted_exclusions {
        blocked_targets.insert(exclusion.target_rule.clone());
        if !is_structural_block_reason(&exclusion.structural_exclusion_reason) {
            continue;
        }
        let derivation = derive_plumbing_cooldown(report, &exclusion.target_rule)?;
        for candidate in &derivation.cooled_target_rules {
            plumbing_cooldown
                .cooled_target_rules
                .insert(candidate.target_rule.clone());
        }
        plumbing_cooldown.derivations.push(derivation);
    }
    Ok(())
}

fn build_concept_candidate(
    report: &ExistingGrammarMapReport,
    options: &ConceptGrindOptions,
    blocked_targets: &BTreeSet<String>,
    plumbing_cooldown: &PlumbingCooldownState,
    persisted_exclusions: &[PersistedBlockedExclusion],
) -> Result<CandidateBuild> {
    let cooldown_selection =
        plumbing_cooldown_selection(report, blocked_targets, plumbing_cooldown);
    let excluded_rules: BTreeSet<String> =
        cooldown_selection.excluded_rules.iter().cloned().collect();
    let gap = select_concept_gap_excluding(report, options, &excluded_rules)?;
    let remaining_candidate_count = report
        .unmapped_rules
        .iter()
        .filter(|rule| !excluded_rules.contains(&rule.name))
        .count();
    let structural_exclusion_reason = join_audit_values(
        persisted_exclusions
            .iter()
            .map(|exclusion| exclusion.structural_exclusion_reason.as_str()),
    );
    let matched_feature = join_audit_values(
        persisted_exclusions
            .iter()
            .map(|exclusion| exclusion.matched_feature.as_str()),
    );
    let evidence_rule_or_parent = join_audit_values(
        persisted_exclusions
            .iter()
            .map(|exclusion| exclusion.evidence_rule_or_parent.as_str()),
    );
    let selected_post_filter_candidate = Some(CandidateSelectionSummary {
        concept: gap.concept.clone(),
        query: gap.query.clone(),
        target_rule: gap.target_rule.clone(),
        target_line: gap.target_line,
        reason: gap.reason.clone(),
    });

    Ok(CandidateBuild {
        gap,
        audit: CandidateBuildAudit {
            persisted_exclusions: persisted_exclusions.to_vec(),
            plumbing_cooldown_selection: cooldown_selection,
            excluded_count: excluded_rules.len(),
            excluded_rules: excluded_rules.into_iter().collect(),
            structural_exclusion_reason,
            matched_feature,
            evidence_rule_or_parent,
            remaining_candidate_count,
            selected_post_filter_candidate,
        },
    })
}

fn join_audit_values<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let mut unique = BTreeSet::new();
    for value in values {
        let value = value.trim();
        if !value.is_empty() {
            unique.insert(value.to_string());
        }
    }
    if unique.is_empty() {
        "none".to_string()
    } else {
        unique.into_iter().collect::<Vec<_>>().join("; ")
    }
}

fn selector_contract_check(
    report: &ExistingGrammarMapReport,
    options: &ConceptGrindOptions,
    blocked_targets: &BTreeSet<String>,
    plumbing_cooldown: &PlumbingCooldownState,
    persisted_exclusions: &[PersistedBlockedExclusion],
) -> Result<SelectorContractReport> {
    let candidate = build_concept_candidate(
        report,
        options,
        blocked_targets,
        plumbing_cooldown,
        persisted_exclusions,
    )?;
    let persisted_blocked_rules: BTreeSet<String> = persisted_exclusions
        .iter()
        .map(|exclusion| exclusion.target_rule.clone())
        .collect();
    let required_blocked_rules: BTreeSet<String> = SELECTOR_CONTRACT_BLOCKED_RULES
        .iter()
        .map(|rule| (*rule).to_string())
        .collect();
    let missing_rules = required_blocked_rules
        .difference(&persisted_blocked_rules)
        .cloned()
        .collect::<Vec<_>>();
    let exposed_rules = candidate
        .audit
        .selected_post_filter_candidate
        .as_ref()
        .filter(|selected| required_blocked_rules.contains(&selected.target_rule))
        .map(|selected| vec![selected.target_rule.clone()])
        .unwrap_or_default();
    let missing_audit_fields = missing_candidate_audit_fields(&candidate.audit);
    let status = if missing_rules.is_empty()
        && exposed_rules.is_empty()
        && missing_audit_fields.is_empty()
    {
        "ok"
    } else {
        "selector_contract_failed"
    };

    Ok(SelectorContractReport {
        status: status.to_string(),
        required_blocked_rules: required_blocked_rules.into_iter().collect(),
        persisted_blocked_rules: persisted_blocked_rules.into_iter().collect(),
        missing_rules,
        exposed_rules,
        missing_audit_fields,
        candidate_build: Some(candidate.audit),
    })
}

fn missing_candidate_audit_fields(audit: &CandidateBuildAudit) -> Vec<String> {
    let mut missing = Vec::new();
    if audit.structural_exclusion_reason.trim().is_empty() {
        missing.push("structural_exclusion_reason".to_string());
    }
    if audit.matched_feature.trim().is_empty() {
        missing.push("matched_feature".to_string());
    }
    if audit.evidence_rule_or_parent.trim().is_empty() {
        missing.push("evidence_rule_or_parent".to_string());
    }
    if audit.selected_post_filter_candidate.is_none() {
        missing.push("selected_post_filter_candidate".to_string());
    }
    missing
}

fn derive_plumbing_cooldown(
    report: &ExistingGrammarMapReport,
    blocked_target_rule: &str,
) -> Result<PlumbingCooldownDerivation> {
    let rules = grammar_query_engine::parse_grammar_file(grammar_pest_path())?;
    Ok(derive_plumbing_cooldown_from_rules(
        report,
        &rules,
        blocked_target_rule,
    ))
}

fn derive_plumbing_cooldown_from_rules(
    report: &ExistingGrammarMapReport,
    rules: &[grammar_query_engine::GrammarRuleDefinition],
    blocked_target_rule: &str,
) -> PlumbingCooldownDerivation {
    let rule_by_name: BTreeMap<&str, &grammar_query_engine::GrammarRuleDefinition> = rules
        .iter()
        .map(|rule| (rule.name.as_str(), rule))
        .collect();
    let blocked_target_expansion_tree =
        expand_rule_leaf_set(rules, &rule_by_name, blocked_target_rule, 2, None);
    let blocked_leaf_set: BTreeSet<String> = blocked_target_expansion_tree
        .normalized_leaf_rules
        .iter()
        .cloned()
        .collect();
    let mut cooled_target_rules = Vec::new();

    for candidate in &report.unmapped_rules {
        if candidate.name == blocked_target_rule {
            continue;
        }
        let expansion_tree = expand_rule_leaf_set(
            rules,
            &rule_by_name,
            &candidate.name,
            2,
            Some((blocked_target_rule, &blocked_target_expansion_tree)),
        );
        let candidate_leaf_set: BTreeSet<String> = expansion_tree
            .normalized_leaf_rules
            .iter()
            .cloned()
            .collect();
        let Some(relationship_type) =
            plumbing_cooldown_relationship(&blocked_leaf_set, &candidate_leaf_set)
        else {
            continue;
        };
        cooled_target_rules.push(PlumbingCooldownCandidate {
            target_rule: candidate.name.clone(),
            leaf_owning_concepts: leaf_owning_concepts(report, &candidate_leaf_set),
            normalized_leaf_rules: expansion_tree.normalized_leaf_rules.clone(),
            expansion_tree,
            relationship_type,
        });
    }

    PlumbingCooldownDerivation {
        blocked_target_rule: blocked_target_rule.to_string(),
        blocked_target_leaf_owning_concepts: leaf_owning_concepts(report, &blocked_leaf_set),
        blocked_target_normalized_leaf_rules: blocked_target_expansion_tree
            .normalized_leaf_rules
            .clone(),
        blocked_target_expansion_tree,
        cooled_target_rules,
        fallback_status: "not_evaluated_until_next_selection".to_string(),
    }
}

fn plumbing_cooldown_relationship(
    blocked_leaf_set: &BTreeSet<String>,
    candidate_leaf_set: &BTreeSet<String>,
) -> Option<PlumbingCooldownRelationship> {
    if blocked_leaf_set.is_empty() || candidate_leaf_set.is_empty() {
        return None;
    }
    if candidate_leaf_set == blocked_leaf_set {
        return Some(PlumbingCooldownRelationship::Equal);
    }
    if candidate_leaf_set.is_superset(blocked_leaf_set) {
        return Some(PlumbingCooldownRelationship::Wrapper);
    }
    if candidate_leaf_set.is_subset(blocked_leaf_set) {
        return Some(PlumbingCooldownRelationship::Child);
    }
    None
}

fn expand_rule_leaf_set(
    rules: &[grammar_query_engine::GrammarRuleDefinition],
    rule_by_name: &BTreeMap<&str, &grammar_query_engine::GrammarRuleDefinition>,
    rule_name: &str,
    wrapper_expansion_budget: u8,
    known_expansion: Option<(&str, &RuleExpansionNode)>,
) -> RuleExpansionNode {
    let rule = rule_by_name.get(rule_name).copied();
    let dependencies = grammar_query_engine::direct_dependencies(rules, rule_name);
    let pure_wrapper_or_alternation =
        rule.is_some_and(|rule| is_pure_wrapper_or_alternation_rhs(&rule.rhs, &dependencies));
    let should_expand =
        wrapper_expansion_budget > 0 && pure_wrapper_or_alternation && !dependencies.is_empty();

    if !should_expand {
        let normalized_leaf_rules = if is_shared_grammar_stop_rule(rule_name) {
            Vec::new()
        } else {
            vec![rule_name.to_string()]
        };
        return RuleExpansionNode {
            rule: rule_name.to_string(),
            line: rule.map(|rule| rule.line),
            pure_wrapper_or_alternation,
            normalized_leaf_rules,
            children: Vec::new(),
        };
    }

    let mut children = Vec::new();
    let mut leaves = BTreeSet::new();
    for dependency in dependencies {
        let child = if known_expansion.is_some_and(|(known_rule, _)| known_rule == dependency) {
            known_expansion.expect("checked").1.clone()
        } else {
            expand_rule_leaf_set(
                rules,
                rule_by_name,
                &dependency,
                wrapper_expansion_budget - 1,
                known_expansion,
            )
        };
        leaves.extend(child.normalized_leaf_rules.iter().cloned());
        children.push(child);
    }

    RuleExpansionNode {
        rule: rule_name.to_string(),
        line: rule.map(|rule| rule.line),
        pure_wrapper_or_alternation,
        normalized_leaf_rules: leaves.into_iter().collect(),
        children,
    }
}

fn is_pure_wrapper_or_alternation_rhs(rhs: &str, dependencies: &[String]) -> bool {
    if dependencies.is_empty() || rhs.contains('"') || rhs.contains('\'') {
        return false;
    }

    let dependency_names: BTreeSet<&str> = dependencies.iter().map(String::as_str).collect();
    for token in pest_rhs_identifier_tokens(rhs) {
        if is_pest_wrapper_builtin_identifier(&token) {
            continue;
        }
        if !dependency_names.contains(token.as_str()) {
            return false;
        }
    }
    true
}

fn pest_rhs_identifier_tokens(rhs: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in rhs.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch);
        } else if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn is_pest_wrapper_builtin_identifier(token: &str) -> bool {
    matches!(
        token,
        "SOI" | "EOI" | "ANY" | "ASCII_DIGIT" | "ASCII_ALPHA" | "ASCII_ALPHANUMERIC" | "WHITESPACE"
    )
}

fn leaf_owning_concepts(
    report: &ExistingGrammarMapReport,
    leaf_rules: &BTreeSet<String>,
) -> BTreeMap<String, Vec<String>> {
    let mut owners: BTreeMap<String, Vec<String>> = leaf_rules
        .iter()
        .map(|rule| (rule.clone(), Vec::new()))
        .collect();
    for concept in &report.concepts {
        for owned_rule in &concept.owned_rules {
            if let Some(rule_owners) = owners.get_mut(&owned_rule.name) {
                rule_owners.push(concept.concept.clone());
            }
        }
    }
    owners
}

const CONCEPT_GRIND_TOTAL_STEPS: u8 = 7;

fn run_grind(options: ConceptGrindOptions, sink: &mut dyn FlowSink) -> Result<()> {
    if !options.dry_run && !options.allow_dirty {
        ensure_clean_working_tree()
            .context("working tree must be clean before concept-grind, or pass --allow-dirty")?;
    }

    sink.emit(FlowEvent::SessionStarted {
        workflow: "concept-grind".to_string(),
        set: options
            .concept
            .clone()
            .unwrap_or_else(|| "auto-gap".to_string()),
        max_iterations: options.max_iterations,
        baseline_corpus_passing: 0,
        baseline_corpus_total: 0,
        baseline_grammar_rules: concept_grind_rule_count(),
    });

    let mut blocked_targets = BTreeSet::new();
    let mut plumbing_cooldown = PlumbingCooldownState::default();
    let mut persisted_exclusions = load_persisted_blocked_exclusions()?;
    let session_dir = grammar_concept_log_root().join(format!("grind-{}", unix_timestamp()));
    fs::create_dir_all(&session_dir)
        .with_context(|| format!("create {}", session_dir.display()))?;
    let mut metrics = ConceptGrindMetrics::new(session_dir.clone());
    metrics.write(&session_dir)?;
    write_json(
        session_dir.join("persisted_blocked_exclusions.json"),
        &persisted_exclusions,
    )?;
    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: format!("concept-grind log: {}", session_dir.display()),
    });

    let selector_contract_map = run_map_existing(MapExistingOptions {
        json: false,
        expand_deps: true,
    })?;
    seed_selector_exclusions(
        &selector_contract_map,
        &persisted_exclusions,
        &mut blocked_targets,
        &mut plumbing_cooldown,
    )?;
    let selector_contract = selector_contract_check(
        &selector_contract_map,
        &options,
        &blocked_targets,
        &plumbing_cooldown,
        &persisted_exclusions,
    )?;
    write_json(
        session_dir.join("selector_contract.json"),
        &selector_contract,
    )?;
    if selector_contract.status == "selector_contract_failed" {
        bail!(
            "selector_contract_failed: missing_rules={:?} exposed_rules={:?} missing_audit_fields={:?}",
            selector_contract.missing_rules,
            selector_contract.exposed_rules,
            selector_contract.missing_audit_fields
        );
    }

    for iteration in 1..=options.max_iterations {
        if sink.stop_requested() {
            sink.emit(FlowEvent::SessionFinished {
                reason: SessionEndReason::StopRequested,
            });
            return Ok(());
        }
        let iteration_start = Instant::now();
        let iteration_dir = session_dir.join(format!("iteration-{iteration:03}"));

        metrics.step_started(sink, iteration, 1, "map grammar gaps");
        fs::create_dir_all(&iteration_dir)
            .with_context(|| format!("create {}", iteration_dir.display()))?;

        let before = run_map_existing(MapExistingOptions {
            json: false,
            expand_deps: true,
        })?;
        write_json(iteration_dir.join("map_before.json"), &before)?;
        let candidate_build = build_concept_candidate(
            &before,
            &options,
            &blocked_targets,
            &plumbing_cooldown,
            &persisted_exclusions,
        )?;
        let cooldown_selection = &candidate_build.audit.plumbing_cooldown_selection;
        write_json(
            iteration_dir.join("plumbing_cooldown_selection.json"),
            cooldown_selection,
        )?;
        write_json(
            iteration_dir.join("candidate_build_audit.json"),
            &candidate_build.audit,
        )?;
        let gap = candidate_build.gap;
        write_json(iteration_dir.join("gap.json"), &gap)?;

        sink.emit(FlowEvent::WorkflowIterationStarted {
            index: iteration,
            max_iterations: options.max_iterations,
            title: gap.concept.clone(),
            detail: format!(
                "target_rule={}\nquery={}\nlog={}",
                gap.target_rule,
                gap.query,
                iteration_dir.display()
            ),
        });
        metrics.step_finished(
            sink,
            &session_dir,
            iteration,
            1,
            true,
            Some(format!(
                "mapped={} unmapped={}",
                before.mapped_rule_count, before.unmapped_rule_count
            )),
        )?;

        metrics.step_started(sink, iteration, 2, "build boundary prompt");
        let boundary_prompt = build_boundary_prompt(&gap, &before)?;
        fs::write(iteration_dir.join("boundary_prompt.md"), &boundary_prompt).with_context(
            || {
                format!(
                    "write {}",
                    iteration_dir.join("boundary_prompt.md").display()
                )
            },
        )?;
        metrics.step_finished(
            sink,
            &session_dir,
            iteration,
            2,
            true,
            Some("wrote boundary_prompt.md".to_string()),
        )?;

        if options.dry_run {
            sink.emit(FlowEvent::SessionFinished {
                reason: SessionEndReason::DryRunStop,
            });
            return Ok(());
        }

        metrics.step_started(sink, iteration, 3, "boundary agent");
        let boundary = refactor_hotspot::invoke_agent(
            options.agent,
            &boundary_prompt,
            &iteration_dir.join("boundary_transcript.ndjson"),
            sink,
        )?;
        fs::write(
            iteration_dir.join("boundary_response.md"),
            &boundary.assistant_text,
        )
        .with_context(|| {
            format!(
                "write {}",
                iteration_dir.join("boundary_response.md").display()
            )
        })?;
        if !boundary.success {
            bail!(
                "{} boundary agent exited with status {}; transcript: {}",
                options.agent.label(),
                boundary.exit_code,
                iteration_dir.join("boundary_transcript.ndjson").display()
            );
        }
        let boundary_decision = parse_boundary_decision(&boundary.assistant_text)?;
        write_json(
            iteration_dir.join("boundary_decision.json"),
            &boundary_decision,
        )?;
        if let BoundaryOwner::Blocked(reason) = &boundary_decision.owner {
            metrics.step_finished(
                sink,
                &session_dir,
                iteration,
                3,
                true,
                Some(format!("blocked: {reason}")),
            )?;
            if options.concept.is_some() || options.target_rule.is_some() {
                return Err(anyhow!("boundary decision blocked concept-grind: {reason}"));
            }
            sink.emit(FlowEvent::Note {
                level: NoteLevel::Warn,
                text: format!(
                    "skipping blocked target {} for this run: {reason}",
                    gap.target_rule
                ),
            });
            if is_structural_block_reason(reason) {
                let derivation = derive_plumbing_cooldown(&before, &gap.target_rule)?;
                for candidate in &derivation.cooled_target_rules {
                    plumbing_cooldown
                        .cooled_target_rules
                        .insert(candidate.target_rule.clone());
                }
                plumbing_cooldown.derivations.push(derivation.clone());
                write_json(
                    iteration_dir.join("plumbing_cooldown_derivation.json"),
                    &derivation,
                )?;
                write_json(
                    session_dir.join("plumbing_cooldown_state.json"),
                    &plumbing_cooldown,
                )?;
                sink.emit(FlowEvent::Note {
                    level: NoteLevel::Info,
                    text: format!(
                        "cooled {} nested shared/plumbing candidate(s) after blocking {}",
                        derivation.cooled_target_rules.len(),
                        gap.target_rule
                    ),
                });
            }
            let exclusion = PersistedBlockedExclusion {
                target_rule: gap.target_rule.clone(),
                normalized_blocked_reason: slug(reason).replace('-', "_"),
                structural_exclusion_reason: reason.clone(),
                matched_feature: classify_blocked_feature(&gap.target_rule, reason),
                evidence_rule_or_parent: gap.target_rule.clone(),
                source_run: session_dir.clone(),
                source_iteration: iteration,
            };
            write_json(iteration_dir.join("blocked_exclusion.json"), &exclusion)?;
            persisted_exclusions.push(exclusion);
            write_json(
                session_dir.join("persisted_blocked_exclusions.json"),
                &persisted_exclusions,
            )?;
            blocked_targets.insert(gap.target_rule);
            continue;
        }
        let gap = apply_boundary_decision(gap, &boundary_decision)?;
        metrics.step_finished(
            sink,
            &session_dir,
            iteration,
            3,
            true,
            Some("parsed boundary decision".to_string()),
        )?;

        let mut fastpath = attempt_no_pest_concept_fastpath(&gap, &boundary_decision, true)?;
        if fastpath.fastpath_attempted {
            let level = if fastpath.fastpath_result == "success" {
                NoteLevel::Info
            } else {
                NoteLevel::Warn
            };
            sink.emit(FlowEvent::Note {
                level,
                text: format!(
                    "no-PEST fast-path {} for {}{}",
                    fastpath.fastpath_result,
                    gap.concept,
                    fastpath
                        .fallback_reason
                        .as_ref()
                        .map(|reason| format!(": {reason}"))
                        .unwrap_or_default()
                ),
            });
        }
        let fastpath_skipped_patch = fastpath.fastpath_result == "success";
        fastpath.patch_agent_started = !fastpath_skipped_patch;
        write_json(iteration_dir.join("no_pest_fastpath.json"), &fastpath)?;

        metrics.step_started(sink, iteration, 4, "patch agent");
        if fastpath_skipped_patch {
            metrics.step_finished(
                sink,
                &session_dir,
                iteration,
                4,
                true,
                Some("no-PEST fast-path skipped patch agent".to_string()),
            )?;
        } else {
            let patch_prompt =
                build_patch_prompt(&gap, &boundary_decision, &boundary.assistant_text)?;
            fs::write(iteration_dir.join("patch_prompt.md"), &patch_prompt).with_context(|| {
                format!("write {}", iteration_dir.join("patch_prompt.md").display())
            })?;
            let patch = refactor_hotspot::invoke_agent(
                options.agent,
                &patch_prompt,
                &iteration_dir.join("patch_transcript.ndjson"),
                sink,
            )?;
            fs::write(
                iteration_dir.join("patch_response.md"),
                &patch.assistant_text,
            )
            .with_context(|| {
                format!(
                    "write {}",
                    iteration_dir.join("patch_response.md").display()
                )
            })?;
            if !patch.success {
                bail!(
                    "{} patch agent exited with status {}; transcript: {}",
                    options.agent.label(),
                    patch.exit_code,
                    iteration_dir.join("patch_transcript.ndjson").display()
                );
            }
            metrics.step_finished(
                sink,
                &session_dir,
                iteration,
                4,
                true,
                Some("patch agent completed".to_string()),
            )?;
        }

        metrics.step_started(sink, iteration, 5, "gate and repair");
        let mut gate = run_concept_grind_gates(&gap, &before, &iteration_dir);
        for repair_index in 1..=options.repair_attempts {
            let Err(failure) = gate else {
                break;
            };
            sink.emit(FlowEvent::Note {
                level: NoteLevel::Warn,
                text: format!(
                    "gate failed: {}; repair attempt {repair_index}/{}",
                    failure.label, options.repair_attempts
                ),
            });
            fs::write(
                iteration_dir.join(format!("repair-{repair_index}-failure.txt")),
                format!("{}\n\n{}", failure.label, failure.output),
            )?;
            let repair_prompt = build_repair_prompt(&gap, &failure)?;
            fs::write(
                iteration_dir.join(format!("repair-{repair_index}-prompt.md")),
                &repair_prompt,
            )?;
            let repair = refactor_hotspot::invoke_agent(
                options.agent,
                &repair_prompt,
                &iteration_dir.join(format!("repair-{repair_index}-transcript.ndjson")),
                sink,
            )?;
            fs::write(
                iteration_dir.join(format!("repair-{repair_index}-response.md")),
                &repair.assistant_text,
            )?;
            if !repair.success {
                bail!(
                    "{} repair agent exited with status {}; transcript: {}",
                    options.agent.label(),
                    repair.exit_code,
                    iteration_dir
                        .join(format!("repair-{repair_index}-transcript.ndjson"))
                        .display()
                );
            }
            gate = run_concept_grind_gates(&gap, &before, &iteration_dir);
        }
        if let Err(failure) = gate {
            metrics.step_finished(
                sink,
                &session_dir,
                iteration,
                5,
                false,
                Some(failure.label.clone()),
            )?;
            return Err(anyhow!("{} failed\n{}", failure.label, failure.output));
        }
        metrics.step_finished(
            sink,
            &session_dir,
            iteration,
            5,
            true,
            Some("all gates passed".to_string()),
        )?;

        metrics.step_started(sink, iteration, 6, "update map and maturity");
        let _pre_map_contract = match run_quality_contract_with_single_repair(
            &gap,
            &before,
            &options,
            sink,
            &iteration_dir,
            "pre_map_update",
        ) {
            Ok(report) => report,
            Err(error) => {
                metrics.step_finished(
                    sink,
                    &session_dir,
                    iteration,
                    6,
                    false,
                    Some(format!("{error:#}")),
                )?;
                return Err(error);
            }
        };
        let mut after = run_map_existing(MapExistingOptions {
            json: false,
            expand_deps: true,
        })?;
        write_json(iteration_dir.join("map_after.json"), &after)?;
        let maturity = run_maturity(MaturityOptions {
            concept: gap.concept.clone(),
            json: false,
            update: true,
            fresh_fixture: true,
        })?;
        write_json(iteration_dir.join("maturity.json"), &maturity)?;
        metrics.step_finished(
            sink,
            &session_dir,
            iteration,
            6,
            true,
            Some(format!(
                "mapped {} -> {}, unmapped {} -> {}",
                before.mapped_rule_count,
                after.mapped_rule_count,
                before.unmapped_rule_count,
                after.unmapped_rule_count
            )),
        )?;

        metrics.step_started(sink, iteration, 7, "commit and summarize");
        let final_contract = match run_quality_contract_with_single_repair(
            &gap,
            &before,
            &options,
            sink,
            &iteration_dir,
            "pre_commit",
        ) {
            Ok(report) => report,
            Err(error) => {
                metrics.step_finished(
                    sink,
                    &session_dir,
                    iteration,
                    7,
                    false,
                    Some(format!("{error:#}")),
                )?;
                return Err(error);
            }
        };
        let final_maturity = persist_quality_contract_maturity(&final_contract)?;
        write_json(iteration_dir.join("maturity_final.json"), &final_maturity)?;
        after = run_map_existing(MapExistingOptions {
            json: false,
            expand_deps: true,
        })?;
        write_json(iteration_dir.join("map_final.json"), &after)?;
        if let Err(failure) = run_gap_closure_gate(&gap, &before, &after) {
            metrics.step_finished(
                sink,
                &session_dir,
                iteration,
                7,
                false,
                Some(failure.label.clone()),
            )?;
            return Err(anyhow!("{} failed\n{}", failure.label, failure.output));
        }
        if fastpath_skipped_patch {
            ensure_no_grammar_pest_worktree_diff()
                .context("fast-path iteration produced a grammar.pest diff")?;
        }
        let committed = if options.no_commit {
            false
        } else {
            commit_concept_grind_iteration(&gap, iteration)?
        };
        let summary = ConceptGrindIterationSummary {
            iteration,
            concept: gap.concept.clone(),
            query: gap.query.clone(),
            target_rule: gap.target_rule.clone(),
            fixture_passed: final_contract.fixture_result.passed,
            maturity_state: final_maturity.state.clone(),
            mapped_rule_count_before: before.mapped_rule_count,
            mapped_rule_count_after: after.mapped_rule_count,
            unmapped_rule_count_before: before.unmapped_rule_count,
            unmapped_rule_count_after: after.unmapped_rule_count,
            committed,
        };
        write_json(iteration_dir.join("summary.json"), &summary)?;
        metrics.step_finished(
            sink,
            &session_dir,
            iteration,
            7,
            true,
            Some(if committed {
                "committed iteration".to_string()
            } else {
                "no commit (--no-commit)".to_string()
            }),
        )?;
        if committed {
            sink.emit(FlowEvent::IterationFinished {
                index: iteration,
                outcome: IterationOutcomeSummary::Committed {
                    new_passes: after
                        .mapped_rule_count
                        .saturating_sub(before.mapped_rule_count),
                    corpus_passing: 0,
                    corpus_total: 0,
                    grammar_rules: after.rule_count,
                    duration_secs: iteration_start.elapsed().as_secs(),
                },
            });
        }
    }

    sink.emit(FlowEvent::SessionFinished {
        reason: SessionEndReason::MaxIterationsReached(options.max_iterations),
    });
    Ok(())
}

fn concept_step_started(sink: &mut dyn FlowSink, index: u8, label: &str) {
    sink.emit(FlowEvent::StepStarted {
        index,
        total: CONCEPT_GRIND_TOTAL_STEPS,
        label: label.to_string(),
    });
}

fn concept_step_finished(sink: &mut dyn FlowSink, index: u8, ok: bool, summary: Option<String>) {
    sink.emit(FlowEvent::StepFinished { index, ok, summary });
}

impl ConceptGrindMetrics {
    fn new(session_dir: PathBuf) -> Self {
        Self {
            workflow: "concept-grind".to_string(),
            session_dir,
            started_unix_ms: unix_timestamp_ms(),
            iterations: Vec::new(),
            active_steps: BTreeMap::new(),
        }
    }

    fn step_started(&mut self, sink: &mut dyn FlowSink, iteration: u32, index: u8, label: &str) {
        self.active_steps.insert(
            (iteration, index),
            ActiveConceptGrindStep {
                label: label.to_string(),
                started: Instant::now(),
                started_unix_ms: unix_timestamp_ms(),
            },
        );
        concept_step_started(sink, index, label);
    }

    fn step_finished(
        &mut self,
        sink: &mut dyn FlowSink,
        session_dir: &Path,
        iteration: u32,
        index: u8,
        ok: bool,
        summary: Option<String>,
    ) -> Result<()> {
        let finished_unix_ms = unix_timestamp_ms();
        let active = self
            .active_steps
            .remove(&(iteration, index))
            .unwrap_or_else(|| ActiveConceptGrindStep {
                label: format!("step {index}"),
                started: Instant::now(),
                started_unix_ms: finished_unix_ms,
            });
        let metric = ConceptGrindStepMetric {
            index,
            label: active.label,
            started_unix_ms: active.started_unix_ms,
            finished_unix_ms,
            duration_ms: active.started.elapsed().as_millis(),
            ok,
            summary: summary.clone(),
        };
        let iteration_metrics = self
            .iterations
            .iter_mut()
            .find(|entry| entry.iteration == iteration);
        match iteration_metrics {
            Some(entry) => entry.steps.push(metric),
            None => self.iterations.push(ConceptGrindIterationMetrics {
                iteration,
                steps: vec![metric],
            }),
        }
        self.write(session_dir)?;
        concept_step_finished(sink, index, ok, summary);
        Ok(())
    }

    fn write(&self, session_dir: &Path) -> Result<()> {
        write_json(session_dir.join("metrics.json"), self)
    }
}

fn concept_grind_rule_count() -> usize {
    grammar_query_engine::parse_grammar_file(grammar_pest_path())
        .map(|rules| rules.len())
        .unwrap_or_default()
}

#[cfg(test)]
fn select_concept_gap(
    report: &ExistingGrammarMapReport,
    options: &ConceptGrindOptions,
) -> Result<ConceptGap> {
    select_concept_gap_excluding(report, options, &BTreeSet::new())
}

fn select_concept_gap_excluding(
    report: &ExistingGrammarMapReport,
    options: &ConceptGrindOptions,
    excluded_rules: &BTreeSet<String>,
) -> Result<ConceptGap> {
    if let Some(concept) = &options.concept {
        let query = options
            .query
            .clone()
            .unwrap_or_else(|| concept.replace('_', " "));
        let rule = if let Some(target_rule) = &options.target_rule {
            if excluded_rules.contains(target_rule) {
                bail!("--target-rule {target_rule:?} is excluded in this run");
            }
            report
                .unmapped_rules
                .iter()
                .find(|rule| rule.name == *target_rule)
                .ok_or_else(|| {
                    anyhow!("--target-rule {target_rule:?} is not an unmapped grammar rule")
                })?
        } else {
            report
                .unmapped_rules
                .iter()
                .filter(|rule| !excluded_rules.contains(&rule.name))
                .find(|rule| rule.suggested_concept.as_deref() == Some(concept.as_str()))
                .or_else(|| {
                    report
                        .unmapped_rules
                        .iter()
                        .filter(|rule| !excluded_rules.contains(&rule.name))
                        .find(|rule| rule.name.starts_with(concept))
                })
                .ok_or_else(|| {
                    anyhow!(
                        "--concept {concept:?} has no suggested or prefix-matching unmapped rule; pass --target-rule RULE to make the target explicit"
                    )
                })?
        };
        return Ok(ConceptGap {
            concept: concept.clone(),
            query,
            target_rule: rule.name.clone(),
            target_line: rule.line,
            suggested_existing_owner: rule.suggested_concept.is_some(),
            reason: "explicit --concept".to_string(),
        });
    }

    if options.target_rule.is_some() {
        bail!("--target-rule requires --concept so ownership is explicit");
    }

    if let Some(rule) = report
        .unmapped_rules
        .iter()
        .filter(|rule| !excluded_rules.contains(&rule.name))
        .find(|rule| rule.suggested_concept.is_some())
    {
        let concept = rule.suggested_concept.clone().expect("checked");
        let query = options
            .query
            .clone()
            .unwrap_or_else(|| rule.name.replace('_', " "));
        return Ok(ConceptGap {
            concept,
            query,
            target_rule: rule.name.clone(),
            target_line: rule.line,
            suggested_existing_owner: true,
            reason: "unmapped rule has existing concept-name prefix owner".to_string(),
        });
    }

    let rule = report
        .unmapped_rules
        .iter()
        .filter(|rule| !excluded_rules.contains(&rule.name))
        .next()
        .ok_or_else(|| anyhow!("no unblocked unmapped grammar rules remain"))?;
    let concept = slug(&rule.name).replace('-', "_");
    Ok(ConceptGap {
        concept,
        query: options
            .query
            .clone()
            .unwrap_or_else(|| rule.name.replace('_', " ")),
        target_rule: rule.name.clone(),
        target_line: rule.line,
        suggested_existing_owner: false,
        reason: "first unmapped non-shared grammar rule".to_string(),
    })
}

fn build_boundary_prompt(gap: &ConceptGap, report: &ExistingGrammarMapReport) -> Result<String> {
    let rules = grammar_query_engine::parse_grammar_file(grammar_pest_path())?;
    let grammar_report = build_grammar_query_report(&gap.query, vec![gap.target_rule.clone()], 16)?;
    let concept_path = grammar_concepts_dir().join(format!("{}.toml", gap.concept));
    let concept_text = fs::read_to_string(&concept_path).unwrap_or_else(|_| {
        format!(
            "# No committed concept file exists yet for {}.\n",
            gap.concept
        )
    });
    let target = rules
        .iter()
        .find(|rule| rule.name == gap.target_rule)
        .ok_or_else(|| anyhow!("target rule {} not found", gap.target_rule))?;
    let dependencies = grammar_query_engine::direct_dependencies(&rules, &gap.target_rule);
    let reverse_dependencies = grammar_query_engine::reverse_dependencies(&rules, &gap.target_rule);
    let rules_block = rules_context::render_rules_block(&gap.query);

    Ok(format!(
        r#"You are the CONCEPT_BOUNDARY_REVIEW agent for mtg-parser.

This is a grammar-first workflow. Do not edit files in this stage. Do not run add-card. Do not try to make generated card tests pass.

Goal: decide whether the selected unmapped PEST rule should widen an existing concept, become a new concept, or be blocked as shared/plumbing.

Concept candidate:
- concept: {concept}
- query: {query}
- target PEST rule: {target_rule}:{target_line}
- selection reason: {reason}
- existing owner suggestion: {suggested_existing_owner}

Current map:
- grammar rules: {rule_count}
- mapped rules: {mapped_rule_count}
- unmapped non-shared rules: {unmapped_rule_count}

Target PEST rule:
```pest
{target_name} = {target_rhs}
```

Direct dependencies: {dependencies}
Reverse dependencies: {reverse_dependencies}

Committed concept file:
```toml
{concept_text}
```

Grammar-neighbor report:
```json
{grammar_json}
```

Comprehensive Rules context:
{rules_block}

Return a concise decision block:

CONCEPT_BOUNDARY_DECISION:
OWNER: existing:{concept} | new:<name> | blocked:<reason>
AXES: <axis names and values to add or preserve>
EXAMPLES_TO_ACCEPT: <grammar fixture examples>
COUNTEREXAMPLES_TO_REJECT: <true boundary negatives only>
PEST_PATCH_INTENT: <specific grammar change, or none>
WHY_NOT_CARD_PASS: grammar fixture maturity only
"#,
        concept = gap.concept,
        query = gap.query,
        target_rule = gap.target_rule,
        target_line = gap.target_line,
        reason = gap.reason,
        suggested_existing_owner = gap.suggested_existing_owner,
        rule_count = report.rule_count,
        mapped_rule_count = report.mapped_rule_count,
        unmapped_rule_count = report.unmapped_rule_count,
        target_name = target.name,
        target_rhs = target.rhs,
        dependencies = dependencies.join(", "),
        reverse_dependencies = reverse_dependencies.join(", "),
        concept_text = concept_text,
        grammar_json = serde_json::to_string_pretty(&grammar_report)?,
        rules_block = rules_block,
    ))
}

fn parse_boundary_decision(response: &str) -> Result<BoundaryDecision> {
    if !response.contains("CONCEPT_BOUNDARY_DECISION") {
        bail!("boundary agent response missing CONCEPT_BOUNDARY_DECISION block");
    }
    let owner_raw = required_decision_field(response, "OWNER")?;
    let owner = parse_boundary_owner(&owner_raw)?;
    if matches!(owner, BoundaryOwner::Blocked(_)) {
        return Ok(BoundaryDecision {
            owner,
            owner_raw,
            axes: optional_decision_field(response, "AXES").unwrap_or_default(),
            examples_to_accept: optional_decision_field(response, "EXAMPLES_TO_ACCEPT")
                .unwrap_or_default(),
            counterexamples_to_reject: optional_decision_field(
                response,
                "COUNTEREXAMPLES_TO_REJECT",
            )
            .unwrap_or_default(),
            pest_patch_intent: optional_decision_field(response, "PEST_PATCH_INTENT")
                .unwrap_or_default(),
        });
    }
    let axes = optional_decision_field(response, "AXES").unwrap_or_default();
    let examples_to_accept =
        optional_decision_field(response, "EXAMPLES_TO_ACCEPT").unwrap_or_default();
    let counterexamples_to_reject =
        optional_decision_field(response, "COUNTEREXAMPLES_TO_REJECT").unwrap_or_default();
    let pest_patch_intent = required_decision_field(response, "PEST_PATCH_INTENT")?;
    Ok(BoundaryDecision {
        owner,
        owner_raw,
        axes,
        examples_to_accept,
        counterexamples_to_reject,
        pest_patch_intent,
    })
}

fn parse_boundary_owner(owner: &str) -> Result<BoundaryOwner> {
    let owner = owner.trim();
    if let Some(concept) = owner.strip_prefix("existing:") {
        let concept = boundary_owner_concept_token(concept);
        validate_concept_name(concept)?;
        return Ok(BoundaryOwner::Existing(concept.to_string()));
    }
    if let Some(concept) = owner.strip_prefix("new:") {
        let concept = boundary_owner_concept_token(concept);
        validate_concept_name(concept)?;
        return Ok(BoundaryOwner::New(concept.to_string()));
    }
    if let Some(reason) = owner.strip_prefix("blocked:") {
        let reason = reason.trim();
        if reason.is_empty() {
            bail!("blocked OWNER must include a reason");
        }
        return Ok(BoundaryOwner::Blocked(reason.to_string()));
    }
    bail!("OWNER must be existing:<concept>, new:<concept>, or blocked:<reason>; got {owner:?}");
}

fn boundary_owner_concept_token(owner_suffix: &str) -> &str {
    owner_suffix
        .trim()
        .split(|ch: char| ch.is_ascii_whitespace() || matches!(ch, ',' | ';' | ')' | '('))
        .next()
        .unwrap_or_default()
        .trim()
}

fn apply_boundary_decision(gap: ConceptGap, decision: &BoundaryDecision) -> Result<ConceptGap> {
    let owner = match &decision.owner {
        BoundaryOwner::Existing(concept) | BoundaryOwner::New(concept) => concept,
        BoundaryOwner::Blocked(reason) => {
            bail!("boundary decision blocked concept-grind: {reason}");
        }
    };
    let mut gap = gap;
    if gap.concept != *owner {
        gap.reason = format!("{}; boundary owner changed to {owner}", gap.reason);
        gap.query = owner.replace('_', " ");
        gap.concept = owner.clone();
    }
    Ok(gap)
}

fn required_decision_field(response: &str, field: &str) -> Result<String> {
    optional_decision_field(response, field)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("boundary decision missing {field}: line"))
}

fn optional_decision_field(response: &str, field: &str) -> Option<String> {
    let prefix = format!("{field}:");
    let mut lines = response.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(rest) = line.trim_start().strip_prefix(&prefix) else {
            continue;
        };
        let mut value = rest.trim().to_string();
        while let Some(next) = lines.peek() {
            if decision_field_name(next).is_some() {
                break;
            }
            let next = lines.next().expect("peeked");
            if !value.is_empty() {
                value.push('\n');
            }
            value.push_str(next.trim());
        }
        return Some(value.trim().to_string());
    }
    None
}

fn decision_field_name(line: &str) -> Option<&str> {
    let (field, _) = line.trim_start().split_once(':')?;
    match field.trim() {
        "OWNER"
        | "AXES"
        | "EXAMPLES_TO_ACCEPT"
        | "COUNTEREXAMPLES_TO_REJECT"
        | "PEST_PATCH_INTENT"
        | "WHY_NOT_CARD_PASS" => Some(field.trim()),
        _ => None,
    }
}

fn validate_concept_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("concept name must not be empty");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        bail!("concept name {name:?} must be snake_case ASCII");
    }
    if name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit() || c == '_')
    {
        bail!("concept name {name:?} must start with a lowercase letter");
    }
    Ok(())
}

fn build_patch_prompt(
    gap: &ConceptGap,
    boundary_decision: &BoundaryDecision,
    boundary_response: &str,
) -> Result<String> {
    let concept_path = grammar_concepts_dir().join(format!("{}.toml", gap.concept));
    let fixture_path = grammar_fixtures_dir().join(format!("{}.toml", gap.concept));
    Ok(format!(
        r#"You are the PEST_PATCH agent for mtg-parser's grammar-first concept workflow.

Implement the boundary decision below. This is not an add-card run.

Hard constraints:
- Do not run `cargo xtask add-card`.
- Do not edit `crates/mtg-grammar/tests/generated/` or `crates/mtg-grammar/tests/generated_patterns/`.
- Do not try to make cards pass.
- Prefer widening existing PEST rules over adding one-rule-per-card rules.
- If the target wording is a real axis of `{concept}`, encode it as concept data and grammar fixture coverage, not as a nearby reject.
- Keep edits scoped to `crates/mtg-grammar/src/grammar.pest`, `grammar-concepts/`, and `grammar-fixtures/` unless xtask concept tooling itself is clearly wrong.
- The orchestrator owns validation, maturity, and commit.

Expected files:
- concept file: `{concept_path}`
- fixture file: `{fixture_path}`
- target PEST rule: `{target_rule}`

Boundary decision:
```json
{boundary_json}
```

Raw boundary response:
```text
{boundary_response}
```

Before finishing, ensure the fixture file contains examples that should now match and true counterexamples that should reject.

Return:
CONCEPT_GRIND_RESULT:
CONCEPT: {concept}
PEST_RULES: <roots owned by this concept>
FIXTURE_EXAMPLES: <count>
GRAMMAR_CHANGE: <summary>
BLOCKERS: <none or list>
"#,
        concept = gap.concept,
        concept_path = concept_path.display(),
        fixture_path = fixture_path.display(),
        target_rule = gap.target_rule,
        boundary_json = serde_json::to_string_pretty(boundary_decision)?,
        boundary_response = boundary_response,
    ))
}

fn build_repair_prompt(gap: &ConceptGap, failure: &ConceptGrindGateFailure) -> Result<String> {
    Ok(format!(
        r#"You are the GRAMMAR_FIXTURE_REPAIR agent for mtg-parser's grammar-first concept workflow.

Repair the failed grammar-first gate. Do not run add-card. Do not edit generated tests. Do not try to make cards pass.

Concept: {concept}
Target rule: {target_rule}
Failed gate: {label}

Gate output:
```text
{output}
```

Keep the fix scoped to PEST grammar, grammar concept files, grammar fixtures, or xtask concept tooling if the gate exposes an orchestrator bug.

Return:
CONCEPT_REPAIR_RESULT:
CONCEPT: {concept}
FIX: <summary>
BLOCKERS: <none or list>
"#,
        concept = gap.concept,
        target_rule = gap.target_rule,
        label = failure.label,
        output = failure.output,
    ))
}

fn attempt_no_pest_concept_fastpath(
    gap: &ConceptGap,
    decision: &BoundaryDecision,
    write_files: bool,
) -> Result<NoPestConceptFastpathReport> {
    let concept_path = grammar_concepts_dir().join(format!("{}.toml", gap.concept));
    let fixture_path = grammar_fixtures_dir().join(format!("{}.toml", gap.concept));
    let mut report = NoPestConceptFastpathReport {
        fastpath_attempted: false,
        fastpath_result: "not_eligible".to_string(),
        fallback_reason: None,
        patch_agent_started: true,
        concept: gap.concept.clone(),
        original_target_rule: gap.target_rule.clone(),
        target_rule: gap.target_rule.clone(),
        example_rule: None,
        resolution_reason: None,
        mapped_pest_rules: Vec::new(),
        pest_patch_intent: decision.pest_patch_intent.clone(),
        eligible_shape: false,
        target_rule_exists: false,
        concept_path,
        fixture_path,
        generated_concept: false,
        generated_fixture: false,
        grammar_pest_changed: false,
        quality_contract: None,
    };

    if !matches!(decision.owner, BoundaryOwner::New(_)) {
        report.fallback_reason = Some("boundary owner is not a new concept".into());
        return Ok(report);
    }
    if !decision
        .pest_patch_intent
        .trim()
        .eq_ignore_ascii_case("none")
    {
        report.fallback_reason = Some("PEST_PATCH_INTENT is not none".into());
        return Ok(report);
    }

    let rules = grammar_query_engine::parse_grammar_file(grammar_pest_path())?;
    report.target_rule_exists = rules.iter().any(|rule| rule.name == gap.target_rule);
    if !report.target_rule_exists {
        report.fallback_reason = Some(format!("target rule {} does not exist", gap.target_rule));
        return Ok(report);
    }

    let examples = boundary_text_items(&decision.examples_to_accept);
    let counterexamples = boundary_text_items(&decision.counterexamples_to_reject);
    let axes = boundary_axes(&decision.axes);
    if examples.is_empty() {
        report.fallback_reason = Some("boundary decision has no examples".into());
        return Ok(report);
    }
    if counterexamples.is_empty() {
        report.fallback_reason = Some("boundary decision has no counterexamples".into());
        return Ok(report);
    }
    if axes.is_empty() {
        report.fallback_reason = Some("boundary decision has no axes".into());
        return Ok(report);
    }

    let Some(resolution) = resolve_no_pest_example_rule(&rules, gap, &examples) else {
        report.fallback_reason =
            Some("accepted examples did not resolve to a unique target/root rule".into());
        return Ok(report);
    };
    report.eligible_shape = true;
    report.fastpath_attempted = true;
    report.target_rule = resolution.example_rule.clone();
    report.example_rule = Some(resolution.example_rule.clone());
    report.resolution_reason = Some(resolution.resolution_reason.clone());
    report.mapped_pest_rules = resolution.mapped_pest_rules.clone();

    if !counterexamples_reject_fastpath_rules(&counterexamples, &resolution.mapped_pest_rules) {
        report.fastpath_result = "fallback".to_string();
        report.fallback_reason = Some("one or more counterexamples matched a mapped rule".into());
        return Ok(report);
    }

    let fixture_examples = examples
        .iter()
        .map(|text| FastpathFixtureExample {
            text: text.clone(),
            rule: resolution.example_rule.clone(),
        })
        .collect::<Vec<_>>();
    let grammar_before = fs::read_to_string(grammar_pest_path())
        .with_context(|| format!("read {}", grammar_pest_path().display()))?;
    let concept_doc = build_no_pest_fastpath_concept_document(
        gap,
        &resolution.mapped_pest_rules,
        &axes,
        &examples,
        &counterexamples,
        &resolution.example_rule,
    );
    let fixture_doc = build_no_pest_fastpath_fixture_document(
        gap,
        &resolution.example_rule,
        &fixture_examples,
        &counterexamples,
    );
    if write_files {
        fs::write(&report.concept_path, concept_doc)
            .with_context(|| format!("write {}", report.concept_path.display()))?;
        fs::write(&report.fixture_path, fixture_doc)
            .with_context(|| format!("write {}", report.fixture_path.display()))?;
        report.generated_concept = true;
        report.generated_fixture = true;
    }

    let quality_contract = match run_quality_contract(gap) {
        Ok(report) => report,
        Err(error) => {
            report.fastpath_result = "fallback".to_string();
            report.fallback_reason = Some(format!(
                "fast-path quality contract errored; falling back to patch agent: {error:#}"
            ));
            report.patch_agent_started = true;
            report.grammar_pest_changed = fs::read_to_string(grammar_pest_path())
                .with_context(|| format!("read {}", grammar_pest_path().display()))?
                != grammar_before;
            return Ok(report);
        }
    };
    report.grammar_pest_changed = fs::read_to_string(grammar_pest_path())
        .with_context(|| format!("read {}", grammar_pest_path().display()))?
        != grammar_before;
    let passed = quality_contract.passed
        && quality_contract.maturity_result.state == "grammar_fixture_green"
        && !report.grammar_pest_changed;
    report.quality_contract = Some(quality_contract);
    if passed {
        report.fastpath_result = "success".to_string();
        report.fallback_reason = None;
        report.patch_agent_started = false;
    } else {
        report.fastpath_result = "fallback".to_string();
        report.fallback_reason =
            Some("fast-path quality contract failed; falling back to patch agent".to_string());
        report.patch_agent_started = true;
    }
    Ok(report)
}

#[derive(Debug, Clone)]
struct NoPestRuleResolution {
    example_rule: String,
    resolution_reason: String,
    mapped_pest_rules: Vec<String>,
}

fn resolve_no_pest_example_rule(
    rules: &[grammar_query_engine::GrammarRuleDefinition],
    gap: &ConceptGap,
    examples: &[String],
) -> Option<NoPestRuleResolution> {
    if examples
        .iter()
        .all(|text| parse_pest_rule(&gap.target_rule, text).is_ok())
    {
        return Some(NoPestRuleResolution {
            example_rule: gap.target_rule.clone(),
            resolution_reason: "target_rule_accepts_all_examples".to_string(),
            mapped_pest_rules: vec![gap.target_rule.clone()],
        });
    }

    if examples
        .iter()
        .any(|text| parse_pest_rule(&gap.target_rule, text).is_ok())
    {
        return None;
    }

    let mut candidates = rule_ancestors(rules, &gap.target_rule)
        .into_iter()
        .filter(|rule| semantic_owner_candidate(rule, gap))
        .filter(|rule| {
            examples
                .iter()
                .all(|text| parse_pest_rule(rule, text).is_ok())
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();
    if candidates.len() != 1 {
        return None;
    }
    let example_rule = candidates.remove(0);
    Some(NoPestRuleResolution {
        example_rule: example_rule.clone(),
        resolution_reason: "single_semantic_ancestor_accepts_all_examples".to_string(),
        mapped_pest_rules: vec![example_rule, gap.target_rule.clone()],
    })
}

fn rule_ancestors(
    rules: &[grammar_query_engine::GrammarRuleDefinition],
    target_rule: &str,
) -> Vec<String> {
    let mut ancestors = Vec::new();
    let mut seen = BTreeSet::new();
    let mut stack = grammar_query_engine::reverse_dependencies(rules, target_rule);
    while let Some(rule) = stack.pop() {
        if !seen.insert(rule.clone()) {
            continue;
        }
        ancestors.push(rule.clone());
        stack.extend(grammar_query_engine::reverse_dependencies(rules, &rule));
    }
    ancestors
}

fn semantic_owner_candidate(rule: &str, gap: &ConceptGap) -> bool {
    rule == gap.concept || rule.starts_with(&format!("{}_", gap.concept))
}

fn build_no_pest_fastpath_concept_document(
    gap: &ConceptGap,
    pest_rules: &[String],
    axes: &[ConceptFastpathAxis],
    examples: &[String],
    counterexamples: &[String],
    selected_rule: &str,
) -> String {
    let mut out = String::new();
    for axis in axes {
        out.push_str("[[axis]]\n");
        out.push_str(&format!("name = {:?}\n", axis.name));
        out.push_str(&format!("values = {:?}\n\n", axis.values));
    }
    out.push_str("[boundary]\n");
    out.push_str(&format!(
        "excludes = {:?}\n",
        counterexamples
            .iter()
            .map(|text| format!("Boundary negative: {text}"))
            .collect::<Vec<_>>()
    ));
    out.push_str(&format!(
        "includes = {:?}\n\n",
        examples
            .iter()
            .map(|text| format!("Accepted no-PEST fixture wording: {text}"))
            .collect::<Vec<_>>()
    ));
    out.push_str("[concept]\n");
    out.push_str(&format!("name = {:?}\n", gap.concept));
    out.push_str(&format!("pest_rules = {:?}\n", pest_rules));
    out.push_str(&format!(
        "rules_queries = {:?}\n",
        vec![format!("lex: {}", gap.query)]
    ));
    out.push_str(&format!("rules_terms = {:?}\n\n", query_terms(&gap.query)));
    for text in counterexamples {
        out.push_str("[[counterexample]]\n");
        out.push_str("reason = \"Boundary negative from no-PEST fast-path decision.\"\n");
        out.push_str(&format!("text = {:?}\n\n", text));
    }
    for text in examples {
        out.push_str("[[example]]\n");
        out.push_str(&format!("text = {:?}\n\n", text));
    }
    out.push_str("[grammar_query]\n");
    out.push_str(&format!("candidate_rules = {:?}\n", pest_rules));
    out.push_str(&format!("selected_rule = {:?}\n\n", selected_rule));
    out.push_str("[maturity]\n");
    out.push_str("blockers = []\n");
    out.push_str("pest_grammar = \"bounded\"\n");
    out
}

fn build_no_pest_fastpath_fixture_document(
    gap: &ConceptGap,
    example_rule: &str,
    examples: &[FastpathFixtureExample],
    counterexamples: &[String],
) -> String {
    let mut out = String::new();
    out.push_str("[fixture]\n");
    out.push_str(&format!("concept = {:?}\n", gap.concept));
    out.push_str(&format!("rule = {:?}\n\n", example_rule));
    for example in examples {
        out.push_str("[[example]]\n");
        if example.rule != example_rule {
            out.push_str(&format!("rule = {:?}\n", example.rule));
        }
        out.push_str(&format!("text = {:?}\n", example.text));
        out.push_str("reason = \"Accepted no-PEST boundary example.\"\n\n");
    }
    for text in counterexamples {
        out.push_str("[[counterexample]]\n");
        out.push_str(&format!("text = {:?}\n", text));
        out.push_str("reason = \"Rejected no-PEST boundary counterexample.\"\n\n");
    }
    if let Some(first) = examples.first() {
        out.push_str("[[counterexample]]\n");
        out.push_str(&format!(
            "text = {:?}\n",
            format!("{} trailing", first.text)
        ));
        out.push_str("reason = \"Exact-consumption guard.\"\n");
    }
    out
}

#[derive(Debug, Clone)]
struct FastpathFixtureExample {
    text: String,
    rule: String,
}

fn counterexamples_reject_fastpath_rules(
    counterexamples: &[String],
    pest_rules: &[String],
) -> bool {
    counterexamples.iter().all(|text| {
        pest_rules
            .iter()
            .all(|rule| parse_pest_rule(rule, text).is_err())
    })
}

#[derive(Debug, Clone)]
struct ConceptFastpathAxis {
    name: String,
    values: Vec<String>,
}

fn boundary_axes(text: &str) -> Vec<ConceptFastpathAxis> {
    boundary_text_items(text)
        .into_iter()
        .filter_map(|item| {
            let (name, values) = item
                .split_once('=')
                .or_else(|| item.split_once(':'))
                .map(|(name, values)| (name.trim(), values.trim()))
                .unwrap_or_else(|| (item.trim(), ""));
            let name = slug(name).replace('-', "_");
            if name.is_empty() {
                return None;
            }
            let values = boundary_axis_values(values);
            Some(ConceptFastpathAxis { name, values })
        })
        .collect()
}

fn boundary_axis_values(text: &str) -> Vec<String> {
    let values = text
        .trim()
        .trim_start_matches("add")
        .trim_start_matches("preserve")
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split([',', '|'])
        .map(clean_boundary_item)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if values.is_empty() {
        vec!["present".to_string()]
    } else {
        values
    }
}

fn boundary_text_items(text: &str) -> Vec<String> {
    text.lines()
        .flat_map(|line| line.split(';'))
        .map(clean_boundary_item)
        .filter(|item| !item.is_empty())
        .collect()
}

fn clean_boundary_item(text: &str) -> String {
    text.trim()
        .trim_start_matches('-')
        .trim()
        .trim_matches('"')
        .trim_matches('`')
        .trim()
        .to_string()
}

fn run_quality_contract_with_single_repair(
    gap: &ConceptGap,
    before: &ExistingGrammarMapReport,
    options: &ConceptGrindOptions,
    sink: &mut dyn FlowSink,
    iteration_dir: &Path,
    phase: &str,
) -> Result<ConceptQualityContractReport> {
    let mut report = run_quality_contract(gap)?;
    write_json(
        iteration_dir.join(format!("{phase}_quality_contract.json")),
        &report,
    )?;
    if report.passed {
        return Ok(report);
    }

    let mut failure = quality_contract_failure(&report);
    log_quality_contract_failure(iteration_dir, phase, &failure, false)?;
    sink.emit(FlowEvent::Note {
        level: NoteLevel::Warn,
        text: format!(
            "quality contract failed before {phase}: {}; running one repair attempt",
            failure.concept
        ),
    });

    let gate_failure = quality_contract_gate_failure(&failure)?;
    let repair_prompt = build_repair_prompt(gap, &gate_failure)?;
    fs::write(
        iteration_dir.join(format!("{phase}-quality-contract-repair-prompt.md")),
        &repair_prompt,
    )?;
    let repair = refactor_hotspot::invoke_agent(
        options.agent,
        &repair_prompt,
        &iteration_dir.join(format!("{phase}-quality-contract-repair-transcript.ndjson")),
        sink,
    )?;
    fs::write(
        iteration_dir.join(format!("{phase}-quality-contract-repair-response.md")),
        &repair.assistant_text,
    )?;
    if !repair.success {
        bail!(
            "{} quality-contract repair agent exited with status {}; transcript: {}",
            options.agent.label(),
            repair.exit_code,
            iteration_dir
                .join(format!("{phase}-quality-contract-repair-transcript.ndjson"))
                .display()
        );
    }

    if let Err(failure) = run_concept_grind_gates(gap, before, iteration_dir) {
        fs::write(
            iteration_dir.join(format!("{phase}_quality_contract_repair_gate_failed.txt")),
            format!("{}\n\n{}", failure.label, failure.output),
        )?;
        return Err(anyhow!(
            "quality_contract_failed before {phase}: repair did not preserve existing gate `{}`\n{}",
            failure.label,
            failure.output
        ));
    }

    report = run_quality_contract(gap)?;
    write_json(
        iteration_dir.join(format!("{phase}_quality_contract_after_repair.json")),
        &report,
    )?;
    if report.passed {
        return Ok(report);
    }

    failure = quality_contract_failure(&report);
    log_quality_contract_failure(iteration_dir, phase, &failure, true)?;
    Err(anyhow!(
        "quality_contract_failed before {phase}: concept {} fixture_passed={} maturity_state={}",
        failure.concept,
        failure.fixture_result.passed,
        failure.maturity_result.state
    ))
}

fn run_quality_contract(gap: &ConceptGap) -> Result<ConceptQualityContractReport> {
    let fixture_path = grammar_fixtures_dir().join(format!("{}.toml", gap.concept));
    let fixture_result = run_fixture_file_fresh(&fixture_path).map_err(|failure| {
        anyhow!(
            "{} failed while evaluating quality contract\n{}",
            failure.label,
            failure.output
        )
    })?;
    let maturity_result = run_maturity(MaturityOptions {
        concept: gap.concept.clone(),
        json: false,
        update: false,
        fresh_fixture: true,
    })?;
    let passed = fixture_result.passed && maturity_result.state == "grammar_fixture_green";
    Ok(ConceptQualityContractReport {
        concept: gap.concept.clone(),
        fixture_command: fixture_command_for_log(&fixture_path),
        maturity_command: maturity_command_for_log(&gap.concept, false),
        fixture_result,
        maturity_result,
        passed,
    })
}

fn quality_contract_failure(
    report: &ConceptQualityContractReport,
) -> ConceptQualityContractFailure {
    ConceptQualityContractFailure {
        reason: "quality_contract_failed",
        concept: report.concept.clone(),
        fixture_command: report.fixture_command.clone(),
        maturity_command: report.maturity_command.clone(),
        fixture_result: report.fixture_result.clone(),
        maturity_result: report.maturity_result.clone(),
    }
}

fn persist_quality_contract_maturity(
    report: &ConceptQualityContractReport,
) -> Result<MaturityReport> {
    if !report.passed {
        bail!(
            "refusing to persist failed quality contract for {}",
            report.concept
        );
    }
    let mut maturity = report.maturity_result.clone();
    let concept_file = maturity.concept_file.clone().ok_or_else(|| {
        anyhow!(
            "quality contract passed without a concept file for {}",
            report.concept
        )
    })?;
    write_maturity_to_concept(&concept_file, &maturity.state, &maturity.blockers)?;
    maturity.updated = true;
    Ok(maturity)
}

fn quality_contract_gate_failure(
    failure: &ConceptQualityContractFailure,
) -> Result<ConceptGrindGateFailure> {
    Ok(ConceptGrindGateFailure {
        label: "quality contract".to_string(),
        output: serde_json::to_string_pretty(failure)?,
    })
}

fn log_quality_contract_failure(
    iteration_dir: &Path,
    phase: &str,
    failure: &ConceptQualityContractFailure,
    terminal: bool,
) -> Result<()> {
    let stem = if terminal {
        format!("{phase}_quality_contract_failed")
    } else {
        format!("{phase}_quality_contract_failure")
    };
    write_json(iteration_dir.join(format!("{stem}.json")), failure)?;
    fs::write(
        iteration_dir.join(format!("{stem}.txt")),
        serde_json::to_string_pretty(failure)?,
    )?;
    Ok(())
}

fn fixture_command_for_log(fixture_path: &Path) -> Vec<String> {
    let mut command = vec!["cargo".to_string()];
    command.extend(
        concept_grammar_test_command_args(fixture_path)
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned()),
    );
    command
}

fn maturity_command_for_log(concept: &str, update: bool) -> Vec<String> {
    let mut command = vec![
        "cargo".to_string(),
        "xtask".to_string(),
        "concept-maturity".to_string(),
        concept.to_string(),
        "--json".to_string(),
    ];
    if update {
        command.push("--update".to_string());
    }
    command
}

fn run_concept_grind_gates(
    gap: &ConceptGap,
    before: &ExistingGrammarMapReport,
    iteration_dir: &Path,
) -> Result<(), ConceptGrindGateFailure> {
    let fixture_path = grammar_fixtures_dir().join(format!("{}.toml", gap.concept));
    let fixture = run_fixture_file_fresh(&fixture_path)?;
    write_json(iteration_dir.join("fixture_result.json"), &fixture).map_err(|e| {
        ConceptGrindGateFailure {
            label: "write fixture result".to_string(),
            output: format!("{e:#}"),
        }
    })?;
    if !fixture.passed {
        return Err(ConceptGrindGateFailure {
            label: "grammar fixture".to_string(),
            output: serde_json::to_string_pretty(&fixture).unwrap_or_else(|e| e.to_string()),
        });
    }

    let maturity = run_maturity(MaturityOptions {
        concept: gap.concept.clone(),
        json: false,
        update: false,
        fresh_fixture: true,
    })
    .map_err(|e| ConceptGrindGateFailure {
        label: "concept maturity".to_string(),
        output: format!("{e:#}"),
    })?;
    write_json(iteration_dir.join("maturity_preview.json"), &maturity).map_err(|e| {
        ConceptGrindGateFailure {
            label: "write maturity".to_string(),
            output: format!("{e:#}"),
        }
    })?;
    if maturity.state != "grammar_fixture_green" {
        return Err(ConceptGrindGateFailure {
            label: "concept maturity".to_string(),
            output: serde_json::to_string_pretty(&maturity).unwrap_or_else(|e| e.to_string()),
        });
    }

    let map = run_map_existing(MapExistingOptions {
        json: false,
        expand_deps: true,
    })
    .map_err(|e| ConceptGrindGateFailure {
        label: "grammar map".to_string(),
        output: format!("{e:#}"),
    })?;
    write_json(iteration_dir.join("grammar_debt.json"), &map).map_err(|e| {
        ConceptGrindGateFailure {
            label: "write grammar debt".to_string(),
            output: format!("{e:#}"),
        }
    })?;

    command_gate(
        "cargo check -p xtask",
        "cargo",
        &["check", "-p", "xtask"],
        iteration_dir,
    )?;
    command_gate(
        "cargo test -p mtg-grammar",
        "cargo",
        &["test", "-p", "mtg-grammar"],
        iteration_dir,
    )?;
    run_gap_closure_gate(gap, before, &map)?;
    Ok(())
}

fn run_fixture_file_fresh(
    fixture_path: &Path,
) -> Result<FixtureRunResult, ConceptGrindGateFailure> {
    let args = concept_grammar_test_command_args(fixture_path);
    let output = Command::new("cargo")
        .args(&args)
        .current_dir(repo_root())
        .output()
        .map_err(|e| ConceptGrindGateFailure {
            label: "grammar fixture".to_string(),
            output: format!("{e:#}"),
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let fixture =
        serde_json::from_str::<FixtureRunResult>(&stdout).map_err(|e| ConceptGrindGateFailure {
            label: "grammar fixture".to_string(),
            output: format!(
                "failed to parse concept-grammar-test JSON: {e:#}\n\n{}",
                command_output_text(&output)
            ),
        })?;
    Ok(fixture)
}

fn concept_grammar_test_command_args(fixture_path: &Path) -> Vec<OsString> {
    vec![
        OsString::from("xtask"),
        OsString::from("concept-grammar-test"),
        OsString::from("--json"),
        OsString::from("--fixture"),
        fixture_path.as_os_str().to_owned(),
    ]
}

fn run_maturity_fixture_file(path: &Path, fresh: bool) -> Result<FixtureRunResult> {
    if fresh {
        run_fixture_file_fresh(path).map_err(|failure| {
            anyhow!(
                "{} failed while evaluating concept maturity\n{}",
                failure.label,
                failure.output
            )
        })
    } else {
        run_fixture_file(path)
    }
}

fn run_gap_closure_gate(
    gap: &ConceptGap,
    before: &ExistingGrammarMapReport,
    after: &ExistingGrammarMapReport,
) -> Result<(), ConceptGrindGateFailure> {
    verify_gap_closed(gap, before, after).map_err(|e| ConceptGrindGateFailure {
        label: "gap closure".to_string(),
        output: format!("{e:#}"),
    })
}

fn verify_gap_closed(
    gap: &ConceptGap,
    before: &ExistingGrammarMapReport,
    after: &ExistingGrammarMapReport,
) -> Result<()> {
    if after
        .unmapped_rules
        .iter()
        .any(|rule| rule.name == gap.target_rule)
    {
        bail!(
            "selected gap still unmapped after patch: {} -> {}",
            gap.concept,
            gap.target_rule
        );
    }

    let before_owned = concept_owned_rule_set(before, &gap.concept);
    let after_owned = concept_owned_rule_set(after, &gap.concept);
    if after_owned.is_empty() {
        bail!("concept {:?} has no owned rules after patch", gap.concept);
    }

    let target_still_exists = grammar_query_engine::parse_grammar_file(grammar_pest_path())?
        .iter()
        .any(|rule| rule.name == gap.target_rule);
    let target_owned = after_owned.contains(&gap.target_rule);
    let gained_ownership = after_owned.iter().any(|rule| !before_owned.contains(rule));
    let counts_improved = after.mapped_rule_count > before.mapped_rule_count
        || after.unmapped_rule_count < before.unmapped_rule_count;

    if target_still_exists && !target_owned {
        bail!(
            "target rule {} still exists but is not owned by concept {}",
            gap.target_rule,
            gap.concept
        );
    }
    if !gained_ownership && target_still_exists {
        bail!(
            "concept {} did not gain ownership for target rule {}",
            gap.concept,
            gap.target_rule
        );
    }
    if !counts_improved {
        bail!(
            "grammar map did not improve: mapped {} -> {}, unmapped {} -> {}",
            before.mapped_rule_count,
            after.mapped_rule_count,
            before.unmapped_rule_count,
            after.unmapped_rule_count
        );
    }

    Ok(())
}

fn concept_owned_rule_set(report: &ExistingGrammarMapReport, concept: &str) -> BTreeSet<String> {
    report
        .concepts
        .iter()
        .find(|entry| entry.concept == concept)
        .map(|entry| {
            entry
                .owned_rules
                .iter()
                .map(|rule| rule.name.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn command_gate(
    label: &str,
    program: &str,
    args: &[&str],
    iteration_dir: &Path,
) -> Result<(), ConceptGrindGateFailure> {
    let output = Command::new(program)
        .args(args)
        .current_dir(repo_root())
        .output()
        .map_err(|e| ConceptGrindGateFailure {
            label: label.to_string(),
            output: format!("{e:#}"),
        })?;
    let text = command_output_text(&output);
    let log_name = label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>();
    fs::write(iteration_dir.join(format!("{log_name}.txt")), &text).map_err(|e| {
        ConceptGrindGateFailure {
            label: format!("write {label} output"),
            output: format!("{e:#}"),
        }
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ConceptGrindGateFailure {
            label: label.to_string(),
            output: text,
        })
    }
}

fn command_output_text(output: &std::process::Output) -> String {
    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    if !output.stderr.is_empty() {
        if !text.ends_with('\n') && !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    text
}

fn ensure_clean_working_tree() -> Result<()> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_root())
        .output()
        .context("git status --porcelain")?;
    if !output.status.success() {
        bail!("git status failed\n{}", command_output_text(&output));
    }
    if !String::from_utf8_lossy(&output.stdout).trim().is_empty() {
        bail!("working tree has uncommitted changes");
    }
    Ok(())
}

const CONCEPT_GRIND_COMMIT_PATHS: &[&str] = &[
    "crates/mtg-grammar/src/grammar.pest",
    "grammar-concepts",
    "grammar-fixtures",
];

const PHASE2_GRIND_COMMIT_PATHS: &[&str] = &[
    "ast-fixtures",
    "crates/mtg-grammar/src/ast.rs",
    "crates/mtg-grammar/src/grammar.pest",
    "crates/mtg-grammar/src/parse.rs",
    "crates/mtg-grammar/src/unparse.rs",
    "grammar-fixtures",
];

fn run_phase2_concept_gates(concept: &str, iteration_dir: &Path) -> Result<()> {
    run_phase2_gate_command(
        "concept-parse",
        "cargo",
        &["xtask", "concept-parse", concept],
        iteration_dir,
    )?;
    run_phase2_gate_command(
        "concept-ast-test-update",
        "cargo",
        &["xtask", "concept-ast-test", concept, "--update"],
        iteration_dir,
    )?;
    run_phase2_gate_command(
        "concept-ast-test",
        "cargo",
        &["xtask", "concept-ast-test", concept],
        iteration_dir,
    )?;
    run_phase2_gate_command(
        "cargo-check-xtask",
        "cargo",
        &["check", "-p", "xtask"],
        iteration_dir,
    )?;
    Ok(())
}

fn run_phase2_gate_command(
    label: &str,
    program: &str,
    args: &[&str],
    iteration_dir: &Path,
) -> Result<()> {
    let output = Command::new(program)
        .args(args)
        .current_dir(repo_root())
        .output()
        .with_context(|| format!("{program} {}", args.join(" ")))?;
    let text = command_output_text(&output);
    fs::write(iteration_dir.join(format!("{label}.txt")), &text)
        .with_context(|| format!("write {label} output"))?;
    if output.status.success() {
        Ok(())
    } else {
        bail!("{label} failed\n{text}")
    }
}

fn commit_phase2_grind_iteration(concept: &str, iteration: u32) -> Result<bool> {
    validate_phase2_grind_changed_paths()?;
    let add = Command::new("git")
        .arg("add")
        .args(PHASE2_GRIND_COMMIT_PATHS)
        .current_dir(repo_root())
        .output()
        .context("git add phase2-grind paths")?;
    if !add.status.success() {
        bail!("git add failed\n{}", command_output_text(&add));
    }
    validate_phase2_grind_cached_paths()?;
    let diff = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(repo_root())
        .status()
        .context("git diff --cached --quiet")?;
    if diff.success() {
        return Ok(false);
    }
    let message = format!(
        "Advance Phase 2 concept {concept}\n\nconcept-phase2-grind iteration: {iteration}\n"
    );
    let commit = Command::new("git")
        .args(["commit", "--no-verify", "-m", &message])
        .current_dir(repo_root())
        .output()
        .context("git commit")?;
    if !commit.status.success() {
        bail!("git commit failed\n{}", command_output_text(&commit));
    }
    Ok(true)
}

fn validate_phase2_grind_changed_paths() -> Result<()> {
    let paths = git_changed_paths(&["status", "--porcelain"])?;
    let unexpected: Vec<String> = paths
        .into_iter()
        .filter(|path| !is_phase2_grind_commit_path(path))
        .collect();
    if !unexpected.is_empty() {
        bail!(
            "concept-phase2-grind refuses to commit unexpected changed path(s): {}",
            unexpected.join(", ")
        );
    }
    Ok(())
}

fn validate_phase2_grind_cached_paths() -> Result<()> {
    let paths = git_changed_paths(&["diff", "--cached", "--name-only"])?;
    let unexpected: Vec<String> = paths
        .into_iter()
        .filter(|path| !is_phase2_grind_commit_path(path))
        .collect();
    if !unexpected.is_empty() {
        bail!(
            "concept-phase2-grind staged unexpected path(s): {}",
            unexpected.join(", ")
        );
    }
    Ok(())
}

fn is_phase2_grind_commit_path(path: &str) -> bool {
    PHASE2_GRIND_COMMIT_PATHS
        .iter()
        .any(|allowed| path == *allowed || path.starts_with(&format!("{allowed}/")))
}

fn commit_concept_grind_iteration(gap: &ConceptGap, iteration: u32) -> Result<bool> {
    validate_concept_grind_changed_paths()?;
    let add = Command::new("git")
        .arg("add")
        .args(CONCEPT_GRIND_COMMIT_PATHS)
        .current_dir(repo_root())
        .output()
        .context("git add concept-grind paths")?;
    if !add.status.success() {
        bail!("git add failed\n{}", command_output_text(&add));
    }
    validate_concept_grind_cached_paths()?;
    let diff = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(repo_root())
        .status()
        .context("git diff --cached --quiet")?;
    if diff.success() {
        return Ok(false);
    }
    let message = format!(
        "Advance grammar concept {}\n\nconcept-grind iteration: {iteration}\ntarget rule: {}\nquery: {}\n",
        gap.concept, gap.target_rule, gap.query
    );
    let commit = Command::new("git")
        .args(["commit", "--no-verify", "-m", &message])
        .current_dir(repo_root())
        .output()
        .context("git commit")?;
    if !commit.status.success() {
        bail!("git commit failed\n{}", command_output_text(&commit));
    }
    Ok(true)
}

fn validate_concept_grind_changed_paths() -> Result<()> {
    let paths = git_changed_paths(&["status", "--porcelain"])?;
    let unexpected: Vec<String> = paths
        .into_iter()
        .filter(|path| !is_concept_grind_commit_path(path))
        .collect();
    if !unexpected.is_empty() {
        bail!(
            "concept-grind refuses to commit unexpected changed path(s): {}",
            unexpected.join(", ")
        );
    }
    Ok(())
}

fn ensure_no_grammar_pest_worktree_diff() -> Result<()> {
    let status = Command::new("git")
        .args([
            "diff",
            "--quiet",
            "--",
            "crates/mtg-grammar/src/grammar.pest",
        ])
        .current_dir(repo_root())
        .status()
        .context("git diff --quiet -- crates/mtg-grammar/src/grammar.pest")?;
    if status.success() {
        Ok(())
    } else {
        bail!("fast-path commits must not include a grammar.pest diff")
    }
}

fn validate_concept_grind_cached_paths() -> Result<()> {
    let paths = git_changed_paths(&["diff", "--cached", "--name-only"])?;
    let unexpected: Vec<String> = paths
        .into_iter()
        .filter(|path| !is_concept_grind_commit_path(path))
        .collect();
    if !unexpected.is_empty() {
        bail!(
            "concept-grind staged unexpected path(s): {}",
            unexpected.join(", ")
        );
    }
    Ok(())
}

fn git_changed_paths(args: &[&str]) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root())
        .output()
        .with_context(|| format!("git {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "git {} failed\n{}",
            args.join(" "),
            command_output_text(&output)
        );
    }
    let mut paths = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let path = if args.first() == Some(&"status") {
            line.get(3..).unwrap_or("").trim()
        } else {
            line.trim()
        };
        let path = path
            .rsplit_once(" -> ")
            .map(|(_, right)| right)
            .unwrap_or(path);
        if !path.is_empty() {
            paths.push(path.to_string());
        }
    }
    Ok(paths)
}

fn is_concept_grind_commit_path(path: &str) -> bool {
    CONCEPT_GRIND_COMMIT_PATHS
        .iter()
        .any(|allowed| path == *allowed || path.starts_with(&format!("{allowed}/")))
}

fn read_concept_files() -> Result<Vec<(PathBuf, ConceptDocument, String)>> {
    let dir = grammar_concepts_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry.with_context(|| format!("read entry in {}", dir.display()))?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            paths.push(path);
        }
    }
    paths.sort();

    let mut docs = Vec::new();
    for path in paths {
        let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let doc: ConceptDocument =
            toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        let maturity = concept_maturity_from_toml(&text).unwrap_or_else(|| "unknown".to_string());
        docs.push((path, doc, maturity));
    }
    Ok(docs)
}

fn concept_maturity_from_toml(text: &str) -> Option<String> {
    let value: toml::Value = toml::from_str(text).ok()?;
    value
        .get("maturity")?
        .get("pest_grammar")?
        .as_str()
        .map(str::to_string)
}

fn suggest_concept_owner(rule_name: &str, concepts: &[String]) -> Option<String> {
    concepts
        .iter()
        .filter_map(|concept| {
            let score = shared_prefix_segments(rule_name, concept);
            (score > 0).then_some((score, concept))
        })
        .max_by(|(left_score, left), (right_score, right)| {
            left_score
                .cmp(right_score)
                .then_with(|| right.len().cmp(&left.len()))
        })
        .map(|(_, concept)| concept.clone())
}

fn shared_prefix_segments(rule_name: &str, concept: &str) -> usize {
    rule_name
        .split('_')
        .zip(concept.split('_'))
        .take_while(|(left, right)| left == right)
        .count()
}

fn print_map_existing_report(report: &ExistingGrammarMapReport) {
    println!("grammar rules : {}", report.rule_count);
    println!("concepts      : {}", report.concept_count);
    println!(
        "dependency map: {}",
        if report.dependency_expansion {
            "expanded"
        } else {
            "declared only"
        }
    );
    println!("shared rules  : {}", report.shared_rule_count);
    println!("mapped rules  : {}", report.mapped_rule_count);
    println!("unmapped rules: {}", report.unmapped_rule_count);
    for concept in &report.concepts {
        println!(
            "  {} [{}]: {} root(s), {} owned, {} missing",
            concept.concept,
            concept.maturity,
            concept.found_rules.len(),
            concept.owned_rules.len(),
            concept.missing_rules.len()
        );
        if !concept.owned_rules.is_empty() {
            let owned = concept
                .owned_rules
                .iter()
                .take(12)
                .map(|rule| rule.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let suffix = if concept.owned_rules.len() > 12 {
                ", ..."
            } else {
                ""
            };
            println!("    owned: {owned}{suffix}");
        }
        if !concept.missing_rules.is_empty() {
            println!("    missing: {}", concept.missing_rules.join(", "));
        }
    }
    let suggested = report
        .unmapped_rules
        .iter()
        .filter(|rule| rule.suggested_concept.is_some())
        .count();
    println!("unmapped with concept-name suggestion: {suggested}");
    for rule in report.unmapped_rules.iter().take(20) {
        match &rule.suggested_concept {
            Some(concept) => println!("  {}:{} -> {}", rule.name, rule.line, concept),
            None => println!("  {}:{}", rule.name, rule.line),
        }
    }
}

fn run_maturity(options: MaturityOptions) -> Result<MaturityReport> {
    let concept_file = grammar_concepts_dir().join(format!("{}.toml", options.concept));
    let fixture_file = grammar_fixtures_dir().join(format!("{}.toml", options.concept));
    let concept_file = concept_file.exists().then_some(concept_file);
    let fixture_file = fixture_file.exists().then_some(fixture_file);
    let mut blockers = Vec::new();

    let concept_valid = match &concept_file {
        Some(path) => {
            let concept_blockers = validate_concept_file(path, &options.concept)?;
            let valid = concept_blockers.is_empty();
            blockers.extend(concept_blockers);
            valid
        }
        None => {
            blockers.push("missing grammar-concepts/<concept>.toml".to_string());
            false
        }
    };
    if options.update && concept_file.is_none() {
        blockers.push("cannot update maturity without a concept file".to_string());
    }
    let fixture_result = match &fixture_file {
        Some(path) => match run_maturity_fixture_file(path, options.fresh_fixture) {
            Ok(result) => {
                if !result.passed {
                    blockers.push(format!(
                        "{} grammar fixture case(s) failed",
                        result.failures
                    ));
                }
                Some(result)
            }
            Err(e) => {
                blockers.push(format!("fixture could not run: {e:#}"));
                None
            }
        },
        None => {
            blockers.push("missing grammar-fixtures/<concept>.toml".to_string());
            None
        }
    };

    let state = if concept_valid && fixture_result.as_ref().is_some_and(|r| r.passed) {
        "grammar_fixture_green"
    } else if concept_valid {
        "bounded"
    } else if concept_file.is_some() {
        "blocked"
    } else {
        "discovered"
    }
    .to_string();

    let updated = if options.update {
        if let Some(path) = &concept_file {
            write_maturity_to_concept(path, &state, &blockers)?;
            true
        } else {
            false
        }
    } else {
        false
    };

    Ok(MaturityReport {
        concept: options.concept,
        state,
        concept_file,
        fixture_file,
        blockers,
        updated,
        fixture_result,
    })
}

fn validate_concept_file(path: &Path, expected_name: &str) -> Result<Vec<String>> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let doc: ConceptDocument =
        toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    let mut blockers = Vec::new();
    if doc.concept.name != expected_name {
        blockers.push(format!(
            "concept.name is {:?}, expected {:?}",
            doc.concept.name, expected_name
        ));
    }
    if doc.concept.rules_terms.is_empty() && doc.concept.rules_queries.is_empty() {
        blockers.push("concept must include rules_terms or rules_queries".to_string());
    }
    match &doc.boundary {
        Some(boundary) => {
            if boundary.includes.is_empty() {
                blockers.push("boundary.includes must not be empty".to_string());
            }
            if boundary.excludes.is_empty() {
                blockers.push("boundary.excludes must not be empty".to_string());
            }
        }
        None => blockers.push("missing [boundary] section".to_string()),
    }
    if doc.axis.is_empty() {
        blockers.push("at least one [[axis]] is required".to_string());
    }
    for axis in &doc.axis {
        if axis.name.trim().is_empty() {
            blockers.push("axis name must not be empty".to_string());
        }
        if axis.values.is_empty() && axis.evidence.is_empty() {
            blockers.push(format!(
                "axis {:?} must include values or evidence",
                axis.name
            ));
        }
    }
    if doc.example.is_empty() {
        blockers.push("at least one [[example]] is required".to_string());
    }
    if doc.counterexample.is_empty() {
        blockers.push("at least one [[counterexample]] is required".to_string());
    }
    for example in &doc.example {
        if example.text.trim().is_empty() {
            blockers.push("example text must not be empty".to_string());
        }
    }
    for counterexample in &doc.counterexample {
        if counterexample.text.trim().is_empty() {
            blockers.push("counterexample text must not be empty".to_string());
        }
    }
    Ok(blockers)
}

fn write_maturity_to_concept(path: &Path, state: &str, blockers: &[String]) -> Result<()> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut value: toml::Value =
        toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    let root = value
        .as_table_mut()
        .ok_or_else(|| anyhow!("{} must be a TOML table", path.display()))?;
    let maturity = root
        .entry("maturity")
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .ok_or_else(|| anyhow!("[maturity] in {} must be a table", path.display()))?;
    maturity.insert(
        "pest_grammar".to_string(),
        toml::Value::String(state.to_string()),
    );
    maturity.insert(
        "blockers".to_string(),
        toml::Value::Array(
            blockers
                .iter()
                .map(|blocker| toml::Value::String(blocker.clone()))
                .collect(),
        ),
    );
    let updated = toml::to_string_pretty(&value).context("serialize concept TOML")?;
    fs::write(path, updated).with_context(|| format!("write {}", path.display()))
}

fn choose_fixture_rule(
    concept: &str,
    explicit_rule: Option<&str>,
    report: &grammar_query_engine::GrammarQueryReport,
) -> Result<String> {
    if let Some(rule) = explicit_rule {
        if report
            .candidates
            .iter()
            .any(|candidate| candidate.name == rule)
        {
            return Ok(rule.to_string());
        }
        bail!("--rule {rule:?} was not found in grammar query candidates");
    }
    if report
        .candidates
        .iter()
        .any(|candidate| candidate.name == concept)
    {
        return Ok(concept.to_string());
    }
    report
        .candidates
        .iter()
        .find(|candidate| candidate.name.ends_with(concept) || candidate.name.contains(concept))
        .or_else(|| report.candidates.first())
        .map(|candidate| candidate.name.clone())
        .ok_or_else(|| anyhow!("grammar query returned no candidate rules"))
}

fn build_fixture_document(
    concept: &str,
    rule: &str,
    corpus: &CorpusClusterArtifact,
) -> Result<String> {
    let mut examples = Vec::new();
    let mut nearby_rejects = Vec::new();
    for example in &corpus.examples {
        if parse_pest_rule(rule, &example.clause).is_ok() {
            examples.push(example.clause.clone());
        } else {
            nearby_rejects.push(example.clause.clone());
        }
    }
    examples.sort();
    examples.dedup();
    nearby_rejects.sort();
    nearby_rejects.dedup();
    if examples.is_empty() {
        bail!("no corpus examples matched selected rule {rule:?}");
    }

    let mut out = String::new();
    out.push_str("[fixture]\n");
    out.push_str(&format!("concept = {:?}\n", concept));
    out.push_str(&format!("rule = {:?}\n\n", rule));
    for text in examples.iter().take(8) {
        out.push_str("[[example]]\n");
        out.push_str(&format!("text = {:?}\n\n", text));
    }
    out.push_str("[[counterexample]]\n");
    out.push_str("text = \"Destroy target creature.\"\n");
    out.push_str("reason = \"Baseline unrelated effect.\"\n\n");
    if let Some(first) = examples.first() {
        out.push_str("[[counterexample]]\n");
        out.push_str(&format!("text = {:?}\n", format!("{first} trailing")));
        out.push_str("reason = \"Exact-consumption guard.\"\n\n");
    }
    for text in nearby_rejects.iter().take(8) {
        out.push_str("[[counterexample]]\n");
        out.push_str(&format!("text = {:?}\n", text));
        out.push_str(
            "reason = \"Nearby corpus wording not accepted by the selected grammar rule.\"\n\n",
        );
    }
    Ok(out)
}

fn write_grown_concept_file(
    path: &Path,
    concept: &str,
    query: &str,
    rule: &str,
    corpus: &CorpusClusterArtifact,
    axes: &AxisArtifact,
    grammar_report: &grammar_query_engine::GrammarQueryReport,
) -> Result<()> {
    let matched_examples: Vec<&CorpusExample> = corpus
        .examples
        .iter()
        .filter(|example| parse_pest_rule(rule, &example.clause).is_ok())
        .collect();
    let rejected_nearby: Vec<&CorpusExample> = corpus
        .examples
        .iter()
        .filter(|example| parse_pest_rule(rule, &example.clause).is_err())
        .collect();
    let mut out = String::new();
    out.push_str("[concept]\n");
    out.push_str(&format!("name = {:?}\n", concept));
    out.push_str(&format!("rules_terms = {:?}\n", query_terms(query)));
    out.push_str(&format!(
        "rules_queries = {:?}\n",
        vec![format!("lex: {query}")]
    ));
    out.push_str(&format!("pest_rules = {:?}\n\n", vec![rule.to_string()]));

    out.push_str("[boundary]\n");
    out.push_str(&format!(
        "includes = {:?}\n",
        vec![
            format!("Corpus clauses matching the selected PEST rule {rule}."),
            "Rules-grounded wording discovered from qmd and Oracle corpus clustering.".to_string(),
        ]
    ));
    out.push_str(&format!(
        "excludes = {:?}\n\n",
        vec![
            "Nearby corpus clauses that do not match the selected PEST rule.".to_string(),
            "Parser, unparser, lowering, generated card tests, and corpus pass status.".to_string(),
        ]
    ));

    for axis in &axes.axes {
        out.push_str("[[axis]]\n");
        out.push_str(&format!("name = {:?}\n", axis.name));
        out.push_str(&format!("evidence = {:?}\n\n", axis.evidence));
    }
    if axes.axes.is_empty() {
        out.push_str("[[axis]]\n");
        out.push_str("name = \"wording\"\n");
        out.push_str(&format!("evidence = {:?}\n\n", query_terms(query)));
    }

    for example in matched_examples.iter().take(8) {
        out.push_str("[[example]]\n");
        out.push_str(&format!("card = {:?}\n", example.card));
        out.push_str(&format!("text = {:?}\n\n", example.clause));
    }
    out.push_str("[[counterexample]]\n");
    out.push_str("text = \"Destroy target creature.\"\n");
    out.push_str("reason = \"Baseline unrelated effect.\"\n\n");
    for example in rejected_nearby.iter().take(8) {
        out.push_str("[[counterexample]]\n");
        out.push_str(&format!("card = {:?}\n", example.card));
        out.push_str(&format!("text = {:?}\n", example.clause));
        out.push_str(
            "reason = \"Nearby corpus wording not accepted by the selected grammar rule.\"\n\n",
        );
    }

    out.push_str("[maturity]\n");
    out.push_str("pest_grammar = \"bounded\"\n");
    out.push_str("blockers = []\n\n");

    out.push_str("[grammar_query]\n");
    out.push_str(&format!("selected_rule = {:?}\n", rule));
    out.push_str(&format!(
        "candidate_rules = {:?}\n",
        grammar_report
            .candidates
            .iter()
            .map(|candidate| candidate.name.clone())
            .take(10)
            .collect::<Vec<_>>()
    ));
    fs::write(path, out).with_context(|| format!("write {}", path.display()))
}

fn build_grammar_query_report(
    query: &str,
    rule_names: Vec<String>,
    limit: usize,
) -> Result<grammar_query_engine::GrammarQueryReport> {
    let rules = grammar_query_engine::parse_grammar_file(grammar_pest_path())?;
    let mut report = grammar_query_engine::grammar_query_report(
        &rules,
        &GrammarQuery {
            query: query.to_string(),
            terms: query_terms(query),
            rule_names,
            max_candidates: Some(limit),
        },
    );
    report.duplicate_rhs_shapes.truncate(limit);
    report.similar_rhs_shapes.truncate(limit);
    Ok(report)
}

fn print_grammar_query_report(report: &grammar_query_engine::GrammarQueryReport) {
    println!("query     : {}", report.query);
    println!("rules     : {}", report.rule_count);
    println!("terms     : {}", report.terms.join(", "));
    println!("candidates: {}", report.candidates.len());
    for candidate in &report.candidates {
        println!("  {}:{}", candidate.name, candidate.line);
        if !candidate.matched_by.is_empty() {
            println!("    matched by: {}", candidate.matched_by.join(", "));
        }
        if !candidate.direct_dependencies.is_empty() {
            println!(
                "    deps      : {}",
                candidate.direct_dependencies.join(", ")
            );
        }
        if !candidate.reverse_dependencies.is_empty() {
            println!(
                "    reverse   : {}",
                candidate.reverse_dependencies.join(", ")
            );
        }
    }
    let quantity_groups = report
        .duplicate_rhs_shapes
        .iter()
        .filter(|group| group.quantity_like)
        .count();
    println!(
        "duplicate RHS shape groups: {} ({} quantity-like)",
        report.duplicate_rhs_shapes.len(),
        quantity_groups
    );
    println!(
        "similar RHS shape pairs   : {}",
        report.similar_rhs_shapes.len()
    );
}

fn build_corpus_cluster(
    query: &str,
    sets: &[String],
    limit: usize,
) -> Result<CorpusClusterArtifact> {
    let query_terms = query_terms(query);
    let client = ScryfallClient::new()?;
    let mut total_cards_scanned = 0usize;
    let mut total_matching_clauses = 0usize;
    let mut examples = Vec::new();
    let mut skeleton_counts: BTreeMap<String, usize> = BTreeMap::new();

    for set in sets {
        let cards = client
            .cards_in_set(set)
            .with_context(|| format!("fetch set {set}"))?;
        for card in cards {
            total_cards_scanned += 1;
            let normalized = normalize_oracle_text(&card.oracle_text);
            for clause in oracle_clauses(&normalized) {
                if !matches_terms(&clause, &query_terms) {
                    continue;
                }
                total_matching_clauses += 1;
                let skeleton = skeletonize_clause(&clause);
                *skeleton_counts.entry(skeleton.clone()).or_default() += 1;
                if examples.len() < limit {
                    examples.push(CorpusExample {
                        card: card.name.clone(),
                        set: card.set_code.clone(),
                        collector_number: card.collector_number.clone(),
                        clause,
                        skeleton,
                    });
                }
            }
        }
    }

    let mut skeletons: Vec<SkeletonCount> = skeleton_counts
        .into_iter()
        .map(|(skeleton, count)| SkeletonCount { skeleton, count })
        .collect();
    skeletons.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.skeleton.cmp(&b.skeleton))
    });
    skeletons.truncate(limit);

    Ok(CorpusClusterArtifact {
        query: query.to_string(),
        sets: sets.to_vec(),
        query_terms,
        total_cards_scanned,
        total_matching_clauses,
        examples,
        skeletons,
    })
}

fn run_fixture_file(path: &Path) -> Result<FixtureRunResult> {
    let doc = read_fixture_document(path)?;
    let mut cases = Vec::new();

    for case in &doc.example {
        cases.push(run_fixture_case(
            "example",
            &doc.fixture.rule,
            case,
            "match",
        ));
    }
    for case in &doc.counterexample {
        cases.push(run_fixture_case(
            "counterexample",
            &doc.fixture.rule,
            case,
            "reject",
        ));
    }

    let failures = cases.iter().filter(|case| !case.passed).count();
    let grammar_drift = grammar_drift_summary()?;
    Ok(FixtureRunResult {
        concept: doc.fixture.concept,
        fixture_path: path.to_path_buf(),
        passed: failures == 0,
        total: cases.len(),
        failures,
        grammar_drift,
        cases,
    })
}

fn run_fixture_case(
    kind: &'static str,
    default_rule: &str,
    case: &FixtureCase,
    default_expect: &str,
) -> FixtureCaseResult {
    let rule = case.rule.as_deref().unwrap_or(default_rule).to_string();
    let expected = case.expect.as_deref().unwrap_or(default_expect).to_string();
    let parse_result = parse_pest_rule(&rule, &case.text);
    let matched = parse_result.is_ok();
    let passed = match expected.as_str() {
        "match" => matched,
        "reject" => !matched,
        _ => false,
    };
    FixtureCaseResult {
        kind: kind.to_string(),
        rule,
        text: case.text.clone(),
        expected,
        matched,
        passed,
        reason: case.reason.clone(),
        error: parse_result.err().map(|e| e.to_string()),
    }
}

fn print_fixture_report(result: &FixtureRunResult) {
    println!("concept: {}", result.concept);
    println!("fixture: {}", result.fixture_path.display());
    println!(
        "result : {} ({} case(s), {} failure(s))",
        if result.passed { "pass" } else { "fail" },
        result.total,
        result.failures
    );
    println!(
        "drift  : {} duplicate RHS group(s), {} quantity-like, {} similar pair(s)",
        result.grammar_drift.duplicate_rhs_shape_groups,
        result
            .grammar_drift
            .quantity_like_duplicate_rhs_shape_groups,
        result.grammar_drift.similar_rhs_shape_pairs
    );
    for case in &result.cases {
        let status = if case.passed { "ok" } else { "FAIL" };
        println!(
            "  {status} {} rule={} expected={} matched={} text={:?}",
            case.kind, case.rule, case.expected, case.matched, case.text
        );
        if !case.passed {
            if let Some(error) = &case.error {
                println!("    error: {error}");
            }
        }
    }
}

fn print_parse_report(report: &ParseReport) {
    println!("concept: {}", report.concept);
    println!("fixture: {}", report.fixture_path.display());
    println!(
        "phase2 parse: {} ({} accepted example(s), {} failure(s))",
        if report.passed { "pass" } else { "fail" },
        report.total,
        report.failures
    );
    for case in &report.cases {
        if case.parsed {
            println!(
                "  ok example #{} rule={} text={:?}",
                case.index, case.rule, case.text
            );
        } else {
            println!(
                "  FAIL example #{} rule={} text={:?}",
                case.index, case.rule, case.text
            );
            if let Some(error) = &case.error {
                println!("    error: {error}");
            }
        }
    }
}

fn print_ast_report(report: &AstTestReport) {
    println!("concept: {}", report.concept);
    println!("fixture: {}", report.fixture_path.display());
    println!("snapshot: {}", report.snapshot_path.display());
    println!(
        "phase2 ast: {}{} ({} case(s), {} failure(s))",
        if report.passed { "pass" } else { "fail" },
        if report.updated { " [updated]" } else { "" },
        report.total,
        report.failures
    );
    for case in &report.cases {
        if case.matched {
            println!(
                "  ok example #{} rule={} text={:?}",
                case.index, case.rule, case.text
            );
        } else {
            println!(
                "  FAIL example #{} rule={} text={:?}",
                case.index, case.rule, case.text
            );
            if let Some(error) = &case.error {
                println!("    error: {error}");
            }
        }
    }
}

fn print_phase2_map_report(report: &Phase2MapReport) {
    println!("phase2 denominator: grammar_fixture_green concepts");
    println!("concept files      : {}", report.total_concepts);
    println!("grammar green      : {}", report.grammar_green_concepts);
    println!(
        "parse green        : {} / {} concept(s), {} / {} accepted example(s)",
        report.parse_green_concepts,
        report.grammar_green_concepts,
        report.parsed_examples,
        report.total_accepted_examples
    );
    println!(
        "ast snapshots green: {} / {} concept(s), {} accepted example(s)",
        report.ast_green_concepts, report.grammar_green_concepts, report.ast_snapshot_examples
    );
    println!("parse failed       : {}", report.parse_failed_concepts);
    println!("missing snapshots  : {}", report.missing_snapshot_concepts);
    println!("ast failed         : {}", report.ast_failed_concepts);
    println!("missing fixtures   : {}", report.missing_fixture_concepts);

    for status in &report.concepts {
        let state = match status.ast_status {
            Phase2AstStatus::Pass => "ast_pass",
            Phase2AstStatus::MissingSnapshot => "missing_snapshot",
            Phase2AstStatus::Fail => "ast_fail",
            Phase2AstStatus::ParseFailed => "parse_fail",
            Phase2AstStatus::MissingFixture => "missing_fixture",
            Phase2AstStatus::NotGrammarGreen => "not_grammar_green",
        };
        println!(
            "  {} [{}]: {} ({} accepted, {} parse failure(s))",
            status.concept, status.maturity, state, status.accepted_examples, status.parse_failures
        );
        if let Some(error) = &status.first_error {
            println!("    first error: {error}");
        }
    }
}

fn print_phase_status_report(report: &PhaseStatusReport) {
    println!("verdict: {:?}", report.verdict);
    println!(
        "phase2: ast green {}/{}, parse green {}/{}, ast failed {}, parse failed {}",
        report.current.ast_green_concepts,
        report.current.grammar_green_concepts,
        report.current.parse_green_concepts,
        report.current.grammar_green_concepts,
        report.current.ast_failed_concepts,
        report.current.parse_failed_concepts
    );
    if report.running_processes.is_empty() {
        println!("running: no");
    } else {
        println!("running: yes");
        for process in &report.running_processes {
            println!("  {process}");
        }
    }
    if let Some(path) = &report.latest_batch_summary_path {
        println!("latest batch: {}", path.display());
    }
    if let Some(batch) = &report.latest_batch {
        println!(
            "batch {}: ast {:+}, parse {:+}, commits {}",
            batch.batch, batch.ast_green_delta, batch.parse_green_delta, batch.commits
        );
        if !batch.concepts.is_empty() {
            println!("concepts: {}", batch.concepts.join(", "));
        }
        if !batch.added_statement_variants.is_empty() {
            println!(
                "new Statement variants: {}",
                batch.added_statement_variants.join(", ")
            );
        }
    }
    if report.reasons.is_empty() {
        println!("reasons: none");
    } else {
        println!("reasons:");
        for reason in &report.reasons {
            println!("  - {reason}");
        }
    }
}

fn print_roadmap_report(report: &RoadmapReport) {
    println!("roadmap denominator: rulebook-derived concept candidates");
    println!("total candidates   : {}", report.total_candidates);
    println!("  701 actions      : {}", report.action_candidates);
    println!("  702 abilities    : {}", report.ability_candidates);
    println!("  600 effects      : {}", report.effect_candidates);
    println!("exact concepts     : {}", report.exact_concept_matches);
    println!("mentioned concepts : {}", report.mentioned_by_concept);
    println!("missing candidates : {}", report.missing_candidates);
    println!("grammar green      : {}", report.grammar_green_candidates);
    println!("parse green        : {}", report.parse_green_candidates);
    println!("ast green          : {}", report.ast_green_candidates);
    println!(
        "corpus pressure    : {} candidate(s), {} failure hit(s)",
        report.candidates_with_corpus_failures, report.total_corpus_failure_hits
    );
    println!();
    println!("top candidates by current corpus failure pressure:");
    for candidate in report
        .candidates
        .iter()
        .filter(|candidate| candidate.corpus_failure_hits > 0)
        .take(30)
    {
        let kind = match candidate.kind {
            RoadmapCandidateKind::KeywordAction => "701 action",
            RoadmapCandidateKind::KeywordAbility => "702 ability",
            RoadmapCandidateKind::EffectFamily => "600 effect",
        };
        let coverage = match candidate.coverage {
            RoadmapCoverage::ExactConcept => "exact",
            RoadmapCoverage::MentionedByConcept => "mentioned",
            RoadmapCoverage::Missing => "missing",
        };
        println!(
            "  {} {} {} [{}]: {} corpus failure hit(s)",
            kind, candidate.rule_ref, candidate.name, coverage, candidate.corpus_failure_hits
        );
        if let Some(concept) = candidate.exact_concept.as_ref() {
            println!(
                "    concept: {}{}",
                concept,
                candidate
                    .phase2_status
                    .map(|status| format!(" phase2={status:?}"))
                    .unwrap_or_default()
            );
        } else if !candidate.mentioned_concepts.is_empty() {
            let mentioned = candidate
                .mentioned_concepts
                .iter()
                .take(3)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            println!("    mentioned in: {mentioned}");
        }
        for example in candidate.corpus_failure_examples.iter().take(2) {
            println!("    {}: {:?}", example.card, example.text);
        }
    }
}

fn grammar_drift_summary() -> Result<GrammarDriftSummary> {
    let rules = grammar_query_engine::parse_grammar_file(grammar_pest_path())?;
    let duplicate_rhs_shapes = grammar_query_engine::duplicate_rhs_shape_groups(&rules);
    let quantity_like_duplicate_rhs_shape_groups = duplicate_rhs_shapes
        .iter()
        .filter(|group| group.quantity_like)
        .count();
    let similar_rhs_shape_pairs = grammar_query_engine::similar_rhs_shapes(&rules).len();
    Ok(GrammarDriftSummary {
        duplicate_rhs_shape_groups: duplicate_rhs_shapes.len(),
        quantity_like_duplicate_rhs_shape_groups,
        similar_rhs_shape_pairs,
    })
}

fn tracked_sets() -> Result<Vec<String>> {
    let path = corpus_sets_path();
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let sets: Vec<String> =
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    if sets.is_empty() {
        bail!("{} must contain at least one set code", path.display());
    }
    Ok(sets)
}

fn query_terms(query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    for ch in query.chars() {
        if ch.is_ascii_alphanumeric() || ch == '\'' {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            push_query_term(&mut terms, &mut current);
        }
    }
    if !current.is_empty() {
        push_query_term(&mut terms, &mut current);
    }
    terms
}

fn push_query_term(terms: &mut Vec<String>, current: &mut String) {
    let term = std::mem::take(current);
    if term.len() > 2 && !STOP_WORDS.contains(&term.as_str()) && !terms.contains(&term) {
        terms.push(term);
    }
}

const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "be", "by", "for", "from", "in", "is", "it", "of", "or", "that",
    "the", "this", "to", "with", "would", "you", "your",
];

fn oracle_clauses(text: &str) -> Vec<String> {
    let mut clauses = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let mut start = 0usize;
        for (idx, ch) in line.char_indices() {
            if matches!(ch, '.' | ';') {
                let clause = line[start..=idx].trim();
                if !clause.is_empty() {
                    clauses.push(clause.to_string());
                }
                start = idx + ch.len_utf8();
            }
        }
        let rest = line[start..].trim();
        if !rest.is_empty() {
            clauses.push(rest.to_string());
        }
    }
    clauses
}

fn matches_terms(clause: &str, terms: &[String]) -> bool {
    let lower = clause.to_ascii_lowercase();
    terms.iter().all(|term| lower.contains(term))
}

fn skeletonize_clause(clause: &str) -> String {
    let mut out = Vec::new();
    for raw in clause.split_whitespace() {
        let token = raw.trim_matches(|c: char| c.is_ascii_punctuation() && c != '+');
        let skeleton = if token.chars().all(|c| c.is_ascii_digit()) {
            "N".to_string()
        } else if token.eq_ignore_ascii_case("x") {
            "X".to_string()
        } else if token.starts_with('{') && token.ends_with('}') {
            "MANA".to_string()
        } else if token.contains("+1/+") || token.contains("-1/-") {
            "PT_COUNTER".to_string()
        } else {
            token.to_ascii_lowercase()
        };
        if !skeleton.is_empty() {
            out.push(skeleton);
        }
    }
    out.join(" ")
}

fn infer_axes(examples: &[CorpusExample]) -> Vec<AxisCandidate> {
    let mut evidence: BTreeMap<&'static str, BTreeSet<String>> = BTreeMap::new();
    for example in examples {
        let lower = example.clause.to_ascii_lowercase();
        maybe_axis(
            &mut evidence,
            "amount",
            &lower,
            &["all", "next", "x", "up to"],
        );
        maybe_axis(
            &mut evidence,
            "event",
            &lower,
            &[
                "damage", "destroy", "counter", "draw", "discard", "gain", "lose",
            ],
        );
        maybe_axis(&mut evidence, "source", &lower, &["source", "by", "from"]);
        maybe_axis(
            &mut evidence,
            "recipient",
            &lower,
            &["target", "you", "player", "creature", "permanent"],
        );
        maybe_axis(
            &mut evidence,
            "duration",
            &lower,
            &["this turn", "until", "as long as"],
        );
        maybe_axis(
            &mut evidence,
            "condition",
            &lower,
            &["if", "when", "whenever", "the next time"],
        );
        maybe_axis(
            &mut evidence,
            "zone",
            &lower,
            &[
                "graveyard",
                "library",
                "hand",
                "battlefield",
                "exile",
                "ante",
            ],
        );
        maybe_axis(
            &mut evidence,
            "polarity",
            &lower,
            &["can't", "can", "prevent", "instead", "unless"],
        );
    }
    evidence
        .into_iter()
        .map(|(name, evidence)| AxisCandidate {
            name,
            evidence: evidence.into_iter().take(5).collect(),
        })
        .collect()
}

fn maybe_axis(
    evidence: &mut BTreeMap<&'static str, BTreeSet<String>>,
    name: &'static str,
    text: &str,
    needles: &[&str],
) {
    for needle in needles {
        if text.contains(needle) {
            evidence
                .entry(name)
                .or_default()
                .insert((*needle).to_string());
        }
    }
}

fn discovery_blockers(
    rules: &RulesArtifact,
    corpus: &CorpusClusterArtifact,
    axes: &AxisArtifact,
) -> Vec<String> {
    let mut blockers = Vec::new();
    if rules.query_logs.is_empty() {
        blockers.push("no qmd rules queries were attempted".to_string());
    }
    if corpus.examples.is_empty() {
        blockers.push("no corpus examples matched the query".to_string());
    }
    if axes.axes.is_empty() {
        blockers.push("no grammar axes inferred from examples".to_string());
    }
    blockers
}

fn write_concept_stub(
    path: PathBuf,
    concept: &str,
    query: &str,
    corpus: &CorpusClusterArtifact,
    axes: &AxisArtifact,
) -> Result<()> {
    let mut out = String::new();
    out.push_str("[concept]\n");
    out.push_str(&format!("name = {:?}\n", concept));
    out.push_str(&format!("discovery_query = {:?}\n\n", query));
    out.push_str("[maturity]\n");
    out.push_str("pest_grammar = \"discovered\"\n\n");
    for axis in &axes.axes {
        out.push_str("[[axis]]\n");
        out.push_str(&format!("name = {:?}\n", axis.name));
        out.push_str(&format!("evidence = {:?}\n\n", axis.evidence));
    }
    for example in &corpus.examples {
        out.push_str("[[example]]\n");
        out.push_str(&format!("card = {:?}\n", example.card));
        out.push_str(&format!("text = {:?}\n\n", example.clause));
    }
    fs::write(&path, out).with_context(|| format!("write {}", path.display()))
}

fn write_json<T: Serialize>(path: PathBuf, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value).context("serialize json")?;
    fs::write(&path, json).with_context(|| format!("write {}", path.display()))
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs()
}

fn unix_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_millis()
}

fn slug(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn display_optional_path(path: &Option<PathBuf>) -> String {
    path.as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "missing".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_test_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "mtg-parser-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        path
    }

    #[test]
    fn query_terms_drop_stop_words() {
        assert_eq!(
            query_terms("prevent the next damage"),
            vec!["prevent", "next", "damage"]
        );
    }

    #[test]
    fn oracle_clauses_split_lines_and_sentences() {
        assert_eq!(
            oracle_clauses("Flying\nDestroy target creature. Draw a card."),
            vec!["Flying", "Destroy target creature.", "Draw a card."]
        );
    }

    #[test]
    fn skeletonize_replaces_numeric_tokens() {
        assert_eq!(
            skeletonize_clause("prevent the next 1 damage."),
            "prevent the next N damage"
        );
    }

    #[test]
    fn phase_loop_options_parse_thresholds() {
        let options = parse_phase_loop_options(&[
            "--phase2-batch-size=3".to_string(),
            "--phase2-max-batches".to_string(),
            "4".to_string(),
            "--phase2-max-commits=9".to_string(),
            "--repeat-stop-after=3".to_string(),
            "--phase1-batch-size=6".to_string(),
            "--phase1-max-batches=7".to_string(),
            "--dry-run".to_string(),
        ])
        .expect("phase loop options parse");
        assert_eq!(options.phase2_batch_size, 3);
        assert_eq!(options.phase2_max_batches, Some(4));
        assert_eq!(options.phase2_max_commits, Some(9));
        assert_eq!(options.repeat_stop_after, 3);
        assert_eq!(options.phase1_batch_size, 6);
        assert_eq!(options.phase1_max_batches, Some(7));
        assert!(options.dry_run);
    }

    #[test]
    fn extracts_top_level_statement_variants_only() {
        let variants = extract_statement_variants(
            r#"
#[derive(Debug)]
pub enum Statement {
    ManaCost(ManaCost),
    CounterTargetSpell {
        condition: Option<CounterTargetSpellCondition>,
    },
    DamageEffect(ActivatedDamageEffect),
}

pub enum Other {
    NotAStatement,
}
"#,
        );
        assert_eq!(
            variants,
            ["CounterTargetSpell", "DamageEffect", "ManaCost"]
                .into_iter()
                .map(str::to_string)
                .collect()
        );
    }

    #[test]
    fn parses_boundary_decision_owner() {
        let decision = parse_boundary_decision(
            "CONCEPT_BOUNDARY_DECISION:\n\
             OWNER: existing:counter_target_spell\n\
             AXES: color=red|blue\n\
             EXAMPLES_TO_ACCEPT: Counter target red spell.\n\
             COUNTEREXAMPLES_TO_REJECT: Destroy target creature.\n\
             PEST_PATCH_INTENT: widen counter_target_spell\n",
        )
        .expect("decision parses");
        assert!(matches!(
            &decision.owner,
            BoundaryOwner::Existing(concept) if concept == "counter_target_spell"
        ));
        assert!(decision.pest_patch_intent.contains("widen"));
    }

    #[test]
    fn parses_boundary_owner_with_inline_contrast() {
        let decision = parse_boundary_decision(
            "CONCEPT_BOUNDARY_DECISION:\n\
             OWNER: existing:static_colored_permanents_pt_modification, not existing:static_cost_increase\n\
             AXES: affected_object=status_creatures\n\
             EXAMPLES_TO_ACCEPT: Attacking creatures get +1/+0.\n\
             COUNTEREXAMPLES_TO_REJECT: White spells cost {3} more to cast.\n\
             PEST_PATCH_INTENT: none\n",
        )
        .expect("decision parses");
        assert!(matches!(
            &decision.owner,
            BoundaryOwner::Existing(concept)
                if concept == "static_colored_permanents_pt_modification"
        ));
    }

    #[test]
    fn parses_multiline_boundary_decision_fields() {
        let decision = parse_boundary_decision(
            "CONCEPT_BOUNDARY_DECISION:\n\
             OWNER: existing:destroy\n\
             AXES:\n\
             action = preserve [\"destroy\"]\n\
             target_selector = add [\"all_non_wall_creatures\"]\n\
             EXAMPLES_TO_ACCEPT:\n\
             - \"Destroy all non-Wall creatures.\"\n\
             COUNTEREXAMPLES_TO_REJECT:\n\
             - \"Counter target spell.\"\n\
             PEST_PATCH_INTENT:\n\
             Widen the existing `destroy` concept mapping.\n\
             Prefer folding this into a generalized destroy target shape.\n\
             WHY_NOT_CARD_PASS:\n\
             grammar fixture maturity only\n",
        )
        .expect("decision parses");
        assert!(matches!(
            &decision.owner,
            BoundaryOwner::Existing(concept) if concept == "destroy"
        ));
        assert!(decision.axes.contains("target_selector"));
        assert!(decision
            .pest_patch_intent
            .contains("generalized destroy target shape"));
    }

    #[test]
    fn rejects_malformed_boundary_decision() {
        let err = parse_boundary_decision("OWNER: existing:counter_target_spell")
            .expect_err("missing block marker");
        assert!(err.to_string().contains("CONCEPT_BOUNDARY_DECISION"));
    }

    #[test]
    fn batch_13_fragment_target_resolves_no_pest_fastpath_to_semantic_root() {
        let grammar_before = fs::read_to_string(grammar_pest_path()).expect("read grammar.pest");
        let response = "CONCEPT_BOUNDARY_DECISION:\n\
             OWNER: new:static_play_restriction\n\
             AXES:\n\
             subject = players\n\
             polarity = can't\n\
             action = cast_spells|play_lands\n\
             EXAMPLES_TO_ACCEPT:\n\
             Players can't cast spells with a name originally printed in the Alpha expansion.\n\
             Players can't play lands with a name originally printed in the Alpha expansion.\n\
             Players can't cast spells or play lands with a name originally printed in the Alpha expansion.\n\
             COUNTEREXAMPLES_TO_REJECT:\n\
             The player plays that card if able.\n\
             You may play this card any time you could cast an instant.\n\
             Mana they produce is spent to play that card.\n\
             PEST_PATCH_INTENT: none\n\
             WHY_NOT_CARD_PASS: grammar fixture maturity only\n";
        let decision = parse_boundary_decision(response).expect("parse boundary decision");
        let gap = ConceptGap {
            concept: "static_play_restriction".to_string(),
            query: "static play restriction".to_string(),
            target_rule: "play_restriction_action".to_string(),
            target_line: 1,
            suggested_existing_owner: false,
            reason: "batch 13 no-PEST replay".to_string(),
        };

        let report =
            attempt_no_pest_concept_fastpath(&gap, &decision, false).expect("run fastpath");

        assert!(report.fastpath_attempted);
        assert_eq!(report.fastpath_result, "success");
        assert!(!report.patch_agent_started);
        assert_eq!(report.original_target_rule, "play_restriction_action");
        assert_eq!(
            report.example_rule.as_deref(),
            Some("static_play_restriction")
        );
        assert_eq!(
            report.resolution_reason.as_deref(),
            Some("single_semantic_ancestor_accepts_all_examples")
        );
        assert!(report
            .mapped_pest_rules
            .contains(&"play_restriction_action".to_string()));
        assert!(report
            .mapped_pest_rules
            .contains(&"static_play_restriction".to_string()));
        assert!(!report.grammar_pest_changed);
        let quality = report.quality_contract.expect("quality contract");
        assert!(quality.fixture_result.passed);
        assert_eq!(quality.maturity_result.state, "grammar_fixture_green");
        let grammar_after = fs::read_to_string(grammar_pest_path()).expect("read grammar.pest");
        assert_eq!(
            grammar_after, grammar_before,
            "fastpath changed grammar.pest"
        );
    }

    #[test]
    fn batch_13_root_target_keeps_original_rule_for_no_pest_fastpath() {
        let grammar_before = fs::read_to_string(grammar_pest_path()).expect("read grammar.pest");
        let response = "CONCEPT_BOUNDARY_DECISION:\n\
             OWNER: new:static_colored_permanents_pt_modification\n\
             AXES:\n\
             preserve_effect_kind = static_continuous_pt_modification\n\
             affected_object = colored_permanents|filter=color_word|filter=permanent_type_plural|modifier=pt_modifier\n\
             EXAMPLES_TO_ACCEPT:\n\
             White creatures get +1/+1.\n\
             Black creatures get -1/-1.\n\
             Green creatures get +0/+2.\n\
             COUNTEREXAMPLES_TO_REJECT:\n\
             White spells cost {3} more to cast.\n\
             Activated abilities of white enchantments cost {3} more to activate.\n\
             Enchanted creature gets +1/+1.\n\
             PEST_PATCH_INTENT: none\n\
             WHY_NOT_CARD_PASS: grammar fixture maturity only\n";
        let decision = parse_boundary_decision(response).expect("parse boundary decision");
        let gap = ConceptGap {
            concept: "static_colored_permanents_pt_modification".to_string(),
            query: "static colored permanents pt modification".to_string(),
            target_rule: "static_colored_permanents_get".to_string(),
            target_line: 1,
            suggested_existing_owner: false,
            reason: "batch 13 no-PEST replay".to_string(),
        };

        let report =
            attempt_no_pest_concept_fastpath(&gap, &decision, false).expect("run fastpath");

        assert!(report.fastpath_attempted);
        assert_eq!(report.fastpath_result, "success");
        assert!(!report.patch_agent_started);
        assert_eq!(report.original_target_rule, "static_colored_permanents_get");
        assert_eq!(
            report.example_rule.as_deref(),
            Some("static_colored_permanents_get")
        );
        assert_eq!(
            report.resolution_reason.as_deref(),
            Some("target_rule_accepts_all_examples")
        );
        assert_eq!(
            report.mapped_pest_rules,
            vec!["static_colored_permanents_get".to_string()]
        );
        assert!(!report.grammar_pest_changed);
        let quality = report.quality_contract.expect("quality contract");
        assert!(quality.fixture_result.passed);
        assert_eq!(quality.maturity_result.state, "grammar_fixture_green");
        let grammar_after = fs::read_to_string(grammar_pest_path()).expect("read grammar.pest");
        assert_eq!(
            grammar_after, grammar_before,
            "fastpath changed grammar.pest"
        );
    }

    #[test]
    fn explicit_concept_does_not_fall_back_to_unrelated_rule() {
        let report = ExistingGrammarMapReport {
            rule_count: 1,
            concept_count: 0,
            dependency_expansion: true,
            shared_rule_count: 0,
            mapped_rule_count: 0,
            unmapped_rule_count: 1,
            concepts: Vec::new(),
            unmapped_rules: vec![UnmappedGrammarRule {
                name: "destroy_target".to_string(),
                line: 10,
                suggested_concept: None,
            }],
        };
        let options = ConceptGrindOptions {
            agent: AgentProvider::Codex,
            max_iterations: 1,
            concept: Some("counter_target_spell".to_string()),
            target_rule: None,
            query: None,
            repair_attempts: 0,
            dry_run: true,
            allow_dirty: false,
            no_commit: true,
        };
        let err = select_concept_gap(&report, &options).expect_err("should require target-rule");
        assert!(err.to_string().contains("--target-rule"));
    }

    #[test]
    fn auto_concept_grind_skips_blocked_suggested_rules() {
        let report = ExistingGrammarMapReport {
            rule_count: 2,
            concept_count: 0,
            dependency_expansion: true,
            shared_rule_count: 0,
            mapped_rule_count: 0,
            unmapped_rule_count: 2,
            concepts: Vec::new(),
            unmapped_rules: vec![
                UnmappedGrammarRule {
                    name: "counter_name".to_string(),
                    line: 10,
                    suggested_concept: Some("counter_target_spell".to_string()),
                },
                UnmappedGrammarRule {
                    name: "destroy_target".to_string(),
                    line: 20,
                    suggested_concept: None,
                },
            ],
        };
        let options = ConceptGrindOptions {
            agent: AgentProvider::Codex,
            max_iterations: 1,
            concept: None,
            target_rule: None,
            query: None,
            repair_attempts: 0,
            dry_run: true,
            allow_dirty: false,
            no_commit: true,
        };
        let excluded = BTreeSet::from(["counter_name".to_string()]);
        let gap = select_concept_gap_excluding(&report, &options, &excluded)
            .expect("should skip blocked suggestion");
        assert_eq!(gap.target_rule, "destroy_target");
    }

    #[test]
    fn selector_contract_uses_candidate_build_exclusions() {
        let report = selector_contract_report_for_test();
        let options = auto_grind_options_for_test();
        let persisted = selector_contract_exclusions_for_test();
        let blocked_targets = persisted
            .iter()
            .map(|exclusion| exclusion.target_rule.clone())
            .collect::<BTreeSet<_>>();
        let cooldown = PlumbingCooldownState::default();

        let contract =
            selector_contract_check(&report, &options, &blocked_targets, &cooldown, &persisted)
                .expect("contract runs");

        assert_eq!(contract.status, "ok");
        assert!(contract.exposed_rules.is_empty());
        assert!(contract.missing_audit_fields.is_empty());
        assert_eq!(
            contract
                .candidate_build
                .as_ref()
                .and_then(|audit| audit.selected_post_filter_candidate.as_ref())
                .map(|selected| selected.target_rule.as_str()),
            Some("draw_cards")
        );
    }

    #[test]
    fn selector_contract_fails_closed_when_persisted_rule_is_exposed() {
        let report = selector_contract_report_for_test();
        let options = auto_grind_options_for_test();
        let persisted = selector_contract_exclusions_for_test();
        let blocked_targets = BTreeSet::new();
        let cooldown = PlumbingCooldownState::default();

        let contract =
            selector_contract_check(&report, &options, &blocked_targets, &cooldown, &persisted)
                .expect("contract runs");

        assert_eq!(contract.status, "selector_contract_failed");
        assert_eq!(contract.exposed_rules, vec!["spell_type".to_string()]);
    }

    #[test]
    fn grind_loop_options_parse_resume_path() {
        let options = parse_grind_loop_options(&[
            "--resume".to_string(),
            ".grammar-concept-runs/loop-123".to_string(),
        ])
        .expect("resume options parse");
        assert_eq!(
            options.resume,
            Some(PathBuf::from(".grammar-concept-runs/loop-123"))
        );
    }

    #[test]
    fn grind_loop_state_round_trips_active_experiment() {
        let loop_dir = unique_test_dir("concept-grind-loop-state");
        fs::create_dir_all(&loop_dir).expect("create loop dir");
        let state = ConceptGrindLoopState {
            options: ConceptGrindLoopOptions {
                agent: AgentProvider::Codex,
                batch_size: 3,
                max_batches: Some(10),
                dry_run: false,
                resume: None,
            },
            next_batch: 4,
            active_experiment: Some(GrindLoopExperiment {
                id: "exp_restart_wrapper".to_string(),
                hypothesis: "restart applies wrapper changes".to_string(),
                implementation_request: "change the loop wrapper".to_string(),
                success_metric: "next batch uses changed wrapper".to_string(),
                quality_checks: vec!["cargo check -p xtask".to_string()],
                commit: Some("abc123".to_string()),
            }),
        };

        write_grind_loop_state(&loop_dir, &state).expect("write state");
        let read = read_grind_loop_state(&loop_dir).expect("read state");
        let _ = fs::remove_dir_all(&loop_dir);

        assert_eq!(read.next_batch, 4);
        assert_eq!(read.options.batch_size, 3);
        assert_eq!(
            read.active_experiment.as_ref().map(|experiment| {
                (
                    experiment.id.as_str(),
                    experiment.commit.as_deref().unwrap_or_default(),
                )
            }),
            Some(("exp_restart_wrapper", "abc123"))
        );
    }

    #[test]
    fn plumbing_cooldown_expands_one_nested_wrapper_level() {
        let rules = grammar_query_engine::parse_grammar_rules(
            r#"
blocked_shared = { target_wrapper | replacement_wrapper }
target_wrapper = { target_leaf }
replacement_wrapper = { replacement_leaf }
target_leaf = { "target" }
replacement_leaf = { "replacement" }
equal_alias = { target_wrapper | replacement_wrapper }
larger_wrapper = { blocked_shared | extra_leaf }
child_wrapper = { target_wrapper }
deep_wrapper = { nested_wrapper }
nested_wrapper = { nested_leaf }
nested_leaf = { "nested" }
extra_leaf = { "extra" }
unrelated_rule = { "unrelated" }
"#,
        )
        .expect("grammar parses");
        let report = ExistingGrammarMapReport {
            rule_count: rules.len(),
            concept_count: 1,
            dependency_expansion: true,
            shared_rule_count: 0,
            mapped_rule_count: 2,
            unmapped_rule_count: 5,
            concepts: vec![ConceptRuleMap {
                concept: "target_concept".to_string(),
                maturity: "grammar_fixture_green".to_string(),
                concept_file: PathBuf::from("grammar-concepts/target_concept.toml"),
                declared_rules: vec!["target_leaf".to_string()],
                found_rules: Vec::new(),
                owned_rules: vec![
                    RuleLocationSummary {
                        name: "target_leaf".to_string(),
                        line: 4,
                    },
                    RuleLocationSummary {
                        name: "replacement_leaf".to_string(),
                        line: 5,
                    },
                ],
                missing_rules: Vec::new(),
            }],
            unmapped_rules: vec![
                UnmappedGrammarRule {
                    name: "equal_alias".to_string(),
                    line: 6,
                    suggested_concept: None,
                },
                UnmappedGrammarRule {
                    name: "larger_wrapper".to_string(),
                    line: 7,
                    suggested_concept: None,
                },
                UnmappedGrammarRule {
                    name: "child_wrapper".to_string(),
                    line: 8,
                    suggested_concept: None,
                },
                UnmappedGrammarRule {
                    name: "deep_wrapper".to_string(),
                    line: 9,
                    suggested_concept: None,
                },
                UnmappedGrammarRule {
                    name: "unrelated_rule".to_string(),
                    line: 12,
                    suggested_concept: None,
                },
            ],
        };

        let derivation = derive_plumbing_cooldown_from_rules(&report, &rules, "blocked_shared");
        assert_eq!(
            derivation.blocked_target_normalized_leaf_rules,
            vec!["replacement_leaf", "target_leaf"]
        );
        assert_eq!(
            derivation
                .cooled_target_rules
                .iter()
                .map(|candidate| { (candidate.target_rule.as_str(), candidate.relationship_type,) })
                .collect::<Vec<_>>(),
            vec![
                ("equal_alias", PlumbingCooldownRelationship::Equal),
                ("larger_wrapper", PlumbingCooldownRelationship::Wrapper),
                ("child_wrapper", PlumbingCooldownRelationship::Child),
            ]
        );
        assert!(derivation
            .blocked_target_expansion_tree
            .children
            .iter()
            .any(|child| child.rule == "target_wrapper" && !child.children.is_empty()));
    }

    #[test]
    fn plumbing_cooldown_selection_prefers_non_cooled_candidates_then_falls_back() {
        let report = ExistingGrammarMapReport {
            rule_count: 3,
            concept_count: 0,
            dependency_expansion: true,
            shared_rule_count: 0,
            mapped_rule_count: 0,
            unmapped_rule_count: 3,
            concepts: Vec::new(),
            unmapped_rules: vec![
                UnmappedGrammarRule {
                    name: "blocked_shared".to_string(),
                    line: 1,
                    suggested_concept: None,
                },
                UnmappedGrammarRule {
                    name: "cooled_wrapper".to_string(),
                    line: 2,
                    suggested_concept: None,
                },
                UnmappedGrammarRule {
                    name: "unrelated_rule".to_string(),
                    line: 3,
                    suggested_concept: None,
                },
            ],
        };
        let mut cooldown = PlumbingCooldownState::default();
        cooldown
            .cooled_target_rules
            .insert("cooled_wrapper".to_string());
        let blocked_targets = BTreeSet::from(["blocked_shared".to_string()]);

        let selection = plumbing_cooldown_selection(&report, &blocked_targets, &cooldown);
        assert_eq!(
            selection.fallback_status,
            "cooldown_active_non_cooled_candidate_available"
        );
        assert_eq!(
            selection.excluded_rules,
            vec!["blocked_shared".to_string(), "cooled_wrapper".to_string()]
        );

        let fallback_report = ExistingGrammarMapReport {
            rule_count: 2,
            concept_count: 0,
            dependency_expansion: true,
            shared_rule_count: 0,
            mapped_rule_count: 0,
            unmapped_rule_count: 2,
            concepts: Vec::new(),
            unmapped_rules: vec![
                UnmappedGrammarRule {
                    name: "blocked_shared".to_string(),
                    line: 1,
                    suggested_concept: None,
                },
                UnmappedGrammarRule {
                    name: "cooled_wrapper".to_string(),
                    line: 2,
                    suggested_concept: None,
                },
            ],
        };
        let selection = plumbing_cooldown_selection(&fallback_report, &blocked_targets, &cooldown);
        assert_eq!(selection.fallback_status, "fallback_to_cooled_candidates");
        assert_eq!(selection.excluded_rules, vec!["blocked_shared".to_string()]);
    }

    #[test]
    fn concept_grind_commit_paths_are_scoped() {
        assert!(is_concept_grind_commit_path(
            "crates/mtg-grammar/src/grammar.pest"
        ));
        assert!(is_concept_grind_commit_path(
            "grammar-fixtures/counter_target_spell.toml"
        ));
        assert!(!is_concept_grind_commit_path("xtask/src/concept.rs"));
    }

    #[test]
    fn gap_closure_failure_has_repairable_label() {
        let failure = run_gap_closure_gate(
            &ConceptGap {
                concept: "counter_target_spell".to_string(),
                query: "counter target colored spell".to_string(),
                target_rule: "counter_target_colored_spell".to_string(),
                target_line: 196,
                suggested_existing_owner: true,
                reason: "test".to_string(),
            },
            &map_report_for_test(false),
            &map_report_for_test(false),
        )
        .expect_err("gap should still be open");
        assert_eq!(failure.label, "gap closure");
        assert!(failure.output.contains("still unmapped"));
    }

    #[test]
    fn concept_grind_fixture_gate_uses_fresh_xtask_process() {
        let args = concept_grammar_test_command_args(Path::new(
            "grammar-fixtures/counter_target_spell.toml",
        ));
        let args: Vec<_> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec![
                "xtask",
                "concept-grammar-test",
                "--json",
                "--fixture",
                "grammar-fixtures/counter_target_spell.toml",
            ]
        );
    }

    #[test]
    fn phase2_fresh_parse_accepts_failure_json() {
        use std::os::unix::process::ExitStatusExt;

        let output = std::process::Output {
            status: std::process::ExitStatus::from_raw(1),
            stdout: br#"{
                "concept": "copy_permanent_enter_as",
                "fixture_path": "grammar-fixtures/copy_permanent_enter_as.toml",
                "passed": false,
                "total": 1,
                "failures": 1,
                "cases": [{
                    "index": 1,
                    "rule": "static_you_may_have_source_enter_as_copy",
                    "text": "You may have CARDNAME enter as a copy of any creature.",
                    "parsed": false,
                    "error": "internal grammar/AST mismatch: unexpected rule permanent_type"
                }]
            }"#
            .to_vec(),
            stderr: Vec::new(),
        };

        let report = parse_fresh_concept_parse_output("copy_permanent_enter_as", &output).unwrap();
        assert!(!report.passed);
        assert_eq!(report.failures, 1);
        assert_eq!(
            report.cases[0].rule,
            "static_you_may_have_source_enter_as_copy"
        );
    }

    #[test]
    fn ast_shape_contract_rejects_forbidden_legacy_variant() {
        let contract = FixtureAstShapeContract {
            owner: Some("CounterTargetSpell".to_string()),
            forbid: vec!["CounterTargetColoredSpell".to_string()],
            note: Some("use the concept-owned shape".to_string()),
        };
        let ast = serde_json::json!({
            "CounterTargetColoredSpell": {
                "color": "Red"
            }
        });

        let error = ast_shape_case_error(&ast, Some(&contract)).expect("shape violation");
        assert!(error.contains("expected AST owner `CounterTargetSpell`"));
        assert!(error.contains("forbidden legacy AST variant `CounterTargetColoredSpell`"));
    }

    #[test]
    fn ast_shape_contract_accepts_owner_inside_wrapper() {
        let contract = FixtureAstShapeContract {
            owner: Some("CounterTargetSpell".to_string()),
            forbid: vec!["CounterTargetColoredSpell".to_string()],
            note: None,
        };
        let ast = serde_json::json!({
            "ActivatedAbility": {
                "costs": [],
                "effect": {
                    "CounterTargetSpell": {
                        "condition": null
                    }
                }
            }
        });

        assert!(ast_shape_case_error(&ast, Some(&contract)).is_none());
    }

    #[test]
    fn quality_contract_logs_exact_fixture_and_maturity_commands() {
        assert_eq!(
            fixture_command_for_log(Path::new("grammar-fixtures/counter_target_spell.toml")),
            vec![
                "cargo",
                "xtask",
                "concept-grammar-test",
                "--json",
                "--fixture",
                "grammar-fixtures/counter_target_spell.toml",
            ]
        );
        assert_eq!(
            maturity_command_for_log("counter_target_spell", false),
            vec![
                "cargo",
                "xtask",
                "concept-maturity",
                "counter_target_spell",
                "--json",
            ]
        );
        assert_eq!(
            maturity_command_for_log("counter_target_spell", true),
            vec![
                "cargo",
                "xtask",
                "concept-maturity",
                "counter_target_spell",
                "--json",
                "--update",
            ]
        );
    }

    fn auto_grind_options_for_test() -> ConceptGrindOptions {
        ConceptGrindOptions {
            agent: AgentProvider::Codex,
            max_iterations: 1,
            concept: None,
            target_rule: None,
            query: None,
            repair_attempts: 0,
            dry_run: true,
            allow_dirty: false,
            no_commit: true,
        }
    }

    fn selector_contract_report_for_test() -> ExistingGrammarMapReport {
        let mut unmapped_rules = SELECTOR_CONTRACT_BLOCKED_RULES
            .iter()
            .enumerate()
            .map(|(index, rule)| UnmappedGrammarRule {
                name: (*rule).to_string(),
                line: index + 1,
                suggested_concept: None,
            })
            .collect::<Vec<_>>();
        unmapped_rules.push(UnmappedGrammarRule {
            name: "draw_cards".to_string(),
            line: 99,
            suggested_concept: None,
        });

        ExistingGrammarMapReport {
            rule_count: unmapped_rules.len(),
            concept_count: 0,
            dependency_expansion: true,
            shared_rule_count: 0,
            mapped_rule_count: 0,
            unmapped_rule_count: unmapped_rules.len(),
            concepts: Vec::new(),
            unmapped_rules,
        }
    }

    fn selector_contract_exclusions_for_test() -> Vec<PersistedBlockedExclusion> {
        SELECTOR_CONTRACT_BLOCKED_RULES
            .iter()
            .enumerate()
            .map(|(index, rule)| PersistedBlockedExclusion {
                target_rule: (*rule).to_string(),
                normalized_blocked_reason: "shared_plumbing".to_string(),
                structural_exclusion_reason: "shared/plumbing".to_string(),
                matched_feature: "shared_plumbing".to_string(),
                evidence_rule_or_parent: (*rule).to_string(),
                source_run: PathBuf::from(".grammar-concept-runs/grind-test"),
                source_iteration: index as u32 + 1,
            })
            .collect()
    }

    fn map_report_for_test(target_owned: bool) -> ExistingGrammarMapReport {
        ExistingGrammarMapReport {
            rule_count: 1,
            concept_count: 1,
            dependency_expansion: true,
            shared_rule_count: 0,
            mapped_rule_count: usize::from(target_owned),
            unmapped_rule_count: usize::from(!target_owned),
            concepts: vec![ConceptRuleMap {
                concept: "counter_target_spell".to_string(),
                maturity: "grammar_fixture_green".to_string(),
                concept_file: PathBuf::from("grammar-concepts/counter_target_spell.toml"),
                declared_rules: vec!["counter_target_spell".to_string()],
                found_rules: Vec::new(),
                owned_rules: if target_owned {
                    vec![RuleLocationSummary {
                        name: "counter_target_colored_spell".to_string(),
                        line: 196,
                    }]
                } else {
                    Vec::new()
                },
                missing_rules: Vec::new(),
            }],
            unmapped_rules: if target_owned {
                Vec::new()
            } else {
                vec![UnmappedGrammarRule {
                    name: "counter_target_colored_spell".to_string(),
                    line: 196,
                    suggested_concept: Some("counter_target_spell".to_string()),
                }]
            },
        }
    }
}
