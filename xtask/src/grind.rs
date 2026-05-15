//! `cargo xtask grind` — the autonomous TDD-style meta-loop.
//!
//! Phase 1: run refactor-hotspot iterations one at a time, counting
//! consecutive no-ops. Stop the refactor phase when the streak hits
//! `--stop-after` or the iteration ceiling.
//!
//! Phase 2: hand off to add-card on the cleaner grammar foundation.
//!
//! Gate failures in either phase are routed to a freeform repair agent
//! before grind gives up — the user-facing intent is "I want to walk
//! away; don't stop for human input."

use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, fmt};

use anyhow::{anyhow, bail, Context, Result};

use crate::add_card;
use crate::console_sink::ConsoleSink;
use crate::flow::{AgentProvider, FlowEvent, FlowSink, NoteLevel};
use crate::paths::{grind_log_root, repo_root};
use crate::refactor_hotspot::{self, IterationOutcome};

const DEFAULT_STOP_AFTER: u32 = 3;
const DEFAULT_MAX_REFACTOR_ITERATIONS: u32 = 50;
const DEFAULT_MAX_COMMITS_PER_THEME: u32 = 5;
const DEFAULT_MAX_REPAIRS_PER_THEME: u32 = 2;
const DEFAULT_LOW_VALUE_STOP_AFTER: u32 = 2;
const DEFAULT_REPAIR_ATTEMPTS: u8 = 1;
const AUTO_REFACTOR_THEMES: &[&str] = &[
    "damage",
    "destroy",
    "prevention",
    "triggered-abilities",
    "keyword-abilities",
    "unparse-templates",
];

const HELP: &str = "\
cargo xtask grind [--set CODE]
                  [--stop-after N] [--max-refactor-iterations N]
                  [--max-commits-per-theme N] [--max-repairs-per-theme N]
                  [--low-value-stop-after N]
                  [--max-card-iterations N] [--repair-attempts N]
                  [--theme THEME] [--target PATH] [--fixed-refactor-target]
                  [--agent codex|claude] [--ui console|tui]
                  [--allow-dirty] [--dry-run]

Autonomous TDD-style meta-loop. Phase 1 automatically picks a narrow
effect-frame refactor theme, runs refactor-hotspot until that theme reaches
its no-op, commit, repair, or low-value budget, then picks the next theme.
Explicit --theme or --target keeps the old fixed-target behavior. Phase 2 then
runs add-card on the cleaner foundation. Gate failures route to a repair agent
before giving up.
";

