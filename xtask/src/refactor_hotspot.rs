//! Refactor-oriented workflow. Unlike `add-card`, this flow is not
//! trying to make the next card pass. It runs a staged, qmd-grounded
//! grammar refactor: inventory, cluster, plan, implement, gate, commit.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};

use crate::console_sink::ConsoleSink;
use crate::flow::{
    AgentProvider, FlowEvent, FlowSink, IterationOutcomeSummary, NoteLevel, SessionEndReason,
};
use crate::paths::{refactor_hotspot_log_root, repo_root};

const DEFAULT_CHURN_WINDOW: usize = 200;
const DEFAULT_MAX_ITERATIONS: u32 = 3;
const TOTAL_STEPS: u8 = 9;

const HOT_FILES: &[(&str, &str)] = &[
    ("grammar.pest", "crates/mtg-grammar/src/grammar.pest"),
    ("ast.rs", "crates/mtg-grammar/src/ast.rs"),
    ("parse.rs", "crates/mtg-grammar/src/parse.rs"),
    ("unparse.rs", "crates/mtg-grammar/src/unparse.rs"),
    ("semantic/ir.rs", "crates/mtg-semantic/src/ir.rs"),
    ("semantic/lower.rs", "crates/mtg-semantic/src/lower.rs"),
    ("grammar/tests/prop.rs", "crates/mtg-grammar/tests/prop.rs"),
    (
        "semantic/tests/prop.rs",
        "crates/mtg-semantic/tests/prop.rs",
    ),
    ("corpus_status.json", "corpus_status.json"),
];

const GRAMMAR_CORE_FILES: &[&str] = &[
    "crates/mtg-grammar/src/grammar.pest",
    "crates/mtg-grammar/src/ast.rs",
    "crates/mtg-grammar/src/parse.rs",
    "crates/mtg-grammar/src/unparse.rs",
];

pub fn run(args: &[String]) -> ExitCode {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    let opts = match Options::parse(args) {
        Ok(opts) => opts,
        Err(e) => {
            eprintln!("refactor-hotspot: {e:#}");
            return ExitCode::FAILURE;
        }
    };
    let mut sink: Box<dyn FlowSink> = Box::new(ConsoleSink::new());
    match run_with_sink(opts, sink.as_mut()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("refactor-hotspot: {e:#}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    theme: Option<Theme>,
    target: Option<String>,
    out: Option<PathBuf>,
    print: bool,
    dry_run: bool,
    allow_dirty: bool,
    agent: AgentProvider,
    churn_window: usize,
    max_iterations: u32,
}

impl Options {
    pub fn parse(args: &[String]) -> Result<Self> {
        let mut theme = None::<Theme>;
        let mut target = None::<String>;
        let mut out = None::<PathBuf>;
        let mut print = false;
        let mut dry_run = false;
        let mut allow_dirty = false;
        let mut agent = AgentProvider::Codex;
        let mut churn_window = DEFAULT_CHURN_WINDOW;
        let mut max_iterations = DEFAULT_MAX_ITERATIONS;

        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--theme" => {
                    let value = iter
                        .next()
                        .ok_or_else(|| anyhow!("--theme requires a value"))?;
                    theme = Some(Theme::parse(value)?);
                }
                s if s.starts_with("--theme=") => {
                    theme = Some(Theme::parse(&s["--theme=".len()..])?);
                }
                "--target" => {
                    target = Some(
                        iter.next()
                            .ok_or_else(|| anyhow!("--target requires a value"))?
                            .to_string(),
                    );
                }
                s if s.starts_with("--target=") => {
                    target = Some(s["--target=".len()..].to_string());
                }
                "--out" => {
                    out = Some(PathBuf::from(
                        iter.next()
                            .ok_or_else(|| anyhow!("--out requires a value"))?,
                    ));
                }
                s if s.starts_with("--out=") => {
                    out = Some(PathBuf::from(&s["--out=".len()..]));
                }
                "--print" => print = true,
                "--dry-run" => dry_run = true,
                "--allow-dirty" => allow_dirty = true,
                "--ui" => {
                    let _ = iter
                        .next()
                        .ok_or_else(|| anyhow!("--ui requires a value"))?;
                }
                s if s.starts_with("--ui=") => {}
                "--agent" => {
                    let value = iter
                        .next()
                        .ok_or_else(|| anyhow!("--agent requires a value"))?;
                    agent = parse_agent(value)?;
                }
                s if s.starts_with("--agent=") => {
                    agent = parse_agent(&s["--agent=".len()..])?;
                }
                "--churn-window" => {
                    let value = iter
                        .next()
                        .ok_or_else(|| anyhow!("--churn-window requires a value"))?;
                    churn_window = value
                        .parse()
                        .with_context(|| format!("--churn-window value: {value:?}"))?;
                }
                s if s.starts_with("--churn-window=") => {
                    churn_window = s["--churn-window=".len()..]
                        .parse()
                        .with_context(|| format!("--churn-window value: {s:?}"))?;
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
                other => bail!("unknown argument: {other}\n\n{HELP}"),
            }
        }
        if max_iterations == 0 {
            bail!("--max-iterations must be at least 1");
        }

        Ok(Self {
            theme,
            target,
            out,
            print,
            dry_run,
            allow_dirty,
            agent,
            churn_window,
            max_iterations,
        })
    }
}

