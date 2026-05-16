use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrammarRuleDefinition {
    pub name: String,
    pub rhs: String,
    pub line: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrammarQuery {
    pub query: String,
    #[serde(default)]
    pub terms: Vec<String>,
    #[serde(default)]
    pub rule_names: Vec<String>,
    #[serde(default)]
    pub max_candidates: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrammarQueryReport {
    pub query: String,
    pub terms: Vec<String>,
    pub explicit_rule_names: Vec<String>,
    pub rule_count: usize,
    pub candidates: Vec<GrammarRuleCandidate>,
    pub duplicate_rhs_shapes: Vec<RhsShapeGroup>,
    pub similar_rhs_shapes: Vec<RhsSimilarity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrammarRuleCandidate {
    pub name: String,
    pub line: usize,
    pub rhs: String,
    pub matched_by: Vec<String>,
    pub direct_dependencies: Vec<String>,
    pub reverse_dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RhsShapeGroup {
    pub shape: String,
    pub rules: Vec<RuleLocation>,
    pub quantity_like: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleLocation {
    pub name: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RhsSimilarity {
    pub left: RuleLocation,
    pub right: RuleLocation,
    pub similarity: f32,
    pub shared_identifiers: Vec<String>,
    pub quantity_like: bool,
}

pub fn parse_grammar_rules(grammar: &str) -> Result<Vec<GrammarRuleDefinition>> {
    let lines: Vec<&str> = grammar.lines().collect();
    let mut rules = Vec::new();
    let mut i = 0usize;

    while i < lines.len() {
        let Some(name) = rule_name_from_declaration(lines[i]) else {
            i += 1;
            continue;
        };

        let line = i + 1;
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

        rules.push(GrammarRuleDefinition { name, rhs, line });
    }

    Ok(rules)
}

pub fn parse_grammar_file(path: impl AsRef<Path>) -> Result<Vec<GrammarRuleDefinition>> {
    let path = path.as_ref();
    let grammar = fs::read_to_string(path)
        .with_context(|| format!("read grammar file {}", path.display()))?;
    parse_grammar_rules(&grammar)
}

pub fn grammar_query_report(
    rules: &[GrammarRuleDefinition],
    query: &GrammarQuery,
) -> GrammarQueryReport {
    let candidates = find_candidate_rules(rules, query)
        .into_iter()
        .map(|rule| {
            let direct_dependencies = direct_dependencies(rules, &rule.name);
            let reverse_dependencies = reverse_dependencies(rules, &rule.name);
            GrammarRuleCandidate {
                name: rule.name.clone(),
                line: rule.line,
                rhs: rule.rhs.clone(),
                matched_by: candidate_match_reasons(rule, query),
                direct_dependencies,
                reverse_dependencies,
            }
        })
        .collect();

    GrammarQueryReport {
        query: query.query.clone(),
        terms: normalized_query_terms(query),
        explicit_rule_names: query.rule_names.clone(),
        rule_count: rules.len(),
        candidates,
        duplicate_rhs_shapes: duplicate_rhs_shape_groups(rules),
        similar_rhs_shapes: similar_rhs_shapes(rules),
    }
}

pub fn find_candidate_rules<'a>(
    rules: &'a [GrammarRuleDefinition],
    query: &GrammarQuery,
) -> Vec<&'a GrammarRuleDefinition> {
    let explicit: BTreeSet<String> = query.rule_names.iter().cloned().collect();
    let terms = normalized_query_terms(query);
    let mut scored = Vec::new();

    for rule in rules {
        let mut score = 0usize;
        if explicit.contains(&rule.name) {
            score += 100;
        }
        for term in &terms {
            score += rule_term_score(rule, term);
        }
        if score > 0 {
            scored.push((score, rule));
        }
    }

    scored.sort_by(|(left_score, left_rule), (right_score, right_rule)| {
        right_score
            .cmp(left_score)
            .then_with(|| left_rule.line.cmp(&right_rule.line))
    });

    if let Some(max) = query.max_candidates {
        scored.truncate(max);
    }

    scored.into_iter().map(|(_, rule)| rule).collect()
}

pub fn direct_dependencies(rules: &[GrammarRuleDefinition], rule_name: &str) -> Vec<String> {
    let Some(rule) = rules.iter().find(|rule| rule.name == rule_name) else {
        return Vec::new();
    };
    let names = rule_name_set(rules);
    rhs_identifiers(&rule.rhs)
        .into_iter()
        .filter(|id| id != rule_name && names.contains(id))
        .collect()
}

pub fn reverse_dependencies(rules: &[GrammarRuleDefinition], rule_name: &str) -> Vec<String> {
    rules
        .iter()
        .filter(|rule| rule.name != rule_name)
        .filter(|rule| {
            direct_dependencies(rules, &rule.name)
                .iter()
                .any(|dep| dep == rule_name)
        })
        .map(|rule| rule.name.clone())
        .collect()
}

pub fn duplicate_rhs_shape_groups(rules: &[GrammarRuleDefinition]) -> Vec<RhsShapeGroup> {
    let mut by_shape: BTreeMap<String, Vec<RuleLocation>> = BTreeMap::new();
    for rule in rules {
        by_shape
            .entry(rhs_shape(&rule.rhs))
            .or_default()
            .push(RuleLocation {
                name: rule.name.clone(),
                line: rule.line,
            });
    }

    by_shape
        .into_iter()
        .filter_map(|(shape, rules)| {
            if rules.len() < 2 {
                return None;
            }
            Some(RhsShapeGroup {
                quantity_like: is_quantity_like_shape(&shape),
                shape,
                rules,
            })
        })
        .collect()
}

pub fn similar_rhs_shapes(rules: &[GrammarRuleDefinition]) -> Vec<RhsSimilarity> {
    let mut out = Vec::new();
    for (i, left) in rules.iter().enumerate() {
        let left_atoms = rhs_shape_atoms(&left.rhs);
        if left_atoms.is_empty() {
            continue;
        }
        for right in rules.iter().skip(i + 1) {
            let right_atoms = rhs_shape_atoms(&right.rhs);
            if right_atoms.is_empty() {
                continue;
            }
            let shared_identifiers = shared_identifiers(&left_atoms, &right_atoms);
            if shared_identifiers.len() < 2 {
                continue;
            }
            let similarity = skeleton_similarity(&left_atoms, &right_atoms);
            if similarity < 0.86 {
                continue;
            }
            out.push(RhsSimilarity {
                left: RuleLocation {
                    name: left.name.clone(),
                    line: left.line,
                },
                right: RuleLocation {
                    name: right.name.clone(),
                    line: right.line,
                },
                similarity,
                quantity_like: is_quantity_like_atoms(&left_atoms)
                    || is_quantity_like_atoms(&right_atoms),
                shared_identifiers,
            });
        }
    }
    out.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.left.name.cmp(&b.left.name))
            .then_with(|| a.right.name.cmp(&b.right.name))
    });
    out
}

fn rule_name_from_declaration(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return None;
    }
    let eq_pos = find_unquoted_char(trimmed, '=')?;
    let head = trimmed[..eq_pos].trim();
    if head.is_empty() {
        return None;
    }
    if head.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && head
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    {
        Some(head.to_string())
    } else {
        None
    }
}

