use mtg_grammar::{
    ActionTiming, ActivatedAbility, BalanceSameWayAction, BasicLandType, CardCount,
    CastRestriction, Color, DamageAmount, DamageLifeGainCap, DamageRecipient, EachPlayerAction,
    ImperativeAction, Keyword, ManaCost, MixedPtModifier, ModalMode, OptionalCost, PermanentType,
    PhysicalAction, PreventionRecipient, PtModifier, SourceObject, SpellType, StaticAbility,
    TriggeredAbility, Variable, VariableDefinition, Zone,
};
use serde::{Deserialize, Serialize};

/// Semantic IR for one Oracle-text effect. The grammar's syntactic
/// `Statement` is lowered into one of these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardEffect {
    /// Normalized mana cost. Generic mana is summed across pips, each
    /// color has a dedicated counter, so `{1}{1}{R}` lowers identically
    /// to `{2}{R}`.
    ManaCost(ManaValue),
    /// "Cast this spell only <restriction>."
    CastRestriction(CastRestriction),
    /// "Counter target spell."
    CounterTargetSpell,
    /// "Destroy target creature."
    DestroyTargetCreature,
    /// "Regenerate target creature."
    RegenerateTargetCreature,
    /// "This spell costs <mana> more to cast for each target beyond the first."
    ThisSpellCostsManaMoreToCastForEachTargetBeyondTheFirst { mana: ManaCost },
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
    /// "<source name> deals X damage to <recipient> and <recipient>."
    NamedSourceDealsVariableDamageToDamageRecipients {
        source_name: String,
        amount: Variable,
        recipients: Vec<DamageRecipient>,
    },
    /// "Prevent all combat damage that would be dealt this turn."
    PreventAllCombatDamageThisTurn,
    /// "Spend only <color> mana on X."
    SpendOnlyColorManaOnVariable { color: Color, variable: Variable },
    /// "You gain life equal to the damage dealt, but not more life than ..."
    YouGainLifeEqualToDamageDealtCapped { caps: Vec<DamageLifeGainCap> },
    /// "If it's a <type>, it can't be regenerated this turn, and if it
    /// would die this turn, exile it instead."
    IfItsPermanentCantBeRegeneratedAndWouldDieExileInsteadThisTurn { permanent_type: PermanentType },
    /// "Destroy target <permanent_type> or <permanent_type>."
    DestroyTargetPermanentChoice { permanent_types: Vec<PermanentType> },
    /// "Destroy target <permanent_type>."
    DestroyTargetPermanent { permanent_type: PermanentType },
    /// "Destroy all <permanent_type>s."
    DestroyAll { permanent_type: PermanentType },
    /// "Destroy all <basic_land_type>s."
    DestroyAllBasicLands { basic_land_type: BasicLandType },
    /// A single keyword ability such as `Flying` or `Enchant artifact`.
    Keyword(Keyword),
    /// "Target player draws N cards."
    TargetPlayerDrawsCards { count: CardCount },
    /// "Target player gains N life."
    TargetPlayerGainsLife { amount: u32 },
    /// "Target player activates a mana ability of each <permanent_type>
    /// they control."
    TargetPlayerActivatesManaAbilityOfEachPermanentTheyControl { permanent_type: PermanentType },
    /// "Then that player loses all unspent mana and you add the mana lost
    /// this way."
    ThenThatPlayerLosesUnspentManaAndYouAddManaLostThisWay,
    /// "Add <mana>."
    AddMana { mana: ManaCost },
    /// "Remove this card from your deck before playing if you're not
    /// playing for ante."
    AntePlayRestriction,
    /// "You own target card in the <zone>."
    YouOwnTargetCardInZone { zone: Zone },
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
    /// Ordered imperative actions such as "discard your hand, ante the
    /// top card of your library, then draw seven cards."
    ImperativeActionSequence { actions: Vec<ImperativeAction> },
    /// "Each player <action>."
    EachPlayerPerformsAction { action: EachPlayerAction },
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
    /// "If you do, prevent the next N damage that would be dealt to
    /// <recipient> this turn."
    IfYouDoPreventNextDamageThatWouldBeDealtToRecipientThisTurn {
        amount: DamageAmount,
        recipient: PreventionRecipient,
    },
    /// "If you do, add <mana>."
    IfYouDoAddMana { mana: ManaCost },
    /// "If you do, you gain N life."
    IfYouDoGainLife { amount: u32 },
    /// "If you do, you may cast that card face down as a N/N creature
    /// spell without paying its mana cost."
    IfYouDoCastThatCardFaceDownWithoutPayingManaCost { power: u32, toughness: u32 },
    /// Face-down creature-spell replacement effect that turns it face up
    /// before assigning/dealing damage, being dealt damage, or tapping.
    IfFaceDownSpellCreatureWouldAssignOrDealDamageOrTapTurnFaceUpInstead,
    /// "Target spell or permanent becomes <color>."
    TargetSpellOrPermanentBecomesColor { color: Color },
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
    /// "Target <type> gains <keyword> and gets <modifier> until end of
    /// turn, where ..."
    TargetPermanentGainsKeywordAndGetsUntilEndOfTurn {
        permanent_type: PermanentType,
        keyword: Keyword,
        modifier: MixedPtModifier,
        definitions: Vec<VariableDefinition>,
    },
    /// Balance-style equalization of a controlled permanent type by
    /// sacrificing permanents above the table minimum.
    EachPlayerEqualizesControlledPermanents { permanent_type: PermanentType },
    /// Follow-up Balance-style actions that reuse the preceding
    /// equalization method.
    PlayersDoActionsTheSameWay { actions: Vec<BalanceSameWayAction> },
    /// "As this <permanent_type> enters, choose an opponent."
    AsThisPermanentEntersChooseOpponent { permanent_type: PermanentType },
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
    /// "Activate only during your turn."
    ActivateOnlyDuringYourTurn,
    /// "Activate only as a sorcery."
    ActivateOnlyAsSorcery,
    /// A modal spell choice with one or more printed modes.
    ModalChoice { modes: Vec<ModalMode> },
    /// A static ability with a conditional continuous effect. The
    /// grammar-side AST is reused verbatim until the IR grows real
    /// reference-resolution work to do here.
    StaticAbility(StaticAbility),
    /// A triggered ability ("When <event>, ..."). Reused from the
    /// grammar AST until the IR needs to resolve `this Aura`,
    /// `that creature` and similar references.
    TriggeredAbility(TriggeredAbility),
    /// An activated ability ("<cost>: <effect>."). Reused from the
    /// grammar AST until the IR grows cost payment and effect lowering.
    ActivatedAbility(ActivatedAbility),
    /// Physical dexterity instructions and their conditional results.
    PhysicalAction(PhysicalAction),
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