const HELP: &str = "\
cargo xtask refactor-hotspot [--theme THEME] [--target PATH] [--agent codex|claude]
                             [--max-iterations N] [--dry-run] [--allow-dirty]
                             [--out PATH] [--print] [--ui console|tui]

Autonomous by default: runs N staged grammar-core refactor passes over
grammar.pest, ast.rs, parse.rs, and unparse.rs. Each pass inventories the
current grammar surface, asks the agent to cluster similar grammar shapes, asks
for a generalization plan, invokes a fresh implementation agent with only that
plan, runs `cargo test`, `cargo xtask corpus`, `just audit-page`, and commits
the result. Use --theme or --target to override the default selection. Use
--dry-run or --print to stop after writing/printing the first stage prompt.

Themes:
  grammar-core         Coupled grammar/AST/parser/unparser cleanup.
  parser-boilerplate   Parser helper/extraction cleanup without AST changes.
  damage               Damage phenomenon factoring.
  destroy              Destroy/tap/sacrifice/attach keyword-action factoring.
  prevention           Prevention and replacement-effect factoring.
  keyword-abilities    Keyword ability shape factoring.
  triggered-abilities  Trigger and intervening-if factoring.
  unparse-templates    Unparser template/slot extraction.
";

fn parse_agent(value: &str) -> Result<AgentProvider> {
    match value {
        "codex" => Ok(AgentProvider::Codex),
        "claude" => Ok(AgentProvider::Claude),
        other => bail!("--agent must be 'codex' or 'claude', got {other:?}"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Theme {
    GrammarCore,
    ParserBoilerplate,
    Damage,
    Destroy,
    Prevention,
    KeywordAbilities,
    TriggeredAbilities,
    UnparseTemplates,
}

impl Theme {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "grammar-core" | "grammar" | "core" => Ok(Self::GrammarCore),
            "parser-boilerplate" | "parser" | "parse.rs" => Ok(Self::ParserBoilerplate),
            "damage" => Ok(Self::Damage),
            "destroy" | "keyword-actions" => Ok(Self::Destroy),
            "prevention" | "replacement" => Ok(Self::Prevention),
            "keyword-abilities" | "keywords" => Ok(Self::KeywordAbilities),
            "triggered-abilities" | "triggers" => Ok(Self::TriggeredAbilities),
            "unparse-templates" | "unparse" => Ok(Self::UnparseTemplates),
            other => bail!("unknown refactor theme: {other}\n\n{HELP}"),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::GrammarCore => "grammar-core",
            Self::ParserBoilerplate => "parser-boilerplate",
            Self::Damage => "damage",
            Self::Destroy => "destroy",
            Self::Prevention => "prevention",
            Self::KeywordAbilities => "keyword-abilities",
            Self::TriggeredAbilities => "triggered-abilities",
            Self::UnparseTemplates => "unparse-templates",
        }
    }

    fn qmd_query(self) -> &'static str {
        match self {
            Self::GrammarCore => {
                "grammar syntax oracle text ability statement keyword action keyword ability damage prevention triggered abilities unparse template"
            }
            Self::ParserBoilerplate => {
                "parse grammar syntax oracle text ability statement keyword action keyword ability"
            }
            Self::Damage => {
                "damage deals damage to any target damage recipients prevent damage damage event"
            }
            Self::Destroy => {
                "destroy sacrifice tap attach keyword actions permanent target all permanents"
            }
            Self::Prevention => {
                "prevent damage replacement effects instead prevention effects would be dealt"
            }
            Self::KeywordAbilities => {
                "keyword abilities flying trample first strike landwalk enchant protection"
            }
            Self::TriggeredAbilities => {
                "triggered abilities when whenever at intervening if beginning upkeep"
            }
            Self::UnparseTemplates => {
                "oracle text wording keyword actions keyword abilities damage prevention template"
            }
        }
    }

    fn default_files(self) -> &'static [&'static str] {
        match self {
            Self::GrammarCore => GRAMMAR_CORE_FILES,
            Self::ParserBoilerplate => &["crates/mtg-grammar/src/parse.rs"],
            Self::Damage => &[
                "crates/mtg-grammar/src/grammar.pest",
                "crates/mtg-grammar/src/ast.rs",
                "crates/mtg-grammar/src/parse.rs",
                "crates/mtg-grammar/src/unparse.rs",
            ],
            Self::Destroy => &[
                "crates/mtg-grammar/src/grammar.pest",
                "crates/mtg-grammar/src/ast.rs",
                "crates/mtg-grammar/src/parse.rs",
                "crates/mtg-grammar/src/unparse.rs",
            ],
            Self::Prevention => &[
                "crates/mtg-grammar/src/grammar.pest",
                "crates/mtg-grammar/src/ast.rs",
                "crates/mtg-grammar/src/parse.rs",
                "crates/mtg-grammar/src/unparse.rs",
            ],
            Self::KeywordAbilities => &[
                "crates/mtg-grammar/src/grammar.pest",
                "crates/mtg-grammar/src/ast.rs",
                "crates/mtg-grammar/src/parse.rs",
                "crates/mtg-grammar/src/unparse.rs",
            ],
            Self::TriggeredAbilities => &[
                "crates/mtg-grammar/src/grammar.pest",
                "crates/mtg-grammar/src/ast.rs",
                "crates/mtg-grammar/src/parse.rs",
                "crates/mtg-grammar/src/unparse.rs",
            ],
            Self::UnparseTemplates => &[
                "crates/mtg-grammar/src/ast.rs",
                "crates/mtg-grammar/src/unparse.rs",
            ],
        }
    }

    fn instructions(self) -> &'static str {
        match self {
            Self::GrammarCore => {
                "Refactor the coupled grammar/AST/parser/unparser surface together. Prefer reducing sentence-shaped grammar branches and replacing them with phenomenon-shaped rules, AST data, parser extraction, and unparse templates that move in lockstep. Do not do parser-only cleanup if the grammar shape is the source of complexity."
            }
            Self::ParserBoilerplate => {
                "Extract parser mechanics without changing AST shape or grammar acceptance. Prefer helpers that remove repeated child-pair extraction, list parsing, or rule dispatch boilerplate."
            }
            Self::Damage => {
                "Use Comprehensive Rules damage vocabulary to identify axes such as amount, source, recipient, event timing, prevention, and replacement. Prefer one phenomenon-shaped AST over one variant per sentence."
            }
            Self::Destroy => {
                "Use §701 keyword-action wording for destroy, sacrifice, tap, and attach. Factor shared target/all/list axes before adding or preserving sentence-shaped variants."
            }
            Self::Prevention => {
                "Use §614 and §615 wording to separate replacement/prevention structure from card-specific recipients and amounts."
            }
            Self::KeywordAbilities => {
                "Use §702 names and structure. Prefer data-bearing keyword variants over bespoke sentence rules."
            }
            Self::TriggeredAbilities => {
                "Factor event, optional intervening-if condition, and effect sequence separately. Preserve printed ordering and round-trip behavior."
            }
            Self::UnparseTemplates => {
                "Extract reusable rendering helpers or template slots. Do not alter canonical output text except where tests prove existing output was wrong."
            }
        }
    }
}

