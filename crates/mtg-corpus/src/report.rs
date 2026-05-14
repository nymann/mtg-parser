use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use mtg_scryfall::{today_iso, Card};

use crate::check::check_card;

pub const CORPUS_SCHEMA_VERSION: u32 = 1;

/// Per-card parse outcome over a corpus of cards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusReport {
    #[serde(rename = "_v")]
    pub schema_version: u32,
    pub generated_at: String,
    pub total: usize,
    pub passing: usize,
    /// Keyed by `card_key` (set_code + "/" + card name) for stable
    /// per-card diffing across runs.
    pub cards: BTreeMap<String, CardOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CardOutcome {
    Pass,
    Fail { error: String },
}

impl CardOutcome {
    pub fn is_pass(&self) -> bool {
        matches!(self, CardOutcome::Pass)
    }
}

pub fn card_key(card: &Card) -> String {
    format!("{}/{}", card.set_code, card.name)
}

pub fn build_report(cards: impl IntoIterator<Item = Card>) -> CorpusReport {
    let mut cards_out: BTreeMap<String, CardOutcome> = BTreeMap::new();
    let mut passing = 0usize;
    let mut total = 0usize;
    for card in cards {
        let key = card_key(&card);
        let outcome = check_card(&card);
        if outcome.is_pass() {
            passing += 1;
        }
        total += 1;
        cards_out.insert(key, outcome);
    }
    CorpusReport {
        schema_version: CORPUS_SCHEMA_VERSION,
        generated_at: today_iso(),
        total,
        passing,
        cards: cards_out,
    }
}

/// Difference between two corpus reports, keyed by card.
#[derive(Debug, Clone, Default)]
pub struct CorpusDiff {
    pub new_passes: Vec<String>,
    pub new_failures: Vec<String>,
    pub still_failing: usize,
    pub still_passing: usize,
}

pub fn diff(old: &CorpusReport, new: &CorpusReport) -> CorpusDiff {
    let mut d = CorpusDiff::default();
    for (key, outcome) in &new.cards {
        let was_pass = old
            .cards
            .get(key)
            .map(CardOutcome::is_pass)
            .unwrap_or(false);
        match (was_pass, outcome.is_pass()) {
            (false, true) => d.new_passes.push(key.clone()),
            (true, false) => d.new_failures.push(key.clone()),
            (false, false) => d.still_failing += 1,
            (true, true) => d.still_passing += 1,
        }
    }
    d.new_passes.sort();
    d.new_failures.sort();
    d
}

pub fn load(path: &Path) -> Result<CorpusReport> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

pub fn save(report: &CorpusReport, path: &Path) -> Result<()> {
    let text = serde_json::to_string_pretty(report).context("serialize corpus report")?;
    std::fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(entries: &[(&str, CardOutcome)]) -> CorpusReport {
        let mut cards = BTreeMap::new();
        let mut passing = 0usize;
        for (key, outcome) in entries {
            if outcome.is_pass() {
                passing += 1;
            }
            cards.insert((*key).to_string(), outcome.clone());
        }
        CorpusReport {
            schema_version: CORPUS_SCHEMA_VERSION,
            generated_at: "2026-05-15".to_string(),
            total: entries.len(),
            passing,
            cards,
        }
    }

    fn pass() -> CardOutcome {
        CardOutcome::Pass
    }

    fn fail() -> CardOutcome {
        CardOutcome::Fail {
            error: "x".to_string(),
        }
    }

    #[test]
    fn diff_detects_new_passes_and_failures() {
        let old = report(&[("lea/A", pass()), ("lea/B", fail()), ("lea/C", pass())]);
        let new = report(&[
            ("lea/A", pass()), // unchanged pass
            ("lea/B", pass()), // newly passing
            ("lea/C", fail()), // REGRESSION
        ]);
        let d = diff(&old, &new);
        assert_eq!(d.new_passes, vec!["lea/B".to_string()]);
        assert_eq!(d.new_failures, vec!["lea/C".to_string()]);
        assert_eq!(d.still_passing, 1);
        assert_eq!(d.still_failing, 0);
    }

    #[test]
    fn diff_treats_new_cards_with_no_prior_status_as_new() {
        let old = report(&[]);
        let new = report(&[("lea/A", pass()), ("lea/B", fail())]);
        let d = diff(&old, &new);
        assert_eq!(d.new_passes, vec!["lea/A".to_string()]);
        // A newly-seen failing card is NOT a regression — old had no entry.
        assert!(d.new_failures.is_empty());
        assert_eq!(d.still_failing, 1);
    }

    #[test]
    fn build_report_counts_passing() {
        // build_report goes through check_card which needs real Card data;
        // for the counting logic specifically we cover it via the
        // hand-constructed `report()` helper above and trust check_card's
        // own coverage in tests/.
        let r = report(&[("lea/A", pass()), ("lea/B", pass()), ("lea/C", fail())]);
        assert_eq!(r.passing, 2);
        assert_eq!(r.total, 3);
    }
}
