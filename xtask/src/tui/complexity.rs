//! Complexity time-series for the TUI.
//!
//! At session start, walk back through recent commits that touched the
//! grammar hot files and snapshot the metric we care about: grammar rule
//! count, corpus pass rate, hot-file LOC. As iterations commit during
//! the session, append a "live" snapshot. The view layer turns this into
//! a sparkline so the human can see whether the grammar is actually
//! getting simpler or just churning.

use std::process::Command;

use crate::paths::repo_root;

const HISTORY_TOUCH_FILES: &[&str] = &[
    "crates/mtg-grammar/src/grammar.pest",
    "crates/mtg-grammar/src/ast.rs",
    "crates/mtg-grammar/src/parse.rs",
    "crates/mtg-grammar/src/unparse.rs",
    "corpus_status.json",
];

const HOT_FILES_FOR_LOC: &[&str] = &[
    "crates/mtg-grammar/src/grammar.pest",
    "crates/mtg-grammar/src/ast.rs",
    "crates/mtg-grammar/src/parse.rs",
    "crates/mtg-grammar/src/unparse.rs",
];

const GRAMMAR_PEST: &str = "crates/mtg-grammar/src/grammar.pest";

#[derive(Debug, Clone)]
pub struct MetricSnapshot {
    /// Short commit SHA, or "now" for the live snapshot. Retained for
    /// Debug output during diagnostics; the view layer doesn't read it.
    #[allow(dead_code)]
    pub label: String,
    pub grammar_rules: usize,
    pub corpus_passing: usize,
    pub corpus_total: usize,
    pub hot_file_loc: usize,
}

/// Walk back through commits that touched the grammar hot files,
/// returning oldest→newest metric snapshots. Capped at `limit`.
/// Silently returns an empty vec if git is unavailable or commits can't
/// be read — the sparkline simply renders flat in that case.
pub fn historical_snapshots(limit: usize) -> Vec<MetricSnapshot> {
    let shas = recent_touching_shas(limit);
    let mut snapshots: Vec<MetricSnapshot> = shas
        .into_iter()
        .rev() // oldest first
        .filter_map(|sha| snapshot_for_commit(&sha))
        .collect();
    // Always include a live snapshot at the tail so the sparkline ends
    // at the current state, not at the last commit.
    if let Some(live) = live_snapshot() {
        snapshots.push(live);
    }
    snapshots
}

/// Read the current working-tree state as a snapshot.
pub fn live_snapshot() -> Option<MetricSnapshot> {
    let grammar_rules = count_rules_in_file(GRAMMAR_PEST)?;
    let (corpus_passing, corpus_total) = read_corpus_status_file().unwrap_or((0, 0));
    let hot_file_loc = HOT_FILES_FOR_LOC
        .iter()
        .map(|p| count_lines_in_file(p).unwrap_or(0))
        .sum();
    Some(MetricSnapshot {
        label: "now".to_string(),
        grammar_rules,
        corpus_passing,
        corpus_total,
        hot_file_loc,
    })
}

fn recent_touching_shas(limit: usize) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "log".to_string(),
        "--format=%h".to_string(),
        format!("-{limit}"),
        "--".to_string(),
    ];
    for f in HISTORY_TOUCH_FILES {
        args.push((*f).to_string());
    }
    let Ok(out) = Command::new("git")
        .args(&args)
        .current_dir(repo_root())
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect()
}

fn snapshot_for_commit(sha: &str) -> Option<MetricSnapshot> {
    let grammar_rules = git_show(sha, GRAMMAR_PEST)
        .map(|t| count_pest_rules(&t))
        .unwrap_or(0);
    let (corpus_passing, corpus_total) = git_show(sha, "corpus_status.json")
        .and_then(|t| parse_corpus_json(&t))
        .unwrap_or((0, 0));
    let hot_file_loc: usize = HOT_FILES_FOR_LOC
        .iter()
        .map(|p| git_show(sha, p).map(|t| t.lines().count()).unwrap_or(0))
        .sum();
    Some(MetricSnapshot {
        label: sha.to_string(),
        grammar_rules,
        corpus_passing,
        corpus_total,
        hot_file_loc,
    })
}

fn git_show(sha: &str, path: &str) -> Option<String> {
    let out = Command::new("git")
        .args(["show", &format!("{sha}:{path}")])
        .current_dir(repo_root())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn count_pest_rules(text: &str) -> usize {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                return false;
            }
            let Some((name, _)) = trimmed.split_once('=') else {
                return false;
            };
            let name = name.trim();
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
        .count()
}

fn count_rules_in_file(rel: &str) -> Option<usize> {
    let path = repo_root().join(rel);
    let text = std::fs::read_to_string(path).ok()?;
    Some(count_pest_rules(&text))
}

fn count_lines_in_file(rel: &str) -> Option<usize> {
    let path = repo_root().join(rel);
    let text = std::fs::read_to_string(path).ok()?;
    Some(text.lines().count())
}

fn read_corpus_status_file() -> Option<(usize, usize)> {
    let path = repo_root().join("corpus_status.json");
    let text = std::fs::read_to_string(path).ok()?;
    parse_corpus_json(&text)
}

fn parse_corpus_json(text: &str) -> Option<(usize, usize)> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let passing = value.get("passing")?.as_u64()? as usize;
    let total = value.get("total")?.as_u64()? as usize;
    Some((passing, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_pest_rules_ignores_comments_and_garbage() {
        let text = "\
// comment
rule_a = { \"a\" }
  rule_b = { \"b\" }
// rule_c = { \"c\" }
not a rule
rule_d = _{ \"d\" }
";
        assert_eq!(count_pest_rules(text), 3);
    }

    #[test]
    fn parse_corpus_json_reads_passing_and_total() {
        let json = r#"{"passing": 12, "total": 50, "ignored": []}"#;
        assert_eq!(parse_corpus_json(json), Some((12, 50)));
    }

    #[test]
    fn parse_corpus_json_returns_none_on_malformed() {
        assert_eq!(parse_corpus_json("not json"), None);
        assert_eq!(parse_corpus_json(r#"{"passing": "x"}"#), None);
    }
}
