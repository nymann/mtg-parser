use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/grammar.pest");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let grammar_path = manifest_dir.join("src/grammar.pest");
    let grammar = fs::read_to_string(&grammar_path).expect("read grammar.pest");
    let rules = collect_rule_names(&grammar);

    let mut out = String::new();
    out.push_str("fn rule_by_name(name: &str) -> Option<Rule> {\n");
    out.push_str("    match name {\n");
    for rule in &rules {
        out.push_str(&format!("        \"{rule}\" => Some(Rule::{rule}),\n"));
    }
    out.push_str("        _ => None,\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str("pub fn pest_rule_names() -> &'static [&'static str] {\n");
    out.push_str("    &[\n");
    for rule in &rules {
        out.push_str(&format!("        \"{rule}\",\n"));
    }
    out.push_str("    ]\n");
    out.push_str("}\n");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(out_dir.join("rule_lookup.rs"), out).expect("write rule_lookup.rs");
}

fn collect_rule_names(grammar: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in grammar.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        let Some((name, _)) = trimmed.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if is_rule_name(name) {
            names.insert(name.to_string());
        }
    }
    names
}

fn is_rule_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}
