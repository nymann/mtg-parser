//! Refactor-oriented workflow. Unlike `grammar-fix`, this flow is not
//! trying to make the next card pass. It prepares a bounded,
//! qmd-grounded refactor prompt whose success criteria are unchanged
//! behavior and lower future edit cost.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};

use crate::paths::{refactor_hotspot_log_root, repo_root};

const DEFAULT_THEME: Theme = Theme::ParserBoilerplate;
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

pub fn run(args: &[String]) -> ExitCode {
    match Options::parse(args).and_then(run_inner) {
        Ok(path) => {
            println!("{}", path.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("refactor-hotspot: {e:#}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Clone)]
struct Options {
    theme: Theme,
    target: Option<String>,
    out: Option<PathBuf>,
    print: bool,
    churn_window: usize,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self> {
        let mut theme = DEFAULT_THEME;
        let mut target = None::<String>;
        let mut out = None::<PathBuf>;
        let mut print = false;
        let mut churn_window = DEFAULT_CHURN_WINDOW;

        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--theme" => {
                    let value = iter
                        .next()
                        .ok_or_else(|| anyhow!("--theme requires a value"))?;
                    theme = Theme::parse(value)?;
                }
                s if s.starts_with("--theme=") => {
                    theme = Theme::parse(&s["--theme=".len()..])?;
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
                "-h" | "--help" => bail!("{}", HELP),
                other => bail!("unknown argument: {other}\n\n{HELP}"),
            }
        }

        Ok(Self {
            theme,
            target,
            out,
            print,
            churn_window,
        })
    }
}

const HELP: &str = "\
cargo xtask refactor-hotspot [--theme THEME] [--target PATH] [--out PATH] [--print]

Themes:
  parser-boilerplate   Parser helper/extraction cleanup without AST changes.
  damage               Damage phenomenon factoring.
  destroy              Destroy/tap/sacrifice/attach keyword-action factoring.
  prevention           Prevention and replacement-effect factoring.
  keyword-abilities    Keyword ability shape factoring.
  triggered-abilities  Trigger and intervening-if factoring.
  unparse-templates    Unparser template/slot extraction.
";

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

fn run_inner(opts: Options) -> Result<PathBuf> {
    let prompt = build_prompt(&opts)?;
    if opts.print {
        print!("{prompt}");
    }
    let out = match opts.out {
        Some(path) => path,
        None => {
            let dir = create_log_dir(opts.theme)?;
            dir.join("prompt.md")
        }
    };
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(&out, prompt).with_context(|| format!("write {}", out.display()))?;
    Ok(out)
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

fn build_prompt(opts: &Options) -> Result<String> {
    let target_files = target_files(opts);
    let rules = crate::rules_context::render_rules_block(opts.theme.qmd_query());
    let stats = render_hotspot_stats(opts.churn_window)?;
    let snippets = render_code_snippets(&target_files)?;

    Ok(format!(
        "\
# Refactor Hotspot: {theme}

You are working in `mtg-parser`. This is a refactoring task, not a card-coverage task.

## Goal

Reduce future edit cost in the selected hotspot while preserving parser behavior.

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
        theme = opts.theme.label(),
        theme_instructions = opts.theme.instructions(),
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

fn target_files(opts: &Options) -> Vec<String> {
    if let Some(target) = &opts.target {
        return vec![target.clone()];
    }
    opts.theme
        .default_files()
        .iter()
        .map(|s| (*s).to_string())
        .collect()
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
