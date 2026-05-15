//! Splits `resources/comprehensive_rules.txt` (the WotC Magic
//! Comprehensive Rules) into a browsable, retrievable tree under
//! `resources/rules/`.
//!
//! Layout:
//!   - Numbered rules — one file per section (e.g. `105-colors.md`).
//!     Top-level rules and subrules within the section live in the same
//!     file so each concept is one document.
//!   - §701 Keyword Actions and §702 Keyword Abilities — one file per
//!     top-level rule (e.g. `701.16-sacrifice.md`) because each child
//!     is a distinct concept worth retrieving on its own.
//!   - Glossary — one file per entry (e.g. `glossary/lifelink.md`).
//!   - Every directory gets an `_index.md` listing its children with a
//!     one-line summary.
//!
//! The split writes to `resources/rules.tmp/` first and atomically
//! renames over `resources/rules/` so a failed split never corrupts a
//! previous good output.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use anyhow::{anyhow, bail, Context, Result};

use crate::paths::repo_root;

pub fn run(args: &[String]) -> ExitCode {
    for a in args {
        if a == "-h" || a == "--help" {
            print_help();
            return ExitCode::SUCCESS;
        }
    }
    match split() {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("rules-split: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    print!(
        "cargo xtask rules-split\n\n\
         Split resources/comprehensive_rules.txt into resources/rules/.\n\
         Reads no flags; idempotent; atomic.\n"
    );
}

const KEYWORD_ACTIONS_SECTION: u32 = 701;
const KEYWORD_ABILITIES_SECTION: u32 = 702;

fn split() -> Result<String> {
    let root = repo_root();
    let src_path = root.join("resources/comprehensive_rules.txt");
    let src = fs::read_to_string(&src_path).with_context(|| {
        format!(
            "read {} (run `just rules` to download it)",
            src_path.display()
        )
    })?;
    let src = src.trim_start_matches('\u{feff}');

    let parsed = parse_document(src)?;

    let out_dir = root.join("resources/rules");
    let tmp_dir = root.join("resources/rules.tmp");
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)
            .with_context(|| format!("clear stale {}", tmp_dir.display()))?;
    }
    fs::create_dir_all(&tmp_dir)?;

    let counts = write_tree(&tmp_dir, &parsed)?;

    if out_dir.exists() {
        fs::remove_dir_all(&out_dir)
            .with_context(|| format!("remove existing {}", out_dir.display()))?;
    }
    fs::rename(&tmp_dir, &out_dir)
        .with_context(|| format!("rename {} -> {}", tmp_dir.display(), out_dir.display()))?;

    Ok(format!(
        "wrote {chapters} chapters, {sections} sections, {kw_actions} keyword-action files, \
         {kw_abilities} keyword-ability files, {glossary} glossary entries to {dir}",
        chapters = counts.chapters,
        sections = counts.sections,
        kw_actions = counts.kw_actions,
        kw_abilities = counts.kw_abilities,
        glossary = counts.glossary,
        dir = out_dir.display()
    ))
}

// ---------------------------------------------------------------------------
// Parse
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Document {
    chapters: Vec<Chapter>,
    glossary: Vec<GlossaryEntry>,
}

#[derive(Debug)]
struct Chapter {
    number: u32,
    title: String,
    sections: Vec<Section>,
}

#[derive(Debug)]
struct Section {
    number: u32,
    title: String,
    /// Pre-rule text (intro paragraph that sits between the section
    /// header and the first numbered rule), if any.
    intro: String,
    rules: Vec<TopLevelRule>,
}

#[derive(Debug)]
struct TopLevelRule {
    /// e.g. "701.16"
    number: String,
    title: Option<String>,
    body: String,
    subrules: Vec<SubRule>,
}

#[derive(Debug)]
struct SubRule {
    /// e.g. "701.16a"
    number: String,
    body: String,
}

#[derive(Debug)]
struct GlossaryEntry {
    headword: String,
    body: String,
}

