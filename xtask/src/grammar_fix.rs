//! The grammar-fix orchestrator. Walks a Scryfall set, hands one
//! failing card at a time to a fresh coding agent, gates the
//! result through tier-1/2 tests and the corpus regression check, and
//! commits per-iteration progress.
//!
//! All user-visible output goes through a [`FlowSink`]; this file
//! does not call `println!`. Adding a new display surface (TUI, log
//! file, ...) is done by adding a new sink, not by editing here.
//!
//! Inspired by argentum-press/scripts/fix_parser_gaps.py — narrower in
//! scope (single playbook, single pre-computed context block) but the
//! same deterministic-around-claude shape.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};

use mtg_corpus::{
    card_key, load as load_corpus_report, normalize_oracle_text, CardOutcome, NextCard,
};
use mtg_scryfall::Layout;
use mtg_scryfall::{Card, ScryfallClient};

use crate::console_sink::ConsoleSink;
use crate::flow::{
    AgentProvider, FlowEvent, FlowSink, IterationOutcomeSummary, NoteLevel, SessionEndReason,
};
use crate::paths::{
    ast_rs_path, corpus_status_path, generated_pattern_tests_dir, generated_pattern_tests_manifest,
    generated_tests_dir, generated_tests_manifest, grammar_fix_log_root, grammar_pest_path,
    lower_rs_path, repo_root,
};

const DEFAULT_SET: &str = "lea";
/// 0 means unbounded; positive values cap the loop.
const DEFAULT_MAX_ITERATIONS: u32 = 0;
const DEFAULT_SUPERVISOR_ATTEMPTS: u8 = 1;
const TOTAL_STEPS: u8 = 12;

