// Developer workflow CLI. Subcommands are added per milestone:
//   M1: `test`
//   M2: `next-card`, `corpus`, `refresh-corpus`
//   later: `add-card`, `bench`, `diff-lark`

use std::process::ExitCode;

mod add_card;
mod agent_events;
mod ast_coverage;
mod concept;
mod console_sink;
mod corpus_cmd;
mod flow;
mod grammar_audit;
mod grammar_query;
mod grind;
mod next_card;
mod paths;
mod refactor_hotspot;
mod rules_context;
mod rules_split;
mod testrun;
mod tui;

const HELP: &str = "\
cargo xtask <command> [args]

Commands:
  test                        Run Tier 1 tests (hand-written unit tests, <1s budget).
  test --tier 2               Run Tier 1 + Tier 2 (round-trip property tests, <10s budget).
  next-card [--set CODE]      Walk a Scryfall set, generate a failing test for the first
                              card the parser can't round-trip. Defaults to --set lea.
  corpus [--update]           Parse every card in the tracked sets and diff against the
                              committed corpus_status.json. --update overwrites it even
                              if there are regressions (use with care).
  ast-coverage [--fail-on-dead-parser-surface]
                              Parse generated regression tests, report exercised AST
                              variants, and flag variants with unparse arms but no parser
                              construction when requested.
  grammar-audit --diff RANGE --oracle-text TEXT
                              Audit grammar.pest additions for sentence-shaped
                              rule drift. Report-only; emits Markdown by default
                              or JSON with --json.
  corpus-add-set CODE         Add a set to corpus_sets.json, refresh it, then run corpus.
  corpus-advance              Add the next paper core/expansion set once the current newest
              [--max-grammar-left N]  tracked set has at most N actionable failures.
  refresh-corpus [--set CODE] Force re-fetch a set from Scryfall, bypassing the cache.
                              Without --set, refreshes every set tracked by `corpus`.
  rules-split                 Parse resources/comprehensive_rules.txt and emit a
                              browsable tree under resources/rules/. Run `just rules`
                              first to fetch the source document.
  rules-context \"<query>\"     Render the Comprehensive Rules prompt block for a
                              given oracle phrase. Lets you inspect retrieval
                              quality without invoking the full add-card loop.
  concept-discover --query TEXT
                              Create a grammar-first discovery run from qmd rules
                              search, corpus clustering, and inferred grammar axes.
                              Writes .grammar-concept-runs/<ts>-<concept>/.
  concept-grow --query TEXT --concept NAME
                              Run the grammar-first loop end to end: discover,
                              choose a PEST rule, write concept and fixture files,
                              run fixtures, and update grammar maturity.
  concept-grammar-test CONCEPT
                              Run grammar-fixtures/<concept>.toml at PEST-rule
                              level. Does not require AST, unparse, lowering, or
                              card corpus success.
  concept-parse CONCEPT       Phase 2 parser/AST readiness check: full-parser
                              parse accepted grammar fixture examples and report
                              AST JSON. Does not require unparse or lowering.
  concept-ast-test CONCEPT    Phase 2 AST snapshot gate for accepted grammar
                              fixture examples. Use --update to write snapshots.
  concept-phase2-map          Inventory grammar-green concepts against Phase 2
                              parse and AST snapshot gates.
  concept-phase2-grind        Autonomous Phase 2 parser/AST loop over
                              grammar-green concepts. Use --ui tui for a live view.
  concept-roadmap             Inventory rulebook-derived concept candidates
                              and annotate concept/corpus coverage.
  concept-grammar-query --query TEXT
                              Query grammar.pest for candidate rules, dependencies,
                              reverse dependencies, and duplicate RHS shape drift.
  concept-maturity CONCEPT    Report grammar-only maturity from concept and fixture
                              artifacts. Does not mark cards passing.
  concept-map-existing        Inventory grammar.pest rules against committed concept
                              files and report mapped versus legacy/unmapped rules.
  concept-grind --agent codex [--ui console|tui]
                              Autonomous grammar-first loop: pick a concept gap,
                              run boundary and PEST patch agents, gate fixtures,
                              update maturity, and commit.
  concept-grind-loop          Run concept-grind in batches, review metrics after
                              each batch, accept/revert the prior experiment, and
                              apply the next optimization experiment.
  refactor-hotspot            Run a qmd-grounded autonomous refactor workflow.
              [--theme THEME] Defaults to grammar-core. Other themes include
              [--target PATH] damage, destroy, prevention, keyword-abilities,
              [--out PATH]    triggered-abilities, and unparse-templates.
              [--ui console|tui]
  grind       [--set CODE]    Meta-loop: run refactor-hotspot until N consecutive
              [--stop-after N]    no-ops, commit budget, repair budget, or low-value
              [--max-refactor-iterations N]  streak, then hand off to add-card on
              [--max-commits-per-theme N]    the cleaner foundation. Gate failures
              [--max-repairs-per-theme N]    route to a freeform repair agent before
              [--low-value-stop-after N]     giving up. Use --theme / --target to
              [--max-card-iterations N]      steer the refactor phase. Defaults:
              [--repair-attempts N]          --stop-after 3,
              [--theme THEME] [--target PATH]  --max-refactor-iterations 50,
              [--agent codex|claude] [--ui ...]  --max-commits-per-theme 5,
              [--allow-dirty] [--dry-run]    --max-repairs-per-theme 2,
                                             --low-value-stop-after 2.
  add-card    [--set CODE]    Orchestrated loop: pick the next failing card in
              [--max-iterations N]  the corpus, hand it to a coding agent, gate the
              [--dry-run] [--allow-dirty]  result through tier-1/2 + corpus + commit. When
              [--ui console|tui]          --set is not given, walks the tracked sets
              [--agent codex|claude]      newest-first and auto-advances to the next
              [--supervisor-attempts N]    paper expansion once the current one is fully
              [--focused-repair-attempts N] covered. Defaults: --max-iterations 0 (unbounded);
              [--no-supervisor]            --focused-repair-attempts 0 (unbounded);
                                          --agent codex; one supervisor repair attempt on
                                          unknown infrastructure errors; --dry-run builds
                                          the prompt and stops; --allow-dirty skips the
                                          clean-tree precondition; --ui tui opens a
                                          full-screen interactive view.

