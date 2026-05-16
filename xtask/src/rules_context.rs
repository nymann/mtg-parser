//! Builds the "Comprehensive Rules Context" block for the add-card
//! prompt. Two layers stacked:
//!
//! 1. **Always-load** — a small fixed set of high-leverage docs (the
//!    keyword indexes, damage, replacement, prevention). Provides a
//!    floor of canonical context even when retrieval scores stay low.
//! 2. **Dynamic top-K** — `qmd query` over the `mtg-rules` collection
//!    using typed `lex:` queries. The add-card workflow puts a focused
//!    failure phrase first, then the card's normalized oracle text as
//!    fallback, querying each line independently because BM25 doesn't
//!    handle long mixed-vocabulary phrases well. This uses qmd's query
//!    command but avoids LLM expansion/reranking so the prompt is
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
    pub query_logs: Vec<RulesQueryLog>,
}

impl RulesSearch {
    fn all_paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.always_loaded.iter().chain(self.dynamic_hits.iter())
    }
}

pub struct RulesQueryLog {
    pub query: String,
    pub hits: Vec<String>,
    pub error: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
struct QmdHit {
    file: String,
    #[allow(dead_code)]
    score: Option<f32>,
}

/// Renders the `## Comprehensive Rules` block to inline in the prompt.
/// `query` is normally produced by [`rules_query_from_failure`].
pub fn render_rules_block(query: &str) -> String {
    let (block, _) = render_rules_block_with_search(query);
    block
}

pub fn rules_query_from_failure(normalized_oracle: &str, parse_error: &str) -> String {
    let mut queries = Vec::new();
    if let Some(focused) = focused_failure_query(normalized_oracle, parse_error) {
        queries.push(focused);
    }
    queries.extend(
        normalized_oracle
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string),
    );
    dedupe_queries(queries).join("\n")
}

pub fn render_rules_block_with_search(query: &str) -> (String, RulesSearch) {
    let search = build_rules_context(query);
    let block = render_from_search(&search);
    (block, search)
}

pub fn render_search_log(search: &RulesSearch) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "qmd retrieval: {} always-loaded, {} dynamic hits, {} queries\n",
        search.always_loaded.len(),
        search.dynamic_hits.len(),
        search.queries_attempted,
    ));
    for log in &search.query_logs {
        out.push_str(&format!("query: {:?}\n", log.query));
        if let Some(error) = &log.error {
            out.push_str(&format!("  error: {error}\n"));
            continue;
        }
        if log.hits.is_empty() {
            out.push_str("  hits: none\n");
        } else {
            for hit in &log.hits {
                out.push_str(&format!("  hit: {hit}\n"));
            }
        }
    }
    for note in &search.notes {
        out.push_str(&format!("note: {note}\n"));
    }
    out
}

