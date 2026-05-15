use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Statement {
    ManaCost(ManaCost),
    /// "Cast this spell only <restriction>."
    CastRestriction(CastRestriction),
    /// "Counter target spell."
    CounterTargetSpell,
    /// "This spell costs <mana> more to cast for each target beyond the first."
    ThisSpellCostsManaMoreToCastForEachTargetBeyondTheFirst {
        mana: ManaCost,
    },
    DestroyTargetCreature,
    /// "Regenerate target creature."
    RegenerateTargetCreature,
    /// "<source name> deals X damage divided evenly, rounded down, among
    /// any number of targets."
    NamedSourceDealsVariableDamageDividedEvenlyRoundedDownAmongAnyNumberOfTargets {
        source_name: String,
        amount: Variable,
    },
    /// "<source name> deals X damage to any target."
    NamedSourceDealsVariableDamageToAnyTarget {
        source_name: String,
        amount: Variable,
    },
    /// "<source name> deals N damage to any target."
    NamedSourceDealsDamageToAnyTarget {
        source_name: String,
        amount: u32,
    },
    /// "<source name> deals X damage to <recipient> and <recipient>."
    NamedSourceDealsVariableDamageToDamageRecipients {
        source_name: String,
        amount: Variable,
        recipients: Vec<DamageRecipient>,
    },
    /// "Prevent all combat damage that would be dealt this turn."
    PreventAllCombatDamageThisTurn,
    /// "Spend only <color> mana on X."
    SpendOnlyColorManaOnVariable {
        color: Color,
        variable: Variable,
    },
    /// "As this <source> enters, you lose life equal to your life total."
    AsSourceEntersYouLoseLifeEqualToYourLifeTotal {
        source: SourceObject,
    },
    /// "You gain life equal to the damage dealt, but not more life than ..."
    YouGainLifeEqualToDamageDealtCapped {
        caps: Vec<DamageLifeGainCap>,
    },
    /// "If you can't, you lose the game."
    IfYouCantYouLoseTheGame,
    /// "If it's a <type>, it can't be regenerated this turn, and if it
    /// would die this turn, exile it instead."
    IfItsPermanentCantBeRegeneratedAndWouldDieExileInsteadThisTurn {
        permanent_type: PermanentType,
    },
    /// "Destroy target <permanent_type> or <permanent_type>."
    DestroyTargetPermanentChoice {
        permanent_types: Vec<PermanentType>,
    },
    /// "Destroy target <permanent_type>."
    DestroyTargetPermanent {
        permanent_type: PermanentType,
    },
    /// "That <permanent_type>'s controller may attach this Aura to a/an
    /// <permanent_type> of their choice."
    ThatPermanentsControllerMayAttachThisAuraToPermanentOfTheirChoice {
        controller_of: PermanentType,
        attach_to: PermanentType,
    },
    /// "Destroy all <permanent_type>s."
    DestroyAll {
        permanent_type: PermanentType,
    },
    /// "Destroy all <basic_land_type>s."
    DestroyAllBasicLands {
        basic_land_type: BasicLandType,
    },
    Keyword(Keyword),
    TargetPlayerDrawsCards {
        count: CardCount,
    },
    /// "If you would draw a card during your draw step, instead you may
    /// skip that draw."
    IfYouWouldDrawCardDuringYourDrawStepInsteadYouMaySkipThatDraw,
    /// "Target player gains N life."
    TargetPlayerGainsLife {
        amount: u32,
    },
    /// "Target player activates a mana ability of each <permanent_type>
    /// they control."
    TargetPlayerActivatesManaAbilityOfEachPermanentTheyControl {
        permanent_type: PermanentType,
    },
    /// "Then that player loses all unspent mana and you add the mana lost
    /// this way."
    ThenThatPlayerLosesUnspentManaAndYouAddManaLostThisWay,
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
    /// "Copy target <spell_type> [or <spell_type>] spell, except that the
    /// copy is <color>."
    CopyTargetSpellExceptCopyIsColor {
        spell_types: Vec<SpellType>,
        color: Color,
    },
    /// "You may choose new targets for the copy."
    YouMayChooseNewTargetsForTheCopy,
    /// "<action>, [<action>, ]then <action>."
    ImperativeActionSequence {
        actions: Vec<ImperativeAction>,
    },
    /// "Each player <action>."
    EachPlayerPerformsAction {
        action: EachPlayerAction,
    },
    /// "Until end of turn, <timing>, you may <cost>."
    UntilEndOfTurnYouMayPayCostAtTiming {
        timing: ActionTiming,
        cost: OptionalCost,
    },
    /// "Prevent the next N damage that would be dealt to <recipient> this turn."
    PreventNextDamageThatWouldBeDealtToRecipientThisTurn {
        amount: DamageAmount,
        recipient: PreventionRecipient,
    },
    /// "If you do, prevent the next N damage that would be dealt to <recipient> this turn."
    IfYouDoPreventNextDamageThatWouldBeDealtToRecipientThisTurn {
        amount: DamageAmount,
        recipient: PreventionRecipient,
    },
    /// "If you do, add <mana>."
    IfYouDoAddMana {
        mana: ManaCost,
    },
    /// "If you do, you gain N life."
    IfYouDoGainLife {
        amount: u32,
    },
    /// "If you do, until your next turn, you can't be attacked except
    /// by creatures with <keyword>[ and/or <keyword>]."
    IfYouDoUntilYourNextTurnYouCantBeAttackedExceptByCreaturesWithKeywords {
        keywords: Vec<Keyword>,
    },
    /// "If you do, you may cast that card face down as a N/N creature
    /// spell without paying its mana cost."
    IfYouDoCastThatCardFaceDownWithoutPayingManaCost {
        power: u32,
        toughness: u32,
    },
    /// Face-down creature-spell replacement effect that turns it face up
    /// before assigning/dealing damage, being dealt damage, or tapping.
    IfFaceDownSpellCreatureWouldAssignOrDealDamageOrTapTurnFaceUpInstead,
    /// "Target spell or permanent becomes <color>."
    TargetSpellOrPermanentBecomesColor {
        color: Color,
    },
    /// "Target <type> gets <modifier> until end of turn."
    TargetPermanentGetsUntilEndOfTurn {
        permanent_type: PermanentType,
        modifier: PtModifier,
    },
    /// "Target <type> gets <modifier> until end of turn.", where the
    /// modifier may contain a printed variable.
    TargetPermanentGetsMixedUntilEndOfTurn {
        permanent_type: PermanentType,
        modifier: MixedPtModifier,
    },
    /// "Target <type> gains <keyword> until end of turn."
    TargetPermanentGainsKeywordUntilEndOfTurn {
        permanent_type: PermanentType,
        keyword: Keyword,
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
    /// "If this ability has been activated N or more times this turn,
    /// sacrifice this <source> at the beginning of the next end step."
    IfThisAbilityActivatedAtLeastTimesThisTurnSacrificeSourceAtNextEndStep {
        threshold: u32,
        source: SourceObject,
    },
    /// "Activate only during your upkeep."
    ActivateOnlyDuringYourUpkeep,
    /// "Activate only during combat."
    ActivateOnlyDuringCombat,
    /// "Activate only during your turn."
    ActivateOnlyDuringYourTurn,
    /// "Activate only during your turn and only once each turn."
    ActivateOnlyDuringYourTurnAndOnlyOnceEachTurn,
    /// "Activate only as a sorcery."
    ActivateOnlyAsSorcery,
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
    /// "Target player gains N life."
    TargetPlayerGainsLife { amount: u32 },
    /// "Prevent the next N damage that would be dealt to <recipient> this turn."
    PreventNextDamageThatWouldBeDealtToRecipientThisTurn {
        amount: DamageAmount,
        recipient: PreventionRecipient,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageLifeGainCap {
    /// "the player's life total before the damage was dealt"
    PlayerLifeTotalBeforeDamageDealt,
    /// "the planeswalker's loyalty before the damage was dealt"
    PlaneswalkerLoyaltyBeforeDamageDealt,
    /// "the creature's toughness"
    CreatureToughness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageRecipient {
    /// "each creature with <keyword>"
    EachCreatureWithKeyword { keyword: Keyword },
    /// "each creature without <keyword>"
    EachCreatureWithoutKeyword { keyword: Keyword },
    /// "each player"
    EachPlayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageAmount {
    Number(u32),
    Variable(Variable),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreventionRecipient {
    /// "any target"
    AnyTarget,
    /// "that permanent or player"
    ThatPermanentOrPlayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CastRestriction {
    /// "before the <step> step"
    BeforeStep { step: Step },
    /// "during the <step> step"
    DuringStep { step: Step },
    /// "during your <step> step"
    DuringYourStep { step: Step },
    /// "during combat before blockers are declared"
    DuringCombatBeforeBlockersAreDeclared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Step {
    CombatDamage,
    DeclareAttackers,
    DeclareBlockers,
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
    /// "search your library for a card"
    SearchYourLibraryForACard,
    /// "put that card into your hand"
    PutThatCardIntoYourHand,
    /// "shuffle"
    Shuffle,
    /// "draw N cards"
    DrawCards { count: CardCount },
    /// "tap this <source>"
    TapSource { source: SourceObject },
    /// "sacrifice a/an <permanent_type> of an opponent's choice"
    SacrificePermanentOfOpponentsChoice { permanent_type: PermanentType },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EachPlayerAction {
    /// "antes the top card of their library"
    AnteTopCardOfTheirLibrary,
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
    /// "any time you could cast an instant"
    AnyTimeYouCouldCastAnInstant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptionalCost {
    /// "pay N life"
    PayLife { amount: u32 },
    /// "pay <mana>"
    PayMana { mana: ManaCost },
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
    /// "a/an <basic_land_type> is tapped for mana"
    BasicLandTypeIsTappedForMana { land_type: BasicLandType },
    /// "a/an <basic_land_type> <controller> becomes <status>"
    BasicLandTypeControllerBecomesStatus {
        land_type: BasicLandType,
        controller: PermanentController,
        status: ObjectStatus,
    },
    /// "you play a/an <permanent_type>"
    YouPlayPermanent { permanent_type: PermanentType },
    /// "enchanted <permanent_type> dies"
    EnchantedPermanentDies { permanent_type: PermanentType },
    /// "enchanted <object> becomes <status>"
    EnchantedObjectBecomesStatus {
        object: EnchantedObject,
        status: ObjectStatus,
    },
    /// "the beginning of the next end step"
    BeginningOfTheNextEndStep,
    /// "the beginning of the chosen player's upkeep"
    BeginningOfChosenPlayersUpkeep,
    /// "the beginning of each player's upkeep"
    BeginningOfEachPlayersUpkeep,
    /// "the beginning of each player's draw step"
    BeginningOfEachPlayersDrawStep,
    /// "the beginning of your upkeep"
    BeginningOfYourUpkeep,
    /// "this <source> is put into a graveyard from the battlefield"
    SourcePutIntoGraveyardFromBattlefield { source: SourceObject },
    /// "this <source> is dealt damage"
    SourceIsDealtDamage { source: SourceObject },
    /// "a/an <permanent_type> is put into a graveyard from the battlefield"
    PermanentPutIntoGraveyardFromBattlefield { permanent_type: PermanentType },
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
    /// "you're dealt damage"
    YouAreDealtDamage,
    /// "this <source> deals damage to an opponent"
    SourceDealsDamageToAnOpponent { source: SourceObject },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterveningIf {
    /// "if it's on the battlefield"
    ItsOnTheBattlefield,
    /// "if enchanted <object> has <keyword>"
    EnchantedHasKeyword {
        object: EnchantedObject,
        keyword: Keyword,
    },
    /// "if it wasn't the first <permanent_type> you played this turn"
    ItWasntFirstPermanentYouPlayedThisTurn { permanent_type: PermanentType },
    /// "if this <source> attacked or blocked this combat"
    SourceAttackedOrBlockedThisCombat { source: SourceObject },
    /// "if this <source> is <status>"
    SourceIsStatus {
        source: SourceObject,
        status: ObjectStatus,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermanentController {
    You,
    Opponent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerEffect {
    /// "destroy that creature if it attacked this turn"
    DestroyThatCreatureIfItAttackedThisTurn,
    /// "destroy it"
    DestroyIt,
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
    /// "this <source> deals N damage to you"
    SourceDealsDamageToYou { source: SourceObject, amount: u32 },
    /// "this <source> deals N damage to you unless you pay <mana_cost>"
    SourceDealsDamageToYouUnlessYouPay {
        source: SourceObject,
        amount: u32,
        cost: ManaCost,
    },
    /// "this <source> deals N damage to that <permanent_type>"
    SourceDealsDamageToThatPermanent {
        source: SourceObject,
        amount: u32,
        recipient: PermanentType,
    },
    /// "this <source> deals X damage to that player, where X is <expr>"
    SourceDealsVariableDamageToThatPlayer {
        source: SourceObject,
        amount: Variable,
        definitions: Vec<VariableDefinition>,
    },
    /// "that player draws an additional card"
    ThatPlayerDrawsAnAdditionalCard,
    /// "that player discards a card at random"
    ThatPlayerDiscardsCardAtRandom,
    /// "this <source> deals damage equal to that <permanent_type>'s
    /// toughness to the <permanent_type>'s controller"
    SourceDealsDamageEqualToThatPermanentsToughnessToThePermanentsController {
        source: SourceObject,
        permanent_type: PermanentType,
    },
    /// "this <source> deals damage to that player equal to the number of
    /// <basic_land_type>s they control"
    SourceDealsDamageEqualToNumberOfBasicLandsTheyControlToThatPlayer {
        source: SourceObject,
        basic_land_type: BasicLandType,
    },
    /// "remove a <pt_modifier> counter from it"
    RemoveCounterFromIt { counter: PtModifier },
    /// "put a <pt_modifier> counter on it"
    PutCounterOnIt { counter: PtModifier },
    /// "put that many <counter> counters on this <source>"
    PutThatManyNamedCountersOnSource {
        counter_name: String,
        source: SourceObject,
    },
    /// "you may remove a <counter> counter from this <source>"
    YouMayRemoveNamedCounterFromSource {
        counter_name: String,
        source: SourceObject,
    },
    /// "this <source> gains \"<static ability>\"" — the source object
    /// gains quoted rules text as part of a triggered ability.
    SourceGainsStaticAbility {
        source: SourceObject,
        ability: StaticAbility,
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
    /// "sacrifice this <source> unless you pay <mana_cost>"
    SacrificeSourceUnlessYouPay {
        source: SourceObject,
        cost: ManaCost,
    },
    /// "sacrifice that many nontoken permanents"
    SacrificeThatManyNontokenPermanents,
    /// "you lose the game"
    YouLoseTheGame,
    /// "you gain N life"
    YouGainLife { amount: u32 },
    /// "you may pay <mana_cost>"
    YouMayPayMana { cost: ManaCost },
    /// "If you do, you gain N life."
    IfYouDoGainLife { amount: u32 },
    /// "unless you pay <mana_cost>, <action>[ and <action>]*"
    UnlessYouPayManaDoActions {
        cost: ManaCost,
        actions: Vec<ImperativeAction>,
    },
    /// "at the beginning of each of your upkeeps for the rest of the
    /// game, remove all <counter> counters from a <type> that ..."
    DelayedRemoveAllNamedCountersFromLinkedPermanent {
        counter_name: String,
        permanent_type: PermanentType,
        source: SourceObject,
    },
    /// "its controller adds an additional <mana_symbol>"
    ItsControllerAddsAdditionalMana { mana: ManaSymbol },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceObject {
    /// "this <permanent_type>"
    This(PermanentType),
    /// "this Aura"
    ThisAura,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectStatus {
    Tapped,
    Untapped,
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
    /// "Tap target <permanent_type>, <permanent_type>, or <permanent_type>."
    TapTargetPermanentChoice {
        permanent_types: Vec<PermanentType>,
    },
    /// "Untap this <permanent_type>."
    Untap(SourceObject),
    /// "Untap target <permanent_type>."
    UntapTargetPermanent {
        permanent_type: PermanentType,
    },
    /// "Untap enchanted <object>."
    UntapEnchanted(EnchantedObject),
    /// "Regenerate this <permanent_type>."
    Regenerate(SourceObject),
    /// "Counter target <color> spell."
    CounterTargetColoredSpell {
        color: Color,
    },
    /// "Destroy target <permanent_type>."
    DestroyTargetPermanent {
        permanent_type: PermanentType,
    },
    /// "Destroy target <creature_type>."
    DestroyTargetCreatureType {
        creature_type: CreatureType,
    },
    /// "Look at target player's hand."
    LookAtTargetPlayersHand,
    /// "Draw N cards."
    DrawCards {
        count: CardCount,
    },
    /// "Target player discards N cards."
    TargetPlayerDiscardsCards {
        count: CardCount,
    },
    /// "Target creature with power N or less can't be blocked this turn."
    TargetCreatureWithPowerOrLessCantBeBlockedThisTurn {
        power: u32,
    },
    /// "Target <type> gains <keyword> until end of turn."
    TargetPermanentGainsKeywordUntilEndOfTurn {
        permanent_type: PermanentType,
        keyword: Keyword,
    },
    /// "Enchanted <type> gets <modifier> until end of turn."
    EnchantedGetsUntilEndOfTurn {
        permanent_type: PermanentType,
        modifier: PtModifier,
    },
    /// "This <source> gets <modifier> until end of turn."
    SourceGetsUntilEndOfTurn {
        source: SourceObject,
        modifier: PtModifier,
    },
    /// "This <source> gains <keyword> until end of turn."
    SourceGainsKeywordUntilEndOfTurn {
        source: SourceObject,
        keyword: Keyword,
    },
    /// "This <source> becomes a N/N <creature_type> <permanent_type>+
    /// until end of combat."
    SourceBecomesCreatureUntilEndOfCombat {
        source: SourceObject,
        power: u32,
        toughness: u32,
        creature_type: CreatureType,
        permanent_types: Vec<PermanentType>,
    },
    /// "The next time a/an <color> source of your choice would deal
    /// damage to you this turn, prevent that damage."
    PreventNextDamageFromColoredSource {
        color: Color,
    },
    /// "The next time an unblocked creature of your choice would deal
    /// combat damage to you this turn, prevent all but N of that damage."
    PreventAllButDamageFromUnblockedCreature {
        amount: u32,
    },
    /// "The next time a source of your choice would deal damage to
    /// target <permanent_type> this turn, that source deals that damage
    /// to you instead."
    NextDamageFromSourceToTargetPermanentIsDealtToYouInstead {
        permanent_type: PermanentType,
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
    /// "You may choose a creature card in your hand whose mana cost could
    /// be paid by some amount of, or all of, the mana you spent on {X}."
    ChooseCreatureCardInHandPayableByManaSpentOnVariable {
        variable: Variable,
    },
    /// "Target <permanent_type> becomes a/an <basic_land_type> until
    /// this <source> leaves the battlefield."
    TargetPermanentBecomesBasicLandTypeUntilSourceLeavesBattlefield {
        permanent_type: PermanentType,
        land_type: BasicLandType,
        source: SourceObject,
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
    Reach,
    Haste,
    Defender,
    Banding,
    Trample,
    Islandwalk,
    Mountainwalk,
    Swampwalk,
    Indestructible,
    Fear,
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
pub enum SpellType {
    Instant,
    Sorcery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreatureType {
    Goblin,
    Golem,
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
    /// "<color> spells cost <mana> more to cast." — cost-increase effect
    /// for spells matching the color filter.
    ColoredSpellsCostManaMoreToCast { color: Color, mana: ManaCost },
    /// "Activated abilities of <color> <permanent_type>s cost <mana> more
    /// to activate." — cost-increase effect for activated abilities of
    /// permanents matching the color and type filters.
    ActivatedAbilitiesOfColoredPermanentsCostManaMoreToActivate {
        color: Color,
        permanent_type: PermanentType,
        mana: ManaCost,
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
    /// "Other <creature_type>s get <modifier> and have <keyword>." —
    /// P/T modifier plus keyword grant for other creatures of a subtype.
    OtherCreatureTypeGetAndHaveKeyword {
        creature_type: CreatureType,
        modifier: PtModifier,
        keyword: Keyword,
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
    /// "Enchanted <object> has \"<triggered ability>\"." — grants quoted
    /// triggered rules text to the enchanted object.
    EnchantedHasTriggeredAbility {
        object: EnchantedObject,
        ability: TriggeredAbility,
    },
    /// "Enchanted <object> loses <keyword>." — keyword-removing effect
    /// on the enchanted object.
    EnchantedLosesKeyword {
        object: EnchantedObject,
        keyword: Keyword,
    },
    /// "Enchanted <object> is a/an <basic_land_type>." — type-changing
    /// effect that makes the enchanted object a basic land subtype.
    EnchantedIsBasicLandType {
        object: EnchantedObject,
        land_type: BasicLandType,
    },
    /// "Enchanted <object> has <keyword> and can't be enchanted by other
    /// Auras." — keyword-granting effect plus an Aura attachment
    /// restriction on the enchanted object.
    EnchantedHasKeywordAndCantBeEnchantedByOtherAuras {
        object: EnchantedObject,
        keyword: Keyword,
    },
    /// "Enchanted <object> can attack as though it had <keyword>." —
    /// permission effect that grants an attack-enabling quality such as
    /// haste for attack eligibility.
    EnchantedCanAttackAsThoughItHad {
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
    /// "Enchanted <object> can't be blocked except by <creature_type>s."
    /// — evasion restriction that allows only a creature subtype to block.
    EnchantedCantBeBlockedExceptByCreatureType {
        object: EnchantedObject,
        except_type: CreatureType,
    },
    /// "You control enchanted <object>." — continuous control-changing
    /// effect from an Aura to the object it enchants.
    YouControlEnchanted { object: EnchantedObject },
    /// "You have no maximum hand size." — maximum hand size modifier.
    YouHaveNoMaximumHandSize,
    /// "You don't lose the game for having 0 or less life." — state-based
    /// action exception.
    YouDontLoseGameForHavingZeroOrLessLife,
    /// "If you would gain life, draw that many cards instead." —
    /// replacement effect for life gain.
    IfYouWouldGainLifeDrawThatManyCardsInstead,
    /// "If an effect causes you to discard a card, discard it, but you
    /// may put it on top of your library instead of into your graveyard."
    IfEffectCausesYouToDiscardCardYouMayPutItOnTopOfYourLibraryInstead,
    /// "You may play any number of <permanent_type>s on each of your
    /// turns." — permission effect that lifts the normal per-turn play
    /// limit for that permanent type.
    YouMayPlayAnyNumberOfPermanentsOnEachOfYourTurns { permanent_type: PermanentType },
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
    /// "This <permanent_type> attacks each combat if able."
    SourceAttacksEachCombatIfAble { source: SourceObject },
    /// "This <permanent_type> can't be blocked by <creature_type>s."
    SourceCantBeBlockedByCreatureType {
        source: SourceObject,
        blocked_by: CreatureType,
    },
    /// "This <permanent_type> doesn't untap during your untap step."
    SourceDoesntUntapDuringYourUntapStep { source: SourceObject },
    /// "This <permanent_type> can't block creatures with power N or greater."
    SourceCantBlockCreaturesWithPowerOrGreater { source: SourceObject, power: u32 },
    /// "<source name>'s power and toughness are each equal to the number
    /// of non-<creature_type> creatures you control."
    NamedSourcePowerToughnessEachEqualToNonCreatureTypeCreaturesYouControl {
        source_name: String,
        excluded_type: CreatureType,
    },
    /// "All <basic_land_type>s are <basic_land_type>s."
    BasicLandsAreBasicLands {
        from: BasicLandType,
        to: BasicLandType,
    },
    /// "All <basic_land_type>s are N/N <color> creatures that are still lands."
    BasicLandsAreColoredCreaturesStillLands {
        land_type: BasicLandType,
        power: u32,
        toughness: u32,
        color: Color,
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
    /// "Remove target creature defending player controls from combat."
    RemoveTargetCreatureDefendingPlayerControlsFromCombat,
    /// "Creatures it was blocking that had become blocked by only that
    /// creature this combat become unblocked."
    CreaturesItWasBlockingBecomeUnblocked,
    /// "You may have it block an attacking creature of your choice."
    YouMayHaveItBlockAttackingCreatureOfYourChoice,
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
    /// "<source name> is/isn't attacking"
    SourceIsAttacking {
        source_name: String,
        is_attacking: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContinuousEffect {
    /// "it's a/an <types> with power and toughness each equal to its
    /// mana value" — the enchanted permanent gains the listed types
    /// and a characteristic-defining P/T equal to its mana value.
    BecomesWithPtFromManaValue { types: Vec<PermanentType> },
    /// "its power and toughness are each equal to the number of
    /// <basic_land_type>s <controller> controls"
    SourcePowerToughnessEachEqualToBasicLandsControlled {
        land_type: BasicLandType,
        controller: LandCountController,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LandCountController {
    You,
    DefendingPlayer,
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
