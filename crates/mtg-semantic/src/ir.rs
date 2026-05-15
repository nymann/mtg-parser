use mtg_grammar::{Keyword, StaticAbility, TriggeredAbility};
use serde::{Deserialize, Serialize};

/// Semantic IR for one Oracle-text effect. The grammar's syntactic
/// `Statement` is lowered into one of these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardEffect {
    /// Normalized mana cost. Generic mana is summed across pips, each
    /// color has a dedicated counter, so `{1}{1}{R}` lowers identically
    /// to `{2}{R}`.
    ManaCost(ManaValue),
    /// "Destroy target creature."
    DestroyTargetCreature,
    /// A single keyword ability such as `Flying` or `Enchant artifact`.
    Keyword(Keyword),
    /// "Target player draws N cards."
    TargetPlayerDrawsCards { count: u32 },
    /// A static ability with a conditional continuous effect. The
    /// grammar-side AST is reused verbatim until the IR grows real
    /// reference-resolution work to do here.
    StaticAbility(StaticAbility),
    /// A triggered ability ("When <event>, ..."). Reused from the
    /// grammar AST until the IR needs to resolve `this Aura`,
    /// `that creature` and similar references.
    TriggeredAbility(TriggeredAbility),
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