pub fn run_with_sink(opts: Options, sink: &mut dyn FlowSink) -> Result<ExitCode> {
    if !opts.dry_run && !opts.print && !opts.allow_dirty {
        ensure_clean_working_tree()
            .context("working tree must be clean (or pass --allow-dirty)")?;
    }
    if opts.out.is_some() && opts.max_iterations > 1 {
        bail!("--out can only be used with --max-iterations 1");
    }
    if (opts.dry_run || opts.print) && opts.max_iterations > 1 {
        sink.emit(FlowEvent::Note {
            level: NoteLevel::Info,
            text: "dry-run/print mode only builds the first prompt".to_string(),
        });
    }

    let (baseline_corpus_passing, baseline_corpus_total) = read_corpus_pp_total();
    sink.emit(FlowEvent::SessionStarted {
        workflow: "refactor-hotspot".to_string(),
        set: opts
            .target
            .clone()
            .or_else(|| opts.theme.map(|theme| theme.label().to_string()))
            .unwrap_or_else(|| "grammar-core".to_string()),
        max_iterations: opts.max_iterations,
        baseline_corpus_passing,
        baseline_corpus_total,
        baseline_grammar_rules: count_grammar_rules(),
    });

    let iterations = if opts.dry_run || opts.print {
        1
    } else {
        opts.max_iterations
    };

    for iteration in 1..=iterations {
        run_iteration(&opts, sink, iteration)?;
    }
    let reason = if opts.dry_run || opts.print {
        SessionEndReason::DryRunStop
    } else {
        SessionEndReason::MaxIterationsReached(iterations)
    };
    sink.emit(FlowEvent::SessionFinished { reason });
    Ok(ExitCode::SUCCESS)
}