fn parse_document(src: &str) -> Result<Document> {
    // The TOC ends with `Glossary` then `Credits` (single-line entries);
    // the actual glossary section begins at the second occurrence of
    // `Glossary`; the trailing copyright block begins at the second
    // `Credits`. Numbered rules live between the first `Credits` and the
    // second `Glossary`.
    let lines: Vec<&str> = src.lines().collect();
    let glossary_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| (*l == "Glossary").then_some(i))
        .collect();
    let credits_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| (*l == "Credits").then_some(i))
        .collect();
    let glossary_line = *glossary_indices
        .get(1)
        .ok_or_else(|| anyhow!("could not locate Glossary section (expected two occurrences)"))?;
    let chapters_start = credits_indices
        .first()
        .map(|i| i + 1)
        .ok_or_else(|| anyhow!("could not locate TOC terminator `Credits`"))?;

    let body_lines = &lines[chapters_start..glossary_line];
    let chapters = parse_chapters(body_lines)?;

    // Glossary entries live between the second Glossary marker and the
    // tail "Credits" line.
    let glossary_end = lines
        .iter()
        .enumerate()
        .skip(glossary_line + 1)
        .find_map(|(idx, l)| (l == &"Credits").then_some(idx))
        .unwrap_or(lines.len());
    let glossary_lines = &lines[glossary_line + 1..glossary_end];
    let glossary = parse_glossary(glossary_lines);

    Ok(Document { chapters, glossary })
}

fn matches_chapter_header(line: &str) -> Option<(u32, &str)> {
    // "1. Game Concepts", "8. Multiplayer Rules" — single-digit number,
    // then ". ", then a Title-cased name.
    let (num, rest) = split_leading_dotted_number(line)?;
    if (1..=9).contains(&num) && !rest.is_empty() && first_char_is_uppercase(rest) {
        Some((num, rest))
    } else {
        None
    }
}

fn matches_section_header(line: &str) -> Option<(u32, &str)> {
    // "100. General", "701. Keyword Actions" — three-digit number.
    let (num, rest) = split_leading_dotted_number(line)?;
    if (100..=999).contains(&num) && !rest.is_empty() && first_char_is_uppercase(rest) {
        Some((num, rest))
    } else {
        None
    }
}

/// Match `\d{3}\.\d+\.` at the start of a line, returning (rule-number,
/// rest). The rule-number is returned without the trailing dot.
fn matches_top_level_rule(line: &str) -> Option<(String, &str)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || i > 4 || bytes.get(i) != Some(&b'.') {
        return None;
    }
    let section_len = i;
    i += 1;
    let after_section = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == after_section || bytes.get(i) != Some(&b'.') {
        return None;
    }
    let rule_end = i;
    i += 1;
    if bytes.get(i) != Some(&b' ') {
        return None;
    }
    let number = line[..rule_end].to_string();
    let _ = section_len;
    Some((number, &line[i + 1..]))
}

/// Match `\d{3}\.\d+[a-z]` at the start of a line. Unlike top-level
/// rules, subrules don't print a period after the letter — the body
/// follows directly after a space (`100.1a A two-player game ...`).
fn matches_subrule(line: &str) -> Option<(String, &str)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || bytes.get(i) != Some(&b'.') {
        return None;
    }
    i += 1;
    let after_dot = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == after_dot {
        return None;
    }
    if bytes.get(i).map(|b| b.is_ascii_lowercase()) != Some(true) {
        return None;
    }
    let letter_end = i + 1;
    if bytes.get(letter_end) != Some(&b' ') {
        return None;
    }
    let number = line[..letter_end].to_string();
    Some((number, &line[letter_end + 1..]))
}

fn split_leading_dotted_number(line: &str) -> Option<(u32, &str)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || bytes.get(i) != Some(&b'.') {
        return None;
    }
    let num: u32 = line[..i].parse().ok()?;
    let rest = line[i + 1..].trim_start();
    Some((num, rest))
}

fn first_char_is_uppercase(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_uppercase())
}

