//! Corpus harness: composes `mtg-scryfall` (data) with `mtg-grammar`
//! (parsing) to drive the find-next-card workflow and the full-corpus
//! regression check.
//!
//! The grammar/semantic crates do not depend on this; this is an
//! outer adapter.

mod check;
mod next;
mod normalize;
mod report;

pub use check::{check_card, round_trip};
pub use next::{find_next_failing_card, NextCard};
pub use normalize::normalize_oracle_text;
pub use report::{
    build_report, card_key, diff, load, save, CardOutcome, CorpusDiff, CorpusReport,
    CORPUS_SCHEMA_VERSION,
};
