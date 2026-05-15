use mtg_grammar::Statement;
use serde::{Deserialize, Serialize};

/// Semantic IR for one Oracle-text effect. The grammar's syntactic
/// `Statement` is lowered into one of these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardEffect {
    /// Normalized mana cost. Generic mana is summed across pips, each
    /// color has a dedicated counter, so `{1}{1}{R}` lowers identically
    /// to `{2}{R}`.
    ManaCost(ManaValue),
    /// A grammar statement that has no semantic normalization yet.
    ///
    /// Keeping these in one wrapper avoids a parallel enum that needs a
    /// new variant for every syntax-only grammar addition. Promote a
    /// statement out of this wrapper only when lowering changes its
    /// meaning or validates something the grammar cannot.
    Syntactic(Statement),
    /// Two or more lowered effects, in source order — the lowering of
    /// a multi-ability card.
    Compound(Vec<CardEffect>),
}

/// Per-color mana totals. The "mana value" of a cost is [`Self::total`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManaValue {
    pub generic: u32,
    pub white: u32,
    pub blue: u32,
    pub black: u32,
    pub red: u32,
    pub green: u32,
    pub colorless: u32,
}

impl ManaValue {
    pub fn total(&self) -> u32 {
        self.generic + self.white + self.blue + self.black + self.red + self.green + self.colorless
    }
}
