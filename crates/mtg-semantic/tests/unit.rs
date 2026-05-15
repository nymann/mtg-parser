// Tier 1 lowering unit tests. Hand-written AST → expected IR.

use mtg_grammar::{
    BasicLandType, DamageAmount, Keyword, ManaCost, ManaSymbol, PermanentType, PreventionRecipient,
    PtModifier, Sign, SignedNumber, SignedPtComponent, SignedVariable, SourceObject, Statement,
    Variable,
};
use mtg_semantic::{lower, CardEffect, ManaValue};

fn mc(symbols: Vec<ManaSymbol>) -> Statement {
    Statement::ManaCost(ManaCost { symbols })
}

fn mana(symbols: Vec<ManaSymbol>) -> ManaCost {
    ManaCost { symbols }
}

fn signed(sign: Sign, magnitude: u32) -> SignedNumber {
    SignedNumber { sign, magnitude }
}

#[test]
fn lowers_destroy_target_creature() {
    assert_eq!(
        lower(&Statement::DestroyTargetCreature).unwrap(),
        CardEffect::DestroyTargetCreature,
    );
}

#[test]
fn lowers_destroy_target_permanent_choice() {
    let permanent_types = vec![PermanentType::Artifact, PermanentType::Enchantment];
    assert_eq!(
        lower(&Statement::DestroyTargetPermanentChoice {
            permanent_types: permanent_types.clone(),
        })
        .unwrap(),
        CardEffect::DestroyTargetPermanentChoice { permanent_types },
    );
}

#[test]
fn lowers_destroy_target_permanent() {
    assert_eq!(
        lower(&Statement::DestroyTargetPermanent {
            permanent_type: PermanentType::Land,
        })
        .unwrap(),
        CardEffect::DestroyTargetPermanent {
            permanent_type: PermanentType::Land,
        },
    );
}

#[test]
fn lowers_destroy_all_basic_lands() {
    assert_eq!(
        lower(&Statement::DestroyAllBasicLands {
            basic_land_type: BasicLandType::Plains,
        })
        .unwrap(),
        CardEffect::DestroyAllBasicLands {
            basic_land_type: BasicLandType::Plains,
        },
    );
}

#[test]
fn lowers_regenerate_target_creature() {
    assert_eq!(
        lower(&Statement::RegenerateTargetCreature).unwrap(),
        CardEffect::RegenerateTargetCreature,
    );
}

#[test]
fn lowers_counter_target_spell() {
    assert_eq!(
        lower(&Statement::CounterTargetSpell).unwrap(),
        CardEffect::CounterTargetSpell,
    );
}

#[test]
fn lowers_prevent_next_damage_to_recipient_this_turn() {
    assert_eq!(
        lower(
            &Statement::PreventNextDamageThatWouldBeDealtToRecipientThisTurn {
                amount: DamageAmount::Variable(mtg_grammar::Variable::X),
                recipient: PreventionRecipient::AnyTarget,
            }
        )
        .unwrap(),
        CardEffect::PreventNextDamageThatWouldBeDealtToRecipientThisTurn {
            amount: DamageAmount::Variable(mtg_grammar::Variable::X),
            recipient: PreventionRecipient::AnyTarget,
        },
    );
}

#[test]
fn lowers_if_you_do_prevent_next_damage_to_recipient_this_turn() {
    assert_eq!(
        lower(
            &Statement::IfYouDoPreventNextDamageThatWouldBeDealtToRecipientThisTurn {
                amount: DamageAmount::Number(1),
                recipient: PreventionRecipient::ThatPermanentOrPlayer,
            }
        )
        .unwrap(),
        CardEffect::IfYouDoPreventNextDamageThatWouldBeDealtToRecipientThisTurn {
            amount: DamageAmount::Number(1),
            recipient: PreventionRecipient::ThatPermanentOrPlayer,
        },
    );
}

#[test]
fn lowers_target_permanent_gets_until_end_of_turn() {
    assert_eq!(
        lower(&Statement::TargetPermanentGetsUntilEndOfTurn {
            permanent_type: PermanentType::Creature,
            modifier: PtModifier {
                power: signed(Sign::Plus, 3),
                toughness: signed(Sign::Plus, 3),
            },
        })
        .unwrap(),
        CardEffect::TargetPermanentGetsUntilEndOfTurn {
            permanent_type: PermanentType::Creature,
            modifier: PtModifier {
                power: signed(Sign::Plus, 3),
                toughness: signed(Sign::Plus, 3),
            },
        },
    );
}

#[test]
fn lowers_target_permanent_gets_mixed_until_end_of_turn() {
    let modifier = mtg_grammar::MixedPtModifier {
        power: SignedPtComponent::Variable(SignedVariable {
            sign: Sign::Plus,
            variable: Variable::X,
        }),
        toughness: SignedPtComponent::Number(signed(Sign::Plus, 0)),
    };
    assert_eq!(
        lower(&Statement::TargetPermanentGetsMixedUntilEndOfTurn {
            permanent_type: PermanentType::Creature,
            modifier,
        })
        .unwrap(),
        CardEffect::TargetPermanentGetsMixedUntilEndOfTurn {
            permanent_type: PermanentType::Creature,
            modifier,
        },
    );
}

#[test]
fn lowers_prevent_all_combat_damage_this_turn() {
    assert_eq!(
        lower(&Statement::PreventAllCombatDamageThisTurn).unwrap(),
        CardEffect::PreventAllCombatDamageThisTurn,
    );
}