pub fn run(args: &[String]) -> ExitCode {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    let opts = match Options::parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("grind: {e:#}");
            return ExitCode::FAILURE;
        }
    };
    let mut sink: Box<dyn FlowSink> = Box::new(ConsoleSink::new());
    match run_with_sink(opts, sink.as_mut()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("grind: {e:#}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    pub set: Option<String>,
    pub stop_after: u32,
    pub max_refactor_iterations: u32,
    pub max_commits_per_theme: u32,
    pub max_repairs_per_theme: u32,
    pub low_value_stop_after: u32,
    /// 0 = unbounded (matches add-card's convention).
    pub max_card_iterations: u32,
    pub repair_attempts: u8,
    pub agent: AgentProvider,
    pub theme: Option<String>,
    pub target: Option<String>,
    pub fixed_refactor_target: bool,
    pub allow_dirty: bool,
    pub dry_run: bool,
}

impl Options {
    pub fn parse(args: &[String]) -> Result<Self> {
        let mut set = None::<String>;
        let mut stop_after = DEFAULT_STOP_AFTER;
        let mut max_refactor_iterations = DEFAULT_MAX_REFACTOR_ITERATIONS;
        let mut max_commits_per_theme = DEFAULT_MAX_COMMITS_PER_THEME;
        let mut max_repairs_per_theme = DEFAULT_MAX_REPAIRS_PER_THEME;
        let mut low_value_stop_after = DEFAULT_LOW_VALUE_STOP_AFTER;
        let mut max_card_iterations = 0u32;
        let mut repair_attempts = DEFAULT_REPAIR_ATTEMPTS;
        let mut agent = AgentProvider::Codex;
        let mut theme = None::<String>;
        let mut target = None::<String>;
        let mut fixed_refactor_target = false;
        let mut allow_dirty = false;
        let mut dry_run = false;

        let mut iter = args.iter();
        while let Some(a) = iter.next() {
            match a.as_str() {
                "--set" => set = iter.next().cloned(),
                s if s.starts_with("--set=") => set = Some(s["--set=".len()..].to_string()),
                "--stop-after" => {
                    let v = iter
                        .next()
                        .ok_or_else(|| anyhow!("--stop-after requires a value"))?;
                    stop_after = v
                        .parse()
                        .with_context(|| format!("--stop-after value: {v:?}"))?;
                }
                s if s.starts_with("--stop-after=") => {
                    stop_after = s["--stop-after=".len()..]
                        .parse()
                        .with_context(|| format!("--stop-after value: {s:?}"))?;
                }
                "--max-refactor-iterations" => {
                    let v = iter
                        .next()
                        .ok_or_else(|| anyhow!("--max-refactor-iterations requires a value"))?;
                    max_refactor_iterations = v
                        .parse()
                        .with_context(|| format!("--max-refactor-iterations value: {v:?}"))?;
                }
                s if s.starts_with("--max-refactor-iterations=") => {
                    max_refactor_iterations = s["--max-refactor-iterations=".len()..]
                        .parse()
                        .with_context(|| format!("--max-refactor-iterations value: {s:?}"))?;
                }
                "--max-commits-per-theme" => {
                    let v = iter
                        .next()
                        .ok_or_else(|| anyhow!("--max-commits-per-theme requires a value"))?;
                    max_commits_per_theme = v
                        .parse()
                        .with_context(|| format!("--max-commits-per-theme value: {v:?}"))?;
                }
                s if s.starts_with("--max-commits-per-theme=") => {
                    max_commits_per_theme = s["--max-commits-per-theme=".len()..]
                        .parse()
                        .with_context(|| format!("--max-commits-per-theme value: {s:?}"))?;
                }
                "--max-repairs-per-theme" => {
                    let v = iter
                        .next()
                        .ok_or_else(|| anyhow!("--max-repairs-per-theme requires a value"))?;
                    max_repairs_per_theme = v
                        .parse()
                        .with_context(|| format!("--max-repairs-per-theme value: {v:?}"))?;
                }
                s if s.starts_with("--max-repairs-per-theme=") => {
                    max_repairs_per_theme = s["--max-repairs-per-theme=".len()..]
                        .parse()
                        .with_context(|| format!("--max-repairs-per-theme value: {s:?}"))?;
                }
                "--low-value-stop-after" => {
                    let v = iter
                        .next()
                        .ok_or_else(|| anyhow!("--low-value-stop-after requires a value"))?;
                    low_value_stop_after = v
                        .parse()
                        .with_context(|| format!("--low-value-stop-after value: {v:?}"))?;
                }
                s if s.starts_with("--low-value-stop-after=") => {
                    low_value_stop_after = s["--low-value-stop-after=".len()..]
                        .parse()
                        .with_context(|| format!("--low-value-stop-after value: {s:?}"))?;
                }
                "--max-card-iterations" => {
                    let v = iter
                        .next()
                        .ok_or_else(|| anyhow!("--max-card-iterations requires a value"))?;
                    max_card_iterations = v
                        .parse()
                        .with_context(|| format!("--max-card-iterations value: {v:?}"))?;
                }
                s if s.starts_with("--max-card-iterations=") => {
                    max_card_iterations = s["--max-card-iterations=".len()..]
                        .parse()
                        .with_context(|| format!("--max-card-iterations value: {s:?}"))?;
                }
                "--repair-attempts" => {
                    let v = iter
                        .next()
                        .ok_or_else(|| anyhow!("--repair-attempts requires a value"))?;
                    repair_attempts = v
                        .parse()
                        .with_context(|| format!("--repair-attempts value: {v:?}"))?;
                }
                s if s.starts_with("--repair-attempts=") => {
                    repair_attempts = s["--repair-attempts=".len()..]
                        .parse()
                        .with_context(|| format!("--repair-attempts value: {s:?}"))?;
                }
                "--agent" => {
                    let v = iter
                        .next()
                        .ok_or_else(|| anyhow!("--agent requires a value"))?;
                    agent = parse_agent(v)?;
                }
                s if s.starts_with("--agent=") => {
                    agent = parse_agent(&s["--agent=".len()..])?;
                }
                "--theme" => theme = iter.next().cloned(),
                s if s.starts_with("--theme=") => {
                    theme = Some(s["--theme=".len()..].to_string());
                }
                "--target" => target = iter.next().cloned(),
                s if s.starts_with("--target=") => {
                    target = Some(s["--target=".len()..].to_string());
                }
                "--fixed-refactor-target" => fixed_refactor_target = true,
                "--allow-dirty" => allow_dirty = true,
                "--dry-run" => dry_run = true,
                // --ui is consumed by main.rs before we get here; tolerate it.
                "--ui" => {
                    let _ = iter.next();
                }
                s if s.starts_with("--ui=") => {}
                other => bail!("unknown argument: {other}"),
            }
        }

        if stop_after == 0 {
            bail!("--stop-after must be greater than 0");
        }
        if max_refactor_iterations == 0 {
            bail!("--max-refactor-iterations must be greater than 0");
        }
        if max_commits_per_theme == 0 {
            bail!("--max-commits-per-theme must be greater than 0");
        }
        if max_repairs_per_theme == 0 {
            bail!("--max-repairs-per-theme must be greater than 0");
        }
        if low_value_stop_after == 0 {
            bail!("--low-value-stop-after must be greater than 0");
        }

        Ok(Self {
            set,
            stop_after,
            max_refactor_iterations,
            max_commits_per_theme,
            max_repairs_per_theme,
            low_value_stop_after,
            max_card_iterations,
            repair_attempts,
            agent,
            theme,
            target,
            fixed_refactor_target,
            allow_dirty,
            dry_run,
        })
    }
}

fn parse_agent(value: &str) -> Result<AgentProvider> {
    match value {
        "codex" => Ok(AgentProvider::Codex),
        "claude" => Ok(AgentProvider::Claude),
        other => bail!("--agent must be 'codex' or 'claude', got {other:?}"),
    }
}

#[derive(Debug, Clone, Default)]
struct ThemeState {
    commits: u32,
    repairs: u32,
    no_ops: u32,
    low_value_commits: u32,
    last_corpus_passing: Option<usize>,
    last_grammar_rules: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExhaustionReason {
    NoOps,
    CommitBudget,
    RepairBudget,
    LowValue,
}

impl fmt::Display for ExhaustionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoOps => write!(f, "no-op budget reached"),
            Self::CommitBudget => write!(f, "commit budget reached"),
            Self::RepairBudget => write!(f, "repair budget reached"),
            Self::LowValue => write!(f, "low-value commit budget reached"),
        }
    }
}

