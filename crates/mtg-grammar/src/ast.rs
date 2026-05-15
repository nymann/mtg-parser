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
    /// "Regenerate target creature."
    RegenerateTargetCreature,
    /// "<source name> deals <amount> damage <recipients>."
    NamedSourceDealsDamage {
        #[serde(flatten)]
        event: NamedDamageEvent,
    },
    /// Damage prevention effect whose replacement event is "prevent
    /// <amount> [combat] damage that would be dealt" and whose duration
    /// is "this turn".
    PreventDamageThisTurn {
        #[serde(flatten)]
        effect: DamagePreventionEffect<PreventionRecipient>,
    },
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
    /// "If you can't, this <source> deals N damage to you."
    IfYouCantSourceDealsDamageToYou {
        source: SourceObject,
        amount: DamageAmount,
    },
    /// "If it's a <type>, it can't be regenerated this turn, and if it
    /// would die this turn, exile it instead."
    IfItsPermanentCantBeRegeneratedAndWouldDieExileInsteadThisTurn {
        permanent_type: PermanentType,
    },
    /// CR 701 destroy keyword action, with the target/all/list axis captured
    /// as data instead of separate sentence-shaped variants.
    Destroy {
        target: DestroyTarget,
    },
    /// "That <permanent_type>'s controller may attach this Aura to a/an
    /// <permanent_type> of their choice."
    ThatPermanentsControllerMayAttachThisAuraToPermanentOfTheirChoice {
        controller_of: PermanentType,
        attach_to: PermanentType,
    },
    Keyword(Keyword),
    /// "<keyword>, <keyword>[, ...]"
    KeywordList(Vec<Keyword>),
    /// "<keyword>; <keyword>[; ...]"
    SemicolonKeywordList(Vec<Keyword>),
    TargetPlayerDrawsCards {
        count: CardCount,
    },
    /// "Target player discards N cards at random."
    TargetPlayerDiscardsCardsAtRandom {
        count: CardCount,
    },
    /// "If you would draw a card during your draw step, instead you may
    /// skip that draw."
    IfYouWouldDrawCardDuringYourDrawStepInsteadYouMaySkipThatDraw,
    /// "Look at the top N cards of target player's library, then put them
    /// back in any order."
    LookAtTopCardsOfTargetPlayersLibraryThenPutThemBackInAnyOrder {
        count: CardCount,
    },
    /// "You may have that player shuffle."
    YouMayHaveThatPlayerShuffle,
    /// "Target player gains N life."
    TargetPlayerGainsLife {
        amount: u32,
    },
    /// "Tap all <permanent_type>s target player controls and that player
    /// loses all unspent mana."
    TapAllPermanentsTargetPlayerControlsAndThatPlayerLosesUnspentMana {
        permanent_type: PermanentType,
    },
    /// "Target player activates a mana ability of each <permanent_type>
    /// they control."
    TargetPlayerActivatesManaAbilityOfEachPermanentTheyControl {
        permanent_type: PermanentType,
    },
    /// "Then that player loses all unspent mana and you add the mana lost
    /// this way."
    ThenThatPlayerLosesUnspentManaAndYouAddManaLostThisWay,
    /// "Change the text of target spell or permanent by replacing all
    /// instances of one basic land type with another."
    ChangeTextOfTargetSpellOrPermanentReplacingBasicLandType,
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
    /// "If you do," followed by a this-turn damage prevention effect.
    IfYouDoPreventDamageThisTurn {
        #[serde(flatten)]
        effect: DamagePreventionEffect<PreventionRecipient>,
    },
    /// "If you do, add <mana>."
    IfYouDoAddMana {
        mana: ManaCost,
    },
    /// "If you do, untap this <source>."
    IfYouDoUntap {
        source: SourceObject,
    },
    /// "If you do, untap the <permanent_type>."
    IfYouDoUntapReferencedPermanent {
        permanent_type: PermanentType,
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
    /// "Only this <source>'s owner may activate this ability."
    OnlySourcesOwnerMayActivateThisAbility {
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
    /// "Activate only during an opponent's turn, before attackers are declared."
    ActivateOnlyDuringOpponentsTurnBeforeAttackersDeclared,
    /// "Activate only as a sorcery."
    ActivateOnlyAsSorcery,
    /// "Destroy it at the beginning of the next end step if it didn't attack this turn."
    DestroyItAtBeginningOfNextEndStepIfItDidntAttackThisTurn,
    /// "Choose one —" followed by one or more bullet-pointed modes.
    ModalChoice {
        modes: Vec<ModalMode>,
    },
    StaticAbility(StaticAbility),
    ActivatedAbility(ActivatedAbility),
    ActivatedAbilityWithActivationPermission {
        ability: ActivatedAbility,
        permission: ActivationPermission,
    },
    TriggeredAbility(TriggeredAbility),
    /// Physical dexterity instructions and their conditional results.
    PhysicalAction(PhysicalAction),
    /// Two or more abilities printed on one card, in source order,
    /// separated by newlines on the printed face. A single-ability
    /// card is never wrapped in `Compound`, so each piece of card
    /// text has exactly one canonical AST.
    Compound(Vec<Statement>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DestroyTarget {
    /// Shared target axis for the CR 701 destroy keyword action.
    /// "target <permanent_type> [or <permanent_type>]"
    TargetPermanents(Vec<PermanentType>),
    /// "target <color> permanent"
    TargetColoredPermanent(Color),
    /// "target <creature_type>"
    TargetCreatureType(CreatureType),
    /// "all <permanent_type>s[, <permanent_type>s, and <permanent_type>s]"
    AllPermanents(Vec<PermanentType>),
    /// "all <basic_land_type>s"
    AllBasicLands(BasicLandType),
}

impl Statement {
    pub(crate) fn destroy(target: DestroyTarget) -> Self {
        Statement::Destroy { target }
    }

    pub(crate) fn target_permanent_until_end_of_turn(
        permanent_type: PermanentType,
        effect: TargetPermanentEndOfTurnEffect,
    ) -> Self {
        match effect {
            TargetPermanentEndOfTurnEffect::Gets(modifier) => {
                match (modifier.power, modifier.toughness) {
                    (SignedPtComponent::Number(power), SignedPtComponent::Number(toughness)) => {
                        Statement::TargetPermanentGetsUntilEndOfTurn {
                            permanent_type,
                            modifier: PtModifier { power, toughness },
                        }
                    }
                    _ => Statement::TargetPermanentGetsMixedUntilEndOfTurn {
                        permanent_type,
                        modifier,
                    },
                }
            }
            TargetPermanentEndOfTurnEffect::GainsKeyword(keyword) => {
                Statement::TargetPermanentGainsKeywordUntilEndOfTurn {
                    permanent_type,
                    keyword,
                }
            }
            TargetPermanentEndOfTurnEffect::GainsKeywordAndGets {
                keyword,
                modifier,
                definitions,
            } => Statement::TargetPermanentGainsKeywordAndGetsUntilEndOfTurn {
                permanent_type,
                keyword,
                modifier,
                definitions,
            },
        }
    }

    pub(crate) fn if_you_do(effect: IfYouDoEffect) -> Self {
        match effect {
            IfYouDoEffect::PreventDamageThisTurn { effect } => {
                Statement::IfYouDoPreventDamageThisTurn { effect }
            }
            IfYouDoEffect::AddMana { mana } => Statement::IfYouDoAddMana { mana },
            IfYouDoEffect::Untap { source } => Statement::IfYouDoUntap { source },
            IfYouDoEffect::UntapReferencedPermanent { permanent_type } => {
                Statement::IfYouDoUntapReferencedPermanent { permanent_type }
            }
            IfYouDoEffect::GainLife { amount } => Statement::IfYouDoGainLife { amount },
        }
    }

    pub(crate) fn damage_prevention_effect(
        effect: DamagePreventionEffect<PreventionRecipient>,
    ) -> Self {
        Statement::PreventDamageThisTurn { effect }
    }
}

impl<R> DamagePreventionEffect<R> {
    pub(crate) fn this_turn(
        amount: DamagePreventionAmount,
        kind: Option<DamageKind>,
        recipient: Option<R>,
    ) -> Self {
        Self {
            amount,
            kind,
            recipient,
            duration: DamagePreventionDuration::ThisTurn,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum IfYouDoEffect {
    PreventDamageThisTurn {
        effect: DamagePreventionEffect<PreventionRecipient>,
    },
    AddMana {
        mana: ManaCost,
    },
    Untap {
        source: SourceObject,
    },
    UntapReferencedPermanent {
        permanent_type: PermanentType,
    },
    GainLife {
        amount: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum TargetPermanentEndOfTurnEffect {
    Gets(MixedPtModifier),
    GainsKeyword(Keyword),
    GainsKeywordAndGets {
        keyword: Keyword,
        modifier: MixedPtModifier,
        definitions: Vec<VariableDefinition>,
    },
}

impl TargetPermanentEndOfTurnEffect {
    pub(crate) fn gets_numbered(modifier: PtModifier) -> Self {
        Self::Gets(MixedPtModifier {
            power: SignedPtComponent::Number(modifier.power),
            toughness: SignedPtComponent::Number(modifier.toughness),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColoredTargetEffect {
    /// "Counter target <color> spell."
    CounterSpell { color: Color },
    /// "Destroy target <color> permanent."
    DestroyPermanent { color: Color },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModalMode {
    /// "Counter target <color> spell."
    CounterTargetColoredSpell { color: Color },
    /// "Destroy target <color> permanent."
    DestroyTargetColoredPermanent { color: Color },
    /// "Target player gains N life."
    TargetPlayerGainsLife { amount: u32 },
    /// "Prevent <amount> [combat] damage that would be dealt [to <recipient>] this turn."
    PreventDamageThisTurn {
        #[serde(flatten)]
        effect: DamagePreventionEffect<PreventionRecipient>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageRecipients {
    /// "to any target"
    AnyTarget,
    /// "divided evenly, rounded down, among any number of targets"
    DividedEvenlyRoundedDownAmongAnyNumberOfTargets,
    /// "to <recipient> and <recipient>"
    List(Vec<DamageRecipient>),
}

pub type NamedDamageEvent = DamageEvent<String, DamageRecipients>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageAmount {
    Number(u32),
    Variable(Variable),
    ThatPermanentsToughness(PermanentType),
    NumberOfBasicLandsTheyControl(BasicLandType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamagePrevention<R, A = DamageAmount> {
    pub amount: A,
    pub recipient: R,
}

/// A CR 615 prevention effect: the CR 614 replacement event being
/// created, plus the context-specific recipient and duration axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamagePreventionEffect<R = PreventionRecipient> {
    /// The prevented replacement event's amount axis: "all" or "the next N".
    pub amount: DamagePreventionAmount,
    /// The prevented replacement event's optional damage kind, such as "combat".
    pub kind: Option<DamageKind>,
    /// The optional object or player the prevented damage would be dealt to.
    pub recipient: Option<R>,
    /// The effect's printed duration.
    pub duration: DamagePreventionDuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamagePreventionAmount {
    /// "all"
    All,
    /// "the next N"
    Next(DamageAmount),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamagePreventionDuration {
    /// "this turn"
    ThisTurn,
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

/// The condition part of a triggered ability:
/// "<event>, [if <intervening-if>,]".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerCondition {
    pub event: TriggerEvent,
    pub intervening_if: Option<InterveningIf>,
}

/// "When/Whenever <event>, [if <intervening-if>,] <effect>[. <effect>]*."
///
/// Triggered abilities are stored in the same order as printed: the
/// triggering event, an optional intervening-if condition, then the
/// effect sequence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggeredAbility {
    pub event: TriggerEvent,
    pub intervening_if: Option<InterveningIf>,
    pub effects: Vec<TriggerEffect>,
}

impl TriggeredAbility {
    pub fn condition(&self) -> TriggerCondition {
        TriggerCondition {
            event: self.event,
            intervening_if: self.intervening_if,
        }
    }

    pub fn from_parts(condition: TriggerCondition, effects: Vec<TriggerEffect>) -> Self {
        Self {
            event: condition.event,
            intervening_if: condition.intervening_if,
            effects,
        }
    }
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
    /// "a player taps a/an <permanent_type> for mana"
    PlayerTapsPermanentForMana { permanent_type: PermanentType },
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
    /// "this <source> dies"
    SourceDies { source: SourceObject },
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
    /// "the beginning of your draw step"
    BeginningOfYourDrawStep,
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
    /// "if this card is in your <zone> with N or more <type> cards above it"
    ThisCardInYourZoneWithCardsAboveIt {
        zone: Zone,
        count: u32,
        card_type: PermanentType,
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
    /// Damage dealt by a trigger, with source, amount, recipient, and
    /// optional replacement/prevention-style condition captured as axes.
    SourceDealsDamage(TriggeredDamage),
    /// "that player draws an additional card"
    ThatPlayerDrawsAnAdditionalCard,
    /// "that player discards a card at random"
    ThatPlayerDiscardsCardAtRandom,
    /// "that player adds N mana of any type that <permanent_type> produced"
    ThatPlayerAddsManaOfAnyTypeThatPermanentProduced {
        amount: u32,
        permanent_type: PermanentType,
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
    /// "sacrifice a/an <permanent_type> other than this <source>"
    SacrificePermanentOtherThanSource {
        permanent_type: PermanentType,
        source: SourceObject,
    },
    /// "sacrifice that many nontoken permanents"
    SacrificeThatManyNontokenPermanents,
    /// "you lose the game"
    YouLoseTheGame,
    /// "you gain N life"
    YouGainLife { amount: u32 },
    /// "<player> loses <amount> life"
    PlayerLosesLife {
        player: LifeLossPlayer,
        amount: LifeLossAmount,
    },
    /// "you may pay <mana_cost>"
    YouMayPayMana { cost: ManaCost },
    /// "tap enchanted <object>"
    TapEnchanted(EnchantedObject),
    /// "you may put this card onto the battlefield"
    YouMayPutThisCardOntoTheBattlefield,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggeredDamage {
    #[serde(flatten)]
    pub event: DamageEvent<TriggerDamageSource, TriggerDamageRecipient>,
    pub condition: Option<TriggerDamageCondition>,
    pub definitions: Vec<VariableDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageEvent<S, R> {
    pub source: S,
    pub amount: DamageAmount,
    pub recipient: R,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageAssignment<R> {
    pub amount: DamageAmount,
    pub recipient: R,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerDamageSource {
    Source(SourceObject),
    It,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerDamageRecipient {
    You,
    ThatPlayer,
    ThatPermanent(PermanentType),
    ThatPermanentController(PermanentType),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerDamageCondition {
    UnlessYouPay(ManaCost),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifeLossPlayer {
    /// "its owner"
    ItsOwner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifeLossAmount {
    /// "N"
    Number(u32),
    /// "half their life, rounded <direction>"
    HalfTheirLife { rounding: Rounding },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationPermission {
    /// "Only this <source>'s owner may activate this ability."
    OnlySourcesOwner { source: SourceObject },
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
    /// "Destroy target <color> permanent."
    DestroyTargetColoredPermanent {
        color: Color,
    },
    /// CR 701 destroy keyword action, with the target/all/list axis captured
    /// as data instead of separate sentence-shaped variants.
    Destroy {
        target: DestroyTarget,
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
    /// Activated damage prevention/replacement effects, with amount,
    /// source, recipient, and event timing captured as axes.
    DamageEffect(ActivatedDamageEffect),
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
    /// "Choose target non-<creature_type> creature the active player has
    /// controlled continuously since the beginning of the turn."
    ChooseTargetNonCreatureTypeCreatureActivePlayerControlledContinuouslySinceBeginningOfTurn {
        excluded_type: CreatureType,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivatedDamageEffect {
    /// "This <source> deals N damage to <recipient> [and M damage to <recipient>]."
    SourceDealsDamage {
        source: SourceObject,
        assignments: Vec<DamageAssignment<ActivatedDamageRecipient>>,
    },
    /// "The next time <source> of your choice would deal [combat] damage
    /// to <recipient> this turn, <effect>."
    NextDamageEvent {
        #[serde(flatten)]
        event: DamageEventPattern<ActivatedDamageSource, ActivatedDamageRecipient>,
        effect: ActivatedDamageEventEffect,
    },
    /// "The next N [combat] damage that would be dealt to <recipient>
    /// this turn is dealt to <destination> instead."
    RedirectNextDamageThisTurn {
        amount: DamageAmount,
        kind: Option<DamageKind>,
        recipient: ActivatedDamageRecipient,
        destination: DamageRedirectionDestination,
    },
    /// This-turn damage prevention effect with activated-ability recipient vocabulary.
    PreventDamageThisTurn {
        #[serde(flatten)]
        effect: DamagePreventionEffect<ActivatedDamageRecipient>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivatedDamageSource {
    /// "a/an <color> source"
    ColoredSource { color: Color },
    /// "an unblocked creature"
    UnblockedCreature,
    /// "a source"
    Source,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageKind {
    Damage,
    CombatDamage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageEventPattern<S, R> {
    pub source: S,
    pub kind: DamageKind,
    pub recipient: R,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivatedDamageRecipient {
    /// "you"
    You,
    /// "any target"
    AnyTarget,
    /// "target <permanent_type>"
    TargetPermanent { permanent_type: PermanentType },
    /// "this <source>"
    Source(SourceObject),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivatedDamageEventEffect {
    /// "prevent that damage"
    PreventThatDamage,
    /// "prevent all but N of that damage"
    PreventAllBut { amount: u32 },
    /// "that source deals that damage to you instead"
    RedirectToYou,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageRedirectionDestination {
    /// "its owner"
    ItsOwner,
}

impl ActivatedEffect {
    pub(crate) fn destroy(target: DestroyTarget) -> Self {
        ActivatedEffect::Destroy { target }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeywordAbility {
    Named(NamedKeywordAbility),
    /// §702 landwalk abilities are parameterized by the basic land type.
    Landwalk(BasicLandType),
    /// §702 protection abilities are parameterized by the protected quality.
    Protection(Color),
    /// §702 enchant abilities are parameterized by the object this Aura can
    /// enchant.
    Enchant(EnchantObject),
}

pub type Keyword = KeywordAbility;

impl KeywordAbility {
    pub const FIRST_STRIKE: Self = Self::Named(NamedKeywordAbility::FirstStrike);
    pub const FLYING: Self = Self::Named(NamedKeywordAbility::Flying);
    pub const REACH: Self = Self::Named(NamedKeywordAbility::Reach);
    pub const HASTE: Self = Self::Named(NamedKeywordAbility::Haste);
    pub const DEFENDER: Self = Self::Named(NamedKeywordAbility::Defender);
    pub const BANDING: Self = Self::Named(NamedKeywordAbility::Banding);
    pub const TRAMPLE: Self = Self::Named(NamedKeywordAbility::Trample);
    pub const PLAINSWALK: Self = Self::Landwalk(BasicLandType::Plains);
    pub const ISLANDWALK: Self = Self::Landwalk(BasicLandType::Island);
    pub const SWAMPWALK: Self = Self::Landwalk(BasicLandType::Swamp);
    pub const MOUNTAINWALK: Self = Self::Landwalk(BasicLandType::Mountain);
    pub const FORESTWALK: Self = Self::Landwalk(BasicLandType::Forest);
    pub const INDESTRUCTIBLE: Self = Self::Named(NamedKeywordAbility::Indestructible);
    pub const FEAR: Self = Self::Named(NamedKeywordAbility::Fear);

    #[allow(non_upper_case_globals)]
    pub const FirstStrike: Self = Self::FIRST_STRIKE;
    #[allow(non_upper_case_globals)]
    pub const Flying: Self = Self::FLYING;
    #[allow(non_upper_case_globals)]
    pub const Reach: Self = Self::REACH;
    #[allow(non_upper_case_globals)]
    pub const Haste: Self = Self::HASTE;
    #[allow(non_upper_case_globals)]
    pub const Defender: Self = Self::DEFENDER;
    #[allow(non_upper_case_globals)]
    pub const Banding: Self = Self::BANDING;
    #[allow(non_upper_case_globals)]
    pub const Trample: Self = Self::TRAMPLE;
    #[allow(non_upper_case_globals)]
    pub const Plainswalk: Self = Self::PLAINSWALK;
    #[allow(non_upper_case_globals)]
    pub const Islandwalk: Self = Self::ISLANDWALK;
    #[allow(non_upper_case_globals)]
    pub const Swampwalk: Self = Self::SWAMPWALK;
    #[allow(non_upper_case_globals)]
    pub const Mountainwalk: Self = Self::MOUNTAINWALK;
    #[allow(non_upper_case_globals)]
    pub const Forestwalk: Self = Self::FORESTWALK;
    #[allow(non_upper_case_globals)]
    pub const Indestructible: Self = Self::INDESTRUCTIBLE;
    #[allow(non_upper_case_globals)]
    pub const Fear: Self = Self::FEAR;
}

#[derive(Serialize, Deserialize)]
enum KeywordSerde {
    FirstStrike,
    Flying,
    Reach,
    Haste,
    Defender,
    Banding,
    Trample,
    Plainswalk,
    Islandwalk,
    Swampwalk,
    Mountainwalk,
    Forestwalk,
    Landwalk(BasicLandType),
    Indestructible,
    Fear,
    Protection(Color),
    Enchant(EnchantObject),
}

impl From<KeywordAbility> for KeywordSerde {
    fn from(keyword: KeywordAbility) -> Self {
        match keyword {
            KeywordAbility::Named(NamedKeywordAbility::FirstStrike) => Self::FirstStrike,
            KeywordAbility::Named(NamedKeywordAbility::Flying) => Self::Flying,
            KeywordAbility::Named(NamedKeywordAbility::Reach) => Self::Reach,
            KeywordAbility::Named(NamedKeywordAbility::Haste) => Self::Haste,
            KeywordAbility::Named(NamedKeywordAbility::Defender) => Self::Defender,
            KeywordAbility::Named(NamedKeywordAbility::Banding) => Self::Banding,
            KeywordAbility::Named(NamedKeywordAbility::Trample) => Self::Trample,
            KeywordAbility::Landwalk(BasicLandType::Plains) => Self::Plainswalk,
            KeywordAbility::Landwalk(BasicLandType::Island) => Self::Islandwalk,
            KeywordAbility::Landwalk(BasicLandType::Swamp) => Self::Swampwalk,
            KeywordAbility::Landwalk(BasicLandType::Mountain) => Self::Mountainwalk,
            KeywordAbility::Landwalk(BasicLandType::Forest) => Self::Forestwalk,
            KeywordAbility::Named(NamedKeywordAbility::Indestructible) => Self::Indestructible,
            KeywordAbility::Named(NamedKeywordAbility::Fear) => Self::Fear,
            KeywordAbility::Protection(color) => Self::Protection(color),
            KeywordAbility::Enchant(object) => Self::Enchant(object),
        }
    }
}

impl From<KeywordSerde> for KeywordAbility {
    fn from(keyword: KeywordSerde) -> Self {
        match keyword {
            KeywordSerde::FirstStrike => Self::Named(NamedKeywordAbility::FirstStrike),
            KeywordSerde::Flying => Self::Named(NamedKeywordAbility::Flying),
            KeywordSerde::Reach => Self::Named(NamedKeywordAbility::Reach),
            KeywordSerde::Haste => Self::Named(NamedKeywordAbility::Haste),
            KeywordSerde::Defender => Self::Named(NamedKeywordAbility::Defender),
            KeywordSerde::Banding => Self::Named(NamedKeywordAbility::Banding),
            KeywordSerde::Trample => Self::Named(NamedKeywordAbility::Trample),
            KeywordSerde::Plainswalk => Self::Landwalk(BasicLandType::Plains),
            KeywordSerde::Islandwalk => Self::Landwalk(BasicLandType::Island),
            KeywordSerde::Swampwalk => Self::Landwalk(BasicLandType::Swamp),
            KeywordSerde::Mountainwalk => Self::Landwalk(BasicLandType::Mountain),
            KeywordSerde::Forestwalk => Self::Landwalk(BasicLandType::Forest),
            KeywordSerde::Landwalk(land_type) => Self::Landwalk(land_type),
            KeywordSerde::Indestructible => Self::Named(NamedKeywordAbility::Indestructible),
            KeywordSerde::Fear => Self::Named(NamedKeywordAbility::Fear),
            KeywordSerde::Protection(color) => Self::Protection(color),
            KeywordSerde::Enchant(object) => Self::Enchant(object),
        }
    }
}

impl Serialize for KeywordAbility {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        KeywordSerde::from(*self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for KeywordAbility {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        KeywordSerde::deserialize(deserializer).map(Self::from)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamedKeywordAbility {
    FirstStrike,
    Flying,
    Reach,
    Haste,
    Defender,
    Banding,
    Trample,
    Indestructible,
    Fear,
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
    Merfolk,
    Wall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreatureStatus {
    Attacking,
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
    /// "All creatures able to block enchanted <object> do so." —
    /// blocking requirement for creatures that can block the enchanted object.
    AllCreaturesAbleToBlockEnchantedDoSo { object: EnchantedObject },
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
    /// "This <permanent_type> enters tapped."
    SourceEntersTapped { source: SourceObject },
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
    /// "Enchanted <object> doesn't untap during its controller's untap step."
    EnchantedDoesntUntapDuringItsControllersUntapStep { object: EnchantedObject },
    /// "Creatures with power N or greater don't untap during their
    /// controllers' untap steps."
    CreaturesWithPowerOrGreaterDontUntapDuringTheirControllersUntapSteps { power: u32 },
    /// "This <permanent_type> can't block creatures with power N or greater."
    SourceCantBlockCreaturesWithPowerOrGreater { source: SourceObject, power: u32 },
    /// "<source name>'s power and toughness are each equal to the number
    /// of <counted objects> you control."
    NamedSourcePowerToughnessEachEqualToCountYouControl {
        source_name: String,
        count: NamedSourcePowerToughnessCount,
    },
    /// "All <basic_land_type>s are <basic_land_type>s."
    BasicLandsAreBasicLands {
        from: BasicLandType,
        to: BasicLandType,
    },
    /// "All <basic_land_type>s are N/N [<color>] creatures that are still lands."
    BasicLandsAreColoredCreaturesStillLands {
        land_type: BasicLandType,
        power: u32,
        toughness: u32,
        color: Option<Color>,
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
    /// "That creature attacks this turn if able."
    ThatCreatureAttacksThisTurnIfAble,
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
pub enum NamedSourcePowerToughnessCount {
    /// "non-<creature_type> creatures you control"
    NonCreatureTypeCreatures { excluded_type: CreatureType },
    /// "<basic_land_type>s you control"
    BasicLands { land_type: BasicLandType },
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