pub fn run(args: &[String]) -> ExitCode {
    let opts = match Options::parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let mut sink: Box<dyn FlowSink> = Box::new(ConsoleSink::new());
    match run_with_sink(opts, sink.as_mut()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

/// Entry point for callers that want to attach a custom sink (e.g. the
/// TUI). Same flow as [`run`], minus argv parsing.
pub fn run_with_sink(opts: Options, sink: &mut dyn FlowSink) -> Result<ExitCode> {
    if !opts.dry_run && !opts.allow_dirty {
        ensure_clean_working_tree()
            .context("working tree must be clean (or pass --allow-dirty)")?;
    }
    let client = ScryfallClient::new()?;

    let baseline_grammar_rules = count_grammar_rules();
    let (baseline_corpus_passing, baseline_corpus_total) = read_corpus_pp_total();
    sink.emit(FlowEvent::SessionStarted {
        set: opts.set.clone(),
        max_iterations: opts.max_iterations,
        baseline_corpus_passing,
        baseline_corpus_total,
        baseline_grammar_rules,
    });

    let mut iter = 0u32;
    let mut end_reason = None::<SessionEndReason>;
    let mut supervisor_attempts = 0u8;
    while opts.max_iterations == 0 || iter < opts.max_iterations {
        match run_one_iteration(&client, &opts, sink, iter + 1) {
            Err(error) if supervisor_attempts < opts.supervisor_attempts => {
                supervisor_attempts += 1;
                match invoke_supervisor(&opts, sink, iter + 1, supervisor_attempts, &error) {
                    Ok(true) => continue,
                    Ok(false) => {
                        let reason =
                            format!("supervisor could not repair unknown problem: {error:#}");
                        sink.emit(FlowEvent::Note {
                            level: NoteLevel::Error,
                            text: reason.clone(),
                        });
                        end_reason = Some(SessionEndReason::SurfacedToHuman(reason));
                        break;
                    }
                    Err(supervisor_error) => {
                        let reason = format!(
                            "supervisor failed while repairing unknown problem: {supervisor_error:#}; original problem: {error:#}"
                        );
                        sink.emit(FlowEvent::Note {
                            level: NoteLevel::Error,
                            text: reason.clone(),
                        });
                        end_reason = Some(SessionEndReason::SurfacedToHuman(reason));
                        break;
                    }
                }
            }
            Err(error) => {
                let reason = format!("unknown problem: {error:#}");
                sink.emit(FlowEvent::Note {
                    level: NoteLevel::Error,
                    text: reason.clone(),
                });
                end_reason = Some(SessionEndReason::SurfacedToHuman(reason));
                break;
            }
            Ok(outcome) => match outcome {
                IterationOutcome::AllPass => {
                    end_reason = Some(SessionEndReason::AllPass);
                    break;
                }
                IterationOutcome::DryRunStop => {
                    end_reason = Some(SessionEndReason::DryRunStop);
                    break;
                }
                IterationOutcome::SurfaceToHuman(reason) => {
                    end_reason = Some(SessionEndReason::SurfacedToHuman(reason));
                    break;
                }
                IterationOutcome::Committed => {
                    iter += 1;
                    supervisor_attempts = 0;
                }
            },
        }
    }
    let reason = end_reason.unwrap_or(SessionEndReason::MaxIterationsReached(opts.max_iterations));
    let exit_code = match &reason {
        SessionEndReason::SurfacedToHuman(_) => ExitCode::FAILURE,
        _ => ExitCode::SUCCESS,
    };
    sink.emit(FlowEvent::SessionFinished { reason });
    Ok(exit_code)
}

#[derive(Debug, Clone)]
pub struct Options {
    pub set: String,
    pub max_iterations: u32,
    pub supervisor_attempts: u8,
    pub dry_run: bool,
    pub allow_dirty: bool,
    pub agent: AgentProvider,
}

impl Options {
    pub fn parse(args: &[String]) -> Result<Self> {
        let mut set = None::<String>;
        let mut max_iterations = DEFAULT_MAX_ITERATIONS;
        let mut supervisor_attempts = DEFAULT_SUPERVISOR_ATTEMPTS;
        let mut dry_run = false;
        let mut allow_dirty = false;
        let mut agent = AgentProvider::Codex;

        let mut iter = args.iter();
        while let Some(a) = iter.next() {
            match a.as_str() {
                "--set" => set = iter.next().cloned(),
                s if s.starts_with("--set=") => set = Some(s["--set=".len()..].to_string()),
                "--max-iterations" => {
                    let v = iter
                        .next()
                        .ok_or_else(|| anyhow!("--max-iterations requires a value"))?;
                    max_iterations = v
                        .parse()
                        .with_context(|| format!("--max-iterations value: {v:?}"))?;
                }
                s if s.starts_with("--max-iterations=") => {
                    max_iterations = s["--max-iterations=".len()..]
                        .parse()
                        .with_context(|| format!("--max-iterations value: {s:?}"))?;
                }
                "--supervisor-attempts" => {
                    let v = iter
                        .next()
                        .ok_or_else(|| anyhow!("--supervisor-attempts requires a value"))?;
                    supervisor_attempts = v
                        .parse()
                        .with_context(|| format!("--supervisor-attempts value: {v:?}"))?;
                }
                s if s.starts_with("--supervisor-attempts=") => {
                    supervisor_attempts = s["--supervisor-attempts=".len()..]
                        .parse()
                        .with_context(|| format!("--supervisor-attempts value: {s:?}"))?;
                }
                "--no-supervisor" => supervisor_attempts = 0,
                "--dry-run" => dry_run = true,
                "--allow-dirty" => allow_dirty = true,
                "--agent" => {
                    let v = iter
                        .next()
                        .ok_or_else(|| anyhow!("--agent requires a value"))?;
                    agent = parse_agent(v)?;
                }
                s if s.starts_with("--agent=") => {
                    agent = parse_agent(&s["--agent=".len()..])?;
                }
                // The --ui flag is handled in main.rs (it picks the
                // sink before we get here); silently swallow it if it
                // slips through.
                "--ui" => {
                    let _ = iter.next();
                }
                s if s.starts_with("--ui=") => {}
                other => bail!("unknown argument: {other}"),
            }
        }
        Ok(Self {
            set: set.unwrap_or_else(|| DEFAULT_SET.to_string()),
            max_iterations,
            supervisor_attempts,
            dry_run,
            allow_dirty,
            agent,
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

enum IterationOutcome {
    AllPass,
    DryRunStop,
    Committed,
    SurfaceToHuman(String),
}

fn run_one_iteration(
    client: &ScryfallClient,
    opts: &Options,
    sink: &mut dyn FlowSink,
    iter_index: u32,
) -> Result<IterationOutcome> {
    let iter_start = Instant::now();

    // Step 1 — find the next failing card.
    sink.emit(FlowEvent::StepStarted {
        index: 1,
        total: TOTAL_STEPS,
        label: "find next failing card".into(),
    });
    let (card, error, normalized) = match find_next_failing_card_from_status(client, &opts.set)? {
        NextCard::AllPass => {
            sink.emit(FlowEvent::StepFinished {
                index: 1,
                ok: true,
                summary: Some(format!("set '{}' fully covered", opts.set)),
            });
            return Ok(IterationOutcome::AllPass);
        }
        NextCard::Failing {
            card,
            reason,
            normalized,
        } => (card, reason, normalized),
    };
    sink.emit(FlowEvent::StepFinished {
        index: 1,
        ok: true,
        summary: Some(format!(
            "{} ({}/{})",
            card.name, card.set_code, card.collector_number
        )),
    });
    sink.emit(FlowEvent::IterationStarted {
        index: iter_index,
        max_iterations: opts.max_iterations,
        card: card.clone(),
        normalized: normalized.clone(),
        round_trip_error: error.clone(),
    });

    // Step 2 — create the log dir.
    sink.emit(FlowEvent::StepStarted {
        index: 2,
        total: TOTAL_STEPS,
        label: "create log dir".into(),
    });
    let log_dir = create_log_dir(&card)?;
    sink.emit(FlowEvent::StepFinished {
        index: 2,
        ok: true,
        summary: Some(format!("{}", log_dir.display())),
    });

    // The generated-test paths are deterministic from the card slug; we
    // compute them now so prompts can reference them but write the files
    // only past the dry-run gate.
    let test_path = generated_test_path(&card);
    let pattern_test_path = generated_pattern_test_path(&card);

    // Step 3 — snapshot card.json + baseline corpus stats.
    sink.emit(FlowEvent::StepStarted {
        index: 3,
        total: TOTAL_STEPS,
        label: "snapshot card.json + baseline".into(),
    });
    let card_json = serde_json::to_string_pretty(&card)?;
    std::fs::write(log_dir.join("card.json"), card_json)?;
    let baseline_pass_count = read_corpus_passing(&corpus_status_path()).unwrap_or(0);
    sink.emit(FlowEvent::StepFinished {
        index: 3,
        ok: true,
        summary: None,
    });

    // Step 4 — build + write the prompt.
    sink.emit(FlowEvent::StepStarted {
        index: 4,
        total: TOTAL_STEPS,
        label: "build prompt".into(),
    });
    let prompt = build_prompt(&card, &error, &normalized, &test_path)?;
    std::fs::write(log_dir.join("prompt.md"), &prompt)?;
    sink.emit(FlowEvent::StepFinished {
        index: 4,
        ok: true,
        summary: Some(format!("wrote prompt.md ({} bytes)", prompt.len())),
    });

    if opts.dry_run {
        return Ok(IterationOutcome::DryRunStop);
    }

    // Step 5 — write deterministic generated tests first.
    sink.emit(FlowEvent::StepStarted {
        index: 5,
        total: TOTAL_STEPS,
        label: "write generated tests".into(),
    });
    let patterns = extract_patterns(&normalized);
    write_pattern_tests(&pattern_test_path, &card, &patterns)
        .context("generate pattern test file")?;
    register_generated_pattern_test(&pattern_test_path)
        .context("register generated pattern test")?;
    write_generated_test(&test_path, &card, &normalized).context("generate test file")?;
    register_generated_test(&test_path).context("register generated test")?;
    sink.emit(FlowEvent::StepFinished {
        index: 5,
        ok: true,
        summary: Some(format!(
            "{} patterns + round-trip · {}",
            patterns.len(),
            pattern_test_path
                .strip_prefix(repo_root())
                .unwrap_or(&pattern_test_path)
                .display()
        )),
    });

    // Step 6 — focused generated tests before invoking the agent. If
    // these are already green, the card was fixed by a previous commit
    // or deterministic generated-test write and no LM work is needed.
    sink.emit(FlowEvent::StepStarted {
        index: 6,
        total: TOTAL_STEPS,
        label: "focused generated tests".into(),
    });
    let focused_before = run_focused_generated_tests(&card)?;
    std::fs::write(
        log_dir.join("focused_before_agent.txt"),
        focused_before.summary_text(),
    )?;
    sink.emit(FlowEvent::StepFinished {
        index: 6,
        ok: true,
        summary: Some(focused_before.short_summary()),
    });

    // Step 7 — build the orchestrator-owned repair recipe. This is the
    // deterministic half of the LM conversation: the orchestrator
    // gathers the failing commands, generated tests, and relevant code
    // map, then gives the LM a bounded patch task.
    sink.emit(FlowEvent::StepStarted {
        index: 7,
        total: TOTAL_STEPS,
        label: "build repair recipe".into(),
    });
    let transcript_path = log_dir.join("transcript.ndjson");
    let mut agent_ran = false;
    let pattern_prompt = if focused_before.success() {
        None
    } else {
        let prompt = build_pattern_prompt(
            &card,
            &error,
            &normalized,
            &test_path,
            &pattern_test_path,
            &patterns,
            &focused_before,
        )?;
        std::fs::write(log_dir.join("agent_recipe.md"), &prompt)?;
        Some(prompt)
    };
    sink.emit(FlowEvent::StepFinished {
        index: 7,
        ok: true,
        summary: Some(if pattern_prompt.is_some() {
            "recipe written for focused failure".into()
        } else {
            "skipped; focused tests already pass".into()
        }),
    });

    // Step 8 — delegate to the configured agent only when the
    // orchestrator-owned focused tests say there is real work left.
    sink.emit(FlowEvent::StepStarted {
        index: 8,
        total: TOTAL_STEPS,
        label: format!("{} agent repair", opts.agent.label()),
    });
    if let Some(pattern_prompt) = pattern_prompt {
        agent_ran = true;
        let agent_outcome = invoke_agent(opts.agent, &pattern_prompt, &transcript_path, sink)?;
        std::fs::write(log_dir.join("response.md"), &agent_outcome.assistant_text)?;
        sink.emit(FlowEvent::StepFinished {
            index: 8,
            ok: agent_outcome.success,
            summary: Some(format!(
                "exit={} · {} assistant blocks",
                agent_outcome.exit_code, agent_outcome.assistant_blocks
            )),
        });
        if !agent_outcome.success {
            let reason = format!(
                "{} agent exited with status {}",
                opts.agent.label(),
                agent_outcome.exit_code
            );
            sink.emit(FlowEvent::IterationFinished {
                index: iter_index,
                outcome: IterationOutcomeSummary::SurfacedToHuman {
                    reason: reason.clone(),
                },
            });
            return Ok(IterationOutcome::SurfaceToHuman(reason));
        }
    } else {
        sink.emit(FlowEvent::StepFinished {
            index: 8,
            ok: true,
            summary: Some("skipped; focused tests already pass".into()),
        });
    }

    // Step 9 — deterministic validation of the agent's patch, or a
    // second confirmation when the agent was skipped.
    sink.emit(FlowEvent::StepStarted {
        index: 9,
        total: TOTAL_STEPS,
        label: "focused validation".into(),
    });
    let focused_after = run_focused_generated_tests(&card)?;
    std::fs::write(
        log_dir.join("focused_after_agent.txt"),
        focused_after.summary_text(),
    )?;
    sink.emit(FlowEvent::StepFinished {
        index: 9,
        ok: focused_after.success(),
        summary: Some(focused_after.short_summary()),
    });
    if !focused_after.success() {
        let reason = if agent_ran {
            "focused generated tests still fail after agent repair"
        } else {
            "focused generated tests failed without invoking the agent"
        }
        .to_string();
        sink.emit(FlowEvent::IterationFinished {
            index: iter_index,
            outcome: IterationOutcomeSummary::SurfacedToHuman {
                reason: reason.clone(),
            },
        });
        return Ok(IterationOutcome::SurfaceToHuman(reason));
    }

    // Step 10 — tier 1 + tier 2.
    sink.emit(FlowEvent::StepStarted {
        index: 10,
        total: TOTAL_STEPS,
        label: "cargo xtask test --tier 2".into(),
    });
    let mut tests_ok = run_xtask(&["test", "--tier", "2"])?;
    sink.emit(FlowEvent::StepFinished {
        index: 10,
        ok: tests_ok,
        summary: None,
    });
    if !tests_ok {
        let repair_ok =
            invoke_downstream_repair(opts, sink, &card, &normalized, &log_dir, iter_index)?;
        if repair_ok {
            sink.emit(FlowEvent::StepStarted {
                index: 9,
                total: TOTAL_STEPS,
                label: "focused validation after downstream repair".into(),
            });
            let focused_after_repair = run_focused_generated_tests(&card)?;
            std::fs::write(
                log_dir.join("focused_after_downstream_repair.txt"),
                focused_after_repair.summary_text(),
            )?;
            sink.emit(FlowEvent::StepFinished {
                index: 9,
                ok: focused_after_repair.success(),
                summary: Some(focused_after_repair.short_summary()),
            });
            if focused_after_repair.success() {
                sink.emit(FlowEvent::StepStarted {
                    index: 10,
                    total: TOTAL_STEPS,
                    label: "cargo xtask test --tier 2 after downstream repair".into(),
                });
                tests_ok = run_xtask(&["test", "--tier", "2"])?;
                sink.emit(FlowEvent::StepFinished {
                    index: 10,
                    ok: tests_ok,
                    summary: None,
                });
                if tests_ok {
                    sink.emit(FlowEvent::Note {
                        level: NoteLevel::Info,
                        text: "downstream repair validated; continuing".into(),
                    });
                } else {
                    let reason = "cargo xtask test --tier 2 still failed after downstream repair"
                        .to_string();
                    sink.emit(FlowEvent::IterationFinished {
                        index: iter_index,
                        outcome: IterationOutcomeSummary::SurfacedToHuman {
                            reason: reason.clone(),
                        },
                    });
                    return Ok(IterationOutcome::SurfaceToHuman(reason));
                }
            } else {
                let reason = "focused generated tests failed after downstream repair".to_string();
                sink.emit(FlowEvent::IterationFinished {
                    index: iter_index,
                    outcome: IterationOutcomeSummary::SurfacedToHuman {
                        reason: reason.clone(),
                    },
                });
                return Ok(IterationOutcome::SurfaceToHuman(reason));
            }
        } else {
            let reason = "downstream repair agent could not repair tier-2 failure".to_string();
            sink.emit(FlowEvent::IterationFinished {
                index: iter_index,
                outcome: IterationOutcomeSummary::SurfacedToHuman {
                    reason: reason.clone(),
                },
            });
            return Ok(IterationOutcome::SurfaceToHuman(reason));
        }
    }
    // Step 11 — corpus regression gate.
    sink.emit(FlowEvent::StepStarted {
        index: 11,
        total: TOTAL_STEPS,
        label: "cargo xtask corpus (regression gate)".into(),
    });
    let corpus_ok = run_xtask(&["corpus"])?;
    sink.emit(FlowEvent::StepFinished {
        index: 11,
        ok: corpus_ok,
        summary: None,
    });
    if !corpus_ok {
        let reason = "cargo xtask corpus reported a regression".to_string();
        sink.emit(FlowEvent::IterationFinished {
            index: iter_index,
            outcome: IterationOutcomeSummary::SurfacedToHuman {
                reason: reason.clone(),
            },
        });
        return Ok(IterationOutcome::SurfaceToHuman(reason));
    }

    // Step 12 — commit.
    let new_pass_count = read_corpus_passing(&corpus_status_path()).unwrap_or(baseline_pass_count);
    let total = read_corpus_total(&corpus_status_path()).unwrap_or(0);
    let new_passes = new_pass_count.saturating_sub(baseline_pass_count);
    let commit_msg = commit_message(&card, new_passes, new_pass_count, total);
    std::fs::write(log_dir.join("commit_message.txt"), &commit_msg)?;
    sink.emit(FlowEvent::StepStarted {
        index: 12,
        total: TOTAL_STEPS,
        label: "git commit".into(),
    });
    match git_commit(&commit_msg).context("git commit")? {
        CommitOutcome::Committed => {
            sink.emit(FlowEvent::StepFinished {
                index: 12,
                ok: true,
                summary: Some(format!(
                    "+{new_passes} pass · status {new_pass_count}/{total}"
                )),
            });
        }
        CommitOutcome::NoChanges => {
            let reason = "no changes to commit after successful tests and corpus gate".to_string();
            sink.emit(FlowEvent::StepFinished {
                index: 12,
                ok: true,
                summary: Some("no changes to commit".into()),
            });
            bail!("{reason}");
        }
    }

    if let Ok(diff) = git_diff_against_head_parent() {
        std::fs::write(log_dir.join("diff.patch"), diff).ok();
    }

    let duration_secs = iter_start.elapsed().as_secs();
    let grammar_rules = count_grammar_rules();
    sink.emit(FlowEvent::IterationFinished {
        index: iter_index,
        outcome: IterationOutcomeSummary::Committed {
            new_passes,
            corpus_passing: new_pass_count,
            corpus_total: total,
            grammar_rules,
            duration_secs,
        },
    });

    Ok(IterationOutcome::Committed)
}

// ---------------------------------------------------------------------------
// step helpers
// ---------------------------------------------------------------------------

fn ensure_clean_working_tree() -> Result<()> {
    let out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_root())
        .output()
        .context("run git status")?;
    if !out.status.success() {
        bail!("git status failed");
    }
    if !out.stdout.is_empty() {
        let listing = String::from_utf8_lossy(&out.stdout);
        bail!("working tree is dirty:\n{listing}");
    }
    Ok(())
}

fn create_log_dir(card: &Card) -> Result<PathBuf> {
    let ts = unix_secs();
    let slug = slugify(&card.name);
    let dir = grammar_fix_log_root().join(format!("{ts}-{slug}"));
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    Ok(dir)
}

fn find_next_failing_card_from_status(client: &ScryfallClient, set_code: &str) -> Result<NextCard> {
    let report = load_corpus_report(&corpus_status_path()).context("load corpus status")?;
    let cards = client.cards_in_set(set_code)?;
    for card in cards {
        if card.layout != Layout::Normal {
            continue;
        }
        let normalized = normalize_oracle_text(&card.oracle_text);
        if normalized.is_empty() {
            continue;
        }
        let Some(outcome) = report.cards.get(&card_key(&card)) else {
            continue;
        };
        if let CardOutcome::Fail { error } = outcome {
            return Ok(NextCard::Failing {
                card,
                reason: error.clone(),
                normalized,
            });
        }
    }
    Ok(NextCard::AllPass)
}

fn create_supervisor_log_dir(iter_index: u32, attempt: u8) -> Result<PathBuf> {
    let ts = unix_secs();
    let dir = grammar_fix_log_root().join(format!(
        "{ts}-supervisor-iter-{iter_index}-attempt-{attempt}"
    ));
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    Ok(dir)
}

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn slugify(name: &str) -> String {
    let mut s = String::with_capacity(name.len());
    let mut prev_underscore = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c.to_ascii_lowercase());
            prev_underscore = false;
        } else if !prev_underscore && !s.is_empty() {
            s.push('_');
            prev_underscore = true;
        }
    }
    s.trim_end_matches('_').to_string()
}

fn generated_test_path(card: &Card) -> PathBuf {
    let slug = slugify(&card.name);
    generated_tests_dir().join(format!("{slug}.rs"))
}

fn generated_pattern_test_path(card: &Card) -> PathBuf {
    let slug = slugify(&card.name);
    generated_pattern_tests_dir().join(format!("{slug}.rs"))
}

fn write_generated_test(path: &Path, card: &Card, normalized: &str) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| anyhow!("test path has no parent"))?;
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let body = render_generated_test(card, normalized);
    std::fs::write(path, body).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PatternCase {
    name: String,
    text: String,
}

fn extract_patterns(normalized: &str) -> Vec<PatternCase> {
    let mut out = Vec::new();
    for line in normalized.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if !line.contains('.') {
            push_pattern(&mut out, "phrase", line);
            continue;
        }
        for sentence in split_sentences_quote_aware(line) {
            let sentence = sentence.trim();
            if sentence.is_empty() {
                continue;
            }
            push_pattern(&mut out, pattern_name(sentence), sentence);
            for quoted in quoted_segments(sentence) {
                push_pattern(&mut out, "quoted_keyword", &quoted);
            }
        }
    }
    if out.is_empty() {
        push_pattern(&mut out, "full_card", normalized);
    }
    out
}

