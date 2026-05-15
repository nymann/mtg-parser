use mtg_grammar::{
    ActionTiming, ActivatedAbility, BalanceSameWayAction, CardCount, CastRestriction, Color,
    ImperativeAction, Keyword, ManaCost, MixedPtModifier, ModalMode, OptionalCost, PermanentType,
    PhysicalAction, PtModifier, SourceObject, StaticAbility, TriggeredAbility, VariableDefinition,
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
    /// "Destroy all <permanent_type>s."
    DestroyAll { permanent_type: PermanentType },
    /// A single keyword ability such as `Flying` or `Enchant artifact`.
    Keyword(Keyword),
    /// "Target player draws N cards."
    TargetPlayerDrawsCards { count: CardCount },
    /// "Remove this card from your deck before playing if you're not
    /// playing for ante."
    AntePlayRestriction,
    /// Ordered imperative actions such as "discard your hand, ante the
    /// top card of your library, then draw seven cards."
    ImperativeActionSequence { actions: Vec<ImperativeAction> },
    /// "Until end of turn, <timing>, you may <cost>."
    UntilEndOfTurnYouMayPayCostAtTiming {
        timing: ActionTiming,
        cost: OptionalCost,
    },
    /// "If you do, add <mana>."
    IfYouDoAddMana { mana: ManaCost },
    /// "Target spell or permanent becomes <color>."
    TargetSpellOrPermanentBecomesColor { color: Color },
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
    /// "Activate only during your upkeep."
    ActivateOnlyDuringYourUpkeep,
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
