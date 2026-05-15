//! Builds the "Comprehensive Rules Context" block for the grammar-fix
//! prompt. Two layers stacked:
//!
//! 1. **Always-load** — a small fixed set of high-leverage docs (the
//!    keyword indexes, damage, replacement, prevention). Provides a
//!    floor of canonical context even when retrieval scores stay low.
//! 2. **Dynamic top-K** — `qmd search` over the `mtg-rules` collection
//!    using the card's normalized oracle text, querying each line
//!    independently because BM25 doesn't handle long mixed-vocabulary
//!    phrases well. Pure BM25 (no LLM reranking) so the prompt is
//!    deterministic.
//!
//! Failure is per-line, not global: one bad query produces a note in
//! the block but does not drop the other lines' hits. Notes also
//! surface missing always-load files and unparseable hit URIs, so the
//! `## Comprehensive Rules` block carries enough diagnostics for a
//! human to audit retrieval health from the saved prompt alone.

use std::path::PathBuf;
use std::process::{Command, ExitCode};

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

/// Outcome of one retrieval pass — the file lists for rendering plus
/// the notes that explain any non-fatal degradations.
pub struct RulesSearch {
    pub always_loaded: Vec<PathBuf>,
    pub dynamic_hits: Vec<PathBuf>,
    pub notes: Vec<String>,
    pub queries_attempted: u32,
}

impl RulesSearch {
    fn all_paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.always_loaded.iter().chain(self.dynamic_hits.iter())
    }
}

#[derive(Deserialize, Clone, Debug)]
struct QmdHit {
    file: String,
}

/// Renders the `## Comprehensive Rules` block to inline in the prompt.
/// `query` is normally the card's normalized oracle text.
pub fn render_rules_block(query: &str) -> String {
    render_from_search(&build_rules_context(query))
}

fn build_rules_context(query: &str) -> RulesSearch {
    build_rules_context_with(query, qmd_search_one)
}

/// Pure-logic core. `qmd` is injected so unit tests can exercise the
/// merge/dedupe/notes plumbing without shelling out.
fn build_rules_context_with<F>(query: &str, mut qmd: F) -> RulesSearch
where
    F: FnMut(&str) -> Result<Vec<QmdHit>, String>,
{
    let root = repo_root();
    let mut always_loaded: Vec<PathBuf> = Vec::new();
    let mut dynamic_hits: Vec<PathBuf> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut queries_attempted: u32 = 0;

    for rel in ALWAYS_LOAD {
        let p = root.join(rel);
        if p.exists() {
            always_loaded.push(p);
        } else {
            notes.push(format!("always-load missing: {rel}"));
        }
    }

    'outer: for line in query.lines() {
        let normalized = normalize_line_for_query(line);
        if normalized.is_empty() {
            continue;
        }
        queries_attempted += 1;
        match qmd(&normalized) {
            Ok(hits) => {
                for hit in hits {
                    let Some(rel) = qmd_uri_to_relative(&hit.file) else {
                        notes.push(format!("ignored hit with unresolvable uri: {}", hit.file));
                        continue;
                    };
                    let p = root.join("resources/rules").join(rel);
                    let already_present = always_loaded
                        .iter()
                        .chain(dynamic_hits.iter())
                        .any(|existing| existing == &p);
                    if already_present {
                        continue;
                    }
                    dynamic_hits.push(p);
                    if dynamic_hits.len() >= DYNAMIC_TOTAL_CAP {
                        break 'outer;
                    }
                }
            }
            Err(e) => {
                notes.push(format!("qmd failed for {normalized:?}: {e}"));
            }
        }
    }

    RulesSearch {
        always_loaded,
        dynamic_hits,
        notes,
        queries_attempted,
    }
}