fn push_pattern(out: &mut Vec<PatternCase>, name: &str, text: &str) {
    if out.iter().any(|p| p.text == text) {
        return;
    }
    out.push(PatternCase {
        name: slugify(name),
        text: text.to_string(),
    });
}

fn pattern_name(sentence: &str) -> &'static str {
    let lower = sentence.to_ascii_lowercase();
    if lower.starts_with("when ") || lower.starts_with("whenever ") || lower.starts_with("at ") {
        "triggered_ability"
    } else if lower.starts_with("as long as ") || lower.starts_with("enchanted ") {
        "static_ability"
    } else {
        "sentence"
    }
}

fn split_sentences_quote_aware(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut in_quote = false;
    for (idx, ch) in line.char_indices() {
        match ch {
            '"' => in_quote = !in_quote,
            '.' if !in_quote => {
                let end = idx + ch.len_utf8();
                out.push(line[start..end].trim().to_string());
                start = end;
                while line[start..].starts_with(' ') {
                    start += 1;
                }
            }
            _ => {}
        }
    }
    if start < line.len() {
        out.push(line[start..].trim().to_string());
    }
    out
}

fn quoted_segments(sentence: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = None::<usize>;
    for (idx, ch) in sentence.char_indices() {
        if ch != '"' {
            continue;
        }
        match start.take() {
            Some(s) => {
                let mut text = sentence[s..idx].trim().trim_end_matches('.').to_string();
                if !text.is_empty() {
                    if let Some(first) = text.get_mut(0..1) {
                        first.make_ascii_uppercase();
                    }
                    out.push(text);
                }
            }
            None => start = Some(idx + ch.len_utf8()),
        }
    }
    out
}