impl ThemeState {
    fn selected(&mut self) {
        self.no_ops = 0;
        self.last_corpus_passing = Some(refactor_hotspot::read_corpus_pp_total().0);
        self.last_grammar_rules = Some(refactor_hotspot::count_grammar_rules());
    }

    fn record_commit(
        &mut self,
        corpus_passing: usize,
        grammar_rules: usize,
        opts: &Options,
    ) -> Option<ExhaustionReason> {
        self.commits += 1;
        self.no_ops = 0;

        let corpus_improved = self
            .last_corpus_passing
            .map(|previous| corpus_passing > previous)
            .unwrap_or(false);
        let grammar_simplified = self
            .last_grammar_rules
            .map(|previous| grammar_rules < previous)
            .unwrap_or(false);
        if corpus_improved || grammar_simplified {
            self.low_value_commits = 0;
        } else {
            self.low_value_commits += 1;
        }

        self.last_corpus_passing = Some(corpus_passing);
        self.last_grammar_rules = Some(grammar_rules);

        self.exhaustion_reason(opts)
    }

    fn record_no_change(&mut self, opts: &Options) -> Option<ExhaustionReason> {
        self.no_ops += 1;
        self.exhaustion_reason(opts)
    }

    fn record_repair(&mut self, opts: &Options) -> Option<ExhaustionReason> {
        self.repairs += 1;
        self.no_ops = 0;
        self.exhaustion_reason(opts)
    }

    fn exhaustion_reason(&self, opts: &Options) -> Option<ExhaustionReason> {
        if self.no_ops >= opts.stop_after {
            Some(ExhaustionReason::NoOps)
        } else if self.commits >= opts.max_commits_per_theme {
            Some(ExhaustionReason::CommitBudget)
        } else if self.repairs >= opts.max_repairs_per_theme {
            Some(ExhaustionReason::RepairBudget)
        } else if self.low_value_commits >= opts.low_value_stop_after {
            Some(ExhaustionReason::LowValue)
        } else {
            None
        }
    }
}

pub fn run_with_sink(opts: Options, sink: &mut dyn FlowSink) -> Result<ExitCode> {
    if !opts.dry_run && !opts.allow_dirty {
        ensure_clean_working_tree()
            .context("working tree must be clean (or pass --allow-dirty)")?;
    }

    run_refactor_phase(&opts, sink)?;
    run_add_card_phase(&opts, sink)
}