Flags:
  -h, --help        Show this message.
";

enum Ui {
    Console,
    Tui,
}

fn parse_ui(args: &[String]) -> Result<Ui, String> {
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--ui" => match iter.next().map(String::as_str) {
                Some("console") => return Ok(Ui::Console),
                Some("tui") => return Ok(Ui::Tui),
                Some(other) => {
                    return Err(format!("--ui must be 'console' or 'tui', got {other:?}"))
                }
                None => return Err("--ui requires a value".into()),
            },
            s if s.starts_with("--ui=") => match &s["--ui=".len()..] {
                "console" => return Ok(Ui::Console),
                "tui" => return Ok(Ui::Tui),
                other => return Err(format!("--ui must be 'console' or 'tui', got {other:?}")),
            },
            _ => {}
        }
    }
    Ok(Ui::Console)
}

fn ui_hot_reload(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--ui-hot-reload")
}

fn without_ui_hot_reload(args: &[String]) -> Vec<String> {
    args.iter()
        .filter(|arg| arg.as_str() != "--ui-hot-reload")
        .cloned()
        .collect()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("-h") | Some("--help") => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Some("test") => testrun::run(&args[1..]),
        Some("next-card") => next_card::run(&args[1..]),
        Some("corpus") => corpus_cmd::run(&args[1..]),
        Some("ast-coverage") => ast_coverage::run(&args[1..]),
        Some("grammar-audit") => grammar_audit::run(&args[1..]),
        Some("corpus-add-set") => corpus_cmd::add_set(&args[1..]),
        Some("corpus-advance") => corpus_cmd::advance(&args[1..]),
        Some("refresh-corpus") => corpus_cmd::refresh(&args[1..]),
        Some("rules-split") => rules_split::run(&args[1..]),
        Some("rules-context") => rules_context::run_cli(&args[1..]),
        Some("concept-discover") => concept::discover(&args[1..]),
        Some("concept-grow") => concept::grow(&args[1..]),
        Some("concept-grammar-test") => concept::grammar_test(&args[1..]),
        Some("concept-parse") => concept::parse_concept(&args[1..]),
        Some("concept-ast-test") => concept::ast_test(&args[1..]),
        Some("concept-phase2-map") => concept::phase2_map(&args[1..]),
        Some("concept-phase2-grind") => match parse_ui(&args[1..]) {
            Ok(Ui::Console) => concept::phase2_grind(&args[1..]),
            Ok(Ui::Tui) => {
                match concept::parse_phase2_grind_options(&without_ui_hot_reload(&args[1..])) {
                    Ok(opts) => match if ui_hot_reload(&args[1..]) {
                        tui::run_concept_phase2_grind_hot_reload(opts)
                    } else {
                        tui::run_concept_phase2_grind(opts)
                    } {
                        Ok(code) => code,
                        Err(e) => {
                            eprintln!("tui error: {e:#}");
                            ExitCode::FAILURE
                        }
                    },
                    Err(e) => {
                        eprintln!("{e}");
                        ExitCode::from(2)
                    }
                }
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(2)
            }
        },
        Some("concept-roadmap") => concept::roadmap(&args[1..]),
        Some("concept-grammar-query") => concept::grammar_query(&args[1..]),
        Some("concept-maturity") => concept::maturity(&args[1..]),
        Some("concept-map-existing") => concept::map_existing(&args[1..]),
        Some("concept-grind-loop") => concept::grind_loop(&args[1..]),
        Some("concept-grind") => match parse_ui(&args[1..]) {
            Ok(Ui::Console) => concept::grind(&args[1..]),
            Ok(Ui::Tui) => match concept::parse_grind_options(&without_ui_hot_reload(&args[1..])) {
                Ok(opts) => match if ui_hot_reload(&args[1..]) {
                    tui::run_concept_grind_hot_reload(opts)
                } else {
                    tui::run_concept_grind(opts)
                } {
                    Ok(code) => code,
                    Err(e) => {
                        eprintln!("tui error: {e:#}");
                        ExitCode::FAILURE
                    }
                },
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(2)
                }
            },
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(2)
            }
        },
        Some("tui-view") => match parse_tui_view_args(&args[1..]) {
            Ok(event_log) => match tui::run_viewer(event_log) {
                Ok(code) => code,
                Err(e) => {
                    eprintln!("tui error: {e:#}");
                    ExitCode::FAILURE
                }
            },
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(2)
            }
        },
        Some("refactor-hotspot") => match parse_ui(&args[1..]) {
            Ok(Ui::Console) => refactor_hotspot::run(&args[1..]),
            Ok(Ui::Tui) => {
                match refactor_hotspot::Options::parse(&without_ui_hot_reload(&args[1..])) {
                    Ok(opts) => match if ui_hot_reload(&args[1..]) {
                        tui::run_refactor_hotspot_hot_reload(opts)
                    } else {
                        tui::run_refactor_hotspot(opts)
                    } {
                        Ok(code) => code,
                        Err(e) => {
                            eprintln!("tui error: {e:#}");
                            ExitCode::FAILURE
                        }
                    },
                    Err(e) => {
                        eprintln!("{e}");
                        ExitCode::from(2)
                    }
                }
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(2)
            }
        },
        Some("grind") => match parse_ui(&args[1..]) {
            Ok(Ui::Console) => grind::run(&args[1..]),
            Ok(Ui::Tui) => match grind::Options::parse(&without_ui_hot_reload(&args[1..])) {
                Ok(opts) => match if ui_hot_reload(&args[1..]) {
                    tui::run_grind_hot_reload(opts)
                } else {
                    tui::run_grind(opts)
                } {
                    Ok(code) => code,
                    Err(e) => {
                        eprintln!("tui error: {e:#}");
                        ExitCode::FAILURE
                    }
                },
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(2)
                }
            },
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(2)
            }
        },
        Some("add-card") => match parse_ui(&args[1..]) {
            Ok(Ui::Console) => add_card::run(&args[1..]),
            Ok(Ui::Tui) => match add_card::Options::parse(&without_ui_hot_reload(&args[1..])) {
                Ok(opts) => match if ui_hot_reload(&args[1..]) {
                    tui::run_add_card_hot_reload(opts)
                } else {
                    tui::run_add_card(opts)
                } {
                    Ok(code) => code,
                    Err(e) => {
                        eprintln!("tui error: {e:#}");
                        ExitCode::FAILURE
                    }
                },
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(2)
                }
            },
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(2)
            }
        },
        Some(cmd) => {
            eprintln!("unknown command: {cmd}\n\n{HELP}");
            ExitCode::from(2)
        }
    }
}

fn parse_tui_view_args(args: &[String]) -> Result<std::path::PathBuf, String> {
    let mut event_log = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--event-log" => event_log = iter.next().map(std::path::PathBuf::from),
            s if s.starts_with("--event-log=") => {
                event_log = Some(std::path::PathBuf::from(&s["--event-log=".len()..]));
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    event_log.ok_or_else(|| "--event-log requires a value".to_string())
}