fn write_pattern_tests(path: &Path, card: &Card, patterns: &[PatternCase]) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| anyhow!("pattern test path has no parent"))?;
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let body = render_pattern_tests(card, patterns);
    std::fs::write(path, body).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn render_pattern_tests(card: &Card, patterns: &[PatternCase]) -> String {
    let mut body = format!(
        "// Generated by `cargo xtask grammar-fix`.\n\
         // Parse-only pattern tests for incrementally growing grammar support.\n\
         //\n\
         // Card           : {name}\n\
         // Set            : {set}\n\
         // Collector #    : {collector}\n\n",
        name = card.name,
        set = card.set_code,
        collector = card.collector_number,
    );
    for (i, pattern) in patterns.iter().enumerate() {
        body.push_str(&format!(
            "#[test]\n\
             fn pattern_{idx:02}_{name}() {{\n    \
                 let text = {text:?};\n    \
                 mtg_grammar::parse(text).expect(\"parse pattern\");\n\
             }}\n\n",
            idx = i + 1,
            name = pattern.name,
            text = pattern.text,
        ));
    }
    body
}

fn register_generated_pattern_test(test_path: &Path) -> Result<()> {
    let manifest = generated_pattern_tests_manifest();
    let slug = test_path
        .file_stem()
        .expect("test file has a stem")
        .to_string_lossy()
        .to_string();
    let entry = format!("#[path = \"generated_patterns/{slug}.rs\"]\nmod {slug};\n");
    let mut text = if manifest.exists() {
        std::fs::read_to_string(&manifest)?
    } else {
        "// Manifest of generated parse-only pattern tests.\n\n".to_string()
    };
    if text.contains(&format!("mod {slug};")) {
        return Ok(());
    }
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&entry);
    std::fs::write(&manifest, text)?;
    Ok(())
}

fn render_generated_test(card: &Card, normalized: &str) -> String {
    format!(
        "// Generated by `cargo xtask grammar-fix`.\n\
         // Round-trip regression test for the generated test suite.\n\
         //\n\
         // Card           : {name}\n\
         // Set            : {set}\n\
         // Collector #    : {collector}\n\n\
         #[test]\n\
         fn round_trip() {{\n    \
             let text = {text:?};\n    \
             let ast = mtg_grammar::parse(text).expect(\"parse\");\n    \
             let reprinted = mtg_grammar::unparse(&ast);\n    \
             let ast2 = mtg_grammar::parse(&reprinted).expect(\"reparse\");\n    \
             assert_eq!(ast, ast2);\n\
         }}\n",
        name = card.name,
        set = card.set_code,
        collector = card.collector_number,
        text = normalized,
    )
}

fn register_generated_test(test_path: &Path) -> Result<()> {
    let manifest = generated_tests_manifest();
    let slug = test_path
        .file_stem()
        .expect("test file has a stem")
        .to_string_lossy()
        .to_string();
    let entry = format!("#[path = \"generated/{slug}.rs\"]\nmod {slug};\n");
    let mut text = if manifest.exists() {
        std::fs::read_to_string(&manifest)?
    } else {
        "// Manifest of generated round-trip tests.\n\n".to_string()
    };
    if text.contains(&format!("mod {slug};")) {
        return Ok(());
    }
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&entry);
    std::fs::write(&manifest, text)?;
    Ok(())
}

struct FocusedTestRun {
    pattern: CommandRun,
    round_trip: CommandRun,
}

impl FocusedTestRun {
    fn success(&self) -> bool {
        self.pattern.success && self.round_trip.success
    }

    fn short_summary(&self) -> String {
        format!(
            "patterns={} · round-trip={}",
            status_word(self.pattern.success),
            status_word(self.round_trip.success)
        )
    }

    fn summary_text(&self) -> String {
        format!(
            "$ {}\nexit={}\n{}\n\n$ {}\nexit={}\n{}\n",
            self.pattern.command,
            self.pattern.exit_code,
            self.pattern.output,
            self.round_trip.command,
            self.round_trip.exit_code,
            self.round_trip.output
        )
    }
}

struct CommandRun {
    command: String,
    success: bool,
    exit_code: i32,
    output: String,
}

fn status_word(success: bool) -> &'static str {
    if success {
        "pass"
    } else {
        "fail"
    }
}

fn run_focused_generated_tests(card: &Card) -> Result<FocusedTestRun> {
    let filter = slugify(&card.name);
    Ok(FocusedTestRun {
        pattern: run_cargo_test(&[
            "test",
            "-p",
            "mtg-grammar",
            "--test",
            "generated_patterns",
            &filter,
            "--",
            "--nocapture",
        ])?,
        round_trip: run_cargo_test(&[
            "test",
            "-p",
            "mtg-grammar",
            "--test",
            "generated",
            &filter,
            "--",
            "--nocapture",
        ])?,
    })
}

fn run_cargo_test(args: &[&str]) -> Result<CommandRun> {
    let out = Command::new("cargo")
        .args(args)
        .current_dir(repo_root())
        .output()
        .with_context(|| format!("run cargo {}", args.join(" ")))?;
    let mut output = String::new();
    output.push_str(&String::from_utf8_lossy(&out.stdout));
    output.push_str(&String::from_utf8_lossy(&out.stderr));
    Ok(CommandRun {
        command: format!("cargo {}", args.join(" ")),
        success: out.status.success(),
        exit_code: out.status.code().unwrap_or(-1),
        output,
    })
}

fn read_corpus_passing(path: &Path) -> Result<usize> {
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    Ok(json
        .get("passing")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow!("corpus_status.json has no 'passing' field"))? as usize)
}

fn read_corpus_total(path: &Path) -> Result<usize> {
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    Ok(json
        .get("total")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow!("corpus_status.json has no 'total' field"))? as usize)
}

fn read_corpus_pp_total() -> (usize, usize) {
    let path = corpus_status_path();
    (
        read_corpus_passing(&path).unwrap_or(0),
        read_corpus_total(&path).unwrap_or(0),
    )
}

/// Hand-rolled pest rule counter. Counts top-level rule declarations
/// in `grammar.pest` (lines like `name = { ... }` or `name = _{ ... }`).
/// Ignores comments. Used to compute the "grammar rules added this
/// session" metric.
fn count_grammar_rules() -> usize {
    let text = std::fs::read_to_string(grammar_pest_path()).unwrap_or_default();
    text.lines().filter(|l| is_pest_rule_declaration(l)).count()
}

fn is_pest_rule_declaration(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return false;
    }
    let first = trimmed.chars().next().unwrap_or(' ');
    if !(first.is_ascii_lowercase() || first == '_') {
        return false;
    }
    let Some(eq_pos) = trimmed.find('=') else {
        return false;
    };
    let head = &trimmed[..eq_pos];
    head.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c.is_whitespace())
}

// ---------------------------------------------------------------------------
// prompt
// ---------------------------------------------------------------------------

pub(crate) fn build_prompt(
    card: &Card,
    error: &str,
    normalized: &str,
    test_path: &Path,
) -> Result<String> {
    let test_rel = test_path
        .strip_prefix(repo_root())
        .unwrap_or(test_path)
        .display()
        .to_string();
    let patterns = extract_patterns(normalized);

    Ok(format!(
        "{intro}\n\n\
         {card_block}\n\n\
         {error_block}\n\n\
         {test_block}\n\n\
         {context_block}\n\n\
         {workflow_block}\n\n\
         {constraints_block}\n",
        intro = PROMPT_INTRO,
        card_block = render_card_block(card, normalized),
        error_block = render_error_block(error),
        test_block = render_test_block(&test_rel),
        context_block = render_context_block(card, error, normalized, &patterns, &[]),
        workflow_block = WORKFLOW_BLOCK,
        constraints_block = CONSTRAINTS_BLOCK,
    ))
}

