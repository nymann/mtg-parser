use std::collections::{BTreeMap, BTreeSet};
use std::process::{Command, ExitCode};

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;

use crate::paths::{grammar_pest_path, repo_root};

const HELP: &str = "\
cargo xtask grammar-audit --diff <range> --oracle-text <text> [--json]

Audits grammar.pest additions for sentence-shaped rule drift. The audit is
report-only: findings are calibrated signals for add-card logs and humans.
";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warn,
    BlockCandidate,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditReport {
    pub oracle_text: String,
    pub new_rule_count: usize,
    pub new_rules: Vec<NewRuleAudit>,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NewRuleAudit {
    pub name: String,
    pub segment_count: usize,
    pub oracle_overlap_words: Vec<String>,
    pub best_neighbours: Vec<RuleNeighbour>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleNeighbour {
    pub name: String,
    pub similarity: f32,
    pub rhs_snippet: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub kind: String,
    pub rule: Option<String>,
    pub message: String,
    pub neighbours: Vec<RuleNeighbour>,
}

#[derive(Debug, Clone)]
struct GrammarRule {
    rhs: String,
}

pub fn run(args: &[String]) -> ExitCode {
    match run_inner(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("grammar-audit: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run_inner(args: &[String]) -> Result<()> {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{HELP}");
        return Ok(());
    }

    let mut diff_range = None::<String>;
    let mut oracle_text = None::<String>;
    let mut json = false;

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--diff" => {
                diff_range = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--diff requires a value"))?
                        .to_string(),
                );
            }
            s if s.starts_with("--diff=") => {
                diff_range = Some(s["--diff=".len()..].to_string());
            }
            "--oracle-text" => {
                oracle_text = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--oracle-text requires a value"))?
                        .to_string(),
                );
            }
            s if s.starts_with("--oracle-text=") => {
                oracle_text = Some(s["--oracle-text=".len()..].to_string());
            }
            "--json" => json = true,
            other => bail!("unknown argument: {other}\n\n{HELP}"),
        }
    }

    let diff_range = diff_range.ok_or_else(|| anyhow!("--diff is required"))?;
    let oracle_text = oracle_text.ok_or_else(|| anyhow!("--oracle-text is required"))?;
    let diff = git_grammar_diff(&diff_range)?;
    let grammar = std::fs::read_to_string(grammar_pest_path()).context("read grammar.pest")?;
    let report = audit_grammar_diff(&diff, &grammar, &oracle_text);

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", report_markdown(&report));
    }

    Ok(())
}

pub fn audit_worktree(oracle_text: &str) -> Result<AuditReport> {
    let diff = git_grammar_worktree_diff()?;
    let grammar = std::fs::read_to_string(grammar_pest_path()).context("read grammar.pest")?;
    Ok(audit_grammar_diff(&diff, &grammar, oracle_text))
}