fn run_iteration(opts: &Options, sink: &mut dyn FlowSink, iteration: u32) -> Result<()> {
    let iteration_start = Instant::now();
    let baseline_corpus_passing = read_corpus_pp_total().0;
    let selected = resolve_selection(opts)?;
    sink.emit(FlowEvent::WorkflowIterationStarted {
        index: iteration,
        max_iterations: opts.max_iterations,
        title: selected.theme.label().to_string(),
        detail: format!("{}\n{}", selected.reason, selected.files.join("\n")),
    });

    sink.emit(FlowEvent::StepStarted {
        index: 1,
        total: TOTAL_STEPS,
        label: "create log dir".to_string(),
    });
    let log_dir = match opts.out.clone() {
        Some(path) => path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(refactor_hotspot_log_root),
        None => create_log_dir(selected.theme, iteration)?,
    };
    std::fs::create_dir_all(&log_dir).with_context(|| format!("create {}", log_dir.display()))?;
    sink.emit(FlowEvent::StepFinished {
        index: 1,
        ok: true,
        summary: Some(log_dir.display().to_string()),
    });

    sink.emit(FlowEvent::StepStarted {
        index: 2,
        total: TOTAL_STEPS,
        label: "inventory grammar surface".to_string(),
    });
    let inventory = build_inventory(opts, &selected)?;
    std::fs::write(log_dir.join("inventory.md"), &inventory)
        .with_context(|| format!("write {}", log_dir.join("inventory.md").display()))?;
    sink.emit(FlowEvent::StepFinished {
        index: 2,
        ok: true,
        summary: Some("wrote inventory.md".to_string()),
    });

    sink.emit(FlowEvent::StepStarted {
        index: 3,
        total: TOTAL_STEPS,
        label: "build cluster prompt".to_string(),
    });
    let cluster_prompt = build_cluster_prompt(opts, &selected, &inventory, iteration)?;
    std::fs::write(log_dir.join("cluster_prompt.md"), &cluster_prompt)
        .with_context(|| format!("write {}", log_dir.join("cluster_prompt.md").display()))?;
    if opts.print {
        print!("{cluster_prompt}");
    }
    sink.emit(FlowEvent::StepFinished {
        index: 3,
        ok: true,
        summary: Some("wrote cluster_prompt.md".to_string()),
    });

    if opts.dry_run || opts.print {
        sink.emit(FlowEvent::Note {
            level: NoteLevel::Info,
            text: format!(
                "dry-run: not invoking {}, not planning, not implementing, not running gates",
                opts.agent.label()
            ),
        });
        return Ok(());
    }

    sink.emit(FlowEvent::StepStarted {
        index: 4,
        total: TOTAL_STEPS,
        label: format!("{} cluster stage", opts.agent.label()),
    });
    let cluster_outcome = invoke_agent(
        opts.agent,
        &cluster_prompt,
        &log_dir.join("cluster_transcript.ndjson"),
        sink,
    )?;
    std::fs::write(log_dir.join("clusters.md"), &cluster_outcome.assistant_text)
        .with_context(|| format!("write {}", log_dir.join("clusters.md").display()))?;
    sink.emit(FlowEvent::StepFinished {
        index: 4,
        ok: cluster_outcome.success,
        summary: Some(format!("exit={}", cluster_outcome.exit_code)),
    });
    if !cluster_outcome.success {
        bail!(
            "{} cluster stage exited with status {}; transcript: {}",
            opts.agent.label(),
            cluster_outcome.exit_code,
            log_dir.join("cluster_transcript.ndjson").display()
        );
    }

    let plan_prompt = build_plan_prompt(
        opts,
        &selected,
        &inventory,
        &cluster_outcome.assistant_text,
        iteration,
    )?;
    std::fs::write(log_dir.join("plan_prompt.md"), &plan_prompt)
        .with_context(|| format!("write {}", log_dir.join("plan_prompt.md").display()))?;

    sink.emit(FlowEvent::StepStarted {
        index: 5,
        total: TOTAL_STEPS,
        label: format!("{} plan stage", opts.agent.label()),
    });
    let plan_outcome = invoke_agent(
        opts.agent,
        &plan_prompt,
        &log_dir.join("plan_transcript.ndjson"),
        sink,
    )?;
    std::fs::write(log_dir.join("plan.md"), &plan_outcome.assistant_text)
        .with_context(|| format!("write {}", log_dir.join("plan.md").display()))?;
    sink.emit(FlowEvent::StepFinished {
        index: 5,
        ok: plan_outcome.success,
        summary: Some(format!("exit={}", plan_outcome.exit_code)),
    });
    if !plan_outcome.success {
        bail!(
            "{} plan stage exited with status {}; transcript: {}",
            opts.agent.label(),
            plan_outcome.exit_code,
            log_dir.join("plan_transcript.ndjson").display()
        );
    }

    let implementation_prompt = build_implementation_prompt(
        opts,
        &selected,
        &inventory,
        &plan_outcome.assistant_text,
        iteration,
    )?;
    let prompt_path = opts
        .out
        .clone()
        .unwrap_or_else(|| log_dir.join("prompt.md"));
    std::fs::write(&prompt_path, &implementation_prompt)
        .with_context(|| format!("write {}", prompt_path.display()))?;

    sink.emit(FlowEvent::StepStarted {
        index: 6,
        total: TOTAL_STEPS,
        label: format!("{} implementation stage", opts.agent.label()),
    });
    let implement_outcome = invoke_agent(
        opts.agent,
        &implementation_prompt,
        &log_dir.join("transcript.ndjson"),
        sink,
    )?;
    std::fs::write(
        log_dir.join("response.md"),
        &implement_outcome.assistant_text,
    )
    .with_context(|| format!("write {}", log_dir.join("response.md").display()))?;
    sink.emit(FlowEvent::StepFinished {
        index: 6,
        ok: implement_outcome.success,
        summary: Some(format!(
            "exit={} · prompt={}",
            implement_outcome.exit_code,
            prompt_path.display()
        )),
    });
    if !implement_outcome.success {
        bail!(
            "{} implementation stage exited with status {}; transcript: {}",
            opts.agent.label(),
            implement_outcome.exit_code,
            log_dir.join("transcript.ndjson").display()
        );
    }

    sink.emit(FlowEvent::StepStarted {
        index: 7,
        total: TOTAL_STEPS,
        label: "cargo fmt --all".to_string(),
    });
    run_gate("cargo fmt --all", "cargo", &["fmt", "--all"])?;
    sink.emit(FlowEvent::StepFinished {
        index: 7,
        ok: true,
        summary: None,
    });
    sink.emit(FlowEvent::StepStarted {
        index: 8,
        total: TOTAL_STEPS,
        label: "cargo test".to_string(),
    });
    run_gate("cargo test", "cargo", &["test"])?;
    sink.emit(FlowEvent::StepFinished {
        index: 8,
        ok: true,
        summary: None,
    });
    sink.emit(FlowEvent::StepStarted {
        index: 9,
        total: TOTAL_STEPS,
        label: "corpus, audit, commit".to_string(),
    });
    run_gate("cargo xtask corpus", "cargo", &["xtask", "corpus"])?;
    run_gate("just audit-page", "just", &["audit-page"])?;

    match git_commit(&commit_message(&selected, iteration)?)? {
        CommitOutcome::Committed => {
            let (corpus_passing, corpus_total) = read_corpus_pp_total();
            sink.emit(FlowEvent::StepFinished {
                index: 9,
                ok: true,
                summary: Some(format!("status {corpus_passing}/{corpus_total}")),
            });
            sink.emit(FlowEvent::IterationFinished {
                index: iteration,
                outcome: IterationOutcomeSummary::Committed {
                    new_passes: corpus_passing.saturating_sub(baseline_corpus_passing),
                    corpus_passing,
                    corpus_total,
                    grammar_rules: count_grammar_rules(),
                    duration_secs: iteration_start.elapsed().as_secs(),
                },
            });
        }
        CommitOutcome::NoChanges => bail!("no changes to commit after successful refactor gates"),
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct SelectedRefactor {
    theme: Theme,
    files: Vec<String>,
    reason: String,
}

fn resolve_selection(opts: &Options) -> Result<SelectedRefactor> {
    if let Some(target) = &opts.target {
        let theme = opts.theme.unwrap_or_else(|| infer_theme_for_target(target));
        return Ok(SelectedRefactor {
            theme,
            files: vec![target.clone()],
            reason: format!("explicit target; theme={}", theme.label()),
        });
    }

    if let Some(theme) = opts.theme {
        return Ok(SelectedRefactor {
            theme,
            files: theme
                .default_files()
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            reason: "explicit theme".to_string(),
        });
    }

    Ok(SelectedRefactor {
        theme: Theme::GrammarCore,
        files: GRAMMAR_CORE_FILES.iter().map(|s| (*s).to_string()).collect(),
        reason: "default coupled grammar-core selection; grammar, AST, parser, and unparser are refactored together while churn × LOC stats guide the patch"
            .to_string(),
    })
}

fn infer_theme_for_target(target: &str) -> Theme {
    if target.ends_with("unparse.rs") {
        Theme::UnparseTemplates
    } else if target.ends_with("parse.rs") {
        Theme::ParserBoilerplate
    } else {
        Theme::Damage
    }
}

fn create_log_dir(theme: Theme, iteration: u32) -> Result<PathBuf> {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before unix epoch")?
        .as_secs();
    let dir =
        refactor_hotspot_log_root().join(format!("{since_epoch}-{}-{iteration}", theme.label()));
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    Ok(dir)
}

fn build_inventory(opts: &Options, selected: &SelectedRefactor) -> Result<String> {
    let target_files = &selected.files;
    let stats = render_hotspot_stats(opts.churn_window)?;
    let snippets = render_code_snippets(&target_files)?;
    let grammar_rules = render_grammar_rule_inventory()?;

    Ok(format!(
        "\
# Grammar Refactor Inventory: {theme}

You are working in `mtg-parser`. This is a refactoring task, not a card-coverage task.

## Selection

{selection_reason}

## Selected Files

{files}

## Hotspot Stats

Churn is touches in the last {churn_window} commits. LOC is current working tree line count.

```text
{stats}```

## Grammar Rule Inventory

```text
{grammar_rules}```

## Code Context

{snippets}
",
        theme = selected.theme.label(),
        selection_reason = selected.reason,
        files = target_files
            .iter()
            .map(|p| format!("- `{p}`"))
            .collect::<Vec<_>>()
            .join("\n"),
        churn_window = opts.churn_window,
        stats = stats,
        grammar_rules = grammar_rules,
        snippets = snippets,
    ))
}

fn build_cluster_prompt(
    opts: &Options,
    selected: &SelectedRefactor,
    inventory: &str,
    iteration: u32,
) -> Result<String> {
    let rules = crate::rules_context::render_rules_block(selected.theme.qmd_query());
    Ok(format!(
        "\
# Refactor Cluster Stage: {theme}

You are working in `mtg-parser`. This is autonomous refactor iteration {iteration} of {max_iterations}.

Your only task in this stage is to cluster the current grammar surface. Do not edit files.

Use the Comprehensive Rules context top-down: identify game concepts first, then map existing
sentence-shaped grammar/AST/parser/unparser pieces onto those concepts.

## Output Contract

Write a concise markdown report with:

1. `## Clusters` - concept clusters, each with related grammar rules/AST variants/parser/unparser pieces.
2. `## Recommended Cluster` - the single cluster that should be generalized next.
3. `## Why` - why that cluster has the best leverage now.
4. `## Risks` - behavior/corpus risks to preserve.

Prefer clusters that reduce grammar shape and parser shape together.

## Theme Guidance

{theme_instructions}

{rules}

{inventory}
",
        theme = selected.theme.label(),
        iteration = iteration,
        max_iterations = opts.max_iterations,
        theme_instructions = selected.theme.instructions(),
        rules = rules,
        inventory = inventory,
    ))
}

fn build_plan_prompt(
    opts: &Options,
    selected: &SelectedRefactor,
    inventory: &str,
    clusters: &str,
    iteration: u32,
) -> Result<String> {
    let rules = crate::rules_context::render_rules_block(selected.theme.qmd_query());
    Ok(format!(
        "\
# Refactor Plan Stage: {theme}

You are working in `mtg-parser`. This is autonomous refactor iteration {iteration} of {max_iterations}.

Your only task in this stage is to turn the recommended cluster into a bounded implementation plan.
Do not edit files.

## Output Contract

Write a concise markdown plan with:

1. `## Target Abstraction` - the phenomenon-shaped grammar/AST shape to move toward.
2. `## Code Changes` - exact files and high-level edits.
3. `## Preservation Checks` - representative examples/tests/corpus risks.
4. `## Stop Line` - what must not be included in this iteration.

The plan must be small enough for one implementation pass.

## Theme Guidance

{theme_instructions}

{rules}

## Cluster Report

{clusters}

{inventory}
",
        theme = selected.theme.label(),
        iteration = iteration,
        max_iterations = opts.max_iterations,
        theme_instructions = selected.theme.instructions(),
        rules = rules,
        clusters = clusters,
        inventory = inventory,
    ))
}

fn build_implementation_prompt(
    opts: &Options,
    selected: &SelectedRefactor,
    inventory: &str,
    plan: &str,
    iteration: u32,
) -> Result<String> {
    Ok(format!(
        "\
# Refactor Implementation Stage: {theme}

You are working in `mtg-parser`. This is autonomous refactor iteration {iteration} of {max_iterations}.

Implement exactly the plan below. Do not re-cluster. Do not expand the scope. Do not edit unrelated
tooling, audit pages, or workflow files.

## Required Constraints

1. Preserve corpus behavior unless the refactor naturally fixes existing failures.
2. Do not add one-card special cases.
3. Prefer phenomenon-shaped rules and AST nodes over sentence-shaped variants.
4. Treat grammar, AST, parse, and unparse as one coupled surface.
5. Stop after one coherent refactor. Do not start a second unrelated cleanup.
6. The orchestrator owns `cargo test`, `cargo xtask corpus`, `just audit-page`, and commit.

## Theme Guidance

{theme_instructions}

## Selected Files

{files}

## Approved Plan

{plan}

## Inventory

{inventory}
",
        theme = selected.theme.label(),
        iteration = iteration,
        max_iterations = opts.max_iterations,
        theme_instructions = selected.theme.instructions(),
        files = selected
            .files
            .iter()
            .map(|p| format!("- `{p}`"))
            .collect::<Vec<_>>()
            .join("\n"),
        plan = plan,
        inventory = inventory,
    ))
}

fn render_hotspot_stats(churn_window: usize) -> Result<String> {
    let root = repo_root();
    let mut out = String::new();
    out.push_str("file                                           churn   loc\n");
    out.push_str("-----------------------------------------------------------\n");
    for (label, rel) in HOT_FILES {
        let churn = git_churn(rel, churn_window)?;
        let loc = line_count(&root.join(rel)).unwrap_or(0);
        out.push_str(&format!("{label:<45} {churn:>5} {loc:>6}\n"));
    }
    Ok(out)
}

fn render_code_snippets(files: &[String]) -> Result<String> {
    let root = repo_root();
    let mut out = String::new();
    for rel in files {
        let path = root.join(rel);
        if !is_repo_relative(rel) || !path.exists() {
            out.push_str(&format!(
                "### `{rel}`\n\n_File not found or not repo-relative._\n\n"
            ));
            continue;
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("read {rel}"))?;
        out.push_str(&format!("### `{rel}`\n\n```rust\n"));
        out.push_str(&first_lines(&text, 220));
        out.push_str("\n```\n\n");
    }
    Ok(out)
}

fn render_grammar_rule_inventory() -> Result<String> {
    let rel = "crates/mtg-grammar/src/grammar.pest";
    let path = repo_root().join(rel);
    let text = std::fs::read_to_string(&path).with_context(|| format!("read {rel}"))?;
    let mut out = String::new();

    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        let Some((name, _)) = trimmed.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty()
            || !name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            continue;
        }
        out.push_str(&format!("{:>4}: {name}\n", idx + 1));
    }

    if out.is_empty() {
        bail!("no grammar rules found in {rel}");
    }
    Ok(out)
}