fn build_pattern_prompt(
    card: &Card,
    error: &str,
    normalized: &str,
    full_test_path: &Path,
    pattern_test_path: &Path,
    patterns: &[PatternCase],
    focused: &FocusedTestRun,
) -> Result<String> {
    let mut prompt = build_prompt(card, error, normalized, full_test_path)?;
    prompt.push_str("\n\n");
    prompt.push_str(&render_context_block(
        card,
        error,
        normalized,
        patterns,
        &[pattern_test_path, full_test_path],
    ));
    let pattern_rel = pattern_test_path
        .strip_prefix(repo_root())
        .unwrap_or(pattern_test_path)
        .display()
        .to_string();
    prompt.push_str("\n\n## Deterministic Pattern Phase\n\n");
    prompt.push_str("The orchestrator has generated parse-only tests at `");
    prompt.push_str(&pattern_rel);
    prompt.push_str("`. Make those pattern tests pass before relying on the full-card round-trip test. The patterns were extracted deterministically in source order:\n\n");
    for (i, pattern) in patterns.iter().enumerate() {
        prompt.push_str(&format!(
            "{idx}. `{name}`\n```text\n{text}\n```\n\n",
            idx = i + 1,
            name = pattern.name,
            text = pattern.text
        ));
    }
    prompt.push_str(
        "\n## Focused Test Failure\n\n\
         The orchestrator already ran the focused generated tests. Fix the underlying grammar, AST, \
         parser, unparser, or semantic-flow issue exposed here:\n\n```text\n",
    );
    prompt.push_str(&focused.summary_text());
    if focused_failure_requires_semantic(focused) {
        prompt.push_str("```\n\n");
        prompt.push_str(&render_semantic_context());
        prompt.push_str(
            "\n## Recipe\n\n\
             1. Start from the code map above instead of re-reading the whole repository.\n\
             2. Inspect additional code only if the map is insufficient.\n\
             3. Make the smallest general grammar/AST/parser/unparser change for this pattern. Touch semantic files only because the focused failure points there.\n\
             4. Run the focused generated test command(s) shown above until they pass.\n\
             5. Do not run `cargo xtask corpus`; the orchestrator owns corpus regression checks.\n\
             6. Stop after focused tests pass. The orchestrator will run tier 2, corpus, and commit.\n",
        );
        return Ok(prompt);
    }
    prompt.push_str(
        "```\n\n\
         ## Recipe\n\n\
         1. Start from the code map above instead of re-reading the whole repository.\n\
         2. Inspect additional code only if the map is insufficient.\n\
         3. Make the smallest general grammar/AST/parser/unparser change for this pattern. Touch semantic files only if the focused failure points there.\n\
         4. Run the focused generated test command(s) shown above until they pass.\n\
         5. Do not run `cargo xtask corpus`; the orchestrator owns corpus regression checks.\n\
         6. Stop after focused tests pass. The orchestrator will run tier 2, corpus, and commit.\n",
    );
    Ok(prompt)
}

fn focused_failure_requires_semantic(focused: &FocusedTestRun) -> bool {
    let summary = focused.summary_text().to_ascii_lowercase();
    summary.contains("mtg-semantic")
        || summary.contains("semantic")
        || summary.contains("lower.rs")
        || summary.contains("lowering")
}

fn render_context_block(
    card: &Card,
    error: &str,
    normalized: &str,
    patterns: &[PatternCase],
    test_paths: &[&Path],
) -> String {
    format!(
        "## Retrieved Context\n\n\
         The orchestrator selected these snippets from the generated tests and likely grammar \
         edit sites. Use this context first; inspect whole files only if it is insufficient.\n\n\
         {tests}{code_map}",
        tests = render_generated_test_context(test_paths),
        code_map = render_code_map(card, error, normalized, patterns)
    )
}

fn render_generated_test_context(test_paths: &[&Path]) -> String {
    let mut out = String::new();
    for path in test_paths {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let rel = path.strip_prefix(repo_root()).unwrap_or(path);
        out.push_str(&format!(
            "### Generated Test: {}\n\n```rust\n",
            rel.display()
        ));
        for (idx, line) in text.lines().take(120).enumerate() {
            out.push_str(&format!("{:>4}: {}\n", idx + 1, line));
        }
        out.push_str("```\n\n");
    }
    out
}

fn render_code_map(card: &Card, error: &str, normalized: &str, patterns: &[PatternCase]) -> String {
    let query_terms = context_query_terms(card, error, normalized, patterns);
    let files = [
        grammar_pest_path(),
        ast_rs_path(),
        repo_root().join("crates/mtg-grammar/src/parse.rs"),
        repo_root().join("crates/mtg-grammar/src/unparse.rs"),
    ];
    let mut out = String::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let rel = file.strip_prefix(repo_root()).unwrap_or(&file);
        let snippets = ranked_context_snippets(&text, &query_terms, 4, 8);
        out.push_str(&format!("### {}\n\n", rel.display()));
        if snippets.is_empty() {
            out.push_str("(no direct context matches)\n\n");
            continue;
        }
        for snippet in snippets {
            out.push_str("```text\n");
            for (line_no, line) in snippet {
                out.push_str(&format!("{line_no:>4}: {line}\n"));
            }
            out.push_str("```\n\n");
        }
    }
    out
}

fn context_query_terms(
    card: &Card,
    error: &str,
    normalized: &str,
    patterns: &[PatternCase],
) -> Vec<String> {
    let mut terms = Vec::<String>::new();
    for source in [card.name.as_str(), normalized, error] {
        collect_query_terms(source, &mut terms);
    }
    for pattern in patterns {
        collect_query_terms(&pattern.text, &mut terms);
    }

    let combined = patterns
        .iter()
        .map(|p| p.text.to_ascii_lowercase())
        .chain(std::iter::once(normalized.to_ascii_lowercase()))
        .collect::<Vec<_>>()
        .join("\n");
    add_term(&mut terms, "StaticAbility");
    add_term(&mut terms, "Rule::static");
    add_term(&mut terms, "parse_static");
    add_term(&mut terms, "unparse_static");
    if combined.contains("get") {
        add_term(&mut terms, "PtModifier");
        add_term(&mut terms, "pt_modifier");
        add_term(&mut terms, "modifier");
    }
    if combined.contains("target") {
        add_term(&mut terms, "Target");
        add_term(&mut terms, "target");
    }
    if combined.contains("creature") {
        add_term(&mut terms, "Creature");
        add_term(&mut terms, "PermanentType");
    }
    if combined.contains("artifact") {
        add_term(&mut terms, "Artifact");
        add_term(&mut terms, "PermanentType");
    }
    if combined.contains("enchant") {
        add_term(&mut terms, "Enchant");
        add_term(&mut terms, "Aura");
    }
    if combined.contains("counter") {
        add_term(&mut terms, "Counter");
        add_term(&mut terms, "counter");
    }
    if combined.contains("destroy") {
        add_term(&mut terms, "Destroy");
        add_term(&mut terms, "destroy");
    }
    if combined.contains("tap") || combined.contains("untap") {
        add_term(&mut terms, "tap");
        add_term(&mut terms, "untap");
        add_term(&mut terms, "Tapped");
        add_term(&mut terms, "Untapped");
    }
    for color in ["white", "blue", "black", "red", "green"] {
        if combined.contains(color) {
            add_term(&mut terms, color);
            add_term(&mut terms, "Color");
            add_term(&mut terms, "colored");
        }
    }
    terms
}

fn collect_query_terms(text: &str, out: &mut Vec<String>) {
    for raw in
        text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '+' || c == '/'))
    {
        let token = raw.trim();
        if token.len() < 3 {
            continue;
        }
        let lower = token.to_ascii_lowercase();
        if CONTEXT_STOP_WORDS.contains(&lower.as_str()) {
            continue;
        }
        add_term(out, &lower);
    }
}

fn add_term(out: &mut Vec<String>, term: &str) {
    if !out.iter().any(|existing| existing == term) {
        out.push(term.to_string());
    }
}

