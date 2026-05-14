use mtg_grammar::{parse, unparse};
use mtg_scryfall::{Card, Layout};

use crate::normalize::normalize_oracle_text;
use crate::report::CardOutcome;

/// Check one card against the parser. The outcome is the result of the
/// round-trip assertion `parse(unparse(parse(text))) == parse(text)`.
///
/// Cards whose `layout` is anything but `Normal` are reported as
/// failures with the layout as the reason. Multi-face support will lift
/// them into passing rows once the grammar grows to handle them.
pub fn check_card(card: &Card) -> CardOutcome {
    if card.layout != Layout::Normal {
        return CardOutcome::Fail {
            error: format!("unsupported layout: {:?}", card.layout),
        };
    }
    let text = normalize_oracle_text(&card.oracle_text);
    if text.is_empty() {
        return CardOutcome::Fail {
            error: "empty oracle text (vanilla card)".into(),
        };
    }
    match round_trip(&text) {
        Ok(()) => CardOutcome::Pass,
        Err(reason) => CardOutcome::Fail { error: reason },
    }
}

pub fn round_trip(text: &str) -> Result<(), String> {
    let ast = parse(text).map_err(|e| format!("parse: {e}"))?;
    let reprinted = unparse(&ast);
    let ast2 = parse(&reprinted).map_err(|e| format!("reparse {reprinted:?}: {e}"))?;
    if ast != ast2 {
        return Err(format!(
            "round-trip mismatch: {ast:?} unparsed to {reprinted:?} which parsed to {ast2:?}"
        ));
    }
    Ok(())
}