fn parse_chapters(lines: &[&str]) -> Result<Vec<Chapter>> {
    let mut chapters = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some((num, title)) = matches_chapter_header(line) {
            let mut sections = Vec::new();
            i += 1;
            // Skip blank line(s) after chapter title.
            while i < lines.len() && lines[i].trim().is_empty() {
                i += 1;
            }
            // Read sections until we hit the next chapter header.
            while i < lines.len() {
                if matches_chapter_header(lines[i]).is_some() {
                    break;
                }
                if let Some((sec_num, sec_title)) = matches_section_header(lines[i]) {
                    let (section, advance) = parse_section(sec_num, sec_title, &lines[i + 1..])?;
                    sections.push(section);
                    i += 1 + advance;
                } else {
                    i += 1;
                }
            }
            chapters.push(Chapter {
                number: num,
                title: title.to_string(),
                sections,
            });
        } else {
            i += 1;
        }
    }
    Ok(chapters)
}

/// Returns the parsed section and how many lines (after the header)
/// were consumed.
fn parse_section(number: u32, title: &str, lines: &[&str]) -> Result<(Section, usize)> {
    let mut intro_lines: Vec<&str> = Vec::new();
    let mut rules: Vec<TopLevelRule> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if matches_chapter_header(line).is_some() || matches_section_header(line).is_some() {
            break;
        }
        if let Some((rule_num, rest)) = matches_top_level_rule(line) {
            // Title-of-rule for the §701/§702 children sometimes lives
            // on the same line ("701.16. Sacrifice"). Bodies are usually
            // standalone on subsequent lines.
            let (rule, advance) = parse_top_level_rule(&rule_num, rest, &lines[i + 1..]);
            rules.push(rule);
            i += 1 + advance;
        } else {
            intro_lines.push(line);
            i += 1;
        }
    }
    let section = Section {
        number,
        title: title.to_string(),
        intro: trim_block(&intro_lines.join("\n")),
        rules,
    };
    Ok((section, i))
}

fn parse_top_level_rule(number: &str, first_line: &str, rest: &[&str]) -> (TopLevelRule, usize) {
    // §701/§702 conventionally print the keyword name on the header
    // line — e.g. "701.16. Sacrifice". When the header line has body
    // text instead (e.g. "100.1. These Magic rules apply..."), we treat
    // it as part of the body and leave `title` empty.
    let mut title: Option<String> = None;
    let mut body_lines: Vec<String> = Vec::new();
    let trimmed_first = first_line.trim();
    if looks_like_inline_title(trimmed_first) {
        title = Some(trimmed_first.to_string());
    } else {
        body_lines.push(first_line.to_string());
    }

    let mut subrules: Vec<SubRule> = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        let line = rest[i];
        if matches_chapter_header(line).is_some()
            || matches_section_header(line).is_some()
            || matches_top_level_rule(line).is_some()
        {
            break;
        }
        if let Some((sub_number, sub_rest)) = matches_subrule(line) {
            let (sub, advance) = parse_subrule(&sub_number, sub_rest, &rest[i + 1..]);
            subrules.push(sub);
            i += 1 + advance;
        } else {
            body_lines.push(line.to_string());
            i += 1;
        }
    }
    (
        TopLevelRule {
            number: number.to_string(),
            title,
            body: trim_block(&body_lines.join("\n")),
            subrules,
        },
        i,
    )
}

fn parse_subrule(number: &str, first_line: &str, rest: &[&str]) -> (SubRule, usize) {
    let mut body_lines: Vec<String> = vec![first_line.to_string()];
    let mut i = 0;
    while i < rest.len() {
        let line = rest[i];
        if matches_chapter_header(line).is_some()
            || matches_section_header(line).is_some()
            || matches_top_level_rule(line).is_some()
            || matches_subrule(line).is_some()
        {
            break;
        }
        body_lines.push(line.to_string());
        i += 1;
    }
    (
        SubRule {
            number: number.to_string(),
            body: trim_block(&body_lines.join("\n")),
        },
        i,
    )
}