fn is_repo_relative(path: &str) -> bool {
    let p = Path::new(path);
    !p.is_absolute() && !path.split('/').any(|part| part == "..")
}

fn first_lines(text: &str, max: usize) -> String {
    let mut out = String::new();
    for (idx, line) in text.lines().enumerate() {
        if idx >= max {
            out.push_str(&format!("// ... truncated at {max} lines ..."));
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn line_count(path: &Path) -> Result<usize> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(text.lines().count())
}

fn git_churn(path: &str, window: usize) -> Result<usize> {
    let output = Command::new("git")
        .args([
            "log",
            "--format=",
            "--name-only",
            "-n",
            &window.to_string(),
            "HEAD",
            "--",
            path,
        ])
        .output()
        .context("run git log")?;
    if !output.status.success() {
        bail!("git log failed with {}", output.status);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().filter(|line| *line == path).count())
}

struct AgentOutcome {
    success: bool,
    exit_code: i32,
    assistant_text: String,
}

fn invoke_agent(
    provider: AgentProvider,
    prompt: &str,
    transcript_path: &Path,
    sink: &mut dyn FlowSink,
) -> Result<AgentOutcome> {
    let command = base_agent_command(provider);
    invoke_jsonl_agent(provider, command, prompt, transcript_path, sink)
}

fn base_agent_command(provider: AgentProvider) -> Command {
    match provider {
        AgentProvider::Codex => {
            let mut cmd = Command::new("codex");
            cmd.arg("exec")
                .arg("--dangerously-bypass-approvals-and-sandbox")
                .arg("--json")
                .arg("--cd")
                .arg(repo_root())
                .arg("-");
            cmd
        }
        AgentProvider::Claude => {
            let mut cmd = Command::new("claude");
            cmd.arg("-p")
                .arg("--dangerously-skip-permissions")
                .arg("--output-format")
                .arg("stream-json")
                .arg("--verbose");
            cmd
        }
    }
}

fn invoke_jsonl_agent(
    provider: AgentProvider,
    mut command: Command,
    prompt: &str,
    transcript_path: &Path,
    sink: &mut dyn FlowSink,
) -> Result<AgentOutcome> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .current_dir(repo_root())
        .spawn()
        .with_context(|| format!("spawn {}", provider.label()))?;
    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        stdin.write_all(prompt.as_bytes())?;
    }

    let stdout = child.stdout.take().expect("piped stdout");
    let reader = BufReader::new(stdout);
    let mut transcript = std::fs::File::create(transcript_path)
        .with_context(|| format!("create {}", transcript_path.display()))?;
    let start = Instant::now();
    let mut assistant_text = Vec::<String>::new();

    for raw in reader.lines() {
        let line = raw?;
        writeln!(transcript, "{line}")?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(&line) {
            Ok(parsed) => {
                collect_assistant_text(provider, &parsed, &mut assistant_text);
                let elapsed_secs = start.elapsed().as_secs();
                sink.emit(FlowEvent::AgentEvent {
                    provider,
                    raw: parsed,
                    elapsed_secs,
                });
            }
            Err(_) => sink.emit(FlowEvent::Note {
                level: NoteLevel::Warn,
                text: format!(
                    "non-JSON line from {}: {}",
                    provider.label(),
                    trim(&line, 200)
                ),
            }),
        }
    }

    let status = child.wait()?;
    Ok(AgentOutcome {
        success: status.success(),
        exit_code: status.code().unwrap_or(-1),
        assistant_text: assistant_text.join("\n\n"),
    })
}

