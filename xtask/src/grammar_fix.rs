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

use mtg_corpus::{find_next_failing_card, NextCard};
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
const TOTAL_STEPS: u8 = 9;

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
    while opts.max_iterations == 0 || iter < opts.max_iterations {
        match run_one_iteration(&client, &opts, sink, iter + 1)? {
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
            }
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
    pub dry_run: bool,
    pub allow_dirty: bool,
    pub agent: AgentProvider,
}

impl Options {
    pub fn parse(args: &[String]) -> Result<Self> {
        let mut set = None::<String>;
        let mut max_iterations = DEFAULT_MAX_ITERATIONS;
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
    let (card, error, normalized) = match find_next_failing_card(client, &opts.set)? {
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

    // The promoted-test paths are deterministic from the card slug; we
    // compute them now so prompts can reference them but write the
    // files only past the dry-run gate.
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

    // Step 5 — promote deterministic pattern tests first.
    sink.emit(FlowEvent::StepStarted {
        index: 5,
        total: TOTAL_STEPS,
        label: "promote pattern tests".into(),
    });
    let patterns = extract_patterns(&normalized);
    write_pattern_tests(&pattern_test_path, &card, &patterns)
        .context("generate pattern test file")?;
    register_generated_pattern_test(&pattern_test_path)
        .context("register generated pattern test")?;
    sink.emit(FlowEvent::StepFinished {
        index: 5,
        ok: true,
        summary: Some(format!(
            "{} patterns · {}",
            patterns.len(),
            pattern_test_path
                .strip_prefix(repo_root())
                .unwrap_or(&pattern_test_path)
                .display()
        )),
    });

    // Step 6 — delegate to the configured agent (the one non-deterministic step).
    sink.emit(FlowEvent::StepStarted {
        index: 6,
        total: TOTAL_STEPS,
        label: format!("{} agent", opts.agent.label()),
    });
    let transcript_path = log_dir.join("transcript.ndjson");
    let pattern_prompt = build_pattern_prompt(
        &card,
        &error,
        &normalized,
        &test_path,
        &pattern_test_path,
        &patterns,
    )?;
    std::fs::write(log_dir.join("pattern_prompt.md"), &pattern_prompt)?;
    let agent_outcome = invoke_agent(opts.agent, &pattern_prompt, &transcript_path, sink)?;
    std::fs::write(log_dir.join("response.md"), &agent_outcome.assistant_text)?;
    sink.emit(FlowEvent::StepFinished {
        index: 6,
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

    // If the agent got all parse-only patterns green, add the full
    // round-trip test before the deterministic gates below.
    write_promoted_test(&test_path, &card, &normalized).context("generate promoted test file")?;
    register_generated_test(&test_path).context("register generated test")?;

    // Step 7 — tier 1 + tier 2.
    sink.emit(FlowEvent::StepStarted {
        index: 7,
        total: TOTAL_STEPS,
        label: "cargo xtask test --tier 2".into(),
    });
    let tests_ok = run_xtask(&["test", "--tier", "2"])?;
    sink.emit(FlowEvent::StepFinished {
        index: 7,
        ok: tests_ok,
        summary: None,
    });
    if !tests_ok {
        let reason = "cargo xtask test --tier 2 failed after the agent's pass".to_string();
        sink.emit(FlowEvent::IterationFinished {
            index: iter_index,
            outcome: IterationOutcomeSummary::SurfacedToHuman {
                reason: reason.clone(),
            },
        });
        return Ok(IterationOutcome::SurfaceToHuman(reason));
    }

    // Step 8 — corpus regression gate.
    sink.emit(FlowEvent::StepStarted {
        index: 8,
        total: TOTAL_STEPS,
        label: "cargo xtask corpus (regression gate)".into(),
    });
    let corpus_ok = run_xtask(&["corpus"])?;
    sink.emit(FlowEvent::StepFinished {
        index: 8,
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

    // Step 9 — commit.
    let new_pass_count = read_corpus_passing(&corpus_status_path()).unwrap_or(baseline_pass_count);
    let total = read_corpus_total(&corpus_status_path()).unwrap_or(0);
    let new_passes = new_pass_count.saturating_sub(baseline_pass_count);
    let commit_msg = commit_message(&card, new_passes, new_pass_count, total);
    std::fs::write(log_dir.join("commit_message.txt"), &commit_msg)?;
    sink.emit(FlowEvent::StepStarted {
        index: 9,
        total: TOTAL_STEPS,
        label: "git commit".into(),
    });
    git_commit(&commit_msg).context("git commit")?;
    sink.emit(FlowEvent::StepFinished {
        index: 9,
        ok: true,
        summary: Some(format!(
            "+{new_passes} pass · status {new_pass_count}/{total}"
        )),
    });

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

fn write_promoted_test(path: &Path, card: &Card, normalized: &str) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| anyhow!("test path has no parent"))?;
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let body = render_promoted_test(card, normalized);
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

fn render_promoted_test(card: &Card, normalized: &str) -> String {
    format!(
        "// Generated by `cargo xtask grammar-fix`. Once green and you're\n\
         // happy with the grammar shape, move this into tests/ proper.\n\
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
    let grammar = std::fs::read_to_string(grammar_pest_path()).context("read grammar.pest")?;
    let ast = std::fs::read_to_string(ast_rs_path()).context("read ast.rs")?;
    let lower = std::fs::read_to_string(lower_rs_path()).context("read lower.rs")?;
    let test_rel = test_path
        .strip_prefix(repo_root())
        .unwrap_or(test_path)
        .display()
        .to_string();

    Ok(format!(
        "{intro}\n\n\
         {card_block}\n\n\
         {error_block}\n\n\
         {test_block}\n\n\
         {files_block}\n\n\
         {workflow_block}\n\n\
         {constraints_block}\n",
        intro = PROMPT_INTRO,
        card_block = render_card_block(card, normalized),
        error_block = render_error_block(error),
        test_block = render_test_block(&test_rel),
        files_block = render_files_block(&grammar, &ast, &lower),
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
) -> Result<String> {
    let mut prompt = build_prompt(card, error, normalized, full_test_path)?;
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
        "Work pattern by pattern. Keep changes general, then run `cargo xtask test --tier 2`. \
         The orchestrator will add the full generated round-trip test after you exit successfully.\n",
    );
    Ok(prompt)
}

const PROMPT_INTRO: &str = "\
You are extending the mtg-parser grammar to handle one specific
Magic: The Gathering card. The orchestrator that invoked you is
responsible for the test/corpus/commit gates afterwards. Your job is
the creative step: change the grammar (and AST / lowering if needed)
so the failing test below passes, without breaking anything else.";

const WORKFLOW_BLOCK: &str = "\
## Workflow

1. Read `grammar.pest`, `ast.rs`, `lower.rs`, and the failing test.
2. Decide what general pattern this card is an instance of (a keyword
   ability, a triggered ability, a static effect, ...). Name it.
3. Extend `grammar.pest` to recognize the pattern.
4. Extend `ast.rs` with whatever new node(s) the grammar needs.
5. Extend `lower.rs` so every new AST node has a lowering.
6. Run `cargo xtask test --tier 2` until green.
7. Run `cargo xtask corpus` and confirm zero new regressions.
8. Stop. The orchestrator commits.";

const CONSTRAINTS_BLOCK: &str = "\
## Constraints

1. **Do not modify the unparser** to make round-trip pass. A round-trip
   failure after your grammar change is a signal that the AST design is
   wrong or there's new ambiguity. Think about it; don't paper over it.
2. **Do not add a special-case rule for this one card.** If the pattern
   looks specific to one card, ask yourself what *general* pattern it's
   an instance of and encode that.
3. **Do not touch existing grammar rules unless necessary.** Additive
   changes are safer than modifications. If you must modify, leave a
   one-line comment explaining why.
4. **Do not disable or modify existing tests.** Tests under `tests/unit.rs`
   and `tests/prop.rs` are the contract; the new generated test is what
   you're making pass.
5. **Stay within scope.** Modify only `grammar.pest`, `ast.rs`,
   `lower.rs`, and the generated test file if (and only if) you need to
   fix a generator bug — do not touch `mtg-scryfall`, `mtg-corpus`, or
   `xtask`.
6. **If you can't solve it, say so.** Better to surface \"I don't see
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

fn render_files_block(grammar: &str, ast: &str, lower: &str) -> String {
    format!(
        "## Current files\n\n\
         ### crates/mtg-grammar/src/grammar.pest\n\n\
         ```\n{grammar}\n```\n\n\
         ### crates/mtg-grammar/src/ast.rs\n\n\
         ```rust\n{ast}\n```\n\n\
         ### crates/mtg-semantic/src/lower.rs\n\n\
         ```rust\n{lower}\n```"
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
                .arg("--json")
                .arg("--cd")
                .arg(repo_root())
                .arg("--sandbox")
                .arg("workspace-write")
                .arg("--ask-for-approval")
                .arg("never")
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

// ---------------------------------------------------------------------------
// git
// ---------------------------------------------------------------------------

fn git_commit(message: &str) -> Result<()> {
    let add = Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root())
        .status()?;
    if !add.success() {
        bail!("git add failed");
    }
    let commit = Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(repo_root())
        .status()?;
    if !commit.success() {
        bail!("git commit failed");
    }
    Ok(())
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
