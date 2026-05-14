//! The grammar-fix orchestrator. Walks a Scryfall set, hands one
//! failing card at a time to a fresh `claude -p` agent, gates the
//! result through tier-1/2 tests and the corpus regression check, and
//! commits per-iteration progress.
//!
//! Inspired by argentum-press/scripts/fix_parser_gaps.py — narrower in
//! scope (single playbook, single pre-computed context block) but the
//! same deterministic-around-claude shape: every step except step 5 is
//! deterministic and the orchestrator owns the guardrails.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};

use mtg_corpus::{find_next_failing_card, NextCard};
use mtg_scryfall::{Card, ScryfallClient};

use crate::paths::{
    ast_rs_path, corpus_status_path, generated_tests_dir, generated_tests_manifest,
    grammar_fix_log_root, grammar_pest_path, lower_rs_path, repo_root,
};

const DEFAULT_SET: &str = "lea";
const DEFAULT_MAX_ITERATIONS: u32 = 1;

pub fn run(args: &[String]) -> ExitCode {
    let opts = match Options::parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    match run_inner(opts) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Clone)]
struct Options {
    set: String,
    max_iterations: u32,
    dry_run: bool,
    allow_dirty: bool,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self> {
        let mut set = None::<String>;
        let mut max_iterations = DEFAULT_MAX_ITERATIONS;
        let mut dry_run = false;
        let mut allow_dirty = false;

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
                other => bail!("unknown argument: {other}"),
            }
        }
        Ok(Self {
            set: set.unwrap_or_else(|| DEFAULT_SET.to_string()),
            max_iterations,
            dry_run,
            allow_dirty,
        })
    }
}

fn run_inner(opts: Options) -> Result<ExitCode> {
    if !opts.dry_run && !opts.allow_dirty {
        ensure_clean_working_tree()
            .context("working tree must be clean (or pass --allow-dirty)")?;
    }
    let client = ScryfallClient::new()?;

    let mut iter = 0u32;
    while iter < opts.max_iterations {
        println!();
        println!(
            "== grammar-fix iteration {} / {} ==",
            iter + 1,
            opts.max_iterations
        );
        match run_one_iteration(&client, &opts)? {
            IterationOutcome::AllPass => {
                println!("All cards in {} pass; nothing to do.", opts.set);
                return Ok(ExitCode::SUCCESS);
            }
            IterationOutcome::DryRunStop => return Ok(ExitCode::SUCCESS),
            IterationOutcome::Committed => {
                iter += 1;
            }
            IterationOutcome::SurfaceToHuman(reason) => {
                eprintln!();
                eprintln!("STOP: {reason}");
                eprintln!("Working tree left as-is; inspect .grammar-fix/<latest>/ for context.");
                return Ok(ExitCode::FAILURE);
            }
        }
    }
    println!();
    println!("Reached --max-iterations={}.", opts.max_iterations);
    Ok(ExitCode::SUCCESS)
}

enum IterationOutcome {
    AllPass,
    DryRunStop,
    Committed,
    SurfaceToHuman(String),
}

