use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Statement {
    ManaCost(ManaCost),
    DestroyTargetCreature,
    Keyword(Keyword),
    TargetPlayerDrawsCards {
        count: u32,
    },
    StaticAbility(StaticAbility),
    TriggeredAbility(TriggeredAbility),
    /// Two or more abilities printed on one card, in source order,
    /// separated by newlines on the printed face. A single-ability
    /// card is never wrapped in `Compound`, so each piece of card
    /// text has exactly one canonical AST.
    Compound(Vec<Statement>),
}

/// "When <event>, [if <intervening-if>,] <effect>[. <effect>]*."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggeredAbility {
    pub event: TriggerEvent,
    pub intervening_if: Option<InterveningIf>,
    pub effects: Vec<TriggerEffect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerEvent {
    /// "this Aura enters"
    ThisAuraEnters,
    /// "this Aura leaves the battlefield"
    ThisAuraLeavesTheBattlefield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterveningIf {
    /// "if it's on the battlefield"
    ItsOnTheBattlefield,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerEffect {
    /// "that creature's controller sacrifices it"
    ThatCreaturesControllerSacrificesIt,
    /// `it loses "<keyword>" and gains "<keyword>"` — the source object
    /// rewrites its own printed rules text. Reanimator Auras use this
    /// to switch their `Enchant` target from a graveyard card to the
    /// battlefield permanent they just reanimated.
    LosesAndGainsKeyword {
        loses: Keyword,
        gains: Keyword,
    },
    /// "Return enchanted <type> card to the battlefield under your
    /// control and attach this Aura to it" — pulls the enchanted card
    /// out of its zone and re-attaches the Aura on the battlefield.
    ReturnEnchantedCardAndAttach {
        card_type: PermanentType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Keyword {
    Flying,
    Enchant(EnchantObject),
}

/// What an `Enchant <X>` keyword attaches to. Most Auras name a
/// permanent type; reanimator Auras (Animate Dead, Dance of the Dead,
/// Necromancy) name a card type plus the zone the card lives in. The
/// `PutOntoBattlefieldByThisAura` form only appears inside the quoted
/// rules-text-replacement on the same reanimator Auras.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnchantObject {
    Permanent(PermanentType),
    CardInZone {
        card_type: PermanentType,
        zone: Zone,
    },
    /// "Enchant creature put onto the battlefield with this Aura."
    PutOntoBattlefieldByThisAura {
        card_type: PermanentType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Zone {
    Graveyard,
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

/// A static ability printed on a permanent. Two surface shapes today:
/// the conditional "As long as <cond>, <effect>." form and the
/// unconditional "Enchanted <type> gets <modifier>." P/T modifier form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaticAbility {
    /// "As long as <cond>, <effect>." — continuous effect gated on a
    /// condition.
    Conditional {
        condition: Condition,
        effect: ContinuousEffect,
    },
    /// "Enchanted <type> gets <modifier>." — P/T modifier on the
    /// enchanted permanent, active while the Aura is attached.
    EnchantedGets {
        permanent_type: PermanentType,
        modifier: PtModifier,
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PtModifier {
    pub power: SignedNumber,
    pub toughness: SignedNumber,
}

/// Magnitude plus an explicit printed sign. Magic prints "-0" and "+0"
/// distinctly to convey buff-vs-debuff intent (Animate Dead: "-1/-0").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedNumber {
    pub sign: Sign,
    pub magnitude: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sign {
    Plus,
    Minus,
}