fn brace_delta(line: &str) -> isize {
    let mut delta = 0isize;
    let mut chars = line.chars().peekable();
    let mut quote = None::<char>;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }

        match ch {
            '/' if chars.peek() == Some(&'/') => break,
            '"' | '\'' => quote = Some(ch),
            '{' => delta += 1,
            '}' => delta -= 1,
            _ => {}
        }
    }

    delta
}

fn find_unquoted_char(line: &str, needle: char) -> Option<usize> {
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut chars = line.char_indices().peekable();

    while let Some((idx, ch)) = chars.next() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }

        match ch {
            '/' if chars.peek().map(|(_, c)| *c) == Some('/') => return None,
            '"' | '\'' => quote = Some(ch),
            c if c == needle => return Some(idx),
            _ => {}
        }
    }

    None
}

fn normalized_query_terms(query: &GrammarQuery) -> Vec<String> {
    let mut terms = query.terms.clone();
    terms.extend(words(&query.query));
    terms
        .into_iter()
        .map(|term| term.to_ascii_lowercase())
        .filter(|term| !term.is_empty() && !is_stop_word(term))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn candidate_match_reasons(rule: &GrammarRuleDefinition, query: &GrammarQuery) -> Vec<String> {
    let explicit: BTreeSet<String> = query.rule_names.iter().cloned().collect();
    let mut reasons = Vec::new();
    if explicit.contains(&rule.name) {
        reasons.push(format!("rule:{}", rule.name));
    }
    for term in normalized_query_terms(query) {
        if rule_term_score(rule, &term) > 0 {
            reasons.push(format!("term:{term}"));
        }
    }
    reasons
}

fn rule_term_score(rule: &GrammarRuleDefinition, term: &str) -> usize {
    let name = rule.name.to_ascii_lowercase();
    let rhs = strip_comments_preserving_literals(&rule.rhs).to_ascii_lowercase();
    let mut score = 0usize;
    if name == term {
        score += 20;
    }
    if name.split('_').any(|segment| segment == term) {
        score += 8;
    }
    if name.contains(term) {
        score += 4;
    }
    if rhs_identifiers(&rhs).iter().any(|id| id == term) {
        score += 3;
    }
    if rhs.contains(term) {
        score += 1;
    }
    score
}

fn rule_name_set(rules: &[GrammarRuleDefinition]) -> BTreeSet<String> {
    rules.iter().map(|rule| rule.name.clone()).collect()
}

fn rhs_identifiers(rhs: &str) -> Vec<String> {
    identifier_tokens(&strip_comments_and_literals(rhs))
        .into_iter()
        .filter(|id| !is_pest_builtin_identifier(id))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn identifier_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch);
        } else if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn strip_comments_and_literals(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let mut chars = line.chars().peekable();
        let mut quote = None::<char>;
        let mut escaped = false;

        while let Some(ch) = chars.next() {
            if let Some(q) = quote {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == q {
                    quote = None;
                }
                out.push(' ');
                continue;
            }

            match ch {
                '/' if chars.peek() == Some(&'/') => break,
                '"' | '\'' => {
                    quote = Some(ch);
                    out.push(' ');
                }
                _ => out.push(ch),
            }
        }
        out.push('\n');
    }
    out
}