fn render_from_search(search: &RulesSearch) -> String {
    let mut out = String::new();
    out.push_str("## Comprehensive Rules\n\n");
    out.push_str(
        "Canonical wording from the WotC Magic: The Gathering Comprehensive Rules. \
         Prefer the verb/template used here over inventing new grammar terms — \
         if a phenomenon already has a name in §701 (Keyword Actions) or §702 \
         (Keyword Abilities), use it.\n\n",
    );

    out.push_str(&render_diagnostics(search));
    out.push('\n');

    if search.always_loaded.is_empty() && search.dynamic_hits.is_empty() {
        out.push_str(
            "_`resources/rules/` not built. Run `just rules` and `just rules-index` \
             to populate, then re-run grammar-fix._\n\n",
        );
        return out;
    }

    let root = repo_root();
    for path in search.all_paths() {
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

fn render_diagnostics(search: &RulesSearch) -> String {
    let mut out = String::new();
    let dyn_word = if search.dynamic_hits.len() == 1 {
        "hit"
    } else {
        "hits"
    };
    let q_word = if search.queries_attempted == 1 {
        "query"
    } else {
        "queries"
    };
    out.push_str(&format!(
        "_Retrieval: {} always-loaded · {} dynamic {dyn_word} · {} {q_word}_\n",
        search.always_loaded.len(),
        search.dynamic_hits.len(),
        search.queries_attempted,
    ));
    for note in &search.notes {
        out.push_str(&format!("_- {note}_\n"));
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

// ---------------------------------------------------------------------------
// `cargo xtask rules-context "<query>"` — diagnostic command
// ---------------------------------------------------------------------------

pub fn run_cli(args: &[String]) -> ExitCode {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if args.is_empty() {
        eprintln!("rules-context: missing query argument");
        print_help();
        return ExitCode::from(2);
    }
    // Allow either a single quoted argument or several positional words.
    let query = args.join(" ");
    print!("{}", render_rules_block(&query));
    ExitCode::SUCCESS
}

fn print_help() {
    print!(
        "cargo xtask rules-context \"<query text>\"\n\n\
         Renders the Comprehensive Rules prompt block for the given query.\n\
         Use it to inspect what the agent would see for an oracle phrase\n\
         without invoking the full grammar-fix loop.\n\n\
         The query can be a single quoted string or a series of positional\n\
         words; they're joined with spaces before being split per-line for\n\
         qmd retrieval.\n"
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(file: &str) -> QmdHit {
        QmdHit {
            file: file.to_string(),
        }
    }

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

    /// The whole point of the per-line iteration: one query failing
    /// (qmd error, unparseable line, whatever) must NOT discard the
    /// successful lines' hits.
    #[test]
    fn one_failed_line_still_returns_successful_hits() {
        let query = "line a\nline b";
        let search = build_rules_context_with(query, |q| {
            if q.contains("a") {
                Err("simulated qmd failure".to_string())
            } else {
                Ok(vec![hit("qmd://mtg-rules/glossary/flying.md")])
            }
        });
        assert_eq!(search.dynamic_hits.len(), 1);
        assert!(search.dynamic_hits[0]
            .to_string_lossy()
            .ends_with("glossary/flying.md"));
        assert!(search
            .notes
            .iter()
            .any(|n| n.contains("simulated qmd failure")));
        assert_eq!(search.queries_attempted, 2);
    }

    /// Dynamic hits that overlap the always-load set must not appear
    /// twice in the rendered block.
    #[test]
    fn dynamic_dedupes_against_always_load() {
        let query = "anything";
        let search = build_rules_context_with(query, |_q| {
            // First entry in ALWAYS_LOAD — guaranteed to clash if it exists.
            Ok(vec![hit(
                "qmd://mtg-rules/700-additional-rules/702-keyword-abilities/_index.md",
            )])
        });
        // The dynamic hit collides with always-load #1, so dynamic stays empty.
        assert!(
            search.dynamic_hits.is_empty(),
            "expected zero dynamic hits after dedupe, got {:?}",
            search.dynamic_hits
        );
    }

    /// An unparseable URI (wrong collection prefix, malformed string)
    /// becomes a note rather than crashing or being silently dropped.
    #[test]
    fn unresolvable_uri_becomes_a_note() {
        let query = "anything";
        let search = build_rules_context_with(query, |_q| Ok(vec![hit("not-a-qmd-uri.md")]));
        assert!(search.dynamic_hits.is_empty());
        assert!(search
            .notes
            .iter()
            .any(|n| n.contains("unresolvable uri") && n.contains("not-a-qmd-uri.md")));
    }

    #[test]
    fn diagnostics_header_formats_singular_and_plural() {
        let one = RulesSearch {
            always_loaded: vec![PathBuf::from("a")],
            dynamic_hits: vec![PathBuf::from("b")],
            notes: vec![],
            queries_attempted: 1,
        };
        let line = render_diagnostics(&one);
        assert!(line.contains("1 always-loaded"));
        assert!(line.contains("1 dynamic hit · 1 query_"));

        let many = RulesSearch {
            always_loaded: vec![PathBuf::from("a"), PathBuf::from("b")],
            dynamic_hits: vec![PathBuf::from("c"), PathBuf::from("d")],
            notes: vec![],
            queries_attempted: 3,
        };
        let line = render_diagnostics(&many);
        assert!(line.contains("2 dynamic hits · 3 queries_"));
    }

    #[test]
    fn diagnostics_emits_notes_below_header() {
        let s = RulesSearch {
            always_loaded: vec![],
            dynamic_hits: vec![],
            notes: vec!["first thing".into(), "second thing".into()],
            queries_attempted: 0,
        };
        let text = render_diagnostics(&s);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("_Retrieval:"));
        assert!(lines[1].contains("first thing"));
        assert!(lines[2].contains("second thing"));
    }
}