fn collect_assistant_text(
    provider: AgentProvider,
    parsed: &serde_json::Value,
    assistant_text: &mut Vec<String>,
) {
    match provider {
        AgentProvider::Claude => {
            if parsed.get("type").and_then(|v| v.as_str()) != Some("assistant") {
                return;
            }
            if let Some(content) = parsed
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array())
            {
                for c in content {
                    if c.get("type").and_then(|v| v.as_str()) == Some("text") {
                        if let Some(t) = c.get("text").and_then(|v| v.as_str()) {
                            assistant_text.push(t.to_string());
                        }
                    }
                }
            }
        }
        AgentProvider::Codex => {
            let kind = parsed
                .get("type")
                .or_else(|| parsed.get("event"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !(kind.contains("assistant") || kind.contains("message") || kind.contains("text")) {
                return;
            }
            for key in ["text", "message", "content", "output"] {
                if let Some(text) = parsed.get(key).and_then(|v| v.as_str()) {
                    assistant_text.push(text.to_string());
                    return;
                }
            }
        }
    }
}

fn trim(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n).collect();
        out.push('…');
        out
    }
}

fn run_gate(label: &str, program: &str, args: &[&str]) -> Result<()> {
    let out = Command::new(program)
        .args(args)
        .current_dir(repo_root())
        .output()
        .with_context(|| format!("run {label}"))?;
    if !out.status.success() {
        bail!(
            "{label} failed with {}\n{}",
            out.status,
            command_output_tail(&out, 40)
        );
    }
    Ok(())
}