fn build_rules_context(query: &str) -> RulesSearch {
    build_rules_context_with(query, qmd_query_one)
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
    let mut query_logs = Vec::new();
    let configured_always_paths: Vec<PathBuf> =
        ALWAYS_LOAD.iter().map(|rel| root.join(rel)).collect();

    for (rel, p) in ALWAYS_LOAD.iter().zip(configured_always_paths.iter()) {
        if p.exists() {
            always_loaded.push(p.clone());
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
                let mut hit_log = Vec::new();
                for hit in hits {
                    hit_log.push(hit.file.clone());
                    let Some(rel) = qmd_uri_to_relative(&hit.file) else {
                        notes.push(format!("ignored hit with unresolvable uri: {}", hit.file));
                        continue;
                    };
                    let p = root.join("resources/rules").join(rel);
                    let already_present = always_loaded
                        .iter()
                        .chain(configured_always_paths.iter())
                        .chain(dynamic_hits.iter())
                        .any(|existing| existing == &p);
                    if already_present {
                        continue;
                    }
                    dynamic_hits.push(p);
                    if dynamic_hits.len() >= DYNAMIC_TOTAL_CAP {
                        query_logs.push(RulesQueryLog {
                            query: normalized,
                            hits: hit_log,
                            error: None,
                        });
                        break 'outer;
                    }
                }
                query_logs.push(RulesQueryLog {
                    query: normalized,
                    hits: hit_log,
                    error: None,
                });
            }
            Err(e) => {
                notes.push(format!("qmd failed for {normalized:?}: {e}"));
                query_logs.push(RulesQueryLog {
                    query: normalized,
                    hits: Vec::new(),
                    error: Some(e),
                });
            }
        }
    }

    RulesSearch {
        always_loaded,
        dynamic_hits,
        notes,
        queries_attempted,
        query_logs,
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
             to populate, then re-run add-card._\n\n",
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

fn focused_failure_query(normalized_oracle: &str, parse_error: &str) -> Option<String> {
    let (line_no, col_no) = parse_error_line_col(parse_error).unwrap_or((1, 1));
    let source_line = parse_error_source_line(parse_error).or_else(|| {
        normalized_oracle
            .lines()
            .nth(line_no.saturating_sub(1))
            .map(str::to_string)
    })?;
    let clause = sentence_containing_column(&source_line, col_no);
    let focused = strip_leading_activated_cost(&clause)
        .trim()
        .trim_matches('"')
        .to_string();
    if normalize_line_for_query(&focused).is_empty() {
        None
    } else {
        Some(focused)
    }
}

fn parse_error_line_col(parse_error: &str) -> Option<(usize, usize)> {
    for line in parse_error.lines() {
        let Some(rest) = line.trim().strip_prefix("--> ") else {
            continue;
        };
        let (_, location) = rest.rsplit_once(' ')?;
        let (line_no, col_no) = location.split_once(':')?;
        return Some((line_no.parse().ok()?, col_no.parse().ok()?));
    }
    None
}

fn parse_error_source_line(parse_error: &str) -> Option<String> {
    for line in parse_error.lines() {
        let Some((left, text)) = line.split_once('|') else {
            continue;
        };
        if left.trim().parse::<usize>().is_ok() {
            let source = text.trim_start();
            if !source.is_empty() {
                return Some(source.to_string());
            }
        }
    }
    None
}

fn sentence_containing_column(line: &str, col_no: usize) -> String {
    let split_at = byte_index_for_column(line, col_no).unwrap_or(0);
    let prefix = &line[..split_at.min(line.len())];
    let suffix = &line[split_at.min(line.len())..];
    let start = prefix
        .rmatch_indices(['.', '!', '?'])
        .next()
        .map(|(idx, ch)| idx + ch.len())
        .unwrap_or(0);
    let end = suffix
        .find(['.', '!', '?'])
        .map(|idx| split_at + idx + 1)
        .unwrap_or(line.len());
    line[start..end].trim().to_string()
}

fn strip_leading_activated_cost(clause: &str) -> &str {
    let Some((cost, rest)) = clause.split_once(':') else {
        return clause;
    };
    if cost.len() <= 32 && (cost.contains('{') || cost.contains("Tap")) {
        rest
    } else {
        clause
    }
}

fn byte_index_for_column(text: &str, col_no: usize) -> Option<usize> {
    if col_no == 0 {
        return Some(0);
    }
    text.char_indices()
        .nth(col_no.saturating_sub(1))
        .map(|(idx, _)| idx)
        .or(Some(text.len()))
}

fn dedupe_queries(queries: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for query in queries {
        let normalized = normalize_line_for_query(&query);
        if normalized.is_empty() {
            continue;
        }
        if out
            .iter()
            .any(|existing| normalize_line_for_query(existing) == normalized)
        {
            continue;
        }
        out.push(query);
    }
    out
}

fn truncate_lines(text: &str, max: usize) -> String {
    let mut out = String::with_capacity(text.len().min(max * 80));
    let mut had_more = false;
    for (taken, line) in text.lines().enumerate() {
        if taken >= max {
            had_more = true;
            break;
        }
        out.push_str(line);
        out.push('\n');
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

fn qmd_query_one(query: &str) -> Result<Vec<QmdHit>, String> {
    ensure_qmd_paths()?;
    let structured_query = format!("lex: {query}");
    let output = qmd_command()
        .args([
            "query",
            &structured_query,
            "-c",
            QMD_COLLECTION,
            "--json",
            "-n",
            &DYNAMIC_TOP_K_PER_LINE.to_string(),
            "--no-rerank",
        ])
        .output()
        .map_err(|e| format!("invoke qmd: {e}"))?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Ok(hits) = parse_qmd_hits(&stdout) {
            return Ok(filter_min_score(hits));
        }
    }

    qmd_search_one(query).map_err(|fallback_error| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        format!(
            "qmd query exited with {}: {}; fallback search failed: {fallback_error}",
            output.status,
            stderr.trim()
        )
    })
}

fn qmd_search_one(query: &str) -> Result<Vec<QmdHit>, String> {
    ensure_qmd_paths()?;
    let output = qmd_command()
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
        .map_err(|e| format!("invoke qmd search: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "qmd search exited with {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_qmd_hits(&stdout)
}

fn parse_qmd_hits(stdout: &str) -> Result<Vec<QmdHit>, String> {
    let json_start = stdout
        .find('[')
        .ok_or_else(|| "qmd JSON output did not contain an array".to_string())?;
    serde_json::from_str(&stdout[json_start..]).map_err(|e| format!("parse qmd JSON: {e}"))
}

fn filter_min_score(hits: Vec<QmdHit>) -> Vec<QmdHit> {
    hits.into_iter()
        .filter(|hit| hit.score.unwrap_or(1.0) >= DYNAMIC_MIN_SCORE)
        .collect()
}

fn qmd_index_path() -> PathBuf {
    repo_root().join("target/qmd-index/index.sqlite")
}

fn qmd_config_home() -> PathBuf {
    repo_root().join("target/qmd-config")
}

fn ensure_qmd_paths() -> Result<(), String> {
    let index = qmd_index_path();
    let index_dir = index
        .parent()
        .ok_or_else(|| format!("qmd index path has no parent: {}", index.display()))?;
    std::fs::create_dir_all(index_dir).map_err(|e| format!("create qmd index dir: {e}"))?;
    std::fs::create_dir_all(qmd_config_home())
        .map_err(|e| format!("create qmd config dir: {e}"))?;
    Ok(())
}

fn qmd_command() -> Command {
    let mut command = Command::new("qmd");
    command.env("INDEX_PATH", qmd_index_path());
    command.env("XDG_CONFIG_HOME", qmd_config_home());
    command
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
         without invoking the full add-card loop.\n\n\
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
            score: None,
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
            query_logs: vec![],
        };
        let line = render_diagnostics(&one);
        assert!(line.contains("1 always-loaded"));
        assert!(line.contains("1 dynamic hit · 1 query_"));

        let many = RulesSearch {
            always_loaded: vec![PathBuf::from("a"), PathBuf::from("b")],
            dynamic_hits: vec![PathBuf::from("c"), PathBuf::from("d")],
            notes: vec![],
            queries_attempted: 3,
            query_logs: vec![],
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
            query_logs: vec![],
        };
        let text = render_diagnostics(&s);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("_Retrieval:"));
        assert!(lines[1].contains("first thing"));
        assert!(lines[2].contains("second thing"));
    }

    #[test]
    fn rules_query_starts_with_counter_failure_clause() {
        let normalized = "Counter target spell with mana value X.";
        let error = "parse: parse error:  --> 1:22\n  |\n1 | Counter target spell with mana value X.\n  |                      ^---\n  |\n  = expected counter_unless_cost";
        let query = rules_query_from_failure(normalized, error);
        let lines: Vec<&str> = query.lines().collect();
        assert_eq!(lines[0], "Counter target spell with mana value X.");
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn rules_query_strips_activated_cost_from_failure_clause() {
        let normalized = "{T}: Target creature you control with toughness less than this creature's power gains flying until end of turn. Destroy that creature at the beginning of the next end step.";
        let error = "parse: parse error:  --> 1:36\n  |\n1 | {T}: Target creature you control with toughness less than this creature's power gains flying until end of turn. Destroy that creature at the beginning of the next end step.\n  |                                    ^---\n  |\n  = expected target_permanent_gains_keyword_eot_effect";
        let query = rules_query_from_failure(normalized, error);
        let lines: Vec<&str> = query.lines().collect();
        assert_eq!(
            lines[0],
            "Target creature you control with toughness less than this creature's power gains flying until end of turn."
        );
        assert_eq!(lines[1], normalized);
    }

    #[test]
    fn parses_qmd_query_json_after_progress_lines() {
        let stdout = "Warning: docs need embeddings\nStructured search: 1 queries\n[{\"file\":\"qmd://mtg-rules/glossary/damage.md\",\"score\":0.7}]";
        let hits = parse_qmd_hits(stdout).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file, "qmd://mtg-rules/glossary/damage.md");
        assert_eq!(hits[0].score, Some(0.7));
    }
}