fn strip_comments_preserving_literals(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let mut chars = line.chars().peekable();
        let mut quote = None::<char>;
        let mut escaped = false;

        while let Some(ch) = chars.next() {
            if let Some(q) = quote {
                if escaped {
                    escaped = false;
                    out.push(ch);
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == q {
                    quote = None;
                    out.push(' ');
                } else {
                    out.push(ch);
                }
                continue;
            }

            match ch {
                '/' if chars.peek() == Some(&'/') => break,
                '"' | '\'' => {
                    quote = Some(ch);
                    out.push(' ');
                }
                _ => out.push(ch),
            }
        }
        out.push('\n');
    }
    out
}

fn rhs_shape(rhs: &str) -> String {
    rhs_shape_atoms(rhs).join(" ")
}

fn rhs_shape_atoms(rhs: &str) -> Vec<String> {
    let stripped = strip_comments_and_literals(rhs);
    let mut atoms = Vec::new();
    let mut current = String::new();

    for ch in stripped.chars() {
        match ch {
            c if c.is_ascii_alphanumeric() || c == '_' => current.push(c),
            c if matches!(c, '|' | '~' | '?' | '*' | '+' | '(' | ')') => {
                push_shape_identifier(&mut atoms, &mut current);
                atoms.push(c.to_string());
            }
            _ => push_shape_identifier(&mut atoms, &mut current),
        }
    }
    push_shape_identifier(&mut atoms, &mut current);
    atoms
}

fn push_shape_identifier(atoms: &mut Vec<String>, current: &mut String) {
    if current.is_empty() {
        return;
    }
    let id = std::mem::take(current);
    if !is_pest_builtin_identifier(&id) {
        atoms.push(id);
    }
}

fn shared_identifiers(left: &[String], right: &[String]) -> Vec<String> {
    let left_ids: BTreeSet<&str> = left
        .iter()
        .map(String::as_str)
        .filter(|atom| is_identifier_atom(atom))
        .collect();
    let right_ids: BTreeSet<&str> = right
        .iter()
        .map(String::as_str)
        .filter(|atom| is_identifier_atom(atom))
        .collect();
    left_ids
        .intersection(&right_ids)
        .map(|id| (*id).to_string())
        .collect()
}

fn is_identifier_atom(atom: &str) -> bool {
    atom.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
}

fn skeleton_similarity(left: &[String], right: &[String]) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let lcs = lcs_len(left, right) as f32 / left.len().max(right.len()) as f32;
    let axis = one_span_similarity(left, right);
    lcs.max(axis)
}

fn lcs_len(left: &[String], right: &[String]) -> usize {
    let mut prev = vec![0usize; right.len() + 1];
    let mut curr = vec![0usize; right.len() + 1];
    for left_atom in left {
        for (j, right_atom) in right.iter().enumerate() {
            curr[j + 1] = if left_atom == right_atom {
                prev[j] + 1
            } else {
                curr[j].max(prev[j + 1])
            };
        }
        std::mem::swap(&mut prev, &mut curr);
        curr.fill(0);
    }
    prev[right.len()]
}

