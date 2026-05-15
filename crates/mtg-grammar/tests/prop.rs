// Tier 2 property tests. The core invariant: every AST the parser can
// produce must survive `parse(unparse(ast)) == ast`. A failure here
// signals either grammar ambiguity or unparser/grammar drift.
//
// The M3 exit criterion is 1000 cases; that stays inside the <10s tier-2
// budget thanks to the trivial parser/unparser.

use mtg_grammar::{
    parse, unparse, ActivatedAbility, ActivatedCost, ActivatedEffect, CardCount, Color,
    EachPlayerAction, ImperativeAction, ManaCost, ManaSymbol, PermanentType, SourceObject,
    Statement, TriggerEffect, TriggerEvent, TriggeredAbility, Variable,
};
use proptest::prelude::*;

fn arb_mana_symbol() -> impl Strategy<Value = ManaSymbol> {
    prop_oneof![
        // Wider than realistic costs to surface any digit-handling bugs.
        (0u32..=999).prop_map(ManaSymbol::Generic),
        Just(ManaSymbol::White),
        Just(ManaSymbol::Blue),
        Just(ManaSymbol::Black),
        Just(ManaSymbol::Red),
        Just(ManaSymbol::Green),
        Just(ManaSymbol::Colorless),
    ]
}

fn arb_mana_cost() -> impl Strategy<Value = ManaCost> {
    prop::collection::vec(arb_mana_symbol(), 1..16).prop_map(|symbols| ManaCost { symbols })
}

fn arb_card_count() -> impl Strategy<Value = CardCount> {
    (1u32..=10).prop_map(CardCount::Number)
}

fn arb_color() -> impl Strategy<Value = Color> {
    prop_oneof![
        Just(Color::White),
        Just(Color::Blue),
        Just(Color::Black),
        Just(Color::Red),
        Just(Color::Green),
    ]
}

fn arb_permanent_type() -> impl Strategy<Value = PermanentType> {
    prop_oneof![
        Just(PermanentType::Artifact),
        Just(PermanentType::Creature),
        Just(PermanentType::Enchantment),
        Just(PermanentType::Land),
        Just(PermanentType::Planeswalker),
    ]
}

fn arb_imperative_action() -> impl Strategy<Value = ImperativeAction> {
    prop_oneof![
        Just(ImperativeAction::DiscardYourHand),
        Just(ImperativeAction::AnteTopCardOfYourLibrary),
        arb_card_count().prop_map(|count| ImperativeAction::DrawCards { count }),
    ]
}

fn arb_player_casts_colored_spell_pay_mana_trigger() -> impl Strategy<Value = Statement> {
    (arb_color(), arb_mana_cost()).prop_map(|(color, cost)| {
        Statement::TriggeredAbility(TriggeredAbility {
            event: TriggerEvent::PlayerCastsColoredSpell { color },
            intervening_if: None,
            effects: vec![TriggerEffect::YouMayPayMana { cost }],
        })
    })
}

fn arb_target_player_discards_activated_ability() -> impl Strategy<Value = Statement> {
    arb_card_count().prop_map(|count| {
        Statement::ActivatedAbility(ActivatedAbility {
            costs: vec![
                ActivatedCost::Mana(ManaCost {
                    symbols: vec![ManaSymbol::Generic(3)],
                }),
                ActivatedCost::Tap,
            ],
            effect: ActivatedEffect::TargetPlayerDiscardsCards { count },
        })
    })
}

fn arb_statement() -> impl Strategy<Value = Statement> {
    prop_oneof![
        arb_mana_cost().prop_map(Statement::ManaCost),
        arb_mana_cost().prop_map(|mana| Statement::AddMana { mana }),
        Just(Statement::CounterTargetSpell),
        Just(Statement::DestroyTargetCreature),
        Just(Statement::NamedSourceDealsVariableDamageToAnyTarget {
            source_name: "Disintegrate".to_string(),
            amount: Variable::X,
        }),
        arb_permanent_type().prop_map(|permanent_type| {
            Statement::IfItsPermanentCantBeRegeneratedAndWouldDieExileInsteadThisTurn {
                permanent_type,
            }
        }),
        (arb_permanent_type(), arb_permanent_type()).prop_map(|(a, b)| {
            Statement::DestroyTargetPermanentChoice {
                permanent_types: vec![a, b],
            }
        }),
        Just(Statement::RegenerateTargetCreature),
        Just(Statement::ActivateOnlyDuringYourTurn),
        (1u32..=10).prop_map(|threshold| {
            Statement::IfThisAbilityActivatedAtLeastTimesThisTurnSacrificeSourceAtNextEndStep {
                threshold,
                source: SourceObject::This(PermanentType::Creature),
            }
        }),
        Just(Statement::AntePlayRestriction),
        Just(Statement::EachPlayerPerformsAction {
            action: EachPlayerAction::AnteTopCardOfTheirLibrary
        }),
        (1u32..=10).prop_map(|amount| Statement::IfYouDoGainLife { amount }),
        (arb_player_casts_colored_spell_pay_mana_trigger(), 1u32..=10,).prop_map(
            |(trigger, amount)| {
                Statement::Compound(vec![trigger, Statement::IfYouDoGainLife { amount }])
            }
        ),
        arb_player_casts_colored_spell_pay_mana_trigger(),
        arb_target_player_discards_activated_ability(),
        prop::collection::vec(arb_imperative_action(), 2..5)
            .prop_map(|actions| Statement::ImperativeActionSequence { actions }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 1000, ..ProptestConfig::default() })]

    #[test]
    fn round_trip(stmt in arb_statement()) {
        let text = unparse(&stmt);
        let reparsed = parse(&text)
            .map_err(|e| TestCaseError::fail(format!("parse failed on {text:?}: {e}")))?;
        prop_assert_eq!(stmt, reparsed);
    }
}
