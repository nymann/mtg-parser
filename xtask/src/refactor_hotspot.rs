//! Refactor-oriented workflow. Unlike `add-card`, this flow is not
//! trying to make the next card pass. It runs a bounded, qmd-grounded
//! agent refactor whose success criteria are unchanged behavior and
//! lower future edit cost.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};

use crate::agent_events;
use crate::flow::AgentProvider;
use crate::paths::{refactor_hotspot_log_root, repo_root};

const DEFAULT_CHURN_WINDOW: usize = 200;

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

const AUTO_HOTSPOTS: &[(&str, Theme)] = &[
    ("crates/mtg-grammar/src/parse.rs", Theme::ParserBoilerplate),
    ("crates/mtg-grammar/src/unparse.rs", Theme::UnparseTemplates),
    ("crates/mtg-grammar/src/grammar.pest", Theme::Damage),
    ("crates/mtg-grammar/src/ast.rs", Theme::Damage),
];

pub fn run(args: &[String]) -> ExitCode {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    match Options::parse(args).and_then(run_inner) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("refactor-hotspot: {e:#}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Clone)]
struct Options {
    theme: Option<Theme>,
    target: Option<String>,
    out: Option<PathBuf>,
    print: bool,
    dry_run: bool,
    allow_dirty: bool,
    agent: AgentProvider,
    churn_window: usize,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self> {
        let mut theme = None::<Theme>;
        let mut target = None::<String>;
        let mut out = None::<PathBuf>;
        let mut print = false;
        let mut dry_run = false;
        let mut allow_dirty = false;
        let mut agent = AgentProvider::Codex;
        let mut churn_window = DEFAULT_CHURN_WINDOW;

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
                other => bail!("unknown argument: {other}\n\n{HELP}"),
            }
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
        })
    }
}

const HELP: &str = "\
cargo xtask refactor-hotspot [--theme THEME] [--target PATH] [--agent codex|claude]
                             [--dry-run] [--allow-dirty] [--out PATH] [--print]

Autonomous by default: ranks source hotspots by churn × LOC, builds a
qmd-grounded prompt for the top candidate, invokes the agent, runs `cargo test`,
`cargo xtask corpus`, `just audit-page`, and commits the result. Use --theme or
--target to override auto-selection. Use --dry-run or --print to stop after
writing/printing the prompt.

Themes:
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