fn command_output_tail(out: &std::process::Output, max_lines: usize) -> String {
    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(&out.stdout));
    if !out.stderr.is_empty() {
        if !text.ends_with('\n') && !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&String::from_utf8_lossy(&out.stderr));
    }
    let lines = text.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return "(no output)".to_string();
    }
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

fn read_corpus_pp_total() -> (usize, usize) {
    let path = repo_root().join("corpus_status.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return (0, 0);
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return (0, 0);
    };
    let total = value.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let passing = value.get("passing").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    (passing, total)
}

fn count_grammar_rules() -> usize {
    render_grammar_rule_inventory()
        .map(|rules| rules.lines().count())
        .unwrap_or(0)
}

fn ensure_clean_working_tree() -> Result<()> {
    let out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_root())
        .output()
        .context("git status --porcelain")?;
    if !out.status.success() {
        bail!("git status failed");
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    if !stdout.trim().is_empty() {
        bail!("working tree is dirty:\n{stdout}");
    }
    Ok(())
}

enum CommitOutcome {
    Committed,
    NoChanges,
}

fn git_commit(message: &str) -> Result<CommitOutcome> {
    let add = Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root())
        .status()
        .context("git add -A")?;
    if !add.success() {
        bail!("git add failed");
    }

    let diff = Command::new("git")
        .args(["diff", "--cached", "--quiet", "--exit-code"])
        .current_dir(repo_root())
        .status()
        .context("git diff --cached")?;
    if diff.success() {
        return Ok(CommitOutcome::NoChanges);
    }
    if diff.code() != Some(1) {
        bail!("git diff --cached failed");
    }

    let commit = Command::new("git")
        .args(["commit", "--no-verify", "-m", message])
        .current_dir(repo_root())
        .output()
        .context("git commit")?;
    if !commit.status.success() {
        bail!(
            "git commit failed with status {}\nstdout:\n{}\nstderr:\n{}",
            commit.status,
            String::from_utf8_lossy(&commit.stdout),
            String::from_utf8_lossy(&commit.stderr)
        );
    }
    Ok(CommitOutcome::Committed)
}

