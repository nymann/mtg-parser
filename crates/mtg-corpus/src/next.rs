use anyhow::Result;

use mtg_scryfall::{Card, Layout, ScryfallClient};

use crate::check::round_trip;
use crate::normalize::normalize_oracle_text;

/// Outcome of walking a set looking for the first card the grammar
/// can't handle.
pub enum NextCard {
    /// Every card in the set already passes the round-trip assertion.
    AllPass,
    /// `card` is the first failing card; `reason` is the round-trip
    /// error or layout-skip message; `normalized` is the text the
    /// generated test should feed into the parser.
    Failing {
        card: Card,
        reason: String,
        normalized: String,
    },
}

/// Walk a Scryfall set in `order=name` and return the first card that
/// fails the round-trip assertion. Cards with unsupported layouts or
/// empty oracle text are skipped — the corpus report still records
/// them as failures, but `next-card` is for grammar-extension work and
/// those skipped cards need different fixes (multi-face support, etc.).
pub fn find_next_failing_card(client: &ScryfallClient, set_code: &str) -> Result<NextCard> {
    let cards = client.cards_in_set(set_code)?;
    for card in cards {
        if card.layout != Layout::Normal {
            continue;
        }
        let normalized = normalize_oracle_text(&card.oracle_text);
        if normalized.is_empty() {
            continue;
        }
        if let Err(reason) = round_trip(&normalized) {
            return Ok(NextCard::Failing {
                card,
                reason,
                normalized,
            });
        }
    }
    Ok(NextCard::AllPass)
}
