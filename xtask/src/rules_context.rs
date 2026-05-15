//! Builds the "Comprehensive Rules Context" block for the grammar-fix
//! prompt. Two layers stacked:
//!
//! 1. **Always-load** — a small fixed set of high-leverage docs (the
//!    keyword indexes, damage, replacement, prevention). Provides a
//!    floor of canonical context even when retrieval scores stay low.
//! 2. **Dynamic top-K** — `qmd search` over the `mtg-rules` collection
//!    using the card's normalized oracle text. BM25 only (no LLM
//!    reranking) so the prompt is deterministic.
//!
//! Both layers fail soft: if `qmd` is missing, the rules tree hasn't
//! been built, or anything else goes wrong, the block degrades to a
//! single note and the orchestrator keeps running.

use std::path::PathBuf;
use std::process::Command;

use serde::Deserialize;

use crate::paths::repo_root;

const ALWAYS_LOAD: &[&str] = &[
    "resources/rules/700-additional-rules/702-keyword-abilities/_index.md",
    "resources/rules/700-additional-rules/701-keyword-actions/_index.md",
    "resources/rules/100-game-concepts/120-damage.md",
    "resources/rules/600-spells-abilities-and-effects/614-replacement-effects.md",
    "resources/rules/600-spells-abilities-and-effects/615-prevention-effects.md",
];

const MAX_LINES_PER_FILE: usize = 150;
/// Top hits per oracle-text line. Cards have one ability per printed
/// line; querying each line separately gives BM25 a chance to match
/// short, focused phrases rather than a noisy mixed-vocabulary blob.
const DYNAMIC_TOP_K_PER_LINE: u32 = 2;
/// Hard cap on dynamic hits added beyond the always-load set.
const DYNAMIC_TOTAL_CAP: usize = 5;
const DYNAMIC_MIN_SCORE: f32 = 0.3;
const QMD_COLLECTION: &str = "mtg-rules";

/// Renders the `## Comprehensive Rules` block to inline in the prompt.
/// `query` is normally the card's normalized oracle text.
pub fn render_rules_block(query: &str) -> String {
    let mut out = String::new();
    out.push_str("## Comprehensive Rules\n\n");
    out.push_str(
        "Canonical wording from the WotC Magic: The Gathering Comprehensive Rules. \
         Prefer the verb/template used here over inventing new grammar terms — \
         if a phenomenon already has a name in §701 (Keyword Actions) or §702 \
         (Keyword Abilities), use it.\n\n",
    );

    let root = repo_root();
    let mut included: Vec<PathBuf> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    for rel in ALWAYS_LOAD {
        let p = root.join(rel);
        if p.exists() {
            included.push(p);
        }
    }

    match qmd_search_files(query) {
        Ok(paths) => {
            for p in paths {
                if !included.iter().any(|existing| existing == &p) {
                    included.push(p);
                }
            }
        }
        Err(e) => {
            notes.push(format!(
                "_qmd retrieval unavailable: {e}. Using baseline excerpts only._"
            ));
        }
    }

    if included.is_empty() {
        out.push_str(
            "_`resources/rules/` not built. Run `just rules` and `just rules-index` \
             to populate, then re-run grammar-fix._\n\n",
        );
        return out;
    }

    for note in &notes {
        out.push_str(note);
        out.push_str("\n\n");
    }

    for path in &included {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let rel = path.strip_prefix(&root).unwrap_or(path);
        out.push_str(&format!("### {}\n\n", rel.display()));
        out.push_str("```markdown\n");
        out.push_str(&truncate_lines(&text, MAX_LINES_PER_FILE));
        out.push_str("\n```\n\n");
    }

    out
}

fn truncate_lines(text: &str, max: usize) -> String {
    let mut out = String::with_capacity(text.len().min(max * 80));
    let mut taken = 0;
    let mut had_more = false;
    for line in text.lines() {
        if taken >= max {
            had_more = true;
            break;
        }
        out.push_str(line);
        out.push('\n');
        taken += 1;
    }
    // Drop trailing newline.
    if out.ends_with('\n') {
        out.pop();
    }
    if had_more {
        out.push_str(&format!(
            "\n…\n[truncated at {max} lines — see file for the rest]"
        ));
    }
    out
}

#[derive(Deserialize)]
struct QmdHit {
    file: String,
}