fn one_span_similarity(left: &[String], right: &[String]) -> f32 {
    let mut prefix = 0usize;
    while prefix < left.len() && prefix < right.len() && left[prefix] == right[prefix] {
        prefix += 1;
    }

    let mut suffix = 0usize;
    while suffix + prefix < left.len()
        && suffix + prefix < right.len()
        && left[left.len() - 1 - suffix] == right[right.len() - 1 - suffix]
    {
        suffix += 1;
    }

    if left.len() == right.len() && prefix + suffix + 1 == left.len() {
        return 0.90;
    }
    (prefix + suffix) as f32 / left.len().max(right.len()) as f32
}

fn is_quantity_like_shape(shape: &str) -> bool {
    let atoms = shape
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    is_quantity_like_atoms(&atoms)
}

fn is_quantity_like_atoms(atoms: &[String]) -> bool {
    let ids: BTreeSet<&str> = atoms
        .iter()
        .map(String::as_str)
        .filter(|atom| is_identifier_atom(atom))
        .collect();
    ids.contains("variable_name")
        && (ids.contains("number_word") || ids.contains("unsigned_number"))
        && ids.len() <= 2
}

fn is_pest_builtin_identifier(id: &str) -> bool {
    matches!(
        id,
        "_" | "SOI"
            | "EOI"
            | "ANY"
            | "ASCII_DIGIT"
            | "ASCII_ALPHA"
            | "ASCII_ALPHANUMERIC"
            | "ASCII_HEX_DIGIT"
            | "NEWLINE"
            | "WHITESPACE"
    )
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
                | "do"
                | "does"
                | "for"
                | "from"
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
                | "was"
                | "when"
                | "while"
                | "with"
                | "you"
                | "your"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_rule_definitions_with_rhs_and_line_numbers() {
        let grammar = r#"
// leading comment
card_text = { SOI ~ ability ~ EOI }

ability = _{
    draw_cards
  | damage
}
"#;

        let rules = parse_grammar_rules(grammar).unwrap();

        assert_eq!(
            rules,
            vec![
                GrammarRuleDefinition {
                    name: "card_text".into(),
                    rhs: "{ SOI ~ ability ~ EOI }".into(),
                    line: 3,
                },
                GrammarRuleDefinition {
                    name: "ability".into(),
                    rhs: "_{\ndraw_cards\n| damage\n}".into(),
                    line: 5,
                },
            ]
        );
    }

    #[test]
    fn extracts_dependencies_ignoring_string_literals_and_comments() {
        let grammar = r#"
root = { ^"damage" ~ amount ~ "." ~ "\\" ~ '\'' ~ "// not a comment" } // amount_two
amount = { number_word | variable_name }
amount_two = { number_word }
number_word = { "one" }
variable_name = { "X" }
"#;

        let rules = parse_grammar_rules(grammar).unwrap();

        assert_eq!(direct_dependencies(&rules, "root"), vec!["amount"]);
    }

    #[test]
    fn reports_reverse_dependencies() {
        let grammar = r#"
root = { ability | cost }
ability = { amount ~ target }
cost = { amount }
amount = { number_word | variable_name }
target = { "target" }
number_word = { "one" }
variable_name = { "X" }
"#;

        let rules = parse_grammar_rules(grammar).unwrap();

        assert_eq!(
            reverse_dependencies(&rules, "amount"),
            vec!["ability".to_string(), "cost".to_string()]
        );
    }

    #[test]
    fn groups_duplicate_quantity_like_rhs_shapes() {
        let grammar = r#"
card_count = { number_word | variable_name }
damage_amount = { number_word | variable_name }
other_amount = { unsigned_number | variable_name }
number_word = { "one" | "two" }
unsigned_number = @{ ASCII_DIGIT+ }
variable_name = { "X" }
"#;

        let rules = parse_grammar_rules(grammar).unwrap();
        let groups = duplicate_rhs_shape_groups(&rules);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].shape, "number_word | variable_name");
        assert!(groups[0].quantity_like);
        assert_eq!(
            groups[0]
                .rules
                .iter()
                .map(|rule| rule.name.as_str())
                .collect::<Vec<_>>(),
            vec!["card_count", "damage_amount"]
        );
    }

    #[test]
    fn candidate_search_matches_literals_without_dependency_edges() {
        let grammar = r#"
root = { ^"damage" ~ amount }
amount = { number_word }
number_word = { "one" }
"#;

        let rules = parse_grammar_rules(grammar).unwrap();
        let query = GrammarQuery {
            query: "damage".into(),
            ..GrammarQuery::default()
        };

        let candidates = find_candidate_rules(&rules, &query);

        assert_eq!(candidates[0].name, "root");
        assert_eq!(direct_dependencies(&rules, "root"), vec!["amount"]);
    }
}