fn run_refactor_phase(opts: &Options, sink: &mut dyn FlowSink) -> Result<()> {
    // Emit SessionStarted up-front so TUI sinks (session bar, complexity
    // strip) have something to render. refactor_hotspot::run_with_sink
    // does this for the standalone command; grind drives
    // run_single_iteration directly, so we own this emission instead.
    let (baseline_corpus_passing, baseline_corpus_total) = refactor_hotspot::read_corpus_pp_total();
    let baseline_grammar_rules = refactor_hotspot::count_grammar_rules();
    let session_set = opts
        .theme
        .clone()
        .or_else(|| opts.target.clone())
        .unwrap_or_else(|| "grammar-core".to_string());
    sink.emit(FlowEvent::SessionStarted {
        workflow: "grind".to_string(),
        set: session_set,
        max_iterations: opts.max_refactor_iterations,
        baseline_corpus_passing,
        baseline_corpus_total,
        baseline_grammar_rules,
    });

    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: format!(
            "grind: refactor phase — stop each target after {} consecutive no-ops (ceiling {})",
            opts.stop_after, opts.max_refactor_iterations
        ),
    });

    let fixed_mode = opts.fixed_refactor_target || opts.theme.is_some() || opts.target.is_some();
    let mut current_theme = opts.theme.clone();
    let mut exhausted_themes = Vec::<String>::new();
    let mut theme_states = HashMap::<String, ThemeState>::new();
    for iteration in 1..=opts.max_refactor_iterations {
        if !fixed_mode && current_theme.is_none() {
            if all_auto_themes_exhausted(&exhausted_themes) {
                sink.emit(FlowEvent::Note {
                    level: NoteLevel::Info,
                    text: "grind: refactor phase complete — all automatic targets are quiet"
                        .to_string(),
                });
                return Ok(());
            }
            current_theme = Some(select_next_refactor_theme(
                opts,
                sink,
                iteration,
                &exhausted_themes,
            )?);
            let key = active_refactor_key(opts, current_theme.as_deref());
            theme_states.entry(key).or_default().selected();
        }

        // allow_dirty=true for the inner refactor opts because grind already
        // checked the precondition at the top. Subsequent iterations will
        // legitimately leave behind committed (or restored-after-repair)
        // state that the inner check would reject.
        if opts.dry_run {
            sink.emit(FlowEvent::Note {
                level: NoteLevel::Info,
                text: format!(
                    "grind: dry-run would run refactor-hotspot for `{}`",
                    active_refactor_key(opts, current_theme.as_deref())
                ),
            });
            return Ok(());
        }

        let refactor_opts = refactor_hotspot::Options::for_grind(
            current_theme.as_deref(),
            opts.target.clone(),
            opts.agent,
            true,
        )?;

        let outcome = refactor_hotspot::run_single_iteration(&refactor_opts, sink, iteration)?;
        match outcome {
            IterationOutcome::Committed {
                new_passes,
                corpus_passing,
                corpus_total,
                grammar_rules,
                duration_secs,
            } => {
                let key = active_refactor_key(opts, current_theme.as_deref());
                let reason = theme_states.entry(key).or_default().record_commit(
                    corpus_passing,
                    grammar_rules,
                    opts,
                );
                sink.emit(FlowEvent::Note {
                    level: NoteLevel::Info,
                    text: format!(
                        "grind: iter {iteration} committed +{new_passes} ({corpus_passing}/{corpus_total}) — {grammar_rules} grammar rules, {duration_secs}s"
                    ),
                });
                if let Some(reason) = reason {
                    if fixed_mode {
                        sink.emit(FlowEvent::Note {
                            level: NoteLevel::Info,
                            text: format!(
                                "grind: refactor phase complete — current target {reason}"
                            ),
                        });
                        return Ok(());
                    }
                    mark_theme_exhausted(
                        &mut exhausted_themes,
                        current_theme.as_deref(),
                        reason,
                        sink,
                    );
                    current_theme = None;
                }
            }
            IterationOutcome::NoChanges => {
                let key = active_refactor_key(opts, current_theme.as_deref());
                let state = theme_states.entry(key).or_default();
                let reason = state.record_no_change(opts);
                sink.emit(FlowEvent::Note {
                    level: NoteLevel::Info,
                    text: format!(
                        "grind: target no-op {}/{} (iteration {})",
                        state.no_ops, opts.stop_after, iteration
                    ),
                });
                if let Some(reason) = reason {
                    if fixed_mode {
                        sink.emit(FlowEvent::Note {
                            level: NoteLevel::Info,
                            text: format!(
                                "grind: refactor phase complete — current target {reason} after {iteration} iterations"
                            ),
                        });
                        return Ok(());
                    }
                    mark_theme_exhausted(
                        &mut exhausted_themes,
                        current_theme.as_deref(),
                        reason,
                        sink,
                    );
                    current_theme = None;
                }
            }
            IterationOutcome::GateFailed(error) => {
                sink.emit(FlowEvent::Note {
                    level: NoteLevel::Warn,
                    text: format!("grind: refactor gate failed: {error:#}"),
                });
                let repaired = try_repair(opts, sink, "refactor", iteration, &error)?;
                if repaired {
                    let key = active_refactor_key(opts, current_theme.as_deref());
                    let reason = theme_states.entry(key).or_default().record_repair(opts);
                    if let Some(reason) = reason {
                        if fixed_mode {
                            sink.emit(FlowEvent::Note {
                                level: NoteLevel::Info,
                                text: format!(
                                    "grind: refactor phase complete — current target {reason}"
                                ),
                            });
                            return Ok(());
                        }
                        mark_theme_exhausted(
                            &mut exhausted_themes,
                            current_theme.as_deref(),
                            reason,
                            sink,
                        );
                        current_theme = None;
                    }
                } else {
                    // Repair couldn't fix it. Restore the tree so the next
                    // iteration starts clean, count this as a no-op so the
                    // loop still terminates.
                    discard_working_changes(sink)?;
                    let key = active_refactor_key(opts, current_theme.as_deref());
                    let state = theme_states.entry(key).or_default();
                    let reason = state.record_no_change(opts);
                    sink.emit(FlowEvent::Note {
                        level: NoteLevel::Warn,
                        text: format!(
                            "grind: repair exhausted, treating as no-op {}/{}",
                            state.no_ops, opts.stop_after
                        ),
                    });
                    if let Some(reason) = reason {
                        if fixed_mode {
                            sink.emit(FlowEvent::Note {
                                level: NoteLevel::Info,
                                text: format!(
                                    "grind: refactor phase complete — current target {reason}"
                                ),
                            });
                            return Ok(());
                        }
                        mark_theme_exhausted(
                            &mut exhausted_themes,
                            current_theme.as_deref(),
                            reason,
                            sink,
                        );
                        current_theme = None;
                    }
                }
            }
        }
    }

    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: format!(
            "grind: refactor phase hit max iterations ({})",
            opts.max_refactor_iterations
        ),
    });
    Ok(())
}

