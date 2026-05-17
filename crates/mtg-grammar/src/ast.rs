use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Statement {
    ManaCost(ManaCost),
    /// "Cast this spell only <restriction>."
    CastRestriction(CastRestriction),
    /// "Ignore this effect for each creature the player didn't control
    /// continuously since the beginning of the turn."
    IgnoreThisEffectForEachCreaturePlayerDidntControlContinuouslySinceBeginningOfTurn,
    /// "Counter target spell[ <condition>]." — CR 701.5 counter keyword action.
    CounterTargetSpell {
        condition: Option<CounterTargetSpellCondition>,
    },
    /// "Counter target <color> spell."
    CounterTargetColoredSpell {
        color: Color,
    },
    /// "As an additional cost to cast this spell, <cost>."
    AsAdditionalCostToCastThisSpell {
        cost: SpellAdditionalCost,
    },
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
    /// Damage prevention/replacement effects, with source, recipient,
    /// and event timing captured as axes.
    DamageEffect(ActivatedDamageEffect),
    /// Damage prevention effect whose replacement event is "prevent
    /// <amount> [combat] damage that would be dealt" or "prevent
    /// <amount> of that damage".
    PreventDamageThisTurn {
        #[serde(flatten)]
        effect: DamagePreventionEffect<PreventionRecipient>,
        definitions: Vec<VariableDefinition>,
    },
    /// "For each N damage that would be dealt to this <source>, if it
    /// has a <pt_modifier> counter on it, remove a <pt_modifier> counter
    /// from it and prevent that N damage."
    ForEachDamagePreventedByRemovingCounter {
        amount: DamageAmount,
        source: SourceObject,
        counter: PtModifier,
    },
    /// "Spend only <color> mana on X."
    SpendOnlyColorManaOnVariable {
        color: Color,
        variable: Variable,
    },
    /// "If you pay, this <source> deals damage equal to the number of
    /// <counter> counters on it to <recipient>[ and <recipient>]."
    IfYouPaySourceDealsDamage {
        source: SourceObject,
        counter_name: String,
        recipients: Vec<DamageRecipient>,
    },
    /// "As this <source> enters, you lose life equal to your life total."
    AsSourceEntersYouLoseLifeEqualToYourLifeTotal {
        source: SourceObject,
    },
    /// "You gain life equal to the damage <reference>."
    YouGainLifeEqualToDamage {
        reference: DamageLifeGainReference,
    },
    /// "If you can't, you lose the game."
    IfYouCantYouLoseTheGame,
    /// "If you can't, this <source> deals N damage to you."
    IfYouCantSourceDealsDamageToYou {
        source: SourceObject,
        amount: DamageAmount,
    },
    /// "If you win/lose the flip, <effect>." — CR 705 coin-flip result
    /// condition with an activated-effect payload.
    IfYouFlipResult {
        result: CoinFlipResult,
        effect: ActivatedEffect,
    },
    /// "<subject> can't be regenerated." — a CR 614.17 can't effect
    /// restricting the CR 701.15 regenerate keyword action.
    ItCantBeRegenerated {
        subject: RegenerationRestrictionSubject,
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
    /// "Tap target <target>..." / "You may tap or untap target <target>..."
    /// — CR 701.21 tap/untap keyword actions.
    TapUntapTargetPermanentChoice {
        optional: bool,
        action: TapUntapAction,
        target: TapUntapTarget,
    },
    /// CR 701.11 exile keyword action, sharing the same target/all/list axis
    /// currently used by destroy.
    Exile {
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
    /// "Draw a card/N cards/that many cards."
    DrawCards {
        count: CardCount,
    },
    /// "You may draw a card/N cards."
    YouMayDrawCards {
        count: CardCount,
    },
    /// "Target player discards N cards at random."
    TargetPlayerDiscardsCardsAtRandom {
        count: CardCount,
    },
    /// "If you would <event>, you may skip that <event> instead." —
    /// CR 614.10 skip replacement effect.
    IfYouWouldEventYouMaySkipThatInstead {
        event: SkipReplacementEvent,
    },
    /// "<variable> can't be N." — restriction on a printed variable value.
    VariableCantBeNumber {
        variable: Variable,
        value: u32,
    },
    /// "Look at the top N cards of target player's library, then put them
    /// back in any order."
    LookAtTopCardsOfTargetPlayersLibraryThenPutThemBackInAnyOrder {
        count: CardCount,
    },
    /// "You may have that player shuffle."
    YouMayHaveThatPlayerShuffle,
    /// "Target player gains N life."
    TargetPlayerGainsLife {
        amount: LifeAmount,
    },
    /// "Its controller gains <amount>."
    ItsControllerGainsLife {
        amount: LifeAmount,
    },
    /// "Take an extra turn after this one." — CR 500.7 extra turn effect.
    TakeExtraTurnAfterThisOne,
    /// "Tap all <permanent_type>s <actor> controls and <actor> loses all
    /// unspent mana."
    TapAllPermanentsAndPlayerLosesUnspentMana {
        actor: TapAllPermanentsActor,
        permanent_type: PermanentType,
        with_mana_abilities: bool,
    },
    /// "If that player doesn't, <effect>."
    PlayerPaymentFailure {
        effect: PaymentFailureEffect,
    },
    /// "Target player activates a mana ability of each <permanent_type>
    /// they control."
    TargetPlayerActivatesManaAbilityOfEachPermanentTheyControl {
        permanent_type: PermanentType,
    },
    /// Effects that make one player control another player for a bounded duration.
    ControlPlayer {
        controller: ControlPlayerController,
        player: ControlledPlayer,
        duration: ControlPlayerDuration,
    },
    /// Conditional control-player effect, such as control while a chosen spell resolves.
    ConditionalControlPlayer {
        condition: ControlPlayerCondition,
        effect: ControlPlayerEffect,
    },
    /// A control-player duration fragment.
    ControlPlayerDuration {
        duration: ControlPlayerDuration,
    },
    /// A spell resolution duration fragment.
    SpellResolutionDuration,
    /// A control-player condition fragment.
    ControlPlayerCondition {
        condition: ControlPlayerCondition,
    },
    /// "The player plays that card if able."
    PlayReferencedCard {
        player: ControlledPlayer,
        card: ReferencedCard,
        if_able: bool,
    },
    /// Restriction on what mana abilities may be activated in a bounded context.
    ManaAbilityActivationLimit {
        context: ActivationLimitContext,
        player: ControlledPlayer,
        source: ManaAbilitySourceLimit,
        spending: Vec<ManaSpendingPurpose>,
    },
    /// A mana-ability activation restriction context fragment.
    ActivationLimitContext {
        context: ActivationLimitContext,
    },
    /// A mana-ability activation restriction source-scope fragment.
    ManaAbilitySourceLimit {
        source: ManaAbilitySourceLimit,
    },
    /// A produced-mana spending restriction fragment.
    ProducedManaSpendingLimit {
        spending: Vec<ManaSpendingPurpose>,
    },
    /// A produced-mana spending purpose fragment.
    ManaSpendingPurpose {
        purpose: ManaSpendingPurpose,
    },
    /// "Then that player loses all unspent mana and you add the mana lost
    /// this way."
    ThenThatPlayerLosesUnspentManaAndYouAddManaLostThisWay,
    /// "Change the text of target spell or permanent by replacing all
    /// instances of one <term> with another."
    ChangeTextOfTargetSpellOrPermanentReplacing {
        term: TextChangeReplacementTerm,
    },
    /// "Add <mana>." / "Add an amount of <mana> equal to ..."
    AddMana {
        amount: AddManaAmount,
    },
    /// "Remove this card from your deck before playing if you're not
    /// playing for ante."
    AntePlayRestriction,
    /// "You own target card in the <zone>."
    YouOwnTargetCardInZone {
        zone: Zone,
    },
    /// "Return target [<card_type>] card from your <zone> to your <zone/the battlefield>."
    /// "Return target <permanent_type> to its owner's <zone>."
    ReturnTargetCardFromYourZoneToZone {
        card_type: Option<PermanentType>,
        from: Option<Zone>,
        to: ReturnDestination,
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
    /// A standalone label word accepted for generated pattern tests
    /// extracted from quoted label choices.
    Label {
        label: String,
    },
    /// "<action>, [<action>, ]then <action>."
    ImperativeActionSequence {
        actions: Vec<ImperativeAction>,
    },
    /// "Each player <action>[, then <action>]."
    EachPlayerPerformsAction {
        actions: Vec<EachPlayerAction>,
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
        amount: AddManaAmount,
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
    /// "Target <selector> gets <modifier> until end of turn."
    TargetPermanentGetsUntilEndOfTurn {
        target: TargetPermanentSelector,
        modifier: PtModifier,
    },
    /// "Target <selector> gets <modifier> until end of turn.", where the
    /// modifier may contain a printed variable.
    TargetPermanentGetsMixedUntilEndOfTurn {
        target: TargetPermanentSelector,
        modifier: MixedPtModifier,
    },
    /// "Target <selector> gains <keyword> until end of turn."
    TargetPermanentGainsKeywordUntilEndOfTurn {
        target: TargetPermanentSelector,
        keyword: Keyword,
    },
    /// "Target <selector> gains <keyword> and gets <modifier> until end of
    /// turn, where ..."
    TargetPermanentGainsKeywordAndGetsUntilEndOfTurn {
        target: TargetPermanentSelector,
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
    /// "As this <source> enters, choose <choice>." — replacement effect
    /// choice made as the object enters.
    AsSourceEntersChoose {
        source: SourceObject,
        choice: AsEntersChoice,
    },
    /// "This <permanent_type> enters with N <pt_modifier> counters on it."
    ThisPermanentEntersWithCounters {
        source: SourceObject,
        amount: CounterAmount,
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
    /// "Destroy <it/that creature> at the beginning of the next end step
    /// [if it didn't attack this turn]."
    DestroyReferencedCreatureAtBeginningOfNextEndStep {
        target: ReferencedCreature,
        condition: Option<DestroyReferencedCreatureCondition>,
    },
    /// "Then, for each attacking creature you control, choose <label> or
    /// <label>. That creature can't be blocked this combat except by
    /// creatures with <keyword> and creatures in a pile with the chosen
    /// label."
    ForEachAttackingCreatureChooseLabelBlockingRestriction {
        labels: Vec<String>,
        keyword: Keyword,
    },
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
    /// A trigger event fragment used by concept fixtures before a full
    /// triggered-ability payload is attached.
    TriggerEvent(TriggerEvent),
    TriggeredAbility(TriggeredAbility),
    /// Physical dexterity instructions and their conditional results.
    PhysicalAction(PhysicalAction),
    /// Two or more abilities printed on one card, in source order. Most
    /// members are separated by newlines on the printed face; a few
    /// dependent sentence shapes are same-line continuations when unparsed.
    /// A single-ability card is never wrapped in `Compound`, so each piece
    /// of card text has exactly one canonical AST.
    Compound(Vec<Statement>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextChangeReplacementTerm {
    BasicLandType,
    ColorWord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DestroyTarget {
    /// "target permanent"
    TargetPermanent,
    /// Shared target axis for the CR 701 destroy keyword action.
    /// "target <permanent_type> [or <permanent_type>]"
    TargetPermanents(Vec<PermanentType>),
    /// "target <color> permanent"
    TargetColoredPermanent(Color),
    /// "target <status> creature"
    TargetStatusCreature(CreatureStatus),
    /// "target <quality>[, <quality>] creature"
    TargetQualifiedCreature(Vec<CreatureQuality>),
    /// "target <creature_type>"
    TargetCreatureType(CreatureType),
    /// "N target <basic_land_type>s"
    TargetBasicLands {
        count: CardCount,
        land_type: BasicLandType,
    },
    /// "all <permanent_type>s[, <permanent_type>s, and <permanent_type>s]"
    AllPermanents(Vec<PermanentType>),
    /// "all <basic_land_type>s"
    AllBasicLands(BasicLandType),
    /// "it" / "that creature"
    ReferencedCreature(ReferencedCreature),
    /// "that creature if it attacked this turn"
    ThatCreatureIfItAttackedThisTurn,
    /// "all non-<creature_type> creatures that player controls that didn't
    /// attack this turn"
    AllNonCreatureTypeCreaturesThatPlayerControlsThatDidntAttackThisTurn {
        excluded_type: CreatureType,
    },
    /// "all creatures blocking or blocked by it"
    AllCreaturesBlockingOrBlockedByIt,
    /// "it/that creature at the beginning of the next end step [if it didn't
    /// attack this turn]"
    ReferencedCreatureAtBeginningOfNextEndStep {
        target: ReferencedCreature,
        condition: Option<DestroyReferencedCreatureCondition>,
    },
    /// "that creature at end of combat"
    ThatCreatureAtEndOfCombat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegenerationRestrictionSubject {
    It,
    They,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TapUntapAction {
    Tap,
    TapOrUntap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TapUntapTarget {
    /// "target permanent"
    TargetPermanent,
    /// "target <permanent_type> [or <permanent_type>]"
    TargetPermanents(Vec<PermanentType>),
    /// "target <creature_type>"
    TargetCreatureType(CreatureType),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreatureQuality {
    /// "non<permanent_type>"
    NonPermanentType(PermanentType),
    /// "non<color>"
    NonColor(Color),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CounterTargetSpellCondition {
    /// "unless its controller pays <mana>"
    ItsControllerPays(ManaCost),
    /// "with mana value <variable>"
    WithManaValue(Variable),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TapAllPermanentsActor {
    TargetPlayer,
    ThatPlayer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentFailureEffect {
    TapAllPermanentsAndLoseUnspentMana {
        permanent_type: PermanentType,
        with_mana_abilities: bool,
    },
}

impl Statement {
    pub(crate) fn destroy(target: DestroyTarget) -> Self {
        Statement::Destroy { target }
    }

    pub(crate) fn target_permanent_until_end_of_turn(
        target: TargetPermanentSelector,
        effect: TargetPermanentEndOfTurnEffect,
    ) -> Self {
        match effect {
            TargetPermanentEndOfTurnEffect::Gets(modifier) => {
                match (modifier.power, modifier.toughness) {
                    (SignedPtComponent::Number(power), SignedPtComponent::Number(toughness)) => {
                        Statement::TargetPermanentGetsUntilEndOfTurn {
                            target,
                            modifier: PtModifier { power, toughness },
                        }
                    }
                    _ => Statement::TargetPermanentGetsMixedUntilEndOfTurn { target, modifier },
                }
            }
            TargetPermanentEndOfTurnEffect::GainsKeyword(keyword) => {
                Statement::TargetPermanentGainsKeywordUntilEndOfTurn { target, keyword }
            }
            TargetPermanentEndOfTurnEffect::GainsKeywordAndGets {
                keyword,
                modifier,
                definitions,
            } => Statement::TargetPermanentGainsKeywordAndGetsUntilEndOfTurn {
                target,
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
            IfYouDoEffect::AddMana { amount } => Statement::IfYouDoAddMana { amount },
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
        Statement::PreventDamageThisTurn {
            effect,
            definitions: Vec::new(),
        }
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
            event: DamagePreventionEvent::ThatWouldBeDealt,
            kind,
            recipient,
            duration: Some(DamagePreventionDuration::ThisTurn),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum IfYouDoEffect {
    PreventDamageThisTurn {
        effect: DamagePreventionEffect<PreventionRecipient>,
    },
    AddMana {
        amount: AddManaAmount,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkipReplacementEvent {
    /// "draw a card during your draw step"
    DrawCardDuringYourDrawStep,
    /// "begin your turn while this <source> is <status>"
    BeginYourTurnWhileSourceIsStatus {
        source: SourceObject,
        status: ObjectStatus,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetPermanentSelector {
    Permanent(PermanentType),
    CombatRoleCreature { role: CombatRole },
    ControlledCreatureWithToughnessLessThanSourcePower { source: SourceObject },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatRole {
    Attacking,
    Blocking,
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
    TargetPlayerGainsLife { amount: LifeAmount },
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageLifeGainReference {
    /// "the damage dealt, but not more life than ..."
    DamageDealtCapped { caps: Vec<DamageLifeGainCap> },
    /// "the damage dealt to you this turn"
    DamageDealtToYouThisTurn,
    /// "the damage prevented this way"
    DamagePreventedThisWay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageRecipient {
    /// "any target"
    AnyTarget,
    /// "any target of an opponent's choice"
    AnyTargetOfOpponentsChoice,
    /// "you"
    You,
    /// "target <permanent_type>"
    TargetPermanent { permanent_type: PermanentType },
    /// "target creature you control"
    TargetCreatureYouControl,
    /// "each creature"
    EachCreature,
    /// "each creature with <keyword>"
    EachCreatureWithKeyword { keyword: Keyword },
    /// "each creature without <keyword>"
    EachCreatureWithoutKeyword { keyword: Keyword },
    /// "each player"
    EachPlayer,
    /// "that player"
    ThatPlayer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageRecipients {
    /// "to any target"
    AnyTarget,
    /// "to any target of an opponent's choice"
    AnyTargetOfOpponentsChoice,
    /// "divided evenly, rounded down, among any number of targets"
    DividedEvenlyRoundedDownAmongAnyNumberOfTargets,
    /// "to <recipient> and <recipient>"
    List(Vec<DamageRecipient>),
    /// "<amount> damage to <recipient> and <amount> damage to <recipient>"
    Assignments(Vec<DamageAssignment<DamageRecipient>>),
}

pub type NamedDamageEvent = DamageEvent<String, DamageRecipients>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageAmount {
    Number(u32),
    Variable(Variable),
    DamageDealtToYouThisTurn,
    ThatPermanentsToughness(PermanentType),
    NumberOfBasicLandsTheyControl(BasicLandType),
    NumberOfBasicLandsPutIntoGraveyardThisWay(BasicLandType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CounterAmount {
    Number(u32),
    Variable(Variable),
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
    /// The prevented replacement event's amount axis: "all", "the next N",
    /// or "N of".
    pub amount: DamagePreventionAmount,
    /// The prevented replacement event, such as "damage that would be dealt"
    /// or "that damage".
    pub event: DamagePreventionEvent,
    /// The prevented replacement event's optional damage kind, such as "combat".
    pub kind: Option<DamageKind>,
    /// The optional object or player the prevented damage would be dealt to.
    pub recipient: Option<R>,
    /// The effect's printed duration, when present.
    pub duration: Option<DamagePreventionDuration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamagePreventionAmount {
    /// Implicit amount in "that damage".
    ThatDamage,
    /// "all"
    All,
    /// "the next N"
    Next(DamageAmount),
    /// "N of"
    Amount(DamageAmount),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamagePreventionEvent {
    /// "[combat] damage that would be dealt"
    ThatWouldBeDealt,
    /// "that damage"
    OfThatDamage,
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
    /// "this <object>"
    SourceObject(SourceObject),
    /// "creatures banded with this <object>"
    CreaturesBandedWithSource(SourceObject),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticDamagePreventionEffect {
    pub amount: DamagePreventionAmount,
    pub kind: Option<DamageKind>,
    pub source: DamagePreventionSource,
    pub recipients: Vec<PreventionRecipient>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamagePreventionSource {
    /// "<land_subtype>s"
    LandSubtype(LandSubtype),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayManaPlayer {
    /// "you"
    You,
    /// "that player"
    ThatPlayer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayManaAmount {
    /// "<mana_cost>"
    Cost(ManaCost),
    /// "any amount of mana"
    AnyAmountOfMana,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AddManaAmount {
    /// "<mana_cost>"
    Cost(ManaCost),
    /// "an amount of <mana_symbol> equal to the sacrificed <permanent_type>'s mana value"
    EqualToSacrificedPermanentManaValue {
        mana: ManaSymbol,
        permanent_type: PermanentType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellAdditionalCost {
    /// "sacrifice a/an <permanent_type>"
    SacrificePermanent { permanent_type: PermanentType },
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
    /// "during an opponent's turn, before attackers are declared"
    DuringOpponentsTurnBeforeAttackersDeclared,
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
    /// "look at target player's/opponent's hand"
    LookAtTargetHand { player: TargetHandPlayer },
    /// "choose a card from it"
    ChooseCardFromIt,
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
    /// "discard N cards"
    DiscardCards { count: CardCount },
    /// "tap this <source>"
    TapSource { source: SourceObject },
    /// "sacrifice a/an <permanent_type> of an opponent's choice"
    SacrificePermanentOfOpponentsChoice { permanent_type: PermanentType },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetHandPlayer {
    Player,
    Opponent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlPlayerController {
    You,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlledPlayer {
    ThatPlayer,
    ThePlayer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlPlayerEffect {
    pub controller: ControlPlayerController,
    pub player: ControlledPlayer,
    pub duration: ControlPlayerDuration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlPlayerDuration {
    SourceFinishesResolving { source_name: String },
    ThatSpellIsResolving,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlPlayerCondition {
    ChosenCardIsCastAsSpell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferencedCard {
    ThatCard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationLimitContext {
    WhileDoingSo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManaAbilitySourceLimit {
    LandsThatPlayerControls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManaSpendingPurpose {
    ActivateOtherManaAbilitiesOfLandsThePlayerControls,
    PlayThatCard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EachPlayerAction {
    /// "antes the top card of their library"
    AnteTopCardOfTheirLibrary,
    /// "shuffles their hand and graveyard into their library"
    ShuffleTheirHandAndGraveyardIntoTheirLibrary,
    /// "discards their hand"
    DiscardTheirHand,
    /// "draws N cards"
    DrawCards { count: CardCount },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardCount {
    Number(u32),
    Variable(Variable),
    ThatMany,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
            event: self.event.clone(),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerEvent {
    /// "this Aura enters"
    ThisAuraEnters,
    /// "this Aura leaves the battlefield"
    ThisAuraLeavesTheBattlefield,
    /// "a/an <permanent_type> enters"
    PermanentEnters { permanent_type: PermanentType },
    /// "a player casts a/an <color> spell"
    PlayerCastsColoredSpell { color: Color },
    /// "<actor> casts/cast a/an <color or permanent_type> spell"
    CastsSpell {
        actor: TriggerCastActor,
        spell: TriggerCastSpell,
    },
    /// "a player taps a/an <permanent_type> for mana"
    PlayerTapsPermanentForMana { permanent_type: PermanentType },
    /// "<subject> is tapped for mana"
    IsTappedForMana { subject: TappedForManaSubject },
    /// "a/an <basic_land_type> <controller> becomes <status>"
    BasicLandTypeControllerBecomesStatus {
        land_type: BasicLandType,
        controller: PermanentController,
        status: ObjectStatus,
    },
    /// "you play a/an <permanent_type>"
    YouPlayPermanent { permanent_type: PermanentType },
    /// "one or more creatures you control attack"
    OneOrMoreCreaturesYouControlAttack,
    /// "one or more [other] [nontoken] permanents with a name originally
    /// printed in the <expansion> expansion are on the battlefield"
    OneOrMorePermanentsWithOriginalPrintingOnBattlefield {
        other: bool,
        nontoken: bool,
        expansion: String,
    },
    /// "enchanted <permanent_type> dies"
    EnchantedPermanentDies { permanent_type: PermanentType },
    /// "this <source> dies"
    SourceDies { source: SourceObject },
    /// "<object> becomes <status>"
    ObjectBecomesStatus {
        object: ObjectStatusSubject,
        status: ObjectStatus,
    },
    /// "the beginning of the next end step"
    BeginningOfTheNextEndStep,
    /// "the beginning of the end step"
    BeginningOfTheEndStep,
    /// "the beginning of each end step"
    BeginningOfEachEndStep,
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
    /// or "a/an <permanent_type> dies"
    PermanentPutIntoGraveyardFromBattlefield {
        permanent_type: PermanentType,
        wording: DiesWording,
    },
    /// "a/an <permanent_type> dealt damage by this <source> this turn dies"
    PermanentDealtDamageBySourceThisTurnDies {
        permanent_type: PermanentType,
        source: SourceObject,
    },
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
    /// "you control no <basic_land_type>s"
    YouControlNoBasicLands { land_type: BasicLandType },
    /// "a/an <color> creature attacks"
    ColoredCreatureAttacks { color: Color },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiesWording {
    PutIntoGraveyardFromBattlefield,
    Dies,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerCastActor {
    You,
    Player,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerCastSpell {
    Colored { color: Color },
    PermanentType { permanent_type: PermanentType },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterveningIf {
    /// "if it's on the battlefield"
    ItsOnTheBattlefield,
    /// "if no <permanent_type>s are on the battlefield"
    NoPermanentsAreOnTheBattlefield { permanent_type: PermanentType },
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
    /// CR 701 destroy keyword action inside a triggered ability, with the
    /// target/all/list axis captured as data.
    Destroy { target: DestroyTarget },
    /// "destroy that creature if it attacked this turn"
    DestroyThatCreatureIfItAttackedThisTurn,
    /// "destroy all non-<creature_type> creatures that player controls that
    /// didn't attack this turn"
    DestroyAllNonCreatureTypeCreaturesThatPlayerControlsThatDidntAttackThisTurn {
        excluded_type: CreatureType,
    },
    /// "destroy all creatures blocking or blocked by it"
    DestroyAllCreaturesBlockingOrBlockedByIt,
    /// "destroy it"
    DestroyIt,
    /// "destroy that creature at end of combat"
    DestroyThatCreatureAtEndOfCombat,
    /// "that creature's controller sacrifices it"
    ThatCreaturesControllerSacrificesIt,
    /// "their controllers sacrifice them"
    TheirControllersSacrificeThem,
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
    /// "each defending player divides all creatures without <keyword> they
    /// control into a <label> pile and a <label> pile."
    DefendingPlayerDividesCreaturesWithoutKeywordIntoLabeledPiles {
        keyword: Keyword,
        labels: Vec<String>,
    },
    /// "remove a <pt_modifier> counter from it"
    RemoveCounterFromIt { counter: PtModifier },
    /// "put a <pt_modifier> counter on <recipient>"
    PutCounter {
        counter: PtModifier,
        recipient: TriggerCounterRecipient,
    },
    /// "put <amount> <counter> counter(s) on this <source>"
    PutNamedCountersOnSource {
        amount: NamedCounterAmount,
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
    /// "you may have this <source> become a copy of target
    /// <permanent_type>, except ..."
    YouMayHaveSourceBecomeCopyOfTarget {
        source: SourceObject,
        permanent_type: PermanentType,
        exception: CopyException,
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
    /// "[then] sacrifice this <source> unless you pay <cost>"
    SacrificeSourceUnlessYouPay {
        source: SourceObject,
        cost: TriggerPaymentCost,
        prefixed_by_then: bool,
    },
    /// "sacrifice this <source>"
    SacrificeSource { source: SourceObject },
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
    /// "<player> may pay <mana_cost/any amount of mana>"
    YouMayPayMana {
        player: PayManaPlayer,
        amount: PayManaAmount,
    },
    /// "draw a card/N cards"
    DrawCards { count: CardCount },
    /// "you may draw a card/N cards"
    YouMayDrawCards { count: CardCount },
    /// "Prevent <amount> of that damage[, where ...]"
    PreventDamage {
        #[serde(flatten)]
        effect: DamagePreventionEffect<PreventionRecipient>,
        definitions: Vec<VariableDefinition>,
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerCounterRecipient {
    /// "it"
    It,
    /// "this <source>"
    Source(SourceObject),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageAssignmentGroup<R> {
    pub amount: DamageAmount,
    pub recipients: Vec<R>,
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
pub enum LifeAmount {
    /// "N"
    Number(u32),
    /// "X" / "Y"
    Variable(Variable),
    /// "life equal to its power"
    EqualToItsPower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceObject {
    /// "this <permanent_type>"
    This(PermanentType),
    /// "this permanent"
    ThisPermanent,
    /// "this Aura"
    ThisAura,
    /// "that source"
    ThatSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AsEntersChoice {
    /// "a color"
    Color,
    /// "an opponent"
    Opponent,
    /// "a basic land type"
    BasicLandType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectStatus {
    Attacking,
    Tapped,
    Untapped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TappedForManaSubject {
    /// "a/an <basic_land_type>"
    BasicLandType(BasicLandType),
    /// "enchanted <object>"
    Enchanted(EnchantedObject),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectStatusSubject {
    /// "this <source>"
    Source(SourceObject),
    /// "enchanted <object>"
    Enchanted(EnchantedObject),
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
    /// "Activate only during your upkeep."
    ActivateOnlyDuringYourUpkeep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamedCounterAmount {
    /// "a <counter> counter"
    One,
    /// "that many <counter> counters"
    ThatMany,
    /// "a <counter> counter for each <permanent_type> that died this turn"
    OneForEachPermanentThatDiedThisTurn { permanent_type: PermanentType },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerPaymentCost {
    Mana(ManaCost),
    ManaForEachNamedCounterOnIt {
        cost: ManaCost,
        counter_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivatedCost {
    Mana(ManaCost),
    VariableMana(Variable),
    Tap,
    /// "Sacrifice this <permanent_type>"
    Sacrifice(SourceObject),
    /// "Remove a <counter> counter from this <source>"
    RemoveNamedCounterFromSource {
        counter_name: String,
        source: SourceObject,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegenerateRecipient {
    /// "this <permanent_type>"
    Source(SourceObject),
    /// "enchanted <object>"
    Enchanted(EnchantedObject),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivatedEffect {
    /// "Add <mana>."
    AddMana(AddManaAmount),
    /// "Add one mana of any color."
    AddOneManaOfAnyColor,
    /// "Add N mana of any one color."
    AddManaOfAnyOneColor {
        amount: u32,
    },
    /// "Tap [or untap] target <target>."
    TapTargetPermanentChoice {
        action: TapUntapAction,
        target: TapUntapTarget,
    },
    /// "Untap this <permanent_type>."
    Untap(SourceObject),
    /// "Untap target <permanent_type>."
    UntapTargetPermanent {
        permanent_type: PermanentType,
    },
    /// "Untap enchanted <object>."
    UntapEnchanted(EnchantedObject),
    /// "Take an extra turn after this one."
    TakeExtraTurnAfterThisOne,
    /// "Regenerate <permanent>." — CR 701.15 regenerate keyword action.
    Regenerate(RegenerateRecipient),
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
    /// CR 614 replacement effect for the next card draw this turn.
    NextCardDrawReplacement {
        replacement: DrawReplacementEffect,
    },
    /// "Draw N cards."
    DrawCards {
        count: CardCount,
    },
    /// "Flip a coin." — CR 705 coin-flip action.
    FlipCoin,
    /// "<imperative action>, then <imperative action>."
    ImperativeActionSequence {
        actions: Vec<ImperativeAction>,
    },
    /// "Create a N/N [color] [creature_type] <types> token [with <keyword>] [named <name>]."
    /// CR 701.6 create keyword action.
    CreateToken {
        token: TokenDescription,
    },
    /// "Target player discards N cards."
    TargetPlayerDiscardsCards {
        count: CardCount,
    },
    /// "Gain control of target <permanent_type> for as long as you control
    /// this <source>."
    GainControlOfTargetPermanentForAsLongAsYouControlSource {
        permanent_type: PermanentType,
        source: SourceObject,
    },
    /// "Target creature with power N or less can't be blocked this turn."
    TargetCreatureWithPowerOrLessCantBeBlockedThisTurn {
        power: u32,
    },
    /// "Target <selector> gains <keyword> until end of turn."
    TargetPermanentGainsKeywordUntilEndOfTurn {
        target: TargetPermanentSelector,
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
    /// "Put [up to] N <pt_modifier> counter(s) on this <source>."
    PutCountersOnSource {
        amount: CounterAmount,
        up_to: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoinFlipResult {
    Win,
    Lose,
}

/// Replacement effect applied to a card-draw event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawReplacementEffect {
    /// Look at the top N cards of your library, keep one draw, and put the
    /// rest on the bottom in a random order.
    FilterTopLibraryCards { count: CardCount },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenDescription {
    pub power: u32,
    pub toughness: u32,
    pub color: Option<TokenColor>,
    pub creature_type: Option<CreatureType>,
    pub permanent_types: Vec<PermanentType>,
    pub keyword: Option<Keyword>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenColor {
    Color(Color),
    Colorless,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivatedDamageEffect {
    /// "This <source> deals N damage to <recipient> [and M damage to <recipient>]."
    SourceDealsDamage {
        source: SourceObject,
        assignments: Vec<DamageAssignment<ActivatedDamageRecipient>>,
    },
    /// "This <source> deals N damage to <recipient> [and <recipient>]"
    /// with the original assignment phrase boundaries preserved.
    SourceDealsDamageAssignmentGroups {
        source: SourceObject,
        assignments: Vec<DamageAssignmentGroup<ActivatedDamageRecipient>>,
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
    /// "any target of an opponent's choice"
    AnyTargetOfOpponentsChoice,
    /// "each creature"
    EachCreature,
    /// "each player"
    EachPlayer,
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
    pub const VIGILANCE: Self = Self::Named(NamedKeywordAbility::Vigilance);

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
    #[allow(non_upper_case_globals)]
    pub const Vigilance: Self = Self::VIGILANCE;
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
    Vigilance,
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
            KeywordAbility::Named(NamedKeywordAbility::Vigilance) => Self::Vigilance,
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
            KeywordSerde::Vigilance => Self::Named(NamedKeywordAbility::Vigilance),
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
    Vigilance,
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
    Hand,
    Ante,
    Battlefield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReturnDestination {
    YourZone(Zone),
    TheBattlefield,
    ItsOwnersZone(Zone),
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
    Djinn,
    Goblin,
    Golem,
    Insect,
    Merfolk,
    Wall,
    Zombie,
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
    Variable(Variable),
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifeTotalFloorCause {
    Damage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifeTotalFloorPlayer {
    You,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayRestriction {
    pub affected: PlayRestrictionAffected,
    pub actions: Vec<PlayRestrictionAction>,
    pub filter: PlayRestrictionFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayRestrictionAffected {
    Players,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayRestrictionAction {
    CastSpells,
    PlayLands,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayRestrictionFilter {
    OriginalPrinting { expansion: String },
}

/// A static ability printed on a permanent. This covers conditional
/// continuous effects, P/T modifiers on matching objects or enchanted
/// objects, and permission effects that let an enchanted object attack
/// through a keyword restriction such as defender.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaticAbility {
    /// Unconditional continuous effect printed as a standalone static ability.
    Continuous { effect: ContinuousEffect },
    /// "As long as <cond>, <effect>." — continuous effect gated on a
    /// condition.
    Conditional {
        order: ConditionalEffectOrder,
        condition: Condition,
        effect: ContinuousEffect,
    },
    /// Permission effect for spending one color of mana as another.
    ManaSpendingPermission { from: Color, to: Color },
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
    /// "Enchanted <type> gets <modifier> and has <keyword>." — P/T
    /// modifier plus keyword-granting effect on the enchanted permanent.
    EnchantedGetsAndHasKeyword {
        permanent_type: PermanentType,
        modifier: PtModifier,
        keyword: Keyword,
    },
    /// "<color> <permanent_type>s get <modifier>." — P/T modifier on
    /// every permanent matching the color and type filters.
    ColoredPermanentsGet {
        color: Color,
        permanent_type: PermanentType,
        modifier: PtModifier,
    },
    /// "Other <creature_type>s get <modifier> and have <ability>." /
    /// "Other <creature_type> creatures have <ability>." — ability
    /// grant for other objects or creatures of a subtype, optionally
    /// with a P/T modifier.
    OtherCreatureTypeGetAndHaveAbility {
        creature_type: CreatureType,
        subject: OtherCreatureTypeSubject,
        modifier: Option<PtModifier>,
        ability: GrantedAbility,
    },
    /// "<status> creatures [you control] get <modifier> [until end of
    /// turn]." — P/T modifier on creatures matching a combat/tapped state.
    StatusCreaturesYouControlGet {
        status: CreatureStatus,
        controller: StatusCreatureController,
        modifier: PtModifier,
        duration: StatusCreatureGetDuration,
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
    /// "Enchanted <object> is a/an <basic_land_type>/the chosen type."
    /// — type-changing effect that makes the enchanted object a basic
    /// land subtype, either named directly or supplied by a linked
    /// as-enters choice.
    EnchantedIsBasicLandType {
        object: EnchantedObject,
        land_type: BasicLandTypeReference,
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
    /// "<cause> that would reduce <player>'s life total to less than N
    /// reduces it to N instead." — replacement effect that floors a life
    /// total change.
    LifeTotalFloorReplacement {
        cause: LifeTotalFloorCause,
        player: LifeTotalFloorPlayer,
        threshold: u32,
    },
    /// "If an effect causes you to discard a card, discard it, but you
    /// may put it on top of your library instead of into your graveyard."
    IfEffectCausesYouToDiscardCardYouMayPutItOnTopOfYourLibraryInstead,
    /// "You may play any number of <permanent_type>s on each of your
    /// turns." — permission effect that lifts the normal per-turn play
    /// limit for that permanent type.
    YouMayPlayAnyNumberOfPermanentsOnEachOfYourTurns { permanent_type: PermanentType },
    /// "<affected> can't <action> [or <action>]* <filter>."
    PlayRestriction(PlayRestriction),
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
    /// "This <permanent_type> can't attack unless defending player controls
    /// a/an <basic_land_type>."
    SourceCantAttackUnlessDefendingPlayerControlsBasicLand {
        source: SourceObject,
        land_type: BasicLandType,
    },
    /// "This <permanent_type> can't be blocked by <creature_type>s."
    SourceCantBeBlockedByCreatureType {
        source: SourceObject,
        blocked_by: CreatureType,
    },
    /// "This <permanent_type> doesn't untap during your untap step."
    SourceDoesntUntapDuringYourUntapStep { source: SourceObject },
    /// "Enchanted <object> doesn't untap during its controller's untap step."
    EnchantedDoesntUntapDuringItsControllersUntapStep { object: EnchantedObject },
    /// Untap restrictions that apply during untap steps.
    UntapRestrictionDuringUntapSteps { restriction: StaticUntapRestriction },
    /// "This <permanent_type> can't block creatures with power N or greater."
    SourceCantBlockCreaturesWithPowerOrGreater { source: SourceObject, power: u32 },
    /// "<source name>'s power and toughness are each equal to the number
    /// of <counted objects>."
    NamedSourcePowerToughnessEachEqualToCount {
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
    /// "<subject> can block <amount> creature(s) <duration>."
    BlockingCapacityPermission {
        subject: BlockingCapacitySubject,
        amount: BlockingCapacityAmount,
        duration: BlockingCapacityDuration,
    },
    /// "Remove target creature defending player controls from combat."
    RemoveTargetCreatureDefendingPlayerControlsFromCombat,
    /// "Creatures it was blocking that had become blocked by only that
    /// creature this combat become unblocked."
    CreaturesItWasBlockingBecomeUnblocked,
    /// "You may have it block an attacking creature of your choice."
    YouMayHaveItBlockAttackingCreatureOfYourChoice,
    /// "<subject> attacks this turn if able."
    CreaturesAttackThisTurnIfAble { subject: AttackRequirementSubject },
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
pub enum BlockingCapacitySubject {
    /// "target creature defending player controls"
    TargetCreatureDefendingPlayerControls,
    /// "this <permanent_type>"
    Source(SourceObject),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockingCapacityAmount {
    /// "any number of creatures"
    AnyNumber,
    /// "an additional creature"
    AdditionalOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockingCapacityDuration {
    /// "this turn"
    ThisTurn,
    /// "each combat"
    EachCombat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaticUntapRestriction {
    /// "Creatures with power N or greater don't untap during their
    /// controllers' untap steps."
    CreaturesWithPowerOrGreater { power: u32 },
    /// "Players can't untap more than N <permanent_type> during their
    /// untap steps."
    PlayersCantUntapMoreThanPermanents {
        amount: u32,
        permanent_type: PermanentType,
    },
    /// "Players skip their untap steps."
    PlayersSkipTheirUntapSteps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusCreatureController {
    Any,
    You,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusCreatureGetDuration {
    Continuous,
    UntilEndOfTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OtherCreatureTypeSubject {
    /// "Other <creature_type>s"
    TypePlural,
    /// "Other <creature_type> creatures"
    TypeCreatures,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrantedAbility {
    Keyword(Keyword),
    Activated(ActivatedAbility),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CopyException {
    /// "except it's a/an <permanent_type> in addition to its other types"
    PermanentTypeInAdditionToItsOtherTypes { permanent_type: PermanentType },
    /// "except it doesn't copy that <permanent_type>'s color and it has
    /// <ability>"
    DoesntCopyColorAndHasAbility {
        permanent_type: PermanentType,
        ability: CopyGrantedAbility,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CopyGrantedAbility {
    /// "this ability"
    ThisAbility,
    /// A quoted triggered ability granted to the copy.
    TriggeredAbility(Box<TriggeredAbility>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnchantedObject {
    Permanent(PermanentType),
    CreatureType(CreatureType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionalEffectOrder {
    ConditionThenEffect,
    EffectThenCondition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Condition {
    /// "you control a/an <basic_land_type>"
    YouControlBasicLand { land_type: BasicLandType },
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
    /// "this <object> is tapped/untapped"
    SourceIsObjectStatus {
        source: SourceObject,
        status: ObjectStatus,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttackRequirementSubject {
    /// "that creature"
    ThatCreature,
    /// "creatures the active player controls"
    CreaturesActivePlayerControls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferencedCreature {
    It,
    ThatCreature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DestroyReferencedCreatureCondition {
    DidntAttackThisTurn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamedSourcePowerToughnessCount {
    /// "non-<creature_type> creatures you control"
    NonCreatureTypeCreatures { excluded_type: CreatureType },
    /// "<basic_land_type>s you control"
    BasicLands { land_type: BasicLandType },
    /// "creatures named <card name> on the battlefield"
    CreaturesNamedOnTheBattlefield { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContinuousEffect {
    /// "This <permanent_type> gets <modifier>"
    SourceGets {
        source: SourceObject,
        modifier: PtModifier,
    },
    /// "All damage that would be dealt to you by <source> is dealt to
    /// this <object> instead" — static damage redirection replacement effect.
    DamageThatWouldBeDealtToYouBySourceIsDealtToSourceInstead {
        source: StaticDamageSource,
        destination: StaticDamageRedirectionDestination,
    },
    /// "Prevent <amount> [combat] damage <source> would deal to <recipients>"
    /// — CR 615 prevention effect expressed as a static continuous effect.
    PreventDamage {
        effect: StaticDamagePreventionEffect,
    },
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
    /// "Players can't untap more than N <permanent_type> during their
    /// untap steps" and neighbouring static untap restrictions, used as
    /// a conditional continuous effect.
    UntapRestrictionDuringUntapSteps { restriction: StaticUntapRestriction },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaticDamageSource {
    /// "a source"
    Source,
    /// "unblocked creatures"
    UnblockedCreatures,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaticDamageRedirectionDestination {
    /// "this <object>"
    SourceObject(SourceObject),
    /// "that source"
    ThatSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LandSubtype {
    Desert,
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
    /// "the number of <status> <permanent_type>s they controlled at the
    /// beginning of this turn"
    NumberOfStatusPermanentsTheyControlledAtBeginningOfThisTurn {
        status: ObjectStatus,
        permanent_type: PermanentType,
    },
    /// "the amount of mana that player paid this way"
    AmountOfManaThatPlayerPaidThisWay,
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
pub enum BasicLandTypeReference {
    Specific(BasicLandType),
    ChosenType,
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