fn ranked_context_snippets(
    text: &str,
    terms: &[String],
    max_snippets: usize,
    context_lines: usize,
) -> Vec<Vec<(usize, String)>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut scored = Vec::<(usize, usize)>::new();
    for (idx, line) in lines.iter().enumerate() {
        let lower = line.to_ascii_lowercase();
        let score = terms
            .iter()
            .filter(|term| lower.contains(&term.to_ascii_lowercase()))
            .count();
        if score > 0 {
            scored.push((score, idx));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    let mut ranges = Vec::<(usize, usize)>::new();
    for (_, idx) in scored {
        if ranges.len() >= max_snippets {
            break;
        }
        let start = idx.saturating_sub(context_lines);
        let end = (idx + context_lines + 1).min(lines.len());
        if ranges
            .iter()
            .any(|(existing_start, existing_end)| start < *existing_end && end > *existing_start)
        {
            continue;
        }
        ranges.push((start, end));
    }
    ranges.sort();

    ranges
        .into_iter()
        .map(|(start, end)| {
            (start..end)
                .map(|idx| (idx + 1, lines[idx].to_string()))
                .collect()
        })
        .collect()
}

const CONTEXT_STOP_WORDS: &[&str] = &[
    "the", "and", "you", "your", "this", "that", "with", "from", "into", "onto", "until", "turn",
    "card", "parse", "error", "expected",
];

fn render_semantic_context() -> String {
    let file = lower_rs_path();
    let rel = file.strip_prefix(repo_root()).unwrap_or(&file);
    let text = std::fs::read_to_string(&file).unwrap_or_else(|_| "(unable to read file)\n".into());
    let mut out = format!(
        "## Semantic Failure Context\n\n\
         The focused failure references semantic flow, so this reactive context is included for `{}`:\n\n```text\n",
        rel.display()
    );
    for (idx, line) in text.lines().take(160).enumerate() {
        out.push_str(&format!("{:>4}: {}\n", idx + 1, line));
    }
    out.push_str("```\n");
    out
}

fn render_file_excerpt(path: &Path, max_lines: usize) -> String {
    let rel = path.strip_prefix(repo_root()).unwrap_or(path);
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| "(unable to read file)\n".into());
    let mut out = format!("### {}\n\n```text\n", rel.display());
    for (idx, line) in text.lines().take(max_lines).enumerate() {
        out.push_str(&format!("{:>4}: {}\n", idx + 1, line));
    }
    out.push_str("```\n\n");
    out
}

fn build_supervisor_prompt(error: &anyhow::Error, iter_index: u32, attempt: u8) -> Result<String> {
    let git_status = command_stdout("git", &["status", "--short"])?;
    let recent_log_dirs = recent_grammar_fix_logs(8);
    Ok(format!(
        "{intro}\n\n\
         ## Unknown Problem\n\n\
         The grammar-fix orchestrator hit an unexpected error while running iteration {iter_index}. \
         This is not a known grammar gate such as tier-2 failure, corpus regression, or an agent \
         saying it could not solve the card.\n\n\
         Attempt: {attempt}\n\n\
         Error:\n```text\n{error:#}\n```\n\n\
         Current git status:\n```text\n{git_status}```\n\n\
         Recent `.grammar-fix` logs:\n```text\n{recent_log_dirs}```\n\n\
         ## Mission\n\n\
         Diagnose and fix the orchestrator or repository state so the main grammar-fix loop can \
         continue autonomously. Prefer a general fix over a one-off workaround. Keep the patch \
         tightly scoped to the failure mode.\n\n\
         ## Rules\n\n\
         1. Do not bypass deterministic gates. The normal flow must still run tier-2 tests, corpus, \
         and commit steps where applicable.\n\
         2. Do not discard user changes. Work with the current tree.\n\
         3. If the bug is in `xtask`, update `xtask` and add or adjust focused tests where practical.\n\
         4. Run `cargo fmt` and the narrowest relevant tests before exiting successfully.\n\
         5. If you cannot repair it safely, exit non-zero or clearly say why.\n",
        intro = SUPERVISOR_PROMPT_INTRO,
    ))
}

fn build_downstream_repair_prompt(
    card: &Card,
    normalized: &str,
    tier2_output: &str,
) -> Result<String> {
    let git_status = command_stdout("git", &["status", "--short"])?;
    let generated_test = generated_test_path(card);
    let generated_pattern_test = generated_pattern_test_path(card);
    Ok(format!(
        "{intro}\n\n\
         ## Situation\n\n\
         The focused generated grammar tests for this card pass, but the orchestrator's downstream \
         `cargo xtask test --tier 2` gate failed. This usually means the grammar-side AST change \
         needs follow-up wiring in another crate, commonly `mtg-semantic`, or the property-test \
         strategies no longer cover the parser-produced AST.\n\n\
         ## Card\n\n\
         - Name        : {name}\n\
         - Set         : {set}\n\
         - Collector # : {collector}\n\n\
         Normalized oracle text:\n```text\n{normalized}\n```\n\n\
         ## Tier-2 Output\n\n```text\n{tier2_output}\n```\n\n\
         ## Current Git Status\n\n```text\n{git_status}```\n\n\
         ## Generated Test Context\n\n\
         {generated_tests}\n\
         ## Relevant Source Context\n\n\
         {semantic_ir}\n\
         {semantic_lower}\n\
         {grammar_ast}\n\
         ## Mission\n\n\
         Make the downstream tier-2 gate recover from this parser/AST extension. Prefer the \
         repository's established pattern: when the semantic IR does not yet perform deeper \
         resolution, mirror grammar AST nodes through the IR and lowering layer. Keep the patch \
         tightly scoped to the downstream failure.\n\n\
         ## Rules\n\n\
         1. Do not undo the grammar/parser/unparser repair that made the focused generated tests pass.\n\
         2. Do not weaken or disable tests.\n\
         3. Update focused tests or property strategies when they claim coverage over parser-produced ASTs.\n\
         4. Run the narrowest relevant checks before exiting successfully. The orchestrator will rerun \
         focused generated tests, tier 2, corpus, and commit.\n",
        intro = DOWNSTREAM_REPAIR_PROMPT_INTRO,
        name = card.name,
        set = card.set_code,
        collector = card.collector_number,
        normalized = normalized,
        tier2_output = trim(tier2_output, 24_000),
        git_status = git_status,
        generated_tests =
            render_generated_test_context(&[generated_test.as_path(), generated_pattern_test.as_path()]),
        semantic_ir = render_file_excerpt(&repo_root().join("crates/mtg-semantic/src/ir.rs"), 220),
        semantic_lower = render_file_excerpt(&lower_rs_path(), 220),
        grammar_ast = render_file_excerpt(&ast_rs_path(), 220),
    ))
}

const SUPERVISOR_PROMPT_INTRO: &str = "\
You are the grammar-fix supervisor. The normal card-solving agent or deterministic
orchestrator flow encountered an unknown infrastructure problem. Your job is to mend
the automation itself so the outer process can retry without human intervention.";

const DOWNSTREAM_REPAIR_PROMPT_INTRO: &str = "\
You are the grammar-fix downstream repair agent. The card-specific grammar repair has
passed focused validation, but a later deterministic gate exposed required follow-up
wiring. Your job is to make the existing change pass the downstream gate without
weakening the gate.";

const PROMPT_INTRO: &str = "\
You are extending the mtg-parser grammar to handle one specific
Magic: The Gathering card. The orchestrator that invoked you is
responsible for the test/corpus/commit gates afterwards. Your job is
the creative step: change the grammar, AST, parser, and unparser as needed
so the failing test below passes, without breaking anything else. Semantic
wiring is reactive: touch semantic files only when the focused test failure
points there.";

const WORKFLOW_BLOCK: &str = "\
## Workflow

1. Start from the retrieved context and generated test snippets above.
   Do not read whole source files up front.
2. Inspect additional source only when the retrieved snippets are
   insufficient, and prefer narrow line ranges around the relevant rule,
   enum, parser branch, or unparser branch.
3. Decide what general pattern this card is an instance of (a keyword
   ability, a triggered ability, a static effect, ...). Name it.
4. Extend `grammar.pest` to recognize the pattern.
5. Extend `ast.rs` with whatever new node(s) the grammar needs.
6. Extend `parse.rs` and `unparse.rs` so the AST round-trips cleanly.
7. Run only the focused generated test command(s) supplied by the orchestrator.
8. Stop once focused generated tests pass. The orchestrator runs tier 2,
   corpus, and commit gates.";

const CONSTRAINTS_BLOCK: &str = "\
## Constraints

1. **Do not modify only the unparser** to make round-trip pass. A
   round-trip failure after your grammar change is a signal that the AST
   design may be wrong or there's new ambiguity. Think about it; don't
   paper over it.
2. **Do not add a special-case rule for this one card.** If the pattern
   looks specific to one card, ask yourself what *general* pattern it's
   an instance of and encode that.
3. **Do not touch existing grammar rules unless necessary.** Additive
   changes are safer than modifications. If you must modify, leave a
   one-line comment explaining why.
