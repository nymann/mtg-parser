use std::process::ExitCode;

use anyhow::{Context, Result};

use mtg_corpus::{build_report, diff, load, save, CorpusReport};
use mtg_scryfall::ScryfallClient;

use crate::paths::corpus_status_path;

/// Sets included in the corpus. Starts narrow (only Alpha) and grows
/// as the grammar can handle more.
pub const CORPUS_SETS: &[&str] = &["lea"];

pub fn run(args: &[String]) -> ExitCode {
    let force_update = args.iter().any(|a| a == "--update");
    match run_inner(force_update) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

pub fn refresh(args: &[String]) -> ExitCode {
    let single = parse_set(args);
    let sets: Vec<String> = if let Some(s) = single {
        vec![s]
    } else {
        CORPUS_SETS.iter().map(|s| (*s).to_string()).collect()
    };
    match refresh_inner(&sets) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run_inner(force_update: bool) -> Result<ExitCode> {
    let client = ScryfallClient::new()?;
    let new_report = build_corpus(&client, CORPUS_SETS)?;
    let path = corpus_status_path();

    if !path.exists() {
        save(&new_report, &path)?;
        println!(
            "Wrote initial corpus_status.json: {} passing of {} ({} sets).",
            new_report.passing,
            new_report.total,
            CORPUS_SETS.len(),
        );
        return Ok(ExitCode::SUCCESS);
    }

    let old = load(&path)?;
    let d = diff(&old, &new_report);

    println!(
        "Corpus over {} card(s) in {:?}:",
        new_report.total, CORPUS_SETS
    );
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
        return Ok(ExitCode::FAILURE);
    }

    save(&new_report, &path)?;
    println!("Updated {}.", path.display());
    Ok(ExitCode::SUCCESS)
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

fn build_corpus(client: &ScryfallClient, sets: &[&str]) -> Result<CorpusReport> {
    let mut all = Vec::new();
    for &code in sets {
        let cards = client
            .cards_in_set(code)
            .with_context(|| format!("fetch set {code}"))?;
        all.extend(cards);
    }
    Ok(build_report(all))
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
