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

use anyhow::{anyhow, bail, Context, Result};

use crate::add_card;
use crate::console_sink::ConsoleSink;
use crate::flow::{AgentProvider, FlowEvent, FlowSink, NoteLevel};
use crate::paths::{grind_log_root, repo_root};
use crate::refactor_hotspot::{self, IterationOutcome};

const DEFAULT_STOP_AFTER: u32 = 3;
const DEFAULT_MAX_REFACTOR_ITERATIONS: u32 = 50;
const DEFAULT_REPAIR_ATTEMPTS: u8 = 1;

const HELP: &str = "\
cargo xtask grind [--set CODE]
                  [--stop-after N] [--max-refactor-iterations N]
                  [--max-card-iterations N] [--repair-attempts N]
                  [--theme THEME] [--target PATH]
                  [--agent codex|claude] [--ui console|tui]
                  [--allow-dirty] [--dry-run]

Autonomous TDD-style meta-loop. Phase 1 runs refactor-hotspot one iteration
at a time until the no-op streak hits --stop-after (default 3) or
--max-refactor-iterations (default 50) is reached. Phase 2 then runs
add-card on the cleaner foundation. Gate failures route to a repair agent
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
    /// 0 = unbounded (matches add-card's convention).
    pub max_card_iterations: u32,
    pub repair_attempts: u8,
    pub agent: AgentProvider,
    pub theme: Option<String>,
    pub target: Option<String>,
    pub allow_dirty: bool,
    pub dry_run: bool,
}

