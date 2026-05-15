use std::process::ExitCode;

use anyhow::{anyhow, bail, Context, Result};

use mtg_corpus::{build_report, diff, load, save, CorpusReport};
use mtg_scryfall::ScryfallClient;

use crate::paths::{corpus_sets_path, corpus_status_path};

pub fn run(args: &[String]) -> ExitCode {
    let force_update = args.iter().any(|a| a == "--update");
    match run_inner(force_update) {
        Ok(CorpusRun::Success) => ExitCode::SUCCESS,
        Ok(CorpusRun::Failure) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub fn refresh(args: &[String]) -> ExitCode {
    let single = parse_set(args);
    match tracked_sets()
        .map(|tracked| single.map(|s| vec![s]).unwrap_or(tracked))
        .and_then(|sets| refresh_inner(&sets))
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub fn add_set(args: &[String]) -> ExitCode {
    match args {
        [code] => match add_set_inner(code) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e:#}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("usage: cargo xtask corpus-add-set CODE");
            ExitCode::from(2)
        }
    }
}

pub fn advance(args: &[String]) -> ExitCode {
    match parse_advance_options(args).and_then(advance_inner) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CorpusRun {
    Success,
    Failure,
}

fn run_inner(force_update: bool) -> Result<CorpusRun> {
    let client = ScryfallClient::new()?;
    let sets = tracked_sets()?;
    let new_report = build_corpus(&client, &sets)?;
    let path = corpus_status_path();

    if !path.exists() {
        save(&new_report, &path)?;
        println!(
            "Wrote initial corpus_status.json: {} passing of {} ({} sets).",
            new_report.passing,
            new_report.total,
            sets.len(),
        );
        return Ok(CorpusRun::Success);
    }

    let old = load(&path)?;
    let d = diff(&old, &new_report);

    println!("Corpus over {} card(s) in {:?}:", new_report.total, sets);
    println!("  passing now     : {}", new_report.passing);
    println!(
        "  was             : {} (delta {:+})",
        old.passing,
        new_report.passing as i64 - old.passing as i64
    );
    println!("  newly passing   : {}", d.new_passes.len());
    println!("  newly failing   : {}", d.new_failures.len());
    println!("  still failing   : {}", d.still_failing);
    println!("  still passing   : {}", d.still_passing);

    for key in &d.new_passes {
        println!("  + {key}");
    }
    for key in &d.new_failures {
        println!("  ! {key}  (was passing)");
    }

    if !d.new_failures.is_empty() && !force_update {
        eprintln!();
        eprintln!(
            "REGRESSION: {} previously-passing card(s) now fail. corpus_status.json NOT updated.",
            d.new_failures.len(),
        );
        eprintln!("Fix the regressions, or re-run with `--update` to overwrite anyway.");
        return Ok(CorpusRun::Failure);
    }

    save(&new_report, &path)?;
    println!("Updated {}.", path.display());
    Ok(CorpusRun::Success)
}

fn add_set_inner(code: &str) -> Result<()> {
    let code = code.to_lowercase();
    let mut sets = tracked_sets()?;
    if sets.iter().any(|s| s == &code) {
        println!(
            "{code} is already tracked in {}.",
            corpus_sets_path().display()
        );
        return Ok(());
    }

    let client = ScryfallClient::new()?;
    let cards = client
        .refresh_set(&code)
        .with_context(|| format!("refresh {code}"))?;
    if cards.is_empty() {
        bail!("{code} returned no cards");
    }

    sets.push(code.clone());
    save_tracked_sets(&sets)?;
    println!("Added {code} to {}.", corpus_sets_path().display());

    match run_inner(false)? {
        CorpusRun::Success => Ok(()),
        CorpusRun::Failure => bail!("corpus failed after adding {code}"),
    }
}

fn advance_inner(options: AdvanceOptions) -> Result<()> {
    let sets = tracked_sets()?;
    let current = sets
        .last()
        .ok_or_else(|| anyhow!("{} is empty", corpus_sets_path().display()))?;
    let grammar_left = actionable_failures_for_set(current)?;
    if grammar_left > options.max_grammar_left {
        println!(
            "{current} still has {grammar_left} actionable grammar failure(s); threshold is {}. Not advancing.",
            options.max_grammar_left,
        );
        return Ok(());
    }

    let client = ScryfallClient::new()?;
    let all_sets = client.paper_expansion_sets()?;
    let current_index = all_sets
        .iter()
        .position(|s| s.code == *current)
        .ok_or_else(|| anyhow!("{current} is not in Scryfall's paper core/expansion set list"))?;
    let next = all_sets
        .get(current_index + 1)
        .ok_or_else(|| anyhow!("{current} is already the latest paper core/expansion set"))?;
    println!(
        "Advancing corpus from {current} to {} ({}, {}).",
        next.code, next.name, next.released_at
    );
    add_set_inner(&next.code)
}

fn refresh_inner(sets: &[String]) -> Result<()> {
    let client = ScryfallClient::new()?;
    for code in sets {
        let cards = client
            .refresh_set(code)
            .with_context(|| format!("refresh {code}"))?;
        println!("Refreshed {}: {} cards.", code, cards.len());
    }
    Ok(())
}

fn build_corpus(client: &ScryfallClient, sets: &[String]) -> Result<CorpusReport> {
    let mut all = Vec::new();
    for code in sets {
        let cards = client
            .cards_in_set(code)
            .with_context(|| format!("fetch set {code}"))?;
        all.extend(cards);
    }
    Ok(build_report(all))
}

fn tracked_sets() -> Result<Vec<String>> {
    let path = corpus_sets_path();
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let sets: Vec<String> =
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    if sets.is_empty() {
        bail!("{} must contain at least one set code", path.display());
    }
    Ok(sets.into_iter().map(|s| s.to_lowercase()).collect())
}

fn save_tracked_sets(sets: &[String]) -> Result<()> {
    let path = corpus_sets_path();
    let mut text = serde_json::to_string_pretty(sets).context("serialize corpus set list")?;
    text.push('\n');
    std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))
}

fn actionable_failures_for_set(set: &str) -> Result<usize> {
    let report = load(&corpus_status_path())?;
    let prefix = format!("{set}/");
    Ok(report
        .cards
        .iter()
        .filter(|(key, outcome)| {
            key.starts_with(&prefix)
                && matches!(
                    outcome,
                    mtg_corpus::CardOutcome::Fail { error }
                        if !error.starts_with("empty oracle text")
                )
        })
        .count())
}

fn parse_set(args: &[String]) -> Option<String> {
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--set" => return iter.next().cloned(),
            s if s.starts_with("--set=") => return Some(s["--set=".len()..].to_string()),
            _ => {}
        }
    }
    None
}

#[derive(Debug, Clone, Copy)]
struct AdvanceOptions {
    max_grammar_left: usize,
}

fn parse_advance_options(args: &[String]) -> Result<AdvanceOptions> {
    let mut max_grammar_left = 0usize;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--max-grammar-left" => {
                let Some(value) = iter.next() else {
                    bail!("--max-grammar-left requires a value");
                };
                max_grammar_left = value
                    .parse()
                    .with_context(|| format!("parse --max-grammar-left {value:?}"))?;
            }
            s if s.starts_with("--max-grammar-left=") => {
                let value = &s["--max-grammar-left=".len()..];
                max_grammar_left = value
                    .parse()
                    .with_context(|| format!("parse --max-grammar-left {value:?}"))?;
            }
            "-h" | "--help" => {
                bail!("usage: cargo xtask corpus-advance [--max-grammar-left N]");
            }
            other => bail!("unknown corpus-advance argument: {other}"),
        }
    }
    Ok(AdvanceOptions { max_grammar_left })
}
