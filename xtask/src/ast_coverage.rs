use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::paths::{ast_rs_path, generated_tests_dir, repo_root};

pub fn run(args: &[String]) -> ExitCode {
    match run_inner(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run_inner(args: &[String]) -> Result<()> {
    let fail_on_dead_parser_surface = args.iter().any(|a| a == "--fail-on-dead-parser-surface");
    let verbose = args.iter().any(|a| a == "--verbose");
    if args.iter().any(|a| {
        a != "--fail-on-dead-parser-surface" && a != "--verbose" && a != "-h" && a != "--help"
    }) {
        bail!("usage: cargo xtask ast-coverage [--fail-on-dead-parser-surface] [--verbose]");
    }
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("usage: cargo xtask ast-coverage [--fail-on-dead-parser-surface] [--verbose]");
        return Ok(());
    }

    let enums = collect_ast_enum_variants()?;
    let variant_to_enums = variant_to_enums(&enums);
    let mut exercised = BTreeSet::new();
    let mut parsed = 0usize;

    for text in generated_test_texts()? {
        let ast = mtg_grammar::parse(&text)
            .with_context(|| format!("parse generated regression text {text:?}"))?;
        let value = serde_json::to_value(&ast).context("serialize AST for coverage")?;
        collect_exercised_variants(&value, &variant_to_enums, &mut exercised);
        parsed += 1;
    }

    println!("AST coverage from generated regression tests:");
    println!("  parsed texts          : {parsed}");
    println!("  exercised enum arms   : {}", exercised.len());
    if verbose {
        for variant in &exercised {
            println!("  + {variant}");
        }
    }

    let dead = dead_unparse_only_surface(&enums, &exercised)?;
    if dead.is_empty() {
        println!("  dead parser surface   : none");
        return Ok(());
    }

    println!("  dead parser surface   : {}", dead.len());
    for variant in &dead {
        println!("  ! {variant}");
    }

    if fail_on_dead_parser_surface {
        bail!(
            "found {} unparse arm(s) with no parser construction",
            dead.len()
        );
    }

    Ok(())
}

fn generated_test_texts() -> Result<Vec<String>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(generated_tests_dir()).context("read generated tests dir")? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read generated test {}", path.display()))?;
        let oracle = extract_first_text_literal(&text)
            .with_context(|| format!("extract `let text = ...` from {}", path.display()))?;
        out.push(oracle);
    }
    out.sort();
    Ok(out)
}

fn extract_first_text_literal(text: &str) -> Result<String> {
    let start = text
        .find("let text =")
        .ok_or_else(|| anyhow::anyhow!("missing `let text = ...` line"))?;
    let rest = &text[start + "let text =".len()..];
    let quote = rest
        .find('"')
        .ok_or_else(|| anyhow::anyhow!("missing string literal after `let text =`"))?;
    parse_rust_string_literal(&rest[quote..])
}

fn parse_rust_string_literal(input: &str) -> Result<String> {
    let input = input.trim_start();
    let mut chars = input.chars();
    if chars.next() != Some('"') {
        bail!("expected string literal");
    }
    let mut out = String::new();
    let mut escaped = false;
    for ch in chars {
        if escaped {
            match ch {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                other => bail!("unsupported escape `\\{other}` in generated test literal"),
            }
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => return Ok(out),
            other => out.push(other),
        }
    }
    bail!("unterminated string literal")
}

fn collect_ast_enum_variants() -> Result<BTreeMap<String, BTreeSet<String>>> {
    let text = std::fs::read_to_string(ast_rs_path()).context("read ast.rs")?;
    let mut enums = BTreeMap::new();
    let mut current: Option<(String, usize, BTreeSet<String>)> = None;

    for line in text.lines() {
        if current.is_none() {
            if let Some(name) = enum_name_from_line(line) {
                let depth = brace_delta(line).max(0) as usize;
                current = Some((name, depth, BTreeSet::new()));
            }
            continue;
        }

        let (name, depth, variants) = current.as_mut().expect("current enum exists");
        if let Some(variant) = variant_name_from_line(line) {
            variants.insert(variant);
        }
        *depth = depth.saturating_add_signed(brace_delta(line));
        if *depth == 0 {
            let (name, _, variants) = current.take().expect("current enum exists");
            enums.insert(name, variants);
        } else if name.is_empty() {
            unreachable!();
        }
    }

    Ok(enums)
}