fn looks_like_inline_title(s: &str) -> bool {
    // Heuristic: §701/§702 children look like "Sacrifice" or "First
    // Strike" — short, no terminal punctuation, capitalized. Bodies
    // are sentences.
    if s.is_empty() || s.len() > 80 {
        return false;
    }
    if s.ends_with('.') || s.ends_with(':') {
        return false;
    }
    let lower = s.to_ascii_lowercase();
    if lower.contains(" the ") || lower.contains(" a ") || lower.contains(" of ") {
        // "the", "a", "of" mid-sentence is a strong signal of body text.
        // Title case like "Lord of the Pit" wouldn't appear as a rule
        // title in the comprehensive rules.
    }
    s.chars().next().is_some_and(|c| c.is_uppercase())
}

fn parse_glossary(lines: &[&str]) -> Vec<GlossaryEntry> {
    let mut entries = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        // Skip leading blanks.
        while i < lines.len() && lines[i].trim().is_empty() {
            i += 1;
        }
        if i >= lines.len() {
            break;
        }
        let headword = lines[i].trim().to_string();
        i += 1;
        let mut body_lines: Vec<String> = Vec::new();
        while i < lines.len() && !lines[i].trim().is_empty() {
            body_lines.push(lines[i].to_string());
            i += 1;
        }
        if headword.is_empty() || body_lines.is_empty() {
            // Malformed entry — skip silently. The glossary is
            // hand-written prose and occasional formatting noise
            // happens.
            continue;
        }
        entries.push(GlossaryEntry {
            headword,
            body: body_lines.join("\n"),
        });
    }
    entries
}