pub fn audit_grammar_diff(diff: &str, current_grammar: &str, oracle_text: &str) -> AuditReport {
    let mut new_rule_names: Vec<String> = new_rule_names_from_diff(diff)
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    new_rule_names.sort();

    let rules = parse_grammar_rules(current_grammar);
    let oracle_words = meaningful_words(oracle_text);
    let existing_rules: BTreeMap<_, _> = rules
        .iter()
        .filter(|(name, _)| !new_rule_names.contains(name))
        .map(|(name, rule)| (name.clone(), rule.clone()))
        .collect();

    let mut findings = Vec::new();
    if !new_rule_names.is_empty() {
        findings.push(Finding {
            severity: Severity::Info,
            kind: "new_rule_count".into(),
            rule: None,
            message: format!("{} new pest rule(s) introduced", new_rule_names.len()),
            neighbours: Vec::new(),
        });
    }

    let mut new_rules = Vec::new();
    for name in new_rule_names {
        let segments = rule_name_segments(&name);
        let segment_count = segments.len();
        let overlap = overlap_words(&segments, &oracle_words);
        let neighbours = rules
            .get(&name)
            .map(|rule| best_neighbours(rule, &existing_rules))
            .unwrap_or_default();

        if overlap.len() >= 4 && segment_count >= 8 {
            findings.push(Finding {
                severity: Severity::BlockCandidate,
                kind: "oracle_word_overlap".into(),
                rule: Some(name.clone()),
                message: format!(
                    "rule name copies {} meaningful Oracle word(s): {}",
                    overlap.len(),
                    overlap.join(", ")
                ),
                neighbours: Vec::new(),
            });
        } else if overlap.len() >= 3 {
            findings.push(Finding {
                severity: Severity::Warn,
                kind: "oracle_word_overlap".into(),
                rule: Some(name.clone()),
                message: format!("rule name overlaps Oracle wording: {}", overlap.join(", ")),
                neighbours: Vec::new(),
            });
        }

        if segment_count >= 10 {
            findings.push(Finding {
                severity: Severity::Warn,
                kind: "long_rule_name".into(),
                rule: Some(name.clone()),
                message: format!("rule name has {segment_count} snake_case segments"),
                neighbours: Vec::new(),
            });
        }

        if let Some(best) = neighbours.first() {
            if rules
                .get(&name)
                .is_some_and(|rule| is_quantity_like_rule(rule))
                && best.similarity >= 0.86
            {
                findings.push(Finding {
                    severity: Severity::BlockCandidate,
                    kind: "rhs_quantity_duplication".into(),
                    rule: Some(name.clone()),
                    message: format!(
                        "quantity-like RHS duplicates existing rule `{}`; consider reusing or extracting a shared amount/quantity rule instead of adding a parallel rule",
                        best.name
                    ),
                    neighbours: neighbours.clone(),
                });
            } else if best.similarity >= 0.86 {
                findings.push(Finding {
                    severity: Severity::BlockCandidate,
                    kind: "rhs_skeleton_similarity".into(),
                    rule: Some(name.clone()),
                    message: format!(
                        "RHS skeleton is {:.0}% similar to existing rule `{}`",
                        best.similarity * 100.0,
                        best.name
                    ),
                    neighbours: neighbours.clone(),
                });
            } else if best.similarity >= 0.72 {
                findings.push(Finding {
                    severity: Severity::Warn,
                    kind: "rhs_skeleton_similarity".into(),
                    rule: Some(name.clone()),
                    message: format!(
                        "RHS skeleton has a nearby existing rule `{}` ({:.0}% similar)",
                        best.name,
                        best.similarity * 100.0
                    ),
                    neighbours: neighbours.clone(),
                });
            }
        }

        new_rules.push(NewRuleAudit {
            name,
            segment_count,
            oracle_overlap_words: overlap,
            best_neighbours: neighbours,
        });
    }

    AuditReport {
        oracle_text: oracle_text.to_string(),
        new_rule_count: new_rules.len(),
        new_rules,
        findings,
    }
}

pub fn report_markdown(report: &AuditReport) -> String {
    let mut out = String::new();
    out.push_str("# Grammar Audit\n\n");
    out.push_str(&format!("New pest rules: {}\n\n", report.new_rule_count));
    if report.new_rules.is_empty() {
        out.push_str("No new pest rules detected in grammar.pest diff.\n");
        return out;
    }

    out.push_str("## Findings\n\n");
    if report.findings.is_empty() {
        out.push_str("- none\n");
    } else {
        for finding in &report.findings {
            let rule = finding
                .rule
                .as_ref()
                .map(|r| format!(" `{r}`"))
                .unwrap_or_default();
            out.push_str(&format!(
                "- {:?} `{}`{}: {}\n",
                finding.severity, finding.kind, rule, finding.message
            ));
            for neighbour in finding.neighbours.iter().take(3) {
                out.push_str(&format!(
                    "  - neighbour `{}` ({:.0}%): `{}`\n",
                    neighbour.name,
                    neighbour.similarity * 100.0,
                    neighbour.rhs_snippet
                ));
            }
        }
    }

    out.push_str("\n## New Rules\n\n");
    for rule in &report.new_rules {
        out.push_str(&format!(
            "- `{}`: {} segments",
            rule.name, rule.segment_count
        ));
        if !rule.oracle_overlap_words.is_empty() {
            out.push_str(&format!(
                "; Oracle overlap: {}",
                rule.oracle_overlap_words.join(", ")
            ));
        }
        out.push('\n');
        for neighbour in rule.best_neighbours.iter().take(3) {
            out.push_str(&format!(
                "  - neighbour `{}` ({:.0}%): `{}`\n",
                neighbour.name,
                neighbour.similarity * 100.0,
                neighbour.rhs_snippet
            ));
        }
    }
    out
}