fn qmd_search_files(query: &str) -> Result<Vec<PathBuf>, String> {
    let root = repo_root();
    let mut seen: Vec<String> = Vec::new();
    let mut paths: Vec<PathBuf> = Vec::new();

    for line in query.lines() {
        let normalized = normalize_line_for_query(line);
        if normalized.is_empty() {
            continue;
        }
        let hits = qmd_search_one(&normalized)?;
        for hit in hits {
            if seen.iter().any(|f| f == &hit.file) {
                continue;
            }
            seen.push(hit.file.clone());
            if let Some(rel) = qmd_uri_to_relative(&hit.file) {
                paths.push(root.join("resources/rules").join(rel));
            }
            if paths.len() >= DYNAMIC_TOTAL_CAP {
                return Ok(paths);
            }
        }
    }
    Ok(paths)
}

fn qmd_search_one(query: &str) -> Result<Vec<QmdHit>, String> {
    let output = Command::new("qmd")
        .args([
            "search",
            query,
            "-c",
            QMD_COLLECTION,
            "--json",
            "-n",
            &DYNAMIC_TOP_K_PER_LINE.to_string(),
            "--min-score",
            &DYNAMIC_MIN_SCORE.to_string(),
        ])
        .output()
        .map_err(|e| format!("invoke qmd: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "qmd exited with {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let hits: Vec<QmdHit> =
        serde_json::from_str(&stdout).map_err(|e| format!("parse qmd JSON: {e}"))?;
    Ok(hits)
}

/// BM25 is brittle around apostrophe-s, terminal punctuation, and
/// proper-noun-heavy phrases. Strip the worst offenders so each line
/// has a fighting chance of matching the spec.
fn normalize_line_for_query(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        // Skip "'s" anywhere in the line — "Nightmare's" → "Nightmare".
        if c == '\'' && chars.peek().is_some_and(|n| n.eq_ignore_ascii_case(&'s')) {
            chars.next();
            continue;
        }
        // Replace punctuation that BM25 splits on with spaces.
        if c.is_ascii_punctuation() && !matches!(c, '-') {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `qmd://mtg-rules/700-additional-rules/702-9-flying.md` → `700-.../702-9-flying.md`.
fn qmd_uri_to_relative(uri: &str) -> Option<&str> {
    let prefix = format!("qmd://{QMD_COLLECTION}/");
    uri.strip_prefix(&prefix)
}

/// Public entry point for testing — render with a stub of the qmd
/// invocation rather than actually shelling out. Not used in the prompt
/// build path; kept here so a unit test can exercise [`truncate_lines`]
/// and [`qmd_uri_to_relative`] without a live qmd.
#[cfg(test)]
fn render_with_hits(query: &str, hits: &[&std::path::Path]) -> String {
    let _ = query;
    let mut out = String::from("## Comprehensive Rules\n\n");
    for path in hits {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        out.push_str(&format!("### {}\n\n", path.display()));
        out.push_str("```markdown\n");
        out.push_str(&truncate_lines(&text, MAX_LINES_PER_FILE));
        out.push_str("\n```\n\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_keeps_short_files_intact() {
        let s = "line 1\nline 2\nline 3";
        assert_eq!(truncate_lines(s, 5), "line 1\nline 2\nline 3");
    }

    #[test]
    fn truncate_marks_overflow() {
        let s = (1..=10)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let out = truncate_lines(&s, 3);
        assert!(out.starts_with("line 1\nline 2\nline 3"));
        assert!(out.contains("truncated at 3 lines"));
        assert!(!out.contains("line 4"));
    }

    #[test]
    fn qmd_uri_relative_extraction() {
        assert_eq!(
            qmd_uri_to_relative("qmd://mtg-rules/700-additional-rules/702-9-flying.md"),
            Some("700-additional-rules/702-9-flying.md")
        );
        assert_eq!(qmd_uri_to_relative("qmd://other/x.md"), None);
        assert_eq!(qmd_uri_to_relative(""), None);
    }

    #[test]
    fn render_with_hits_handles_missing_paths() {
        let nope = PathBuf::from("/definitely/not/here.md");
        let block = render_with_hits("anything", &[&nope]);
        // Should still produce the header without panicking.
        assert!(block.starts_with("## Comprehensive Rules"));
    }
}