4. **Do not disable or modify existing tests.** Tests under `tests/unit.rs`
   and `tests/prop.rs` are the contract; the new generated test is what
   you're making pass.
5. **Stay within scope.** The default grammar repair files are
   `grammar.pest`, `ast.rs`, `parse.rs`, and `unparse.rs`. Touch
   `mtg-semantic` only if the focused failure requires semantic
   handling. Do not touch generated tests, `mtg-scryfall`, `mtg-corpus`,
   or `xtask` for ordinary card repairs.
6. **Do not run broad gates.** The orchestrator owns `cargo xtask test
   --tier 2`, `cargo xtask corpus`, and commit.
7. **If you can't solve it, say so.** Better to surface \"I don't see
   how to extend the grammar without restructuring X\" than to ship a
   hack. The orchestrator will leave the working tree as-is for human
   triage.";

fn render_card_block(card: &Card, normalized: &str) -> String {
    let normalized_block = if normalized == card.oracle_text {
        String::new()
    } else {
        format!("\nNormalized text (what the parser sees):\n```\n{normalized}\n```\n")
    };
    format!(
        "## Card\n\n\
         - Name           : {name}\n\
         - Set            : {set}\n\
         - Collector #    : {collector}\n\
         - Layout         : {layout:?}\n\n\
         Raw oracle text:\n```\n{oracle}\n```\n{norm}",
        name = card.name,
        set = card.set_code,
        collector = card.collector_number,
        layout = card.layout,
        oracle = card.oracle_text,
        norm = normalized_block,
    )
}

fn render_error_block(error: &str) -> String {
    format!("## Round-trip error from the current grammar\n\n```\n{error}\n```")
}

fn render_test_block(test_rel: &str) -> String {
    format!(
        "## Failing test\n\n\
         The orchestrator just wrote `{test_rel}`. It calls\n\
         `mtg_grammar::parse(text)` on the normalized oracle text and\n\
         asserts that re-parsing the unparsed AST yields the same AST."
    )
}

// ---------------------------------------------------------------------------
// agent subprocess
// ---------------------------------------------------------------------------

struct AgentOutcome {
    success: bool,
    exit_code: i32,
    assistant_text: String,
    assistant_blocks: usize,
}

fn invoke_downstream_repair(
    opts: &Options,
    sink: &mut dyn FlowSink,
    card: &Card,
    normalized: &str,
    parent_log_dir: &Path,
    iter_index: u32,
) -> Result<bool> {
    let log_dir = parent_log_dir.join("downstream-repair-attempt-1");
    std::fs::create_dir_all(&log_dir).with_context(|| format!("create {}", log_dir.display()))?;

    sink.emit(FlowEvent::Note {
        level: NoteLevel::Warn,
        text: format!(
            "tier-2 failed in iteration {iter_index}; invoking {} downstream repair",
            opts.agent.label()
        ),
    });
    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: format!("downstream repair log dir: {}", log_dir.display()),
    });

    let tier2_output = command_output_allow_failure("cargo", &["xtask", "test", "--tier", "2"])?;
    std::fs::write(log_dir.join("tier2_failure.txt"), &tier2_output)?;
    let prompt = build_downstream_repair_prompt(card, normalized, &tier2_output)?;
    std::fs::write(log_dir.join("prompt.md"), &prompt)?;

    let transcript_path = log_dir.join("transcript.ndjson");
    let outcome = invoke_agent(opts.agent, &prompt, &transcript_path, sink)?;
    std::fs::write(log_dir.join("response.md"), &outcome.assistant_text)?;
    if !outcome.success {
        sink.emit(FlowEvent::Note {
            level: NoteLevel::Error,
            text: format!(
                "{} downstream repair exited with status {}",
                opts.agent.label(),
                outcome.exit_code
            ),
        });
        return Ok(false);
    }

    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: "downstream repair finished; running cargo fmt".into(),
    });
    if !run_cargo_fmt()? {
        sink.emit(FlowEvent::Note {
            level: NoteLevel::Error,
            text: "cargo fmt failed after downstream repair".into(),
        });
        return Ok(false);
    }

    Ok(true)
}

fn invoke_supervisor(
    opts: &Options,
    sink: &mut dyn FlowSink,
    iter_index: u32,
    attempt: u8,
    error: &anyhow::Error,
) -> Result<bool> {
    let log_dir = create_supervisor_log_dir(iter_index, attempt)?;
    let error_text = format!("{error:#}");
    std::fs::write(log_dir.join("error.txt"), &error_text)?;
    let prompt = build_supervisor_prompt(error, iter_index, attempt)?;
    std::fs::write(log_dir.join("prompt.md"), &prompt)?;

    sink.emit(FlowEvent::Note {
        level: NoteLevel::Warn,
        text: format!(
            "unknown problem in iteration {iter_index}; invoking {} supervisor attempt {attempt}/{}",
            opts.agent.label(),
            opts.supervisor_attempts
        ),
    });
    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: format!("supervisor log dir: {}", log_dir.display()),
    });

    let transcript_path = log_dir.join("transcript.ndjson");
    let outcome = invoke_agent(opts.agent, &prompt, &transcript_path, sink)?;
    std::fs::write(log_dir.join("response.md"), &outcome.assistant_text)?;
    if !outcome.success {
        sink.emit(FlowEvent::Note {
            level: NoteLevel::Error,
            text: format!(
                "{} supervisor exited with status {}",
                opts.agent.label(),
                outcome.exit_code
            ),
        });
        return Ok(false);
    }

    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: "supervisor finished; running cargo fmt".into(),
    });
    if !run_cargo_fmt()? {
        sink.emit(FlowEvent::Note {
            level: NoteLevel::Error,
            text: "cargo fmt failed after supervisor repair".into(),
        });
        return Ok(false);
    }

    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: "running cargo test -p xtask after supervisor repair".into(),
    });
    let tests_ok = Command::new("cargo")
        .args(["test", "-p", "xtask"])
        .current_dir(repo_root())
        .status()
        .context("run cargo test -p xtask after supervisor repair")?
        .success();
    if !tests_ok {
        sink.emit(FlowEvent::Note {
            level: NoteLevel::Error,
            text: "cargo test -p xtask failed after supervisor repair".into(),
        });
        return Ok(false);
    }

    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: "supervisor repair validated; retrying the same iteration".into(),
    });
    Ok(true)
}

fn invoke_agent(
    provider: AgentProvider,
    prompt: &str,
    transcript_path: &Path,
    sink: &mut dyn FlowSink,
) -> Result<AgentOutcome> {
    match provider {
        AgentProvider::Codex => invoke_codex(prompt, transcript_path, sink),
        AgentProvider::Claude => invoke_claude(prompt, transcript_path, sink),
    }
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

fn invoke_codex(
    prompt: &str,
    transcript_path: &Path,
    sink: &mut dyn FlowSink,
) -> Result<AgentOutcome> {
    invoke_jsonl_agent(
        AgentProvider::Codex,
        base_agent_command(AgentProvider::Codex),
        prompt,
        transcript_path,
        sink,
    )
    .context("spawn `codex exec --json`. Is the Codex CLI installed and authenticated?")
}

fn invoke_claude(
    prompt: &str,
    transcript_path: &Path,
    sink: &mut dyn FlowSink,
) -> Result<AgentOutcome> {
    invoke_jsonl_agent(
        AgentProvider::Claude,
        base_agent_command(AgentProvider::Claude),
        prompt,
        transcript_path,
        sink,
    )
    .context(
        "spawn `claude`. Is the Claude Code CLI installed and on PATH? \
         See https://docs.claude.com/claude-code for setup.",
    )
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
        .spawn()?;
    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        stdin.write_all(prompt.as_bytes())?;
    }

    let stdout = child.stdout.take().expect("piped stdout");
    let reader = BufReader::new(stdout);
    let mut transcript = std::fs::File::create(transcript_path)
        .with_context(|| format!("create {}", transcript_path.display()))?;
    let start = Instant::now();
    let mut assistant_text: Vec<String> = Vec::new();
    let mut assistant_blocks = 0usize;

    for raw in reader.lines() {
        let line = raw?;
        writeln!(transcript, "{line}")?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(&line) {
            Ok(parsed) => {
                let elapsed_secs = start.elapsed().as_secs();
                collect_assistant_text(
                    provider,
                    &parsed,
                    &mut assistant_text,
                    &mut assistant_blocks,
                );
                sink.emit(FlowEvent::AgentEvent {
                    provider,
                    raw: parsed,
                    elapsed_secs,
                });
            }
            Err(_) => {
                sink.emit(FlowEvent::Note {
                    level: NoteLevel::Warn,
                    text: format!(
                        "non-JSON line from {}: {}",
                        provider.label(),
                        trim(&line, 200)
                    ),
                });
            }
        }
    }
    let status = child.wait()?;
    let exit_code = status.code().unwrap_or(-1);
    Ok(AgentOutcome {
        success: status.success(),
        exit_code,
        assistant_text: assistant_text.join("\n\n"),
        assistant_blocks,
    })
}