fn enum_name_from_line(line: &str) -> Option<String> {
    let line = line.trim_start();
    let rest = line.strip_prefix("pub enum ")?;
    Some(
        rest.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
            .next()
            .unwrap_or_default()
            .to_string(),
    )
}

fn variant_name_from_line(line: &str) -> Option<String> {
    let line = line.trim_start();
    if line.is_empty()
        || line.starts_with("#[")
        || line.starts_with("///")
        || line.starts_with("//")
        || line.starts_with('}')
        || line.starts_with("pub ")
    {
        return None;
    }
    let name = line
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .next()
        .unwrap_or_default();
    if name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
    {
        Some(name.to_string())
    } else {
        None
    }
}

fn brace_delta(line: &str) -> isize {
    let opens = line.chars().filter(|ch| *ch == '{').count();
    let closes = line.chars().filter(|ch| *ch == '}').count();
    opens as isize - closes as isize
}

fn variant_to_enums(enums: &BTreeMap<String, BTreeSet<String>>) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (enum_name, variants) in enums {
        for variant in variants {
            out.entry(variant.clone())
                .or_default()
                .push(enum_name.clone());
        }
    }
    out
}

fn collect_exercised_variants(
    value: &Value,
    variant_to_enums: &BTreeMap<String, Vec<String>>,
    out: &mut BTreeSet<String>,
) {
    match value {
        Value::Object(map) => {
            if map.len() == 1 {
                let (key, child) = map.iter().next().expect("one entry");
                if let Some(enums) = variant_to_enums.get(key) {
                    for enum_name in enums {
                        out.insert(format!("{enum_name}::{key}"));
                    }
                }
                collect_exercised_variants(child, variant_to_enums, out);
            } else {
                for child in map.values() {
                    collect_exercised_variants(child, variant_to_enums, out);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_exercised_variants(item, variant_to_enums, out);
            }
        }
        Value::String(s) => {
            if let Some(enums) = variant_to_enums.get(s) {
                for enum_name in enums {
                    out.insert(format!("{enum_name}::{s}"));
                }
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn dead_unparse_only_surface(
    enums: &BTreeMap<String, BTreeSet<String>>,
    exercised: &BTreeSet<String>,
) -> Result<BTreeSet<String>> {
    let root = repo_root();
    let parse_text = std::fs::read_to_string(root.join("crates/mtg-grammar/src/parse.rs"))
        .context("read parse.rs")?;
    let unparse_text = std::fs::read_to_string(root.join("crates/mtg-grammar/src/unparse.rs"))
        .context("read unparse.rs")?;
    let prop_text =
        std::fs::read_to_string(root.join("crates/mtg-grammar/tests/prop.rs")).unwrap_or_default();
    let mut dead = BTreeSet::new();

    for (enum_name, variants) in enums {
        for variant in variants {
            let needle = format!("{enum_name}::{variant}");
            if unparse_text.contains(&needle)
                && !parse_text.contains(&needle)
                && !prop_text.contains(&needle)
                && !exercised.contains(&needle)
            {
                dead.insert(needle);
            }
        }
    }

    Ok(dead)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_generated_text_literal() {
        assert_eq!(
            extract_first_text_literal("let text = \"Flying\\nFirst strike\";").unwrap(),
            "Flying\nFirst strike"
        );
    }

    #[test]
    fn extracts_enum_variants() {
        assert_eq!(
            variant_name_from_line("    Destroy {"),
            Some("Destroy".into())
        );
        assert_eq!(
            variant_name_from_line("    TargetPermanents(Vec<PermanentType>),"),
            Some("TargetPermanents".into())
        );
        assert_eq!(variant_name_from_line("    pub field: u32,"), None);
    }
}