fn git_grammar_diff(range: &str) -> Result<String> {
    let out = Command::new("git")
        .args(["diff", range, "--", "crates/mtg-grammar/src/grammar.pest"])
        .current_dir(repo_root())
        .output()
        .with_context(|| format!("git diff {range} -- grammar.pest"))?;
    if !out.status.success() {
        bail!("git diff {range} failed");
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn git_grammar_worktree_diff() -> Result<String> {
    let out = Command::new("git")
        .args(["diff", "--", "crates/mtg-grammar/src/grammar.pest"])
        .current_dir(repo_root())
        .output()
        .context("git diff -- grammar.pest")?;
    if !out.status.success() {
        bail!("git diff -- grammar.pest failed");
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn new_rule_names_from_diff(diff: &str) -> Vec<String> {
    diff.lines()
        .filter_map(|line| {
            let added = line.strip_prefix('+')?;
            if line.starts_with("+++") {
                return None;
            }
            rule_name_from_declaration(added)
        })
        .collect()
}

fn parse_grammar_rules(grammar: &str) -> BTreeMap<String, GrammarRule> {
    let mut out = BTreeMap::new();
    let lines: Vec<&str> = grammar.lines().collect();
    let mut i = 0usize;
    while i < lines.len() {
        let Some(name) = rule_name_from_declaration(lines[i]) else {
            i += 1;
            continue;
        };

        let mut rhs = String::new();
        let mut depth = brace_delta(lines[i]);
        let first_rhs = lines[i].split_once('=').map(|(_, r)| r).unwrap_or_default();
        rhs.push_str(first_rhs.trim());
        i += 1;
        while i < lines.len() && depth > 0 {
            rhs.push('\n');
            rhs.push_str(lines[i].trim());
            depth += brace_delta(lines[i]);
            i += 1;
        }
        out.insert(name, GrammarRule { rhs });
    }
    out
}

fn rule_name_from_declaration(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return None;
    }
    let first = trimmed.chars().next().unwrap_or(' ');
    if !(first.is_ascii_lowercase() || first == '_') {
        return None;
    }
    let eq_pos = trimmed.find('=')?;
    let head = trimmed[..eq_pos].trim();
    if head.is_empty() {
        return None;
    }
    if head
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        Some(head.to_string())
    } else {
        None
    }
}

fn brace_delta(line: &str) -> isize {
    let mut delta = 0isize;
    let mut in_quote = false;
    let mut escaped = false;
    for ch in line.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_quote => escaped = true,
            '"' => in_quote = !in_quote,
            '{' if !in_quote => delta += 1,
            '}' if !in_quote => delta -= 1,
            _ => {}
        }
    }
    delta
}

fn meaningful_words(text: &str) -> BTreeSet<String> {
    words(text)
        .into_iter()
        .filter(|w| !is_stop_word(w))
        .collect()
}

fn rule_name_segments(name: &str) -> Vec<String> {
    name.split('_')
        .map(str::to_ascii_lowercase)
        .filter(|w| !w.is_empty() && !is_stop_word(w))
        .collect()
}

fn overlap_words(segments: &[String], oracle_words: &BTreeSet<String>) -> Vec<String> {
    let mut out: Vec<String> = segments
        .iter()
        .filter(|s| oracle_words.contains(*s))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    out.sort();
    out
}

fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn is_stop_word(word: &str) -> bool {
    word.len() <= 2
        || matches!(
            word,
            "a" | "an"
                | "and"
                | "are"
                | "as"
                | "at"
                | "be"
                | "by"
                | "can"
                | "cant"
                | "cannot"
                | "do"
                | "does"
                | "during"
                | "each"
                | "for"
                | "from"
                | "had"
                | "has"
                | "have"
                | "if"
                | "in"
                | "into"
                | "is"
                | "it"
                | "its"
                | "may"
                | "of"
                | "on"
                | "or"
                | "that"
                | "the"
                | "their"
                | "this"
                | "to"
                | "unless"
                | "until"
                | "was"
                | "when"
                | "while"
                | "with"
                | "you"
                | "your"
        )
}

fn best_neighbours(
    rule: &GrammarRule,
    existing: &BTreeMap<String, GrammarRule>,
) -> Vec<RuleNeighbour> {
    let lhs_atoms = rhs_atoms(&rule.rhs);
    let mut neighbours: Vec<_> = existing
        .iter()
        .filter_map(|(name, other)| {
            let rhs = rhs_atoms(&other.rhs);
            if shared_identifier_count(&lhs_atoms, &rhs) < 2 {
                return None;
            }
            let similarity = skeleton_similarity(&lhs_atoms, &rhs);
            if similarity < 0.50 {
                return None;
            }
            Some(RuleNeighbour {
                name: name.clone(),
                similarity,
                rhs_snippet: snippet(&other.rhs),
            })
        })
        .collect();
    neighbours.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });
    neighbours.truncate(5);
    neighbours
}

fn rhs_atoms(rhs: &str) -> Vec<String> {
    let mut atoms = Vec::new();
    let mut current = String::new();
    let mut chars = rhs.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                if !current.is_empty() {
                    atoms.push(std::mem::take(&mut current).to_ascii_lowercase());
                }
                while let Some(q) = chars.next() {
                    if q == '\\' {
                        let _ = chars.next();
                    } else if q == '"' {
                        break;
                    }
                }
                atoms.push("lit".into());
            }
            c if c.is_ascii_alphanumeric() || c == '_' => current.push(c),
            c if matches!(c, '~' | '|' | '?' | '*' | '+' | '(' | ')') => {
                if !current.is_empty() {
                    atoms.push(format!(
                        "id:{}",
                        std::mem::take(&mut current).to_ascii_lowercase()
                    ));
                }
                atoms.push(c.to_string());
            }
            _ => {
                if !current.is_empty() {
                    atoms.push(format!(
                        "id:{}",
                        std::mem::take(&mut current).to_ascii_lowercase()
                    ));
                }
            }
        }
    }
    if !current.is_empty() {
        atoms.push(format!("id:{}", current.to_ascii_lowercase()));
    }
    atoms
}

fn shared_identifier_count(a: &[String], b: &[String]) -> usize {
    let a_ids: BTreeSet<&str> = a
        .iter()
        .filter_map(|atom| atom.strip_prefix("id:"))
        .collect();
    let b_ids: BTreeSet<&str> = b
        .iter()
        .filter_map(|atom| atom.strip_prefix("id:"))
        .collect();
    a_ids.intersection(&b_ids).count()
}

fn skeleton_similarity(a: &[String], b: &[String]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let lcs = lcs_len(a, b) as f32 / a.len().max(b.len()) as f32;
    let axis = one_span_similarity(a, b);
    lcs.max(axis)
}

fn is_quantity_like_rule(rule: &GrammarRule) -> bool {
    let atoms = rhs_atoms(&rule.rhs);
    let ids: BTreeSet<&str> = atoms
        .iter()
        .filter_map(|atom| atom.strip_prefix("id:"))
        .filter(|id| *id != "_")
        .collect();
    ids.contains("variable_name")
        && (ids.contains("number_word") || ids.contains("unsigned_number"))
        && ids.len() <= 2
}

fn lcs_len(a: &[String], b: &[String]) -> usize {
    let mut prev = vec![0usize; b.len() + 1];
    let mut curr = vec![0usize; b.len() + 1];
    for ai in a {
        for (j, bj) in b.iter().enumerate() {
            curr[j + 1] = if ai == bj {
                prev[j] + 1
            } else {
                curr[j].max(prev[j + 1])
            };
        }
        std::mem::swap(&mut prev, &mut curr);
        curr.fill(0);
    }
    prev[b.len()]
}