fn select_next_refactor_theme(
    opts: &Options,
    sink: &mut dyn FlowSink,
    iteration: u32,
    exhausted_themes: &[String],
) -> Result<String> {
    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: format!("grind: selecting next refactor target for iteration {iteration}"),
    });

    if opts.dry_run {
        let theme = fallback_theme(iteration, exhausted_themes);
        sink.emit(FlowEvent::Note {
            level: NoteLevel::Info,
            text: format!("grind: dry-run selected fallback refactor theme `{theme}`"),
        });
        return Ok(theme);
    }

    let log_dir = create_log_dir("target-selection", iteration)?;
    let prompt = build_target_selection_prompt(iteration, exhausted_themes)?;
    std::fs::write(log_dir.join("prompt.md"), &prompt)
        .with_context(|| format!("write {}", log_dir.join("prompt.md").display()))?;

    let transcript_path = log_dir.join("transcript.ndjson");
    let outcome = refactor_hotspot::invoke_agent(opts.agent, &prompt, &transcript_path, sink)?;
    std::fs::write(log_dir.join("response.md"), &outcome.assistant_text)
        .with_context(|| format!("write {}", log_dir.join("response.md").display()))?;

    let theme = if outcome.success {
        parse_theme_choice(&outcome.assistant_text)
            .filter(|theme| !exhausted_themes.iter().any(|done| done == theme))
            .unwrap_or_else(|| fallback_theme(iteration, exhausted_themes))
    } else {
        fallback_theme(iteration, exhausted_themes)
    };
    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: format!("grind: selected refactor theme `{theme}`"),
    });
    Ok(theme)
}

