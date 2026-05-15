use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Statement {
    ManaCost(ManaCost),
    DestroyTargetCreature,
    /// "Destroy all <permanent_type>s."
    DestroyAll {
        permanent_type: PermanentType,
    },
    Keyword(Keyword),
    TargetPlayerDrawsCards {
        count: u32,
    },
    /// "Each player chooses a number of <permanent_type>s they control
    /// equal to the number of <permanent_type>s controlled by the player
    /// who controls the fewest, then sacrifices the rest."
    EachPlayerEqualizesControlledPermanents {
        permanent_type: PermanentType,
    },
    /// "Players <action>[ and <action>]* the same way."
    PlayersDoActionsTheSameWay {
        actions: Vec<BalanceSameWayAction>,
    },
    StaticAbility(StaticAbility),
    TriggeredAbility(TriggeredAbility),
    /// Two or more abilities printed on one card, in source order,
    /// separated by newlines on the printed face. A single-ability
    /// card is never wrapped in `Compound`, so each piece of card
    /// text has exactly one canonical AST.
    Compound(Vec<Statement>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BalanceSameWayAction {
    /// "discard cards"
    DiscardCards,
    /// "sacrifice <permanent_type>s"
    SacrificePermanents { permanent_type: PermanentType },
}

/// "When/Whenever <event>, [if <intervening-if>,] <effect>[. <effect>]*."
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
    /// "a/an <permanent_type> enters"
    PermanentEnters { permanent_type: PermanentType },
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
    /// "this <source> deals N damage to that <recipient>'s controller"
    SourceDealsDamageToThatPermanentController {
        source: SourceObject,
        amount: u32,
        recipient: PermanentType,
    },
    /// `it loses "<keyword>" and gains "<keyword>"` — the source object
    /// rewrites its own printed rules text. Reanimator Auras use this
    /// to switch their `Enchant` target from a graveyard card to the
    /// battlefield permanent they just reanimated.
    LosesAndGainsKeyword { loses: Keyword, gains: Keyword },
    /// "Return enchanted <type> card to the battlefield under your
    /// control and attach this Aura to it" — pulls the enchanted card
    /// out of its zone and re-attaches the Aura on the battlefield.
    ReturnEnchantedCardAndAttach { card_type: PermanentType },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceObject {
    /// "this <permanent_type>"
    This(PermanentType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Keyword {
    Flying,
    Defender,
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
    CreatureType(CreatureType),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    White,
    Blue,
    Black,
    Red,
    Green,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreatureType {
    Wall,
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

/// A static ability printed on a permanent. This covers conditional
/// continuous effects, P/T modifiers on matching objects or enchanted
/// objects, and permission effects that let an enchanted object attack
/// through a keyword restriction such as defender.
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
    /// "<color> <permanent_type>s get <modifier>." — P/T modifier on
    /// every permanent matching the color and type filters.
    ColoredPermanentsGet {
        color: Color,
        permanent_type: PermanentType,
        modifier: PtModifier,
    },
    /// "Enchanted <type> gets +X/+Y, where X is <expr>, and Y is <expr>."
    /// — P/T modifier whose printed variables are defined inline.
    EnchantedGetsWithDefinitions {
        permanent_type: PermanentType,
        modifier: VariablePtModifier,
        definitions: Vec<VariableDefinition>,
    },
    /// "Enchanted <object> can attack as though it didn't have
    /// <keyword>." — permission effect that ignores an attacking
    /// restriction such as defender.
    EnchantedCanAttackAsThoughItDidntHave {
        object: EnchantedObject,
        keyword: Keyword,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnchantedObject {
    Permanent(PermanentType),
    CreatureType(CreatureType),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariablePtModifier {
    pub power: SignedVariable,
    pub toughness: SignedVariable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedVariable {
    pub sign: Sign,
    pub variable: Variable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Variable {
    X,
    Y,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableDefinition {
    pub variable: Variable,
    pub value: ValueExpression,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueExpression {
    /// "half the number of <basic_land_type>s you control, rounded <...>"
    HalfNumberOfBasicLandsYouControl {
        basic_land_type: BasicLandType,
        rounding: Rounding,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BasicLandType {
    Forest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rounding {
    Down,
    Up,
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
