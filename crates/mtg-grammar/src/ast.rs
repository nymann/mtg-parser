use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Statement {
    ManaCost(ManaCost),
    DestroyTargetCreature,
    Keyword(Keyword),
    TargetPlayerDrawsCards { count: u32 },
    StaticAbility(StaticAbility),
    /// Two or more abilities printed on one card, in source order,
    /// separated by newlines on the printed face. A single-ability
    /// card is never wrapped in `Compound`, so each piece of card
    /// text has exactly one canonical AST.
    Compound(Vec<Statement>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Keyword {
    Flying,
    Enchant(PermanentType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermanentType {
    Artifact,
    Creature,
    Enchantment,
    Land,
    Planeswalker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManaCost {
    pub symbols: Vec<ManaSymbol>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManaSymbol {
    Generic(u32),
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

/// A static ability of the form "As long as <cond>, <effect>." — a
/// continuous effect that's only active while the condition holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticAbility {
    pub condition: Condition,
    pub effect: ContinuousEffect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Condition {
    /// "enchanted <permanent_type> isn't a/an <negated_type>"
    EnchantedIsNot {
        permanent_type: PermanentType,
        negated_type: PermanentType,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContinuousEffect {
    /// "it's a/an <types> with power and toughness each equal to its
    /// mana value" — the enchanted permanent gains the listed types
    /// and a characteristic-defining P/T equal to its mana value.
    BecomesWithPtFromManaValue { types: Vec<PermanentType> },
}