fn one_span_similarity(a: &[String], b: &[String]) -> f32 {
    let mut prefix = 0usize;
    while prefix < a.len() && prefix < b.len() && a[prefix] == b[prefix] {
        prefix += 1;
    }
    let mut suffix = 0usize;
    while suffix + prefix < a.len()
        && suffix + prefix < b.len()
        && a[a.len() - 1 - suffix] == b[b.len() - 1 - suffix]
    {
        suffix += 1;
    }
    (prefix + suffix) as f32 / a.len().max(b.len()) as f32
}

fn snippet(rhs: &str) -> String {
    let flat = rhs.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.len() <= 100 {
        flat
    } else {
        format!("{}...", &flat[..100])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_new_rule_names_from_diff() {
        let diff = "\
diff --git a/crates/mtg-grammar/src/grammar.pest b/crates/mtg-grammar/src/grammar.pest
@@
+static_source_cant_attack_unless_defending_player_controls_basic_land = {
+    ^\"source\" ~ ^\"can't\"
+}
 context = { old }
";
        assert_eq!(
            new_rule_names_from_diff(diff),
            vec!["static_source_cant_attack_unless_defending_player_controls_basic_land"]
        );
    }

    #[test]
    fn flags_oracle_shaped_long_rule_name() {
        let diff = "\
+static_source_cant_attack_unless_defending_player_controls_basic_land = { ^\"This\" ~ ^\"can't\" ~ ^\"attack\" ~ ^\"unless\" ~ defending_player ~ ^\"controls\" ~ basic_land }
";
        let grammar = "\
static_source_attacks_each_combat_if_able = { source_ref ~ ^\"attacks\" ~ ^\"each\" ~ ^\"combat\" ~ ^\"if\" ~ ^\"able\" ~ \".\" }
static_source_cant_attack_unless_defending_player_controls_basic_land = { source_ref ~ ^\"can't\" ~ ^\"attack\" ~ ^\"unless\" ~ defending_player ~ ^\"controls\" ~ basic_land ~ \".\" }
";
        let report = audit_grammar_diff(
            diff,
            grammar,
            "This creature can't attack unless defending player controls a basic land.",
        );
        assert!(report.findings.iter().any(|f| {
            f.severity == Severity::BlockCandidate && f.kind == "oracle_word_overlap"
        }));
    }

    #[test]
    fn finds_rhs_shape_neighbour_with_literal_changes() {
        let diff = "\
+foo_damage_target = { ^\"foo\" ~ target_creature ~ ^\"deals\" ~ number_word ~ ^\"damage\" ~ \".\" }
";
        let grammar = "\
bar_damage_target = { ^\"bar\" ~ target_creature ~ ^\"deals\" ~ number_word ~ ^\"damage\" ~ \".\" }
foo_damage_target = { ^\"foo\" ~ target_creature ~ ^\"deals\" ~ number_word ~ ^\"damage\" ~ \".\" }
";
        let report = audit_grammar_diff(diff, grammar, "Foo target creature deals three damage.");
        let rule = report
            .new_rules
            .iter()
            .find(|r| r.name == "foo_damage_target")
            .unwrap();
        assert_eq!(rule.best_neighbours[0].name, "bar_damage_target");
        assert!(rule.best_neighbours[0].similarity > 0.85);
    }

    #[test]
    fn flags_duplicate_quantity_like_rule_actionably() {
        let diff = "\
+counter_amount = _{ number_word | variable_name }
";
        let grammar = "\
draw_count = _{ number_word | variable_name }
counter_amount = _{ number_word | variable_name }
";
        let report = audit_grammar_diff(diff, grammar, "Put X counters on this creature.");
        assert!(report.findings.iter().any(|f| {
            f.severity == Severity::BlockCandidate
                && f.kind == "rhs_quantity_duplication"
                && f.message.contains("shared amount/quantity rule")
        }));
    }
}