fn trim_block(s: &str) -> String {
    // Collapse runs of 3+ newlines down to 2 and strip leading/trailing
    // whitespace. Preserves paragraph breaks.
    let trimmed = s.trim();
    let mut out = String::with_capacity(trimmed.len());
    let mut newlines_in_a_row = 0;
    for c in trimmed.chars() {
        if c == '\n' {
            newlines_in_a_row += 1;
            if newlines_in_a_row <= 2 {
                out.push(c);
            }
        } else {
            newlines_in_a_row = 0;
            out.push(c);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Write
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Counts {
    chapters: usize,
    sections: usize,
    kw_actions: usize,
    kw_abilities: usize,
    glossary: usize,
}

fn write_tree(out: &Path, doc: &Document) -> Result<Counts> {
    let mut counts = Counts::default();

    // Root index.
    let mut root_index = String::from("# Magic Comprehensive Rules\n\n");
    root_index.push_str(
        "Generated by `cargo xtask rules-split` from `resources/comprehensive_rules.txt`.\n\n",
    );
    root_index.push_str("## Chapters\n\n");

    for chapter in &doc.chapters {
        let chapter_slug = format!("{:03}-{}", chapter.number * 100, slugify(&chapter.title));
        let chapter_dir = out.join(&chapter_slug);
        fs::create_dir_all(&chapter_dir)?;
        counts.chapters += 1;

        let mut chapter_index = String::new();
        chapter_index.push_str(&format!("# {}. {}\n\n", chapter.number, chapter.title));
        chapter_index.push_str("## Sections\n\n");

        for section in &chapter.sections {
            counts.sections += 1;
            let section_summary = if section.number == KEYWORD_ACTIONS_SECTION {
                let dir =
                    chapter_dir.join(format!("{}-{}", section.number, slugify(&section.title)));
                fs::create_dir_all(&dir)?;
                counts.kw_actions += write_keyword_children(&dir, section)?;
                let rel = relative_to(&dir, &chapter_dir);
                format!(
                    "- [{} {} (directory)]({}/_index.md)",
                    section.number, section.title, rel
                )
            } else if section.number == KEYWORD_ABILITIES_SECTION {
                let dir =
                    chapter_dir.join(format!("{}-{}", section.number, slugify(&section.title)));
                fs::create_dir_all(&dir)?;
                counts.kw_abilities += write_keyword_children(&dir, section)?;
                let rel = relative_to(&dir, &chapter_dir);
                format!(
                    "- [{} {} (directory)]({}/_index.md)",
                    section.number, section.title, rel
                )
            } else {
                let file_name = format!("{}-{}.md", section.number, slugify(&section.title));
                let path = chapter_dir.join(&file_name);
                fs::write(&path, render_section(section))?;
                format!(
                    "- [{} {}]({}) — {}",
                    section.number,
                    section.title,
                    file_name,
                    one_line_summary(&section_first_sentence(section))
                )
            };
            chapter_index.push_str(&section_summary);
            chapter_index.push('\n');
        }

        fs::write(chapter_dir.join("_index.md"), chapter_index)?;
        root_index.push_str(&format!(
            "- [{}. {}]({}/_index.md)\n",
            chapter.number, chapter.title, chapter_slug
        ));
    }

    // Glossary.
    let glossary_dir = out.join("glossary");
    fs::create_dir_all(&glossary_dir)?;
    let mut glossary_index = String::from("# Glossary\n\n");
    let mut seen: BTreeMap<String, u32> = BTreeMap::new();
    for entry in &doc.glossary {
        let base = slugify(&entry.headword);
        let slug = match seen.get(&base) {
            None => {
                seen.insert(base.clone(), 1);
                base
            }
            Some(&n) => {
                seen.insert(base.clone(), n + 1);
                format!("{base}-{}", n + 1)
            }
        };
        let file_name = format!("{slug}.md");
        let path = glossary_dir.join(&file_name);
        fs::write(&path, render_glossary(entry))?;
        glossary_index.push_str(&format!(
            "- [{}]({}) — {}\n",
            entry.headword,
            file_name,
            one_line_summary(&entry.body)
        ));
        counts.glossary += 1;
    }
    fs::write(glossary_dir.join("_index.md"), glossary_index)?;
    root_index.push_str(&format!("- [Glossary]({}/_index.md)\n", "glossary"));

    fs::write(out.join("_index.md"), root_index)?;

    Ok(counts)
}

fn write_keyword_children(dir: &Path, section: &Section) -> Result<usize> {
    let mut index = String::new();
    index.push_str(&format!("# {} {}\n\n", section.number, section.title));
    if !section.intro.is_empty() {
        index.push_str(&section.intro);
        index.push_str("\n\n");
    }
    index.push_str("## Entries\n\n");

    let mut written = 0;
    for rule in &section.rules {
        let file_name = match rule.title.as_deref() {
            Some(t) => format!("{}-{}.md", rule.number, slugify(t)),
            None => format!("{}.md", rule.number),
        };
        let path = dir.join(&file_name);
        fs::write(&path, render_top_level_rule(rule))?;
        let summary = if rule.body.is_empty() {
            rule.subrules
                .first()
                .map(|s| one_line_summary(&s.body))
                .unwrap_or_default()
        } else {
            one_line_summary(&rule.body)
        };
        let display_title = match rule.title.as_deref() {
            Some(t) => format!("{} {}", rule.number, t),
            None => rule.number.clone(),
        };
        index.push_str(&format!(
            "- [{}]({}) — {}\n",
            display_title, file_name, summary
        ));
        written += 1;
    }
    fs::write(dir.join("_index.md"), index)?;
    if written == 0 {
        bail!(
            "section {} {} produced no rule files",
            section.number,
            section.title
        );
    }
    Ok(written)
}

fn render_section(section: &Section) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {} {}\n\n", section.number, section.title));
    if !section.intro.is_empty() {
        out.push_str(&section.intro);
        out.push_str("\n\n");
    }
    for rule in &section.rules {
        out.push_str(&format!("## {}", rule.number));
        if let Some(title) = &rule.title {
            out.push_str(&format!(" — {title}"));
        }
        out.push_str("\n\n");
        if !rule.body.is_empty() {
            out.push_str(&rule.body);
            out.push_str("\n\n");
        }
        for sub in &rule.subrules {
            out.push_str(&format!("### {}\n\n{}\n\n", sub.number, sub.body));
        }
    }
    out
}

fn render_top_level_rule(rule: &TopLevelRule) -> String {
    let mut out = String::new();
    match rule.title.as_deref() {
        Some(t) => out.push_str(&format!("# {} — {}\n\n", rule.number, t)),
        None => out.push_str(&format!("# {}\n\n", rule.number)),
    }
    if !rule.body.is_empty() {
        out.push_str(&rule.body);
        out.push_str("\n\n");
    }
    for sub in &rule.subrules {
        out.push_str(&format!("## {}\n\n{}\n\n", sub.number, sub.body));
    }
    out
}

fn render_glossary(entry: &GlossaryEntry) -> String {
    format!("# {}\n\n{}\n", entry.headword, entry.body)
}

fn section_first_sentence(section: &Section) -> String {
    if !section.intro.is_empty() {
        return section.intro.clone();
    }
    section
        .rules
        .first()
        .map(|r| {
            if !r.body.is_empty() {
                r.body.clone()
            } else {
                r.subrules
                    .first()
                    .map(|s| s.body.clone())
                    .unwrap_or_default()
            }
        })
        .unwrap_or_default()
}

fn one_line_summary(s: &str) -> String {
    let trimmed = s.trim();
    let first_paragraph = trimmed
        .split("\n\n")
        .next()
        .unwrap_or(trimmed)
        .replace('\n', " ");
    let condensed = first_paragraph
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let truncated = if condensed.chars().count() > 140 {
        let mut s: String = condensed.chars().take(137).collect();
        s.push_str("...");
        s
    } else {
        condensed
    };
    truncated
}

fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_dash = true;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_was_dash = false;
        } else if (c.is_whitespace() || matches!(c, '-' | '_' | '/' | ',' | '.' | ':' | ';'))
            && !last_was_dash
        {
            out.push('-');
            last_was_dash = true;
        }
        // Other characters (apostrophes, quotes, parentheses) are dropped.
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "untitled".into()
    } else {
        out
    }
}