fn build_target_selection_prompt(iteration: u32, exhausted_themes: &[String]) -> Result<String> {
    let git_status = command_stdout("git", &["status", "--short"])
        .unwrap_or_else(|_| "(git status unavailable)\n".to_string());
    let recent_commits = command_stdout("git", &["log", "--oneline", "-n", "12"])
        .unwrap_or_else(|_| "(git log unavailable)\n".to_string());
    let diff_stat = command_stdout("git", &["diff", "--stat"])
        .unwrap_or_else(|_| "(git diff unavailable)\n".to_string());
    let grammar_stats = grammar_surface_stats()?;
    let exhausted = if exhausted_themes.is_empty() {
        "(none)\n".to_string()
    } else {
        exhausted_themes
            .iter()
            .map(|theme| format!("- `{theme}`"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    };

    Ok(format!(
        "\
You are selecting the next autonomous grammar refactor target for `mtg-parser`.
This is iteration {iteration} of the outer grind loop.

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

{exhausted}

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
{grammar_stats}```

## Git Status

```text
{git_status}```

## Current Diff Stat

```text
{diff_stat}```

## Recent Commits

```text
{recent_commits}```
"
    ))
}

fn parse_theme_choice(text: &str) -> Option<String> {
    const THEMES: &[&str] = &[
        "damage",
        "destroy",
        "prevention",
        "keyword-abilities",
        "triggered-abilities",
        "unparse-templates",
        "parser-boilerplate",
        "grammar-core",
    ];
    for line in text.lines().rev() {
        let lower = line.trim().to_ascii_lowercase();
        let value = lower
            .strip_prefix("theme:")
            .map(str::trim)
            .unwrap_or(lower.trim());
        for theme in THEMES {
            if value == *theme {
                return Some((*theme).to_string());
            }
        }
    }
    None
}

fn fallback_theme(iteration: u32, exhausted_themes: &[String]) -> String {
    let start = (iteration.saturating_sub(1)) as usize;
    for offset in 0..AUTO_REFACTOR_THEMES.len() {
        let theme = AUTO_REFACTOR_THEMES[(start + offset) % AUTO_REFACTOR_THEMES.len()];
        if !exhausted_themes.iter().any(|done| done == theme) {
            return theme.to_string();
        }
    }
    AUTO_REFACTOR_THEMES[0].to_string()
}

fn all_auto_themes_exhausted(exhausted_themes: &[String]) -> bool {
    AUTO_REFACTOR_THEMES
        .iter()
        .all(|theme| exhausted_themes.iter().any(|done| done == theme))
}

fn active_refactor_key(opts: &Options, theme: Option<&str>) -> String {
    theme
        .or(opts.target.as_deref())
        .unwrap_or("grammar-core")
        .to_string()
}

fn mark_theme_exhausted(
    exhausted_themes: &mut Vec<String>,
    theme: Option<&str>,
    reason: ExhaustionReason,
    sink: &mut dyn FlowSink,
) {
    let Some(theme) = theme else {
        return;
    };
    if !exhausted_themes.iter().any(|done| done == theme) {
        exhausted_themes.push(theme.to_string());
    }
    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: format!("grind: target `{theme}` exhausted ({reason}); selecting another target"),
    });
}

fn grammar_surface_stats() -> Result<String> {
    let root = repo_root();
    let files = [
        "crates/mtg-grammar/src/grammar.pest",
        "crates/mtg-grammar/src/ast.rs",
        "crates/mtg-grammar/src/parse.rs",
        "crates/mtg-grammar/src/unparse.rs",
    ];
    let mut out = String::new();
    for file in files {
        let path = root.join(file);
        let text = std::fs::read_to_string(&path).with_context(|| format!("read {file}"))?;
        let loc = text.lines().count();
        let count = if file.ends_with("grammar.pest") {
            text.lines()
                .filter(|line| {
                    let trimmed = line.trim_start();
                    trimmed
                        .split_once('=')
                        .map(|(name, _)| {
                            !trimmed.starts_with("//")
                                && !name.trim().is_empty()
                                && name
                                    .trim()
                                    .chars()
                                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                        })
                        .unwrap_or(false)
                })
                .count()
        } else {
            text.lines()
                .filter(|line| {
                    line.trim_start().starts_with("pub enum ")
                        || line.trim_start().starts_with("enum ")
                        || line.trim_start().starts_with("fn ")
                })
                .count()
        };
        out.push_str(&format!("{file:<42} loc={loc:<5} surface-count={count}\n"));
    }
    Ok(out)
}

fn run_add_card_phase(opts: &Options, sink: &mut dyn FlowSink) -> Result<ExitCode> {
    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: "grind: handoff to add-card phase".to_string(),
    });

    let add_card_opts = add_card::Options {
        set: opts.set.clone(),
        max_iterations: opts.max_card_iterations,
        // Reuse grind's repair budget for add-card's own supervisor —
        // they serve the same role.
        supervisor_attempts: opts.repair_attempts,
        dry_run: opts.dry_run,
        // Trust grind's top-level precondition; don't re-check.
        allow_dirty: true,
        agent: opts.agent,
    };

    let mut attempts = 0u8;
    loop {
        match add_card::run_with_sink(add_card_opts.clone(), sink) {
            Ok(code) => return Ok(code),
            Err(error) if attempts < opts.repair_attempts => {
                attempts += 1;
                sink.emit(FlowEvent::Note {
                    level: NoteLevel::Warn,
                    text: format!(
                        "grind: add-card failed (attempt {}/{}): {error:#}",
                        attempts, opts.repair_attempts
                    ),
                });
                let repaired = try_repair(opts, sink, "add-card", attempts as u32, &error)?;
                if !repaired {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
    }
}

fn try_repair(
    opts: &Options,
    sink: &mut dyn FlowSink,
    phase: &str,
    iteration: u32,
    error: &anyhow::Error,
) -> Result<bool> {
    sink.emit(FlowEvent::Note {
        level: NoteLevel::Warn,
        text: format!(
            "grind: invoking {} repair agent for {} failure (attempt {})",
            opts.agent.label(),
            phase,
            iteration
        ),
    });

    let log_dir = create_log_dir(phase, iteration)?;
    let prompt = build_repair_prompt(phase, iteration, error)?;
    std::fs::write(log_dir.join("prompt.md"), &prompt)
        .with_context(|| format!("write {}", log_dir.join("prompt.md").display()))?;
    std::fs::write(log_dir.join("error.txt"), format!("{error:#}"))
        .with_context(|| format!("write {}", log_dir.join("error.txt").display()))?;
    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: format!("grind: repair log dir: {}", log_dir.display()),
    });

    let transcript_path = log_dir.join("transcript.ndjson");
    let outcome = refactor_hotspot::invoke_agent(opts.agent, &prompt, &transcript_path, sink)?;
    std::fs::write(log_dir.join("response.md"), &outcome.assistant_text)
        .with_context(|| format!("write {}", log_dir.join("response.md").display()))?;

    if !outcome.success {
        sink.emit(FlowEvent::Note {
            level: NoteLevel::Error,
            text: format!(
                "grind: {} repair agent exited with status {}",
                opts.agent.label(),
                outcome.exit_code
            ),
        });
        return Ok(false);
    }

    if !finalize_repair(sink, phase, iteration)? {
        return Ok(false);
    }
    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: "grind: repair promoted through gates".to_string(),
    });
    Ok(true)
}

fn finalize_repair(sink: &mut dyn FlowSink, phase: &str, iteration: u32) -> Result<bool> {
    for (label, program, args) in [
        ("cargo fmt --all", "cargo", &["fmt", "--all"][..]),
        ("cargo test", "cargo", &["test"][..]),
        ("cargo xtask corpus", "cargo", &["xtask", "corpus"][..]),
        ("just audit-page", "just", &["audit-page"][..]),
    ] {
        sink.emit(FlowEvent::Note {
            level: NoteLevel::Info,
            text: format!("grind: repair gate `{label}`"),
        });
        if let Err(e) = run_command_gate(label, program, args) {
            sink.emit(FlowEvent::Note {
                level: NoteLevel::Error,
                text: format!("grind: repair gate failed: {e:#}"),
            });
            return Ok(false);
        }
    }

    match git_commit_repair(&repair_commit_message(phase, iteration)?)? {
        CommitOutcome::Committed => {
            let (passing, total) = refactor_hotspot::read_corpus_pp_total();
            let grammar_rules = refactor_hotspot::count_grammar_rules();
            sink.emit(FlowEvent::Note {
                level: NoteLevel::Info,
                text: format!(
                    "grind: committed repair ({passing}/{total}, {grammar_rules} grammar rules)"
                ),
            });
            Ok(true)
        }
        CommitOutcome::NoChanges => {
            sink.emit(FlowEvent::Note {
                level: NoteLevel::Info,
                text: "grind: repair left no changes to commit".to_string(),
            });
            // A successful no-diff repair means the agent intentionally
            // restored the failed iteration or found no repository change was
            // needed. Either way the worktree is clean enough to continue.
            Ok(true)
        }
    }
}

fn build_repair_prompt(phase: &str, iteration: u32, error: &anyhow::Error) -> Result<String> {
    let git_status = command_stdout("git", &["status", "--short"])
        .unwrap_or_else(|_| "(git status unavailable)\n".to_string());
    Ok(format!(
        "You are the grind repair agent. The autonomous {phase} phase hit a \
gate failure at iteration {iteration} and the orchestrator handed you the \
error so the outer loop can continue without human input.\n\n\
## Error\n\n```text\n{error:#}\n```\n\n\
## Current git status\n\n```text\n{git_status}```\n\n\
## Mission\n\n\
Diagnose and fix the failure so the grind loop can keep going. Prefer the \
smallest patch that makes the gate green. You are free to edit, replace, or \
discard the currently modified files from the failed iteration. In most cases \
you should repair the patch in place; if the patch is unsalvageable, restore \
the affected files to the last committed state and exit successfully. If you \
cannot make either choice safely, exit non-zero and the orchestrator will \
discard the working tree and treat this iteration as a no-op.\n\n\
## Rules\n\n\
1. Do not weaken or disable existing tests.\n\
2. Do not bypass deterministic gates (tier-2, corpus, audit).\n\
3. You may run focused tests while debugging, but the orchestrator owns the \
final full gates and commit after you exit successfully.\n\
4. Do not leave uncertainty about ownership of modified files: either make \
them part of the repair or restore them.\n\
5. Keep edits tightly scoped to the failure.\n"
    ))
}

fn create_log_dir(phase: &str, iteration: u32) -> Result<PathBuf> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dir = grind_log_root().join(format!("{ts}-{phase}-repair-iter-{iteration}"));
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    Ok(dir)
}

fn ensure_clean_working_tree() -> Result<()> {
    let out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_root())
        .output()
        .context("run git status")?;
    if !out.status.success() {
        bail!("git status failed with {}", out.status);
    }
    let text = String::from_utf8_lossy(&out.stdout);
    if !text.trim().is_empty() {
        bail!(
            "working tree is dirty; pass --allow-dirty to override:\n{}",
            text.trim_end()
        );
    }
    Ok(())
}

fn discard_working_changes(sink: &mut dyn FlowSink) -> Result<()> {
    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: "grind: discarding working tree changes".to_string(),
    });
    let out = Command::new("git")
        .args(["restore", "--worktree", "--staged", "."])
        .current_dir(repo_root())
        .output()
        .context("run git restore")?;
    if !out.status.success() {
        bail!(
            "git restore failed with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    // Untracked files are not touched by `git restore`; let them be — the
    // refactor agent doesn't generally produce untracked files we want to
    // delete blindly.
    Ok(())
}

enum CommitOutcome {
    Committed,
    NoChanges,
}

fn run_command_gate(label: &str, program: &str, args: &[&str]) -> Result<()> {
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

fn git_commit_repair(message: &str) -> Result<CommitOutcome> {
    let add = Command::new("git")
        .args(["add", "-A", "--", ".", ":(exclude).grind/**"])
        .current_dir(repo_root())
        .status()
        .context("git add -A -- . :(exclude).grind/**")?;
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

fn repair_commit_message(phase: &str, iteration: u32) -> Result<String> {
    let stat = git_diff_stat()?;
    Ok(format!(
        "Repair grind {phase} iteration {iteration}\n\nGates: cargo test; cargo xtask corpus; just audit-page.\nPrimary LOC delta:\n{}",
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

fn command_stdout(program: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(repo_root())
        .output()
        .with_context(|| format!("run {program} {}", args.join(" ")))?;
    if !out.status.success() {
        bail!("{program} {} failed", args.join(" "));
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    if text.is_empty() {
        Ok("(empty)\n".to_string())
    } else {
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_defaults() {
        let opts = Options::parse(&[]).expect("parse");
        assert_eq!(opts.stop_after, DEFAULT_STOP_AFTER);
        assert_eq!(
            opts.max_refactor_iterations,
            DEFAULT_MAX_REFACTOR_ITERATIONS
        );
        assert_eq!(opts.max_commits_per_theme, DEFAULT_MAX_COMMITS_PER_THEME);
        assert_eq!(opts.max_repairs_per_theme, DEFAULT_MAX_REPAIRS_PER_THEME);
        assert_eq!(opts.low_value_stop_after, DEFAULT_LOW_VALUE_STOP_AFTER);
        assert_eq!(opts.max_card_iterations, 0);
        assert_eq!(opts.repair_attempts, DEFAULT_REPAIR_ATTEMPTS);
        assert!(!opts.allow_dirty);
        assert!(!opts.dry_run);
        assert!(opts.set.is_none());
        assert!(opts.theme.is_none());
        assert!(opts.target.is_none());
    }

    #[test]
    fn parses_all_flags_space_form() {
        let args = s(&[
            "--set",
            "neo",
            "--stop-after",
            "5",
            "--max-refactor-iterations",
            "10",
            "--max-commits-per-theme",
            "4",
            "--max-repairs-per-theme",
            "3",
            "--low-value-stop-after",
            "2",
            "--max-card-iterations",
            "3",
            "--repair-attempts",
            "2",
            "--agent",
            "claude",
            "--theme",
            "damage",
            "--target",
            "crates/mtg-grammar/src/grammar.pest",
            "--allow-dirty",
            "--dry-run",
        ]);
        let opts = Options::parse(&args).expect("parse");
        assert_eq!(opts.set.as_deref(), Some("neo"));
        assert_eq!(opts.stop_after, 5);
        assert_eq!(opts.max_refactor_iterations, 10);
        assert_eq!(opts.max_commits_per_theme, 4);
        assert_eq!(opts.max_repairs_per_theme, 3);
        assert_eq!(opts.low_value_stop_after, 2);
        assert_eq!(opts.max_card_iterations, 3);
        assert_eq!(opts.repair_attempts, 2);
        assert!(matches!(opts.agent, AgentProvider::Claude));
        assert_eq!(opts.theme.as_deref(), Some("damage"));
        assert_eq!(
            opts.target.as_deref(),
            Some("crates/mtg-grammar/src/grammar.pest")
        );
        assert!(opts.allow_dirty);
        assert!(opts.dry_run);
    }

    #[test]
    fn parses_equals_form() {
        let args = s(&[
            "--set=neo",
            "--stop-after=4",
            "--max-refactor-iterations=20",
            "--max-commits-per-theme=7",
            "--max-repairs-per-theme=4",
            "--low-value-stop-after=3",
            "--repair-attempts=0",
            "--agent=claude",
            "--theme=destroy",
        ]);
        let opts = Options::parse(&args).expect("parse");
        assert_eq!(opts.set.as_deref(), Some("neo"));
        assert_eq!(opts.stop_after, 4);
        assert_eq!(opts.max_refactor_iterations, 20);
        assert_eq!(opts.max_commits_per_theme, 7);
        assert_eq!(opts.max_repairs_per_theme, 4);
        assert_eq!(opts.low_value_stop_after, 3);
        assert_eq!(opts.repair_attempts, 0);
        assert!(matches!(opts.agent, AgentProvider::Claude));
        assert_eq!(opts.theme.as_deref(), Some("destroy"));
    }

    #[test]
    fn rejects_zero_stop_after() {
        let err = Options::parse(&s(&["--stop-after", "0"])).expect_err("should reject");
        assert!(err.to_string().contains("stop-after"));
    }

    #[test]
    fn rejects_zero_max_refactor() {
        let err =
            Options::parse(&s(&["--max-refactor-iterations", "0"])).expect_err("should reject");
        assert!(err.to_string().contains("max-refactor-iterations"));
    }

    #[test]
    fn rejects_zero_theme_budgets() {
        let err = Options::parse(&s(&["--max-commits-per-theme", "0"])).expect_err("should reject");
        assert!(err.to_string().contains("max-commits-per-theme"));

        let err = Options::parse(&s(&["--max-repairs-per-theme", "0"])).expect_err("should reject");
        assert!(err.to_string().contains("max-repairs-per-theme"));

        let err = Options::parse(&s(&["--low-value-stop-after", "0"])).expect_err("should reject");
        assert!(err.to_string().contains("low-value-stop-after"));
    }

    #[test]
    fn rejects_unknown_argument() {
        let err = Options::parse(&s(&["--nope"])).expect_err("should reject");
        assert!(err.to_string().contains("unknown argument"));
    }

    #[test]
    fn swallows_ui_flag() {
        // main.rs strips --ui, but tolerate it if it slips through.
        Options::parse(&s(&["--ui", "tui"])).expect("parse with --ui");
        Options::parse(&s(&["--ui=console"])).expect("parse with --ui=");
    }

    #[test]
    fn rejects_invalid_agent() {
        let err = Options::parse(&s(&["--agent", "gemini"])).expect_err("should reject");
        assert!(err.to_string().contains("codex"));
    }

    #[test]
    fn parses_theme_choice_from_final_line() {
        let text = "I would continue the destroy frame.\n\ntheme: destroy\n";
        assert_eq!(parse_theme_choice(text).as_deref(), Some("destroy"));
    }

    #[test]
    fn fallback_theme_skips_exhausted_themes() {
        let exhausted = vec!["damage".to_string(), "destroy".to_string()];
        assert_eq!(fallback_theme(1, &exhausted), "prevention");
    }

    #[test]
    fn detects_all_auto_themes_exhausted() {
        let exhausted = AUTO_REFACTOR_THEMES
            .iter()
            .map(|theme| (*theme).to_string())
            .collect::<Vec<_>>();
        assert!(all_auto_themes_exhausted(&exhausted));

        let partial = vec!["damage".to_string(), "destroy".to_string()];
        assert!(!all_auto_themes_exhausted(&partial));
    }

    #[test]
    fn theme_state_exhausts_on_commit_budget() {
        let opts = Options::parse(&s(&["--max-commits-per-theme", "2"])).expect("parse");
        let mut state = ThemeState::default();
        assert_eq!(state.record_commit(10, 100, &opts), None);
        assert_eq!(
            state.record_commit(11, 99, &opts),
            Some(ExhaustionReason::CommitBudget)
        );
    }

    #[test]
    fn theme_state_exhausts_on_repair_budget() {
        let opts = Options::parse(&s(&["--max-repairs-per-theme", "2"])).expect("parse");
        let mut state = ThemeState::default();
        assert_eq!(state.record_repair(&opts), None);
        assert_eq!(
            state.record_repair(&opts),
            Some(ExhaustionReason::RepairBudget)
        );
    }

    #[test]
    fn theme_state_exhausts_on_low_value_streak() {
        let opts = Options::parse(&s(&["--low-value-stop-after", "2"])).expect("parse");
        let mut state = ThemeState {
            last_corpus_passing: Some(198),
            last_grammar_rules: Some(220),
            ..ThemeState::default()
        };
        assert_eq!(state.record_commit(198, 220, &opts), None);
        assert_eq!(
            state.record_commit(198, 221, &opts),
            Some(ExhaustionReason::LowValue)
        );
    }
}