fn commit_message(selected: &SelectedRefactor, iteration: u32) -> Result<String> {
    let stat = git_diff_stat()?;
    Ok(format!(
        "Refactor {} hotspot iteration {}\n\nGates: cargo test; cargo xtask corpus; just audit-page.\nBehavior: intended unchanged.\nPrimary LOC delta:\n{}",
        selected.theme.label(),
        iteration,
        stat.trim()
    ))
}

fn git_diff_stat() -> Result<String> {
    let out = Command::new("git")
        .args(["diff", "--stat"])
        .current_dir(repo_root())
        .output()
        .context("git diff --stat")?;
    if !out.status.success() {
        bail!("git diff --stat failed");
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    if text.trim().is_empty() {
        Ok("(no diff)".to_string())
    } else {
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_theme_aliases() {
        assert_eq!(Theme::parse("grammar").unwrap(), Theme::GrammarCore);
        assert_eq!(Theme::parse("parser").unwrap(), Theme::ParserBoilerplate);
        assert_eq!(Theme::parse("damage").unwrap(), Theme::Damage);
        assert_eq!(Theme::parse("keywords").unwrap(), Theme::KeywordAbilities);
    }

    #[test]
    fn rejects_unknown_theme() {
        assert!(Theme::parse("espresso").is_err());
    }

    #[test]
    fn options_swallow_ui_flag() {
        let args = vec!["--ui".to_string(), "tui".to_string()];
        Options::parse(&args).expect("--ui tui should not error");
    }

    #[test]
    fn repo_relative_path_guard() {
        assert!(is_repo_relative("crates/mtg-grammar/src/parse.rs"));
        assert!(!is_repo_relative("../parse.rs"));
        assert!(!is_repo_relative("/tmp/parse.rs"));
    }

    #[test]
    fn first_lines_truncates() {
        let text = "a\nb\nc";
        assert_eq!(
            first_lines(text, 2),
            "a\nb\n// ... truncated at 2 lines ..."
        );
    }
}