fn relative_to(target: &Path, base: &Path) -> String {
    target
        .strip_prefix(base)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| target.display().to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_handles_common_shapes() {
        assert_eq!(slugify("Game Concepts"), "game-concepts");
        assert_eq!(slugify("First Strike"), "first-strike");
        assert_eq!(slugify("Day and Night"), "day-and-night");
        assert_eq!(slugify("C'tan"), "ctan");
        assert_eq!(slugify("Lhurgoyf"), "lhurgoyf");
        assert_eq!(slugify("Power/Toughness"), "power-toughness");
    }

    #[test]
    fn matches_section_header_distinguishes_chapters() {
        assert!(matches_chapter_header("1. Game Concepts").is_some());
        assert!(matches_chapter_header("100. General").is_none());
        assert!(matches_section_header("100. General").is_some());
        assert!(matches_section_header("1. Game Concepts").is_none());
    }

    #[test]
    fn matches_top_level_rule_and_subrule() {
        assert_eq!(
            matches_top_level_rule("701.16. Sacrifice"),
            Some(("701.16".into(), "Sacrifice"))
        );
        assert_eq!(
            matches_subrule("701.16a To sacrifice a permanent..."),
            Some(("701.16a".into(), "To sacrifice a permanent..."))
        );
        // Glossary numbered-list items shouldn't be confused with rules.
        assert!(matches_top_level_rule("2. An activated or triggered ability...").is_none());
    }

    #[test]
    fn parses_minimal_document() {
        let src = "\u{feff}Title\n\nContents\n\n1. Game Concepts\n100. General\n\nGlossary\n\nCredits\n\n\
                   1. Game Concepts\n\n100. General\n\n100.1. Hello.\n\n100.1a World.\n\n\
                   Glossary\n\nAbility\nText on an object.\n\nCredits\n";
        let doc = parse_document(src).expect("parse");
        assert_eq!(doc.chapters.len(), 1);
        assert_eq!(doc.chapters[0].sections.len(), 1);
        assert_eq!(doc.chapters[0].sections[0].rules.len(), 1);
        assert_eq!(doc.chapters[0].sections[0].rules[0].subrules.len(), 1);
        assert_eq!(doc.glossary.len(), 1);
        assert_eq!(doc.glossary[0].headword, "Ability");
    }
}