fn run_inner(opts: Options) -> Result<()> {
    if !opts.dry_run && !opts.print && !opts.allow_dirty {
        ensure_clean_working_tree()
            .context("working tree must be clean (or pass --allow-dirty)")?;
    }

    let selected = resolve_selection(&opts)?;
    println!(
        "selected: {} ({})",
        selected.files.join(", "),
        selected.reason
    );

    let prompt = build_prompt(&opts, &selected)?;
    if opts.print {
        print!("{prompt}");
    }
    let out = match opts.out.clone() {
        Some(path) => path,
        None => {
            let dir = create_log_dir(selected.theme)?;
            dir.join("prompt.md")
        }
    };
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(&out, &prompt).with_context(|| format!("write {}", out.display()))?;
    println!("prompt: {}", out.display());

    if opts.dry_run || opts.print {
        println!(
            "dry-run: not invoking {}, not running gates, not committing",
            opts.agent.label()
        );
        return Ok(());
    }

    let log_dir = out
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(refactor_hotspot_log_root);
    let transcript = log_dir.join("transcript.ndjson");
    let response = log_dir.join("response.md");

    println!("agent: {}", opts.agent.label());
    let outcome = invoke_agent(opts.agent, &prompt, &transcript)?;
    std::fs::write(&response, &outcome.assistant_text)
        .with_context(|| format!("write {}", response.display()))?;
    if !outcome.success {
        bail!(
            "{} exited with status {}; transcript: {}",
            opts.agent.label(),
            outcome.exit_code,
            transcript.display()
        );
    }

    run_gate("cargo fmt --all", "cargo", &["fmt", "--all"])?;
    run_gate("cargo test", "cargo", &["test"])?;
    run_gate("cargo xtask corpus", "cargo", &["xtask", "corpus"])?;
    run_gate("just audit-page", "just", &["audit-page"])?;

    match git_commit(&commit_message(&selected)?)? {
        CommitOutcome::Committed => println!("committed refactor-hotspot result"),
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

    auto_select_hotspot(opts.churn_window)
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

fn auto_select_hotspot(churn_window: usize) -> Result<SelectedRefactor> {
    let root = repo_root();
    let mut best = None::<(String, Theme, usize, usize, usize)>;
    for (path, theme) in AUTO_HOTSPOTS {
        let churn = git_churn(path, churn_window)?;
        let loc = line_count(&root.join(path)).unwrap_or(0);
        let score = churn.saturating_mul(loc);
        match &best {
            Some((_, _, _, _, best_score)) if score <= *best_score => {}
            _ => best = Some(((*path).to_string(), *theme, churn, loc, score)),
        }
    }

    let Some((path, theme, churn, loc, score)) = best else {
        bail!("no auto refactor hotspots configured");
    };
    Ok(SelectedRefactor {
        theme,
        files: vec![path],
        reason: format!(
            "auto-selected by churn × LOC: {churn} × {loc} = {score}; theme={}",
            theme.label()
        ),
    })
}

fn create_log_dir(theme: Theme) -> Result<PathBuf> {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before unix epoch")?
        .as_secs();
    let dir = refactor_hotspot_log_root().join(format!("{since_epoch}-{}", theme.label()));
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    Ok(dir)
}

fn build_prompt(opts: &Options, selected: &SelectedRefactor) -> Result<String> {
    let target_files = &selected.files;
    let rules = crate::rules_context::render_rules_block(selected.theme.qmd_query());
    let stats = render_hotspot_stats(opts.churn_window)?;
    let snippets = render_code_snippets(&target_files)?;

    Ok(format!(
        "\
# Refactor Hotspot: {theme}

You are working in `mtg-parser`. This is a refactoring task, not a card-coverage task.

## Goal

Reduce future edit cost in the selected hotspot while preserving parser behavior.

## Selection

{selection_reason}

## Theme Guidance

{theme_instructions}

## Required Constraints

1. Preserve corpus behavior unless the refactor naturally fixes existing failures.
2. Do not add one-card special cases.
3. Prefer phenomenon-shaped rules and AST nodes over sentence-shaped variants.
4. Use the Comprehensive Rules context for vocabulary and abstraction names.
5. Keep the patch narrow. Touch only files justified by this theme.
6. Run `cargo test`.
7. Run `cargo xtask corpus`.
8. Run `just audit-page` and inspect LOC/churn movement.

## Selected Files

{files}

{rules}
## Hotspot Stats

Churn is touches in the last {churn_window} commits. LOC is current working tree line count.

```text
{stats}```

## Code Context

{snippets}

## Deliverable

Make one coherent refactor commit. In the commit message, include:

- intent
- corpus result
- primary LOC delta
- whether behavior changed
",
        theme = selected.theme.label(),
        theme_instructions = selected.theme.instructions(),
        selection_reason = selected.reason,
        files = target_files
            .iter()
            .map(|p| format!("- `{p}`"))
            .collect::<Vec<_>>()
            .join("\n"),
        rules = rules,
        churn_window = opts.churn_window,
        stats = stats,
        snippets = snippets,
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
) -> Result<AgentOutcome> {
    let command = base_agent_command(provider);
    invoke_jsonl_agent(provider, command, prompt, transcript_path)
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
                for event in agent_events::parse(provider, &parsed) {
                    println!("    [+{elapsed_secs:>3}s] {}", render_agent_event(&event));
                }
            }
            Err(_) => println!(
                "    non-JSON line from {}: {}",
                provider.label(),
                trim(&line, 200)
            ),
        }
    }

    let status = child.wait()?;
    Ok(AgentOutcome {
        success: status.success(),
        exit_code: status.code().unwrap_or(-1),
        assistant_text: assistant_text.join("\n\n"),
    })
}

fn render_agent_event(event: &agent_events::ParsedAgentEvent) -> String {
    use agent_events::{ParsedAgentEvent, ToolUseTarget};

    match event {
        ParsedAgentEvent::Init { model } => format!("init model={model}"),
        ParsedAgentEvent::AssistantText { text } => trim(text, 160),
        ParsedAgentEvent::ToolUse { name, target } => match target {
            ToolUseTarget::File(path) => format!("{name} {path}"),
            ToolUseTarget::Command(cmd) => format!("{name} {cmd}"),
            ToolUseTarget::Pattern(pattern) => format!("{name} {pattern}"),
            ToolUseTarget::Description(desc) => format!("{name} {}", trim(desc, 120)),
            ToolUseTarget::None => name.clone(),
        },
        ParsedAgentEvent::ToolResult {
            first_line,
            is_error,
        } => {
            if *is_error {
                format!("tool error: {}", trim(first_line, 160))
            } else {
                trim(first_line, 160)
            }
        }
        ParsedAgentEvent::Done {
            subtype,
            num_turns,
            total_cost_usd,
        } => format!("done {subtype}; turns={num_turns}; cost=${total_cost_usd:.4}"),
        ParsedAgentEvent::Other => "event".to_string(),
    }
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
    println!("gate: {label}");
    let status = Command::new(program)
        .args(args)
        .current_dir(repo_root())
        .status()
        .with_context(|| format!("run {label}"))?;
    if !status.success() {
        bail!("{label} failed with {status}");
    }
    Ok(())
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

fn commit_message(selected: &SelectedRefactor) -> Result<String> {
    let stat = git_diff_stat()?;
    Ok(format!(
        "Refactor {} hotspot\n\nGates: cargo test; cargo xtask corpus; just audit-page.\nBehavior: intended unchanged.\nPrimary LOC delta:\n{}",
        selected.theme.label(),
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
        assert_eq!(Theme::parse("parser").unwrap(), Theme::ParserBoilerplate);
        assert_eq!(Theme::parse("damage").unwrap(), Theme::Damage);
        assert_eq!(Theme::parse("keywords").unwrap(), Theme::KeywordAbilities);
    }

    #[test]
    fn rejects_unknown_theme() {
        assert!(Theme::parse("espresso").is_err());
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