#[test]
fn lowers_if_you_do_gain_life() {
    assert_eq!(
        lower(&Statement::IfYouDoGainLife { amount: 1 }).unwrap(),
        CardEffect::IfYouDoGainLife { amount: 1 },
    );
}

#[test]
fn lowers_if_you_would_draw_during_draw_step_skip_that_draw() {
    assert_eq!(
        lower(&Statement::IfYouWouldDrawCardDuringYourDrawStepInsteadYouMaySkipThatDraw).unwrap(),
        CardEffect::IfYouWouldDrawCardDuringYourDrawStepInsteadYouMaySkipThatDraw,
    );
}

#[test]
fn lowers_if_you_do_cant_be_attacked_except_by_keyword_creatures() {
    let keywords = vec![Keyword::Flying, Keyword::Islandwalk];
    assert_eq!(
        lower(
            &Statement::IfYouDoUntilYourNextTurnYouCantBeAttackedExceptByCreaturesWithKeywords {
                keywords: keywords.clone(),
            }
        )
        .unwrap(),
        CardEffect::IfYouDoUntilYourNextTurnYouCantBeAttackedExceptByCreaturesWithKeywords {
            keywords,
        },
    );
}

#[test]
fn lowers_target_player_gains_life() {
    assert_eq!(
        lower(&Statement::TargetPlayerGainsLife { amount: 3 }).unwrap(),
        CardEffect::TargetPlayerGainsLife { amount: 3 },
    );
}

#[test]
fn lowers_activation_threshold_sacrifice_delayed_trigger() {
    assert_eq!(
        lower(
            &Statement::IfThisAbilityActivatedAtLeastTimesThisTurnSacrificeSourceAtNextEndStep {
                threshold: 4,
                source: SourceObject::This(PermanentType::Creature),
            }
        )
        .unwrap(),
        CardEffect::IfThisAbilityActivatedAtLeastTimesThisTurnSacrificeSourceAtNextEndStep {
            threshold: 4,
            source: SourceObject::This(PermanentType::Creature),
        },
    );
}

#[test]
fn lowers_add_mana() {
    let mana = mana(vec![
        ManaSymbol::Black,
        ManaSymbol::Black,
        ManaSymbol::Black,
    ]);
    assert_eq!(
        lower(&Statement::AddMana { mana: mana.clone() }).unwrap(),
        CardEffect::AddMana { mana },
    );
}

#[test]
fn lowers_target_player_activates_mana_ability_of_each_permanent_they_control() {
    assert_eq!(
        lower(
            &Statement::TargetPlayerActivatesManaAbilityOfEachPermanentTheyControl {
                permanent_type: PermanentType::Land,
            }
        )
        .unwrap(),
        CardEffect::TargetPlayerActivatesManaAbilityOfEachPermanentTheyControl {
            permanent_type: PermanentType::Land,
        },
    );
}

#[test]
fn lowers_then_that_player_loses_unspent_mana_and_you_add_mana_lost_this_way() {
    assert_eq!(
        lower(&Statement::ThenThatPlayerLosesUnspentManaAndYouAddManaLostThisWay).unwrap(),
        CardEffect::ThenThatPlayerLosesUnspentManaAndYouAddManaLostThisWay,
    );
}

#[test]
fn lowers_pure_generic_mana_cost() {
    assert_eq!(
        lower(&mc(vec![ManaSymbol::Generic(2)])).unwrap(),
        CardEffect::ManaCost(ManaValue {
            generic: 2,
            ..Default::default()
        }),
    );
}

#[test]
fn aggregates_generic_pips() {
    // {1}{1}{1} and {3} share an IR — that's the point of the lower.
    let one_one_one = lower(&mc(vec![
        ManaSymbol::Generic(1),
        ManaSymbol::Generic(1),
        ManaSymbol::Generic(1),
    ]))
    .unwrap();
    let three = lower(&mc(vec![ManaSymbol::Generic(3)])).unwrap();
    assert_eq!(one_one_one, three);
}

#[test]
fn separates_colors() {
    assert_eq!(
        lower(&mc(vec![
            ManaSymbol::White,
            ManaSymbol::Blue,
            ManaSymbol::Black,
            ManaSymbol::Red,
            ManaSymbol::Green,
            ManaSymbol::Colorless,
        ]))
        .unwrap(),
        CardEffect::ManaCost(ManaValue {
            white: 1,
            blue: 1,
            black: 1,
            red: 1,
            green: 1,
            colorless: 1,
            ..Default::default()
        }),
    );
}

#[test]
fn pip_order_does_not_matter() {
    let a = lower(&mc(vec![ManaSymbol::Red, ManaSymbol::Generic(2)])).unwrap();
    let b = lower(&mc(vec![ManaSymbol::Generic(2), ManaSymbol::Red])).unwrap();
    assert_eq!(a, b);
}

#[test]
fn mixed_cost_total_is_correct() {
    let CardEffect::ManaCost(m) = lower(&mc(vec![
        ManaSymbol::Generic(2),
        ManaSymbol::Red,
        ManaSymbol::Red,
    ]))
    .unwrap() else {
        panic!("expected ManaCost");
    };
    assert_eq!(m.total(), 4);
    assert_eq!(m.generic, 2);
    assert_eq!(m.red, 2);
}

#[test]
fn mana_value_total_is_sum_of_counters() {
    let m = ManaValue {
        generic: 5,
        red: 2,
        ..Default::default()
    };
    assert_eq!(m.total(), 7);
}