fn run_one_iteration(client: &ScryfallClient, opts: &Options) -> Result<IterationOutcome> {
    // 1. Find the next failing card.
    let (card, error, normalized) = match find_next_failing_card(client, &opts.set)? {
        NextCard::AllPass => return Ok(IterationOutcome::AllPass),
        NextCard::Failing {
            card,
            reason,
            normalized,
        } => (card, reason, normalized),
    };

    let log_dir = create_log_dir(&card)?;
    println!("Card    : {} ({})", card.name, card.set_code);
    println!("Log dir : {}", log_dir.display());

    // The promoted test path is deterministic from the card slug. We
    // compute it now so the prompt can reference it, but the file
    // itself is written only when we commit to actually running the
    // claude step — dry-run must leave the working tree untouched.
    let test_path = generated_test_path(&card);

    // Snapshot card + baseline corpus status into the log dir.
    let card_json = serde_json::to_string_pretty(&card)?;
    std::fs::write(log_dir.join("card.json"), card_json)?;

    // Build prompt and dump it.
    let prompt = build_prompt(&card, &error, &normalized, &test_path)?;
    std::fs::write(log_dir.join("prompt.md"), &prompt)?;
    println!("Prompt  : {}", log_dir.join("prompt.md").display());
    println!("Prompt size: {} bytes", prompt.len());

    if opts.dry_run {
        println!();
        println!("--dry-run: not invoking claude, not promoting the test, not committing.");
        return Ok(IterationOutcome::DryRunStop);
    }

    // From here on we mutate the working tree.
    let baseline_pass_count = read_corpus_passing(&corpus_status_path()).unwrap_or(0);
    write_promoted_test(&test_path, &card, &normalized).context("generate promoted test file")?;
    register_generated_test(&test_path).context("register generated test")?;

    // 5. Delegate to claude -p. This is the one non-deterministic step.
    let transcript_path = log_dir.join("transcript.txt");
    let claude_outcome = invoke_claude(&prompt, &transcript_path)?;
    std::fs::write(log_dir.join("response.md"), &claude_outcome.tail)?;
    if !claude_outcome.success {
        return Ok(IterationOutcome::SurfaceToHuman(format!(
            "claude -p exited with status {}",
            claude_outcome.exit_code,
        )));
    }

    // 6. Verify with tier 1 + tier 2.
    if !run_xtask(&["test", "--tier", "2"])? {
        return Ok(IterationOutcome::SurfaceToHuman(
            "cargo xtask test --tier 2 failed after the agent's pass".into(),
        ));
    }

    // 7. Corpus regression gate. `cargo xtask corpus` is the source of
    //    truth — it writes corpus_status.json on success and exits
    //    non-zero on any new failure.
    if !run_xtask(&["corpus"])? {
        return Ok(IterationOutcome::SurfaceToHuman(
            "cargo xtask corpus reported a regression".into(),
        ));
    }

    // 8. Commit.
    let new_pass_count = read_corpus_passing(&corpus_status_path()).unwrap_or(baseline_pass_count);
    let total = read_corpus_total(&corpus_status_path()).unwrap_or(0);
    let new_passes = new_pass_count.saturating_sub(baseline_pass_count);
    let commit_msg = commit_message(&card, new_passes, new_pass_count, total);
    std::fs::write(log_dir.join("commit_message.txt"), &commit_msg)?;
    git_commit(&commit_msg).context("git commit")?;

    // Log the diff against the parent for replay.
    if let Ok(diff) = git_diff_against_head_parent() {
        std::fs::write(log_dir.join("diff.patch"), diff).ok();
    }

    println!();
    println!("Committed. New passes: {new_passes}. Status: {new_pass_count}/{total}.");
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

fn write_promoted_test(path: &Path, card: &Card, normalized: &str) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| anyhow!("test path has no parent"))?;
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let body = render_promoted_test(card, normalized);
    std::fs::write(path, body).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn render_promoted_test(card: &Card, normalized: &str) -> String {
    // Same shape as next-card's generator, but without #[ignore] — the
    // orchestrator wants this test to run as part of `cargo xtask test
    // --tier 2`.
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
// claude subprocess
// ---------------------------------------------------------------------------

struct ClaudeOutcome {
    success: bool,
    exit_code: i32,
    /// Tail of claude's stdout. Used as a best-effort summary for the
    /// per-iteration response.md log.
    tail: String,
}

fn invoke_claude(prompt: &str, transcript_path: &Path) -> Result<ClaudeOutcome> {
    println!();
    println!("Invoking `claude -p` (this can take a while)…");
    let mut child = Command::new("claude")
        .arg("-p")
        .arg("--dangerously-skip-permissions")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .current_dir(repo_root())
        .spawn()
        .context(
            "spawn `claude`. Is the Claude Code CLI installed and on PATH? \
             See https://docs.claude.com/claude-code for setup.",
        )?;

    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        stdin.write_all(prompt.as_bytes())?;
    }

    let stdout = child.stdout.take().expect("piped stdout");
    let reader = BufReader::new(stdout);
    let mut transcript = std::fs::File::create(transcript_path)
        .with_context(|| format!("create {}", transcript_path.display()))?;
    let mut tail_lines: Vec<String> = Vec::new();
    const TAIL_KEEP: usize = 80;
    for line in reader.lines() {
        let line = line?;
        writeln!(transcript, "{line}")?;
        println!("[claude] {line}");
        tail_lines.push(line);
        if tail_lines.len() > TAIL_KEEP {
            tail_lines.remove(0);
        }
    }
    let status = child.wait()?;
    let exit_code = status.code().unwrap_or(-1);
    Ok(ClaudeOutcome {
        success: status.success(),
        exit_code,
        tail: tail_lines.join("\n"),
    })
}

fn run_xtask(args: &[&str]) -> Result<bool> {
    println!();
    println!("$ cargo xtask {}", args.join(" "));
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
    use mtg_scryfall::Layout;

    fn sample_card() -> Card {
        Card {
            name: "Air Elemental".into(),
            set_code: "lea".into(),
            collector_number: "46".into(),
            oracle_text: "Flying".into(),
            mana_cost: "{3}{U}{U}".into(),
            layout: Layout::Normal,
        }
    }

    #[test]
    fn options_default_to_lea_one_iteration() {
        let o = Options::parse(&[]).unwrap();
        assert_eq!(o.set, "lea");
        assert_eq!(o.max_iterations, 1);
        assert!(!o.dry_run);
        assert!(!o.allow_dirty);
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
    fn slugify_matches_next_card() {
        // Stays consistent with next-card's slugify so log dirs and
        // generated test filenames agree.
        assert_eq!(slugify("Lightning Bolt"), "lightning_bolt");
        assert_eq!(slugify("Sol'kanar the Tainted"), "sol_kanar_the_tainted");
    }

    #[test]
    fn render_promoted_test_omits_ignore_attribute() {
        let body = render_promoted_test(&sample_card(), "Flying");
        assert!(
            !body.contains("#[ignore"),
            "promoted test must not be #[ignore]'d:\n{body}"
        );
        assert!(body.contains("fn round_trip()"));
        assert!(body.contains("\"Flying\""));
    }

    #[test]
    fn commit_message_has_required_lines() {
        let m = commit_message(&sample_card(), 1, 1, 290);
        assert!(m.starts_with("grammar: support card Air Elemental\n"));
        assert!(m.contains("Card: Air Elemental (lea)"));
        assert!(m.contains("New passes: 1"));
        assert!(m.contains("Status: 1/290"));
    }

    #[test]
    fn render_card_block_inlines_oracle_text() {
        let block = render_card_block(&sample_card(), "Flying");
        assert!(block.contains("Air Elemental"));
        assert!(block.contains("Flying"));
        assert!(block.contains("Normal"));
    }
}