impl Options {
    pub fn parse(args: &[String]) -> Result<Self> {
        let mut set = None::<String>;
        let mut stop_after = DEFAULT_STOP_AFTER;
        let mut max_refactor_iterations = DEFAULT_MAX_REFACTOR_ITERATIONS;
        let mut max_card_iterations = 0u32;
        let mut repair_attempts = DEFAULT_REPAIR_ATTEMPTS;
        let mut agent = AgentProvider::Codex;
        let mut theme = None::<String>;
        let mut target = None::<String>;
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

        Ok(Self {
            set,
            stop_after,
            max_refactor_iterations,
            max_card_iterations,
            repair_attempts,
            agent,
            theme,
            target,
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

pub fn run_with_sink(opts: Options, sink: &mut dyn FlowSink) -> Result<ExitCode> {
    if !opts.dry_run && !opts.allow_dirty {
        ensure_clean_working_tree()
            .context("working tree must be clean (or pass --allow-dirty)")?;
    }

    run_refactor_phase(&opts, sink)?;
    run_add_card_phase(&opts, sink)
}

fn run_refactor_phase(opts: &Options, sink: &mut dyn FlowSink) -> Result<()> {
    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: format!(
            "grind: refactor phase — stop after {} consecutive no-ops (ceiling {})",
            opts.stop_after, opts.max_refactor_iterations
        ),
    });

    // allow_dirty=true for the inner refactor opts because grind already
    // checked the precondition at the top. Subsequent iterations will
    // legitimately leave behind committed (or restored-after-repair)
    // state that the inner check would reject.
    let refactor_opts = refactor_hotspot::Options::for_grind(
        opts.theme.as_deref(),
        opts.target.clone(),
        opts.agent,
        true,
    )?;

    let mut no_op_streak = 0u32;
    for iteration in 1..=opts.max_refactor_iterations {
        let outcome = refactor_hotspot::run_single_iteration(&refactor_opts, sink, iteration)?;
        match outcome {
            IterationOutcome::Committed {
                new_passes,
                corpus_passing,
                corpus_total,
                grammar_rules,
                duration_secs,
            } => {
                no_op_streak = 0;
                sink.emit(FlowEvent::Note {
                    level: NoteLevel::Info,
                    text: format!(
                        "grind: iter {iteration} committed +{new_passes} ({corpus_passing}/{corpus_total}) — {grammar_rules} grammar rules, {duration_secs}s"
                    ),
                });
            }
            IterationOutcome::NoChanges => {
                no_op_streak += 1;
                sink.emit(FlowEvent::Note {
                    level: NoteLevel::Info,
                    text: format!(
                        "grind: no-op {}/{} (iteration {})",
                        no_op_streak, opts.stop_after, iteration
                    ),
                });
                if no_op_streak >= opts.stop_after {
                    sink.emit(FlowEvent::Note {
                        level: NoteLevel::Info,
                        text: format!(
                            "grind: refactor phase complete — {} consecutive no-ops after {} iterations",
                            no_op_streak, iteration
                        ),
                    });
                    return Ok(());
                }
            }
            IterationOutcome::GateFailed(error) => {
                sink.emit(FlowEvent::Note {
                    level: NoteLevel::Warn,
                    text: format!("grind: refactor gate failed: {error:#}"),
                });
                let repaired = try_repair(opts, sink, "refactor", iteration, &error)?;
                if !repaired {
                    // Repair couldn't fix it. Restore the tree so the next
                    // iteration starts clean, count this as a no-op so the
                    // loop still terminates.
                    discard_working_changes(sink)?;
                    no_op_streak += 1;
                    sink.emit(FlowEvent::Note {
                        level: NoteLevel::Warn,
                        text: format!(
                            "grind: repair exhausted, treating as no-op {}/{}",
                            no_op_streak, opts.stop_after
                        ),
                    });
                    if no_op_streak >= opts.stop_after {
                        return Ok(());
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

    // Lightweight validation: anything the repair agent touched should at
    // least format and compile. Deeper gates (tier-2, corpus) are run by
    // the next refactor iteration or by add-card itself.
    if !command_success("cargo", &["fmt", "--all"])? {
        sink.emit(FlowEvent::Note {
            level: NoteLevel::Error,
            text: "grind: cargo fmt failed after repair".to_string(),
        });
        return Ok(false);
    }
    if !command_success("cargo", &["check", "--workspace"])? {
        sink.emit(FlowEvent::Note {
            level: NoteLevel::Error,
            text: "grind: cargo check failed after repair".to_string(),
        });
        return Ok(false);
    }

    sink.emit(FlowEvent::Note {
        level: NoteLevel::Info,
        text: "grind: repair validated (fmt + check)".to_string(),
    });
    Ok(true)
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
smallest patch that makes the gate green. If you cannot repair it safely, \
exit non-zero and the orchestrator will discard the working tree and treat \
this iteration as a no-op.\n\n\
## Rules\n\n\
1. Do not weaken or disable existing tests.\n\
2. Do not bypass deterministic gates (tier-2, corpus, audit).\n\
3. Run `cargo fmt --all` before exiting successfully.\n\
4. Keep edits tightly scoped to the failure.\n"
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

fn command_success(program: &str, args: &[&str]) -> Result<bool> {
    let out = Command::new(program)
        .args(args)
        .current_dir(repo_root())
        .output()
        .with_context(|| format!("run {program} {}", args.join(" ")))?;
    Ok(out.status.success())
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
        assert_eq!(opts.max_refactor_iterations, DEFAULT_MAX_REFACTOR_ITERATIONS);
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
            "--set", "neo",
            "--stop-after", "5",
            "--max-refactor-iterations", "10",
            "--max-card-iterations", "3",
            "--repair-attempts", "2",
            "--agent", "claude",
            "--theme", "damage",
            "--target", "crates/mtg-grammar/src/grammar.pest",
            "--allow-dirty",
            "--dry-run",
        ]);
        let opts = Options::parse(&args).expect("parse");
        assert_eq!(opts.set.as_deref(), Some("neo"));
        assert_eq!(opts.stop_after, 5);
        assert_eq!(opts.max_refactor_iterations, 10);
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
            "--repair-attempts=0",
            "--agent=claude",
            "--theme=destroy",
        ]);
        let opts = Options::parse(&args).expect("parse");
        assert_eq!(opts.set.as_deref(), Some("neo"));
        assert_eq!(opts.stop_after, 4);
        assert_eq!(opts.max_refactor_iterations, 20);
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
        let err = Options::parse(&s(&["--max-refactor-iterations", "0"]))
            .expect_err("should reject");
        assert!(err.to_string().contains("max-refactor-iterations"));
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
}
