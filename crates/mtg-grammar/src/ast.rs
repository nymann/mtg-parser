use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Statement {
    ManaCost(ManaCost),
    /// "Cast this spell only <restriction>."
    CastRestriction(CastRestriction),
    /// "Counter target spell."
    CounterTargetSpell,
    DestroyTargetCreature,
    /// "Destroy all <permanent_type>s."
    DestroyAll {
        permanent_type: PermanentType,
    },
    Keyword(Keyword),
    TargetPlayerDrawsCards {
        count: CardCount,
    },
    /// "Add <mana>."
    AddMana {
        mana: ManaCost,
    },
    /// "Remove this card from your deck before playing if you're not
    /// playing for ante."
    AntePlayRestriction,
    /// "You own target card in the <zone>."
    YouOwnTargetCardInZone {
        zone: Zone,
    },
    /// "Exchange that card with the top card of your library."
    ExchangeThatCardWithTopCardOfYourLibrary,
    /// "<action>, [<action>, ]then <action>."
    ImperativeActionSequence {
        actions: Vec<ImperativeAction>,
    },
    /// "Until end of turn, <timing>, you may <cost>."
    UntilEndOfTurnYouMayPayCostAtTiming {
        timing: ActionTiming,
        cost: OptionalCost,
    },
    /// "If you do, add <mana>."
    IfYouDoAddMana {
        mana: ManaCost,
    },
    /// "If you do, you gain N life."
    IfYouDoGainLife {
        amount: u32,
    },
    /// "Target spell or permanent becomes <color>."
    TargetSpellOrPermanentBecomesColor {
        color: Color,
    },
    /// "Target <type> gains <keyword> and gets <modifier> until end of
    /// turn, where ..."
    TargetPermanentGainsKeywordAndGetsUntilEndOfTurn {
        permanent_type: PermanentType,
        keyword: Keyword,
        modifier: MixedPtModifier,
        definitions: Vec<VariableDefinition>,
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
    /// "As this <permanent_type> enters, choose an opponent."
    AsThisPermanentEntersChooseOpponent {
        permanent_type: PermanentType,
    },
    /// "This <permanent_type> enters with N <pt_modifier> counters on it."
    ThisPermanentEntersWithCounters {
        source: SourceObject,
        amount: u32,
        counter: PtModifier,
    },
    /// "This ability can't cause the total number of <pt_modifier>
    /// counters on this <permanent_type> to be greater than N."
    ThisAbilityCantCauseTotalCountersGreaterThan {
        counter: PtModifier,
        source: SourceObject,
        maximum: u32,
    },
    /// "Activate only during your upkeep."
    ActivateOnlyDuringYourUpkeep,
    /// "Choose one —" followed by one or more bullet-pointed modes.
    ModalChoice {
        modes: Vec<ModalMode>,
    },
    StaticAbility(StaticAbility),
    ActivatedAbility(ActivatedAbility),
    TriggeredAbility(TriggeredAbility),
    /// Physical dexterity instructions and their conditional results.
    PhysicalAction(PhysicalAction),
    /// Two or more abilities printed on one card, in source order,
    /// separated by newlines on the printed face. A single-ability
    /// card is never wrapped in `Compound`, so each piece of card
    /// text has exactly one canonical AST.
    Compound(Vec<Statement>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModalMode {
    /// "Counter target <color> spell."
    CounterTargetColoredSpell { color: Color },
    /// "Destroy target <color> permanent."
    DestroyTargetColoredPermanent { color: Color },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CastRestriction {
    /// "before the <step> step"
    BeforeStep { step: Step },
    /// "during your <step> step"
    DuringYourStep { step: Step },
    /// "during combat before blockers are declared"
    DuringCombatBeforeBlockersAreDeclared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Step {
    CombatDamage,
    DeclareAttackers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BalanceSameWayAction {
    /// "discard cards"
    DiscardCards,
    /// "sacrifice <permanent_type>s"
    SacrificePermanents { permanent_type: PermanentType },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImperativeAction {
    /// "discard your hand"
    DiscardYourHand,
    /// "ante the top card of your library"
    AnteTopCardOfYourLibrary,
    /// "draw N cards"
    DrawCards { count: CardCount },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardCount {
    Number(u32),
    Variable(Variable),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionTiming {
    /// "any time you could activate a mana ability"
    AnyTimeYouCouldActivateAManaAbility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptionalCost {
    /// "pay N life"
    PayLife { amount: u32 },
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
    /// "a player casts a/an <color> spell"
    PlayerCastsColoredSpell { color: Color },
    /// "enchanted <permanent_type> dies"
    EnchantedPermanentDies { permanent_type: PermanentType },
    /// "the beginning of the next end step"
    BeginningOfTheNextEndStep,
    /// "the beginning of the chosen player's upkeep"
    BeginningOfChosenPlayersUpkeep,
    /// "the beginning of each player's upkeep"
    BeginningOfEachPlayersUpkeep,
    /// "the beginning of your upkeep"
    BeginningOfYourUpkeep,
    /// "this <source> is put into a graveyard from the battlefield"
    SourcePutIntoGraveyardFromBattlefield { source: SourceObject },
    /// "the beginning of the upkeep of enchanted <permanent_type>'s
    /// controller"
    BeginningOfUpkeepOfEnchantedPermanentController { permanent_type: PermanentType },
    /// "end of combat"
    EndOfCombat,
    /// "this <source> blocks or becomes blocked by a non-<creature_type>
    /// creature"
    SourceBlocksOrBecomesBlockedByNonCreatureTypeCreature {
        source: SourceObject,
        excluded_type: CreatureType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterveningIf {
    /// "if it's on the battlefield"
    ItsOnTheBattlefield,
    /// "if this <source> attacked or blocked this combat"
    SourceAttackedOrBlockedThisCombat { source: SourceObject },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerEffect {
    /// "destroy that creature if it attacked this turn"
    DestroyThatCreatureIfItAttackedThisTurn,
    /// "destroy that creature at end of combat"
    DestroyThatCreatureAtEndOfCombat,
    /// "that creature's controller sacrifices it"
    ThatCreaturesControllerSacrificesIt,
    /// "this <source> deals N damage to that <recipient>'s controller"
    SourceDealsDamageToThatPermanentController {
        source: SourceObject,
        amount: u32,
        recipient: PermanentType,
    },
    /// "this <source> deals N damage to that player"
    SourceDealsDamageToThatPlayer { source: SourceObject, amount: u32 },
    /// "this <source> deals X damage to that player, where X is <expr>"
    SourceDealsVariableDamageToThatPlayer {
        source: SourceObject,
        amount: Variable,
        definitions: Vec<VariableDefinition>,
    },
    /// "this <source> deals damage equal to that <permanent_type>'s
    /// toughness to the <permanent_type>'s controller"
    SourceDealsDamageEqualToThatPermanentsToughnessToThePermanentsController {
        source: SourceObject,
        permanent_type: PermanentType,
    },
    /// "remove a <pt_modifier> counter from it"
    RemoveCounterFromIt { counter: PtModifier },
    /// `it loses "<keyword>" and gains "<keyword>"` — the source object
    /// rewrites its own printed rules text. Reanimator Auras use this
    /// to switch their `Enchant` target from a graveyard card to the
    /// battlefield permanent they just reanimated.
    LosesAndGainsKeyword { loses: Keyword, gains: Keyword },
    /// "Return enchanted <type> card to the battlefield under your
    /// control and attach this Aura to it" — pulls the enchanted card
    /// out of its zone and re-attaches the Aura on the battlefield.
    ReturnEnchantedCardAndAttach { card_type: PermanentType },
    /// "sacrifice this <source> unless you pay <mana_cost>"
    SacrificeSourceUnlessYouPay {
        source: SourceObject,
        cost: ManaCost,
    },
    /// "you may pay <mana_cost>"
    YouMayPayMana { cost: ManaCost },
    /// "at the beginning of each of your upkeeps for the rest of the
    /// game, remove all <counter> counters from a <type> that ..."
    DelayedRemoveAllNamedCountersFromLinkedPermanent {
        counter_name: String,
        permanent_type: PermanentType,
        source: SourceObject,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceObject {
    /// "this <permanent_type>"
    This(PermanentType),
    /// "this Aura"
    ThisAura,
}

/// "<cost>: <effect>." — an activated ability with explicit printed
/// costs before the colon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivatedAbility {
    pub costs: Vec<ActivatedCost>,
    pub effect: ActivatedEffect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivatedCost {
    Mana(ManaCost),
    VariableMana(Variable),
    Tap,
    /// "Sacrifice this <permanent_type>"
    Sacrifice(SourceObject),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivatedEffect {
    /// "Add <mana>."
    AddMana(ManaCost),
    /// "Add one mana of any color."
    AddOneManaOfAnyColor,
    /// "Add N mana of any one color."
    AddManaOfAnyOneColor {
        amount: u32,
    },
    /// "Untap this <permanent_type>."
    Untap(SourceObject),
    /// "Counter target <color> spell."
    CounterTargetColoredSpell {
        color: Color,
    },
    /// "Enchanted <type> gets <modifier> until end of turn."
    EnchantedGetsUntilEndOfTurn {
        permanent_type: PermanentType,
        modifier: PtModifier,
    },
    /// "The next time a/an <color> source of your choice would deal
    /// damage to you this turn, prevent that damage."
    PreventNextDamageFromColoredSource {
        color: Color,
    },
    /// "Prevent the next N damage that would be dealt to you this turn."
    PreventNextDamageToYouThisTurn {
        amount: u32,
    },
    /// "Put up to X <pt_modifier> counters on this <source>."
    PutUpToVariableCountersOnSource {
        amount: Variable,
        counter: PtModifier,
        source: SourceObject,
    },
    /// "Put a <counter> counter on target non-<basic_land_type> land."
    PutNamedCounterOnTargetNonBasicLand {
        counter_name: String,
        excluded_land_type: BasicLandType,
    },
    PhysicalAction(PhysicalAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhysicalAction {
    /// "If this <source> is on the battlefield, flip it onto the
    /// battlefield from a height of at least N foot/feet."
    IfSourceOnBattlefieldFlipOntoBattlefieldFromHeight {
        source: SourceObject,
        minimum_height_feet: u32,
    },
    /// "If this <source> turns over completely at least once during the
    /// flip, destroy all nontoken permanents it touches."
    IfSourceTurnsOverCompletelyAtLeastOnceDuringFlipDestroyAllNontokenPermanentsItTouches {
        source: SourceObject,
    },
    /// "Then destroy this <source>."
    ThenDestroySource { source: SourceObject },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Keyword {
    FirstStrike,
    Flying,
    Defender,
    Banding,
    Trample,
    Mountainwalk,
    Swampwalk,
    Indestructible,
    Protection(Color),
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
    Ante,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreatureStatus {
    Tapped,
    Untapped,
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
    /// "<status> creatures you control get <modifier>." — P/T modifier
    /// on controlled creatures matching a tapped/untapped state.
    StatusCreaturesYouControlGet {
        status: CreatureStatus,
        modifier: PtModifier,
    },
    /// "Enchanted <type> gets +X/+Y, where X is <expr>, and Y is <expr>."
    /// — P/T modifier whose printed variables are defined inline.
    EnchantedGetsWithDefinitions {
        permanent_type: PermanentType,
        modifier: VariablePtModifier,
        definitions: Vec<VariableDefinition>,
    },
    /// "Enchanted <object> has <keyword>." — keyword-granting effect on
    /// the enchanted object.
    EnchantedHasKeyword {
        object: EnchantedObject,
        keyword: Keyword,
    },
    /// "Enchanted <object> has <keyword> and can't be enchanted by other
    /// Auras." — keyword-granting effect plus an Aura attachment
    /// restriction on the enchanted object.
    EnchantedHasKeywordAndCantBeEnchantedByOtherAuras {
        object: EnchantedObject,
        keyword: Keyword,
    },
    /// "Enchanted <object> can attack as though it didn't have
    /// <keyword>." — permission effect that ignores an attacking
    /// restriction such as defender.
    EnchantedCanAttackAsThoughItDidntHave {
        object: EnchantedObject,
        keyword: Keyword,
    },
    /// "You control enchanted <object>." — continuous control-changing
    /// effect from an Aura to the object it enchants.
    YouControlEnchanted { object: EnchantedObject },
    /// "You may have this <source> enter as a copy of any
    /// <permanent_type> on the battlefield[, except it's a/an
    /// <permanent_type> in addition to its other types]."
    YouMayHaveSourceEnterAsCopyOfAnyPermanentOnBattlefield {
        source: SourceObject,
        permanent_type: PermanentType,
        exception: Option<CopyException>,
    },
    /// "This effect doesn't remove this Aura." — effect-continuity text
    /// for Aura effects that otherwise might remove their own attachment.
    EffectDoesntRemoveThisAura,
    /// "This <permanent_type> doesn't untap during your untap step."
    SourceDoesntUntapDuringYourUntapStep { source: SourceObject },
    /// "All <basic_land_type>s are <basic_land_type>s."
    BasicLandsAreBasicLands {
        from: BasicLandType,
        to: BasicLandType,
    },
    /// "That <type> is a <basic_land_type> for as long as it has a
    /// <counter> counter on it."
    ThatPermanentIsBasicLandTypeWhileHasNamedCounter {
        permanent_type: PermanentType,
        land_type: BasicLandType,
        counter_name: String,
    },
    /// "Target creature defending player controls can block any number
    /// of creatures this turn."
    TargetCreatureDefendingPlayerControlsCanBlockAnyNumberOfCreaturesThisTurn,
    /// "It blocks each attacking creature this turn if able."
    ItBlocksEachAttackingCreatureThisTurnIfAble,
    /// "This turn, instead of declaring blockers, each defending player
    /// chooses ... and divides them into ... piles ..."
    ThisTurnDefendingPlayersMakeRandomBlockingPiles,
    /// "Creatures those players control that can block additional
    /// creatures may likewise be put into additional piles."
    AdditionalBlockersMayBePutIntoAdditionalPiles,
    /// "Assign each pile to a different one of those attacking creatures
    /// at random."
    AssignEachPileToAttackingCreatureAtRandom,
    /// "Each creature in a pile that can block the creature that pile is
    /// assigned to does so."
    CreaturesInAssignedPileBlockIfAble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CopyException {
    /// "except it's a/an <permanent_type> in addition to its other types"
    PermanentTypeInAdditionToItsOtherTypes { permanent_type: PermanentType },
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
pub struct MixedPtModifier {
    pub power: SignedPtComponent,
    pub toughness: SignedPtComponent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignedPtComponent {
    Number(SignedNumber),
    Variable(SignedVariable),
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
    /// "its power"
    ItsPower,
    /// "the number of cards in their hand minus <N>"
    NumberOfCardsInTheirHandMinus { amount: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BasicLandType {
    Plains,
    Island,
    Swamp,
    Mountain,
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