fn collect_assistant_text(
    provider: AgentProvider,
    parsed: &serde_json::Value,
    assistant_text: &mut Vec<String>,
    assistant_blocks: &mut usize,
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
                            *assistant_blocks += 1;
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
                    *assistant_blocks += 1;
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

fn run_xtask(args: &[&str]) -> Result<bool> {
    let status = Command::new("cargo")
        .arg("xtask")
        .args(args)
        .current_dir(repo_root())
        .status()
        .context("run cargo xtask")?;
    Ok(status.success())
}

fn run_cargo_fmt() -> Result<bool> {
    let status = Command::new("cargo")
        .arg("fmt")
        .arg("--all")
        .current_dir(repo_root())
        .status()
        .context("run cargo fmt")?;
    Ok(status.success())
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

fn command_output_allow_failure(program: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(repo_root())
        .output()
        .with_context(|| format!("run {program} {}", args.join(" ")))?;
    let mut text = String::new();
    if !out.stdout.is_empty() {
        text.push_str(&String::from_utf8_lossy(&out.stdout));
    }
    if !out.stderr.is_empty() {
        if !text.ends_with('\n') && !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&String::from_utf8_lossy(&out.stderr));
    }
    if text.is_empty() {
        Ok(format!(
            "(empty output; exit status {})\n",
            out.status.code().unwrap_or(1)
        ))
    } else {
        Ok(text)
    }
}

fn recent_grammar_fix_logs(limit: usize) -> String {
    let root = grammar_fix_log_root();
    let mut entries = match std::fs::read_dir(root) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                let modified = entry.metadata().and_then(|m| m.modified()).ok()?;
                Some((modified, name))
            })
            .collect::<Vec<_>>(),
        Err(_) => return "(none)\n".to_string(),
    };
    entries.sort_by_key(|(modified, _)| *modified);
    entries
        .into_iter()
        .rev()
        .take(limit)
        .map(|(_, name)| name)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

// ---------------------------------------------------------------------------
// git
// ---------------------------------------------------------------------------

enum CommitOutcome {
    Committed,
    NoChanges,
}

fn git_commit(message: &str) -> Result<CommitOutcome> {
    if !run_cargo_fmt()? {
        bail!("cargo fmt failed before git commit");
    }
    if !run_xtask(&["corpus"])? {
        bail!("cargo xtask corpus failed before git commit");
    }

    let add = Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root())
        .status()?;
    if !add.success() {
        bail!("git add failed");
    }
    let diff = Command::new("git")
        .args(["diff", "--cached", "--quiet", "--exit-code"])
        .current_dir(repo_root())
        .status()?;
    if diff.success() {
        return Ok(CommitOutcome::NoChanges);
    }
    if diff.code() != Some(1) {
        bail!("git diff --cached failed");
    }
    let commit = Command::new("git")
        .args(git_commit_args(message))
        .current_dir(repo_root())
        .output()?;
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

fn git_commit_args(message: &str) -> Vec<&str> {
    vec!["commit", "--no-verify", "-m", message]
}

fn git_diff_against_head_parent() -> Result<String> {
    let out = Command::new("git")
        .args(["diff", "HEAD~1..HEAD"])
        .current_dir(repo_root())
        .output()?;
    if !out.status.success() {
        bail!("git diff failed");
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub(crate) fn commit_message(
    card: &Card,
    new_passes: usize,
    pass_count: usize,
    total: usize,
) -> String {
    format!(
        "grammar: support card {name}\n\n\
         Card: {name} ({set})\n\
         New passes: {new_passes}\n\
         Status: {pass_count}/{total}\n",
        name = card.name,
        set = card.set_code,
        new_passes = new_passes,
        pass_count = pass_count,
        total = total,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_default_to_lea_one_iteration() {
        let o = Options::parse(&[]).unwrap();
        assert_eq!(o.set, "lea");
        assert_eq!(o.max_iterations, 0);
        assert!(!o.dry_run);
        assert!(!o.allow_dirty);
        assert_eq!(o.supervisor_attempts, DEFAULT_SUPERVISOR_ATTEMPTS);
        assert_eq!(o.agent, AgentProvider::Codex);
    }

    #[test]
    fn options_parse_long_flags_and_equals_form() {
        let args: Vec<String> = ["--set=neo", "--max-iterations", "5", "--dry-run"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let o = Options::parse(&args).unwrap();
        assert_eq!(o.set, "neo");
        assert_eq!(o.max_iterations, 5);
        assert!(o.dry_run);
    }

    #[test]
    fn options_swallow_ui_flag() {
        // main.rs parses --ui before calling us; we just need to not
        // explode if it slips through.
        let args = vec!["--ui".to_string(), "tui".to_string()];
        Options::parse(&args).expect("--ui tui should not error");
    }

    #[test]
    fn options_parse_agent_flag() {
        let args = vec!["--agent".to_string(), "claude".to_string()];
        let o = Options::parse(&args).unwrap();
        assert_eq!(o.agent, AgentProvider::Claude);
    }

    #[test]
    fn options_parse_supervisor_flags() {
        let args = vec!["--supervisor-attempts=3".to_string()];
        let o = Options::parse(&args).unwrap();
        assert_eq!(o.supervisor_attempts, 3);

        let args = vec!["--no-supervisor".to_string()];
        let o = Options::parse(&args).unwrap();
        assert_eq!(o.supervisor_attempts, 0);
    }

    #[test]
    fn pattern_extraction_is_quote_aware() {
        let text = "Enchant creature card in a graveyard\nWhen this Aura enters, if it's on the battlefield, it loses \"enchant creature card in a graveyard\" and gains \"enchant creature put onto the battlefield with this Aura.\" Return enchanted creature card to the battlefield under your control and attach this Aura to it. When this Aura leaves the battlefield, that creature's controller sacrifices it.\nEnchanted creature gets -1/-0.";
        let patterns = extract_patterns(text);
        assert!(patterns
            .iter()
            .any(|p| p.text == "Enchant creature card in a graveyard"));
        assert!(patterns.iter().any(|p| p.text
            == "When this Aura enters, if it's on the battlefield, it loses \"enchant creature card in a graveyard\" and gains \"enchant creature put onto the battlefield with this Aura.\" Return enchanted creature card to the battlefield under your control and attach this Aura to it."));
        assert!(patterns.iter().any(|p| p.text
            == "When this Aura leaves the battlefield, that creature's controller sacrifices it."));
        assert!(patterns
            .iter()
            .any(|p| p.text == "Enchanted creature gets -1/-0."));
    }

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("Lightning Bolt"), "lightning_bolt");
        assert_eq!(slugify("Sol'kanar the Tainted"), "sol_kanar_the_tainted");
    }

    #[test]
    fn commit_message_has_required_lines() {
        let card = mtg_scryfall::Card {
            name: "Air Elemental".into(),
            set_code: "lea".into(),
            collector_number: "46".into(),
            oracle_text: "Flying".into(),
            mana_cost: "{3}{U}{U}".into(),
            layout: mtg_scryfall::Layout::Normal,
        };
        let m = commit_message(&card, 1, 1, 290);
        assert!(m.starts_with("grammar: support card Air Elemental\n"));
        assert!(m.contains("Card: Air Elemental (lea)"));
        assert!(m.contains("New passes: 1"));
        assert!(m.contains("Status: 1/290"));
    }

    #[test]
    fn git_commit_args_skip_duplicate_mutating_hook() {
        let args = git_commit_args("subject\n\nbody");
        assert_eq!(args, vec!["commit", "--no-verify", "-m", "subject\n\nbody"]);
    }

    #[test]
    fn pest_rule_detector_handles_modifiers_and_comments() {
        assert!(is_pest_rule_declaration("foo = { bar }"));
        assert!(is_pest_rule_declaration("    bar = _{ ASCII }"));
        assert!(is_pest_rule_declaration("baz_qux = @{ x }"));
        assert!(!is_pest_rule_declaration("// foo = { bar }"));
        assert!(!is_pest_rule_declaration("FOO = { bar }"));
        assert!(!is_pest_rule_declaration(""));
        assert!(!is_pest_rule_declaration("    "));
    }
}
